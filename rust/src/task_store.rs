//! Local task record reads with active/archive identity semantics.

use crate::{
    config::Config,
    draft_store::DraftStore,
    git::GitRepository,
    identity::{
        canonical_task_id, extract_id_from_filename, generate_next_id, generate_next_subtask_id,
        normalize_task_id, task_ids_equal,
    },
    records::{self, RecordError, TaskRecord},
    serialization,
    storage::Storage,
    task_file_ids::{collect_task_file_ids, list_task_file_paths},
};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashSet},
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
};

const ARCHIVED_TASK_DIRECTORIES: [&str; 2] = ["archive/tasks", "completed"];

#[derive(Clone, Copy)]
enum TaskMoveOperation {
    Archive,
    Restore,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaskFileEntry {
    pub task: TaskRecord,
    pub path: PathBuf,
}

#[derive(Clone)]
struct DraftFileEntry {
    id: String,
    task: TaskRecord,
    path: PathBuf,
}

struct VacatedLinkRecords {
    tasks: Vec<TaskFileEntry>,
    drafts: Vec<DraftFileEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskDeletionResult {
    pub success: bool,
    pub cleaned_task_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskDemotionResult {
    pub success: bool,
    pub cleaned_task_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AmbiguousTaskIdError {
    pub name: &'static str,
    pub id: String,
    pub candidates: Vec<String>,
    message: String,
}

impl AmbiguousTaskIdError {
    pub(crate) fn new(id: &str, mut candidates: Vec<String>) -> Self {
        candidates.sort();
        let canonical_id = canonical_task_id(id, "task");
        let message = format!(
            "Task ID {canonical_id} is ambiguous; {} files match:\n{}\nRun 'backlog doctor' to preview a safe repair.",
            candidates.len(),
            candidates
                .iter()
                .map(|candidate| format!("  - {candidate}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
        Self {
            name: "AmbiguousTaskIdError",
            id: canonical_id,
            candidates,
            message,
        }
    }
}

impl fmt::Display for AmbiguousTaskIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for AmbiguousTaskIdError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskStoreError {
    Ambiguous(AmbiguousTaskIdError),
    ArchiveFileExists { id: String, filename: String },
    Archived(String),
    HasSubtasks { id: String, child_ids: Vec<String> },
    Dependencies(String),
    NotArchived(String),
    DeleteActive(String),
    RestoreFileExists { id: String, filename: String },
    CreateLock(String),
    TaskLock(String),
    Storage(String),
    Serialization(String),
}

impl fmt::Display for TaskStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ambiguous(error) => error.fmt(formatter),
            Self::Archived(id) => write!(
                formatter,
                "Task {id} is archived. Archived tasks are read-only. Restore it to the board first: backlog task restore {id}"
            ),
            Self::Dependencies(message) => formatter.write_str(message),
            Self::HasSubtasks { id, child_ids } => write!(
                formatter,
                "Task {id} cannot be deleted while its subtasks exist: {}.",
                child_ids.join(", ")
            ),
            Self::DeleteActive(id) => write!(
                formatter,
                "Task {id} is not archived. A task must be archived before it can be deleted."
            ),
            Self::ArchiveFileExists { id, filename } => write!(
                formatter,
                "Task {id} cannot be archived: the archive already has a file named {filename}."
            ),
            Self::NotArchived(id) => write!(
                formatter,
                "Task {id} is not archived. It is already on the board."
            ),
            Self::RestoreFileExists { id, filename } => write!(
                formatter,
                "Task {id} cannot be restored: the board already has a file named {filename}."
            ),
            Self::CreateLock(message) | Self::TaskLock(message) => formatter.write_str(message),
            Self::Storage(message) | Self::Serialization(message) => formatter.write_str(message),
        }
    }
}

impl Error for TaskStoreError {}

fn draft_error(error: crate::draft_store::DraftStoreError) -> TaskStoreError {
    TaskStoreError::Storage(error.to_string())
}

fn task_links_target(task: &TaskRecord, target_id: &str, prefix: &str) -> bool {
    task.dependencies
        .iter()
        .any(|dependency| task_ids_equal(dependency, target_id, prefix))
        || task
            .references
            .iter()
            .any(|reference| is_exact_task_reference(reference, target_id, prefix))
}

fn is_exact_task_reference(reference: &str, target_id: &str, prefix: &str) -> bool {
    let reference = reference.trim();
    let Some((reference_prefix, _)) = reference.split_once('-') else {
        return false;
    };
    let Some((target_prefix, _)) = target_id.trim().split_once('-') else {
        return false;
    };
    !reference_prefix.is_empty()
        && reference_prefix.eq_ignore_ascii_case(target_prefix)
        && task_ids_equal(reference, target_id, prefix)
}

#[derive(Clone, Debug)]
pub struct TaskStore {
    project_root: PathBuf,
    backlog_relative_directory: PathBuf,
    backlog_directory: PathBuf,
    task_prefix: String,
    storage: Storage,
}

impl TaskStore {
    pub fn new(
        project_root: impl Into<PathBuf>,
        backlog_relative_directory: impl Into<PathBuf>,
        task_prefix: impl Into<String>,
    ) -> Self {
        let project_root = project_root.into();
        let backlog_relative_directory = backlog_relative_directory.into();
        let backlog_directory = project_root.join(&backlog_relative_directory);
        let common_git_dir = GitRepository::new(&project_root).common_directory();
        Self {
            storage: Storage::new(&backlog_directory, common_git_dir),
            project_root,
            backlog_relative_directory,
            backlog_directory,
            task_prefix: task_prefix.into(),
        }
    }

    /// Allocate an ID and create a task while holding the shared create lock.
    pub fn create_task(
        &self,
        mut task: TaskRecord,
        config: &Config,
    ) -> Result<TaskFileEntry, TaskStoreError> {
        let lock = self
            .storage
            .acquire_create_lock()
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::TimedOut {
                    TaskStoreError::CreateLock(
                        "Another task create/promote/demote operation is already in progress. Please try again.".into(),
                    )
                } else {
                    TaskStoreError::Storage(error.to_string())
                }
            })?;
        let existing_ids = self.occupied_task_ids(config);
        let id_refs: Vec<&str> = existing_ids.iter().map(String::as_str).collect();
        let zero_padding = config
            .zero_padded_ids
            .filter(|width| width.is_finite() && *width > 0.0)
            .map(|width| width as usize);
        task.id = if let Some(parent_id) = task.parent_task_id.as_deref() {
            generate_next_subtask_id(&id_refs, parent_id, &self.task_prefix, zero_padding)
        } else {
            generate_next_id(&id_refs, &self.task_prefix, zero_padding)
        };

        let draft_store = DraftStore::new(&self.project_root, &self.backlog_relative_directory);
        let dependencies = task.dependencies.clone();
        task.dependencies = crate::dependencies::validate_task_dependencies(
            self,
            &draft_store,
            &dependencies,
            &task,
            &self.task_prefix,
        )?;

        let path = self.task_path(&task.id, &task.title);
        let source = serialization::serialize_task(&task, "")
            .map_err(|error| TaskStoreError::Serialization(error.to_string()))?;
        let saved_task = records::parse_task(&source)
            .map_err(|error| TaskStoreError::Serialization(error.to_string()))?;
        lock.check()
            .map_err(|error| TaskStoreError::CreateLock(error.to_string()))?;
        self.storage
            .write_record(&path, &source, None)
            .map_err(|error| TaskStoreError::Storage(error.to_string()))?;
        Ok(TaskFileEntry {
            task: saved_task,
            path,
        })
    }

    /// Update a uniquely identified active task under its shared per-task entity lock.
    pub fn update_task(
        &self,
        mut task: TaskRecord,
    ) -> Result<Option<TaskFileEntry>, TaskStoreError> {
        let Some(current) = self.load_task(&task.id)? else {
            return Ok(None);
        };
        let lock = self
            .storage
            .acquire_entity_lock("task", &current.task.id, &current.path)
            .map_err(|error| {
                let message = if error.kind() == std::io::ErrorKind::WouldBlock {
                    format!(
                        "Edit failed: {} is being modified by another process; retry if appropriate.",
                        current.task.id
                    )
                } else if error.kind() == std::io::ErrorKind::NotFound {
                    format!(
                        "Edit failed: {} was moved or removed by another process.",
                        current.task.id
                    )
                } else {
                    error.to_string()
                };
                TaskStoreError::TaskLock(message)
            })?;
        let Some(current_after_lock) = self.load_task(&task.id)? else {
            return Ok(None);
        };
        if current_after_lock.path != current.path {
            return Err(TaskStoreError::TaskLock(format!(
                "Edit failed: {} was moved or removed by another process.",
                current.task.id
            )));
        }
        let current = current_after_lock;
        let original = self
            .storage
            .read(&current.path)
            .map_err(|error| TaskStoreError::Storage(error.to_string()))?;
        let existing_task = records::parse_task(&original)
            .map_err(|error| TaskStoreError::Serialization(error.to_string()))?;
        task.id = if !existing_task.id.is_empty()
            && task_ids_equal(&existing_task.id, &task.id, &self.task_prefix)
        {
            existing_task.id.clone()
        } else {
            current.task.id.clone()
        };
        if let (Some(existing_parent), Some(parent)) = (
            existing_task.parent_task_id.as_ref(),
            task.parent_task_id.as_mut(),
        ) && task_ids_equal(existing_parent, parent, &self.task_prefix)
        {
            *parent = existing_parent.clone();
        }
        if task.dependencies != existing_task.dependencies {
            let draft_store = DraftStore::new(&self.project_root, &self.backlog_relative_directory);
            let dependencies = task.dependencies.clone();
            task.dependencies = crate::dependencies::validate_task_dependencies(
                self,
                &draft_store,
                &dependencies,
                &task,
                &self.task_prefix,
            )?;
        }
        let source = serialization::serialize_task(&task, &original)
            .map_err(|error| TaskStoreError::Serialization(error.to_string()))?;
        let saved_task = records::parse_task(&source)
            .map_err(|error| TaskStoreError::Serialization(error.to_string()))?;
        lock.check().map_err(|_| {
            TaskStoreError::TaskLock(format!(
                "Edit failed: {} is being modified by another process; retry if appropriate.",
                current.task.id
            ))
        })?;
        self.storage
            .write_record(&current.path, &source, Some(&current.path))
            .map_err(|error| TaskStoreError::Storage(error.to_string()))?;
        Ok(Some(TaskFileEntry {
            task: saved_task,
            path: current.path,
        }))
    }

    /// Move an active task into the archive without rewriting its Markdown source.
    pub fn archive_task(&self, id: &str) -> Result<bool, TaskStoreError> {
        let Some(task) = self.load_task(id)? else {
            if self.find_archived_task(id)?.is_some() {
                return Err(TaskStoreError::Archived(id.to_owned()));
            }
            return Ok(false);
        };
        let task_lock = self.acquire_move_task_lock(&task)?;
        let create_lock = self
            .storage
            .acquire_create_lock()
            .map_err(|error| TaskStoreError::CreateLock(error.to_string()))?;
        let Some(current) = self.load_task(id)? else {
            return Ok(false);
        };
        let destination = self
            .backlog_directory
            .join("archive/tasks")
            .join(current.path.file_name().unwrap_or_default());
        task_lock.check().map_err(|error| {
            TaskStoreError::TaskLock(format!("Archive failed: {}: {error}", current.task.id))
        })?;
        create_lock.check().map_err(|error| {
            TaskStoreError::CreateLock(format!("Archive failed: {}: {error}", current.task.id))
        })?;
        self.storage
            .move_record(&current.path, &destination)
            .map_err(|error| {
                move_error(error, &current, TaskMoveOperation::Archive, &destination)
            })?;
        Ok(true)
    }

    /// Restore an archived task to the active directory without rewriting its Markdown source.
    pub fn restore_task(&self, id: &str) -> Result<bool, TaskStoreError> {
        if let Some(active) = self.load_task(id)? {
            return Err(TaskStoreError::NotArchived(active.task.id));
        }
        let Some(task) = self.find_archived_task(id)? else {
            return Ok(false);
        };
        let task_lock = self.acquire_move_task_lock(&task)?;
        let create_lock = self
            .storage
            .acquire_create_lock()
            .map_err(|error| TaskStoreError::CreateLock(error.to_string()))?;
        let Some(current) = self.find_archived_task(id)? else {
            return Ok(false);
        };
        let destination = self
            .backlog_directory
            .join("tasks")
            .join(current.path.file_name().unwrap_or_default());
        task_lock.check().map_err(|error| {
            TaskStoreError::TaskLock(format!("Restore failed: {}: {error}", current.task.id))
        })?;
        create_lock.check().map_err(|error| {
            TaskStoreError::CreateLock(format!("Restore failed: {}: {error}", current.task.id))
        })?;
        self.storage
            .move_record(&current.path, &destination)
            .map_err(|error| {
                move_error(error, &current, TaskMoveOperation::Restore, &destination)
            })?;
        Ok(true)
    }

    /// Promote a unique draft while holding its file lock and the shared ID allocator lock.
    pub fn promote_draft(&self, id: &str, config: &Config) -> Result<bool, TaskStoreError> {
        self.promote_draft_inner(id, config, None)
    }

    /// Promote a draft for a status edit, validating links against its newly allocated task ID.
    pub fn promote_draft_for_status_update(
        &self,
        id: &str,
        status: &str,
        config: &Config,
    ) -> Result<bool, TaskStoreError> {
        self.promote_draft_inner(id, config, Some(status))
    }

    fn promote_draft_inner(
        &self,
        id: &str,
        config: &Config,
        status_override: Option<&str>,
    ) -> Result<bool, TaskStoreError> {
        let drafts = DraftStore::new(&self.project_root, &self.backlog_relative_directory);
        let Some(source_path) = drafts.resolve_path(id).map_err(draft_error)? else {
            return Ok(false);
        };
        let filename_id = drafts.filename_id(&source_path).map_err(draft_error)?;
        let draft_lock = self
            .storage
            .acquire_entity_lock("draft", &filename_id, &source_path)
            .map_err(|error| TaskStoreError::TaskLock(error.to_string()))?;
        let create_lock = self
            .storage
            .acquire_create_lock()
            .map_err(|error| TaskStoreError::CreateLock(error.to_string()))?;
        let Some(current_path) = drafts.resolve_path(id).map_err(draft_error)? else {
            return Ok(false);
        };
        if current_path != source_path {
            return Err(TaskStoreError::TaskLock(format!(
                "Draft {filename_id} was moved or removed by another process."
            )));
        }
        let original = self
            .storage
            .read(&source_path)
            .map_err(|error| TaskStoreError::Storage(error.to_string()))?;
        let mut task = match records::parse_task(&original) {
            Ok(task) => task,
            Err(_) => return Ok(false),
        };
        let existing_ids = self.occupied_task_ids(config);
        let existing_refs: Vec<_> = existing_ids.iter().map(String::as_str).collect();
        let padding = config
            .zero_padded_ids
            .filter(|width| width.is_finite() && *width > 0.0)
            .map(|width| width as usize);
        task.id = if let Some(parent) = task.parent_task_id.as_deref() {
            generate_next_subtask_id(&existing_refs, parent, &self.task_prefix, padding)
        } else {
            generate_next_id(&existing_refs, &self.task_prefix, padding)
        };
        if let Some(status) = status_override {
            task.status = status.to_owned();
        } else if task.status.trim().is_empty() || task.status.eq_ignore_ascii_case("draft") {
            task.status = config
                .default_status
                .clone()
                .unwrap_or_else(|| "To Do".into());
        }
        if status_override.is_some() {
            crate::dependencies::validate_transition_dependencies(
                self,
                &drafts,
                &task.dependencies,
                &task,
                &self.task_prefix,
            )?;
        }
        let target = self.task_path(&task.id, &task.title);
        let source = serialization::serialize_task(&task, &original)
            .map_err(|error| TaskStoreError::Serialization(error.to_string()))?;
        draft_lock
            .check()
            .map_err(|error| TaskStoreError::TaskLock(error.to_string()))?;
        create_lock
            .check()
            .map_err(|error| TaskStoreError::CreateLock(error.to_string()))?;
        self.storage
            .write_record(&target, &source, None)
            .map_err(|error| TaskStoreError::Storage(error.to_string()))?;
        if let Err(error) = draft_lock.check().and_then(|()| create_lock.check()) {
            let _ = fs::remove_file(&target);
            return Err(TaskStoreError::TaskLock(error.to_string()));
        }
        if let Err(error) = fs::remove_file(&source_path) {
            let _ = fs::remove_file(&target);
            return Err(TaskStoreError::Storage(error.to_string()));
        }
        Ok(true)
    }

    /// Demote an active task and clean other records before releasing its task ID.
    pub fn demote_task(
        &self,
        id: &str,
        config: &Config,
    ) -> Result<TaskDemotionResult, TaskStoreError> {
        self.demote_task_inner(id, config, false)
    }

    /// Demote a task for a status edit, validating links against its new draft identity.
    pub fn demote_task_for_status_update(
        &self,
        id: &str,
        config: &Config,
    ) -> Result<TaskDemotionResult, TaskStoreError> {
        self.demote_task_inner(id, config, true)
    }

    fn demote_task_inner(
        &self,
        id: &str,
        config: &Config,
        validate_transition_dependencies: bool,
    ) -> Result<TaskDemotionResult, TaskStoreError> {
        let Some(target) = self.load_task(id)? else {
            if self.find_archived_task(id)?.is_some() {
                return Err(TaskStoreError::Archived(id.to_owned()));
            }
            return Ok(TaskDemotionResult {
                success: false,
                cleaned_task_ids: Vec::new(),
            });
        };
        for _ in 0..5 {
            let Some(current) = self.load_task(&target.task.id)? else {
                return Ok(TaskDemotionResult {
                    success: false,
                    cleaned_task_ids: Vec::new(),
                });
            };
            let mut candidates = self.vacated_link_records(&current.task.id);
            candidates.tasks.push(current.clone());
            candidates
                .tasks
                .sort_by_key(|entry| entry.task.id.trim().to_lowercase());
            candidates
                .tasks
                .dedup_by(|left, right| left.path == right.path);
            candidates
                .drafts
                .sort_by_key(|entry| entry.id.trim().to_lowercase());
            let mut task_locks = Vec::new();
            let mut locked_ids = HashSet::new();
            for entry in &candidates.tasks {
                if locked_ids.insert(entry.task.id.trim().to_lowercase()) {
                    task_locks.push(self.acquire_move_task_lock(entry)?);
                }
            }
            let mut draft_locks = Vec::new();
            for draft in &candidates.drafts {
                draft_locks.push(
                    self.storage
                        .acquire_entity_lock("draft", &draft.id, &draft.path)
                        .map_err(|error| TaskStoreError::TaskLock(error.to_string()))?,
                );
            }
            let create_lock = self
                .storage
                .acquire_create_lock()
                .map_err(|error| TaskStoreError::CreateLock(error.to_string()))?;
            let Some(current_after_lock) = self.load_task(&target.task.id)? else {
                if self.find_archived_task(&target.task.id)?.is_some() {
                    return Err(TaskStoreError::Archived(target.task.id.clone()));
                }
                return Ok(TaskDemotionResult {
                    success: false,
                    cleaned_task_ids: Vec::new(),
                });
            };
            if current_after_lock.path != current.path {
                return Err(TaskStoreError::TaskLock(format!(
                    "Task {} was moved or removed by another process.",
                    current.task.id
                )));
            }
            let fresh = self.vacated_link_records(&current.task.id);
            if fresh.tasks.iter().any(|entry| {
                !candidates
                    .tasks
                    .iter()
                    .any(|locked| locked.path == entry.path)
            }) || fresh.drafts.iter().any(|entry| {
                !candidates
                    .drafts
                    .iter()
                    .any(|locked| locked.path == entry.path)
            }) {
                drop(create_lock);
                drop(draft_locks);
                drop(task_locks);
                continue;
            }
            let original = self
                .storage
                .read(&current.path)
                .map_err(|error| TaskStoreError::Storage(error.to_string()))?;
            let mut demoted = records::parse_task(&original)
                .map_err(|error| TaskStoreError::Serialization(error.to_string()))?;
            let draft_store = DraftStore::new(&self.project_root, &self.backlog_relative_directory);
            let occupied = draft_store.occupied_ids();
            let occupied_refs: Vec<_> = occupied.iter().map(String::as_str).collect();
            let padding = config
                .zero_padded_ids
                .filter(|width| width.is_finite() && *width > 0.0)
                .map(|width| width as usize);
            demoted.id = generate_next_id(&occupied_refs, "draft", padding);
            if validate_transition_dependencies {
                demoted.status = "Draft".into();
            }
            demoted.dependencies.retain(|dependency| {
                !task_ids_equal(dependency, &current.task.id, &self.task_prefix)
            });
            demoted.references.retain(|reference| {
                !is_exact_task_reference(reference, &current.task.id, &self.task_prefix)
            });
            if validate_transition_dependencies {
                crate::dependencies::validate_transition_dependencies(
                    self,
                    &draft_store,
                    &demoted.dependencies,
                    &demoted,
                    &self.task_prefix,
                )?;
            }
            let target_draft = draft_store.draft_path(&demoted.id, &demoted.title);
            let contents = serialization::serialize_task(&demoted, &original)
                .map_err(|error| TaskStoreError::Serialization(error.to_string()))?;
            for lock in &task_locks {
                lock.check()
                    .map_err(|error| TaskStoreError::TaskLock(error.to_string()))?;
            }
            for lock in &draft_locks {
                lock.check()
                    .map_err(|error| TaskStoreError::TaskLock(error.to_string()))?;
            }
            create_lock
                .check()
                .map_err(|error| TaskStoreError::CreateLock(error.to_string()))?;
            self.storage
                .write_record(&target_draft, &contents, None)
                .map_err(|error| TaskStoreError::Storage(error.to_string()))?;
            let lock_check = task_locks
                .iter()
                .try_for_each(|lock| lock.check())
                .and_then(|()| draft_locks.iter().try_for_each(|lock| lock.check()))
                .and_then(|()| create_lock.check());
            if let Err(error) = lock_check {
                let _ = fs::remove_file(&target_draft);
                return Err(TaskStoreError::TaskLock(error.to_string()));
            }
            if let Err(error) = fs::remove_file(&current.path) {
                let _ = fs::remove_file(&target_draft);
                return Err(TaskStoreError::Storage(error.to_string()));
            }
            let mut cleaned_task_ids = Vec::new();
            let records_to_clean: Vec<(&Path, &str)> = fresh
                .tasks
                .iter()
                .map(|entry| (entry.path.as_path(), entry.task.id.as_str()))
                .chain(
                    fresh
                        .drafts
                        .iter()
                        .map(|entry| (entry.path.as_path(), entry.task.id.as_str())),
                )
                .collect();
            for (path, record_id) in records_to_clean {
                if let Some(cleaned_id) =
                    self.clean_vacated_links(path, record_id, &current.task.id, &create_lock)?
                {
                    cleaned_task_ids.push(cleaned_id);
                }
            }
            for lock in &task_locks {
                lock.check()
                    .map_err(|error| TaskStoreError::TaskLock(error.to_string()))?;
            }
            for lock in &draft_locks {
                lock.check()
                    .map_err(|error| TaskStoreError::TaskLock(error.to_string()))?;
            }
            create_lock
                .check()
                .map_err(|error| TaskStoreError::CreateLock(error.to_string()))?;
            return Ok(TaskDemotionResult {
                success: true,
                cleaned_task_ids,
            });
        }
        Err(TaskStoreError::TaskLock(format!(
            "Could not take a stable set of locks to clean references to {}. Retry once the tasks and drafts referencing it stop changing.",
            target.task.id
        )))
    }

    /// Delete one archived task after removing every local link that would bind to its freed ID.
    pub fn delete_task(&self, id: &str) -> Result<TaskDeletionResult, TaskStoreError> {
        if let Some(active) = self.load_task(id)? {
            return Err(TaskStoreError::DeleteActive(active.task.id));
        }
        let Some(target) = self.find_archived_task(id)? else {
            return Ok(TaskDeletionResult {
                success: false,
                cleaned_task_ids: Vec::new(),
            });
        };

        for _ in 0..5 {
            let current = self.find_archived_task(&target.task.id)?;
            let Some(current) = current else {
                return Ok(TaskDeletionResult {
                    success: false,
                    cleaned_task_ids: Vec::new(),
                });
            };
            let mut candidates = self.vacated_link_records(&current.task.id);
            candidates.tasks.push(current.clone());
            candidates
                .tasks
                .sort_by_key(|entry| entry.task.id.trim().to_lowercase());
            candidates
                .tasks
                .dedup_by(|left, right| left.path == right.path);
            let mut task_locks = Vec::new();
            let mut locked_ids = HashSet::new();
            for entry in &candidates.tasks {
                if locked_ids.insert(entry.task.id.trim().to_lowercase()) {
                    task_locks.push(self.acquire_move_task_lock(entry)?);
                }
            }
            candidates
                .drafts
                .sort_by_key(|entry| entry.id.trim().to_lowercase());
            let mut draft_locks = Vec::new();
            for draft in &candidates.drafts {
                draft_locks.push(
                    self.storage
                        .acquire_entity_lock("draft", &draft.id, &draft.path)
                        .map_err(|error| TaskStoreError::TaskLock(error.to_string()))?,
                );
            }
            let create_lock = self
                .storage
                .acquire_create_lock()
                .map_err(|error| TaskStoreError::CreateLock(error.to_string()))?;

            let Some(current) = self.find_archived_task(&target.task.id)? else {
                return Ok(TaskDeletionResult {
                    success: false,
                    cleaned_task_ids: Vec::new(),
                });
            };
            let fresh = self.vacated_link_records(&current.task.id);
            if fresh.tasks.iter().any(|entry| {
                !candidates
                    .tasks
                    .iter()
                    .any(|locked| locked.path == entry.path)
            }) || fresh.drafts.iter().any(|entry| {
                !candidates
                    .drafts
                    .iter()
                    .any(|locked| locked.path == entry.path)
            }) {
                drop(create_lock);
                drop(draft_locks);
                drop(task_locks);
                continue;
            }
            let mut children: Vec<String> = self
                .list_tasks()
                .into_iter()
                .chain(self.list_archived_tasks())
                .map(|entry| entry.task)
                .chain(fresh.drafts.iter().map(|entry| entry.task.clone()))
                .filter(|task| {
                    task.parent_task_id.as_deref().is_some_and(|parent| {
                        task_ids_equal(parent, &current.task.id, &self.task_prefix)
                    })
                })
                .map(|task| task.id)
                .collect();
            children.sort_by(|left, right| compare_task_ids(left, right));
            children.dedup();
            if !children.is_empty() {
                return Err(TaskStoreError::HasSubtasks {
                    id: current.task.id,
                    child_ids: children,
                });
            }

            for lock in &task_locks {
                lock.check()
                    .map_err(|error| TaskStoreError::TaskLock(error.to_string()))?;
            }
            for lock in &draft_locks {
                lock.check()
                    .map_err(|error| TaskStoreError::TaskLock(error.to_string()))?;
            }
            create_lock
                .check()
                .map_err(|error| TaskStoreError::CreateLock(error.to_string()))?;

            let mut cleaned_task_ids = Vec::new();
            let records_to_clean: Vec<(&Path, &str)> = fresh
                .tasks
                .iter()
                .map(|entry| (entry.path.as_path(), entry.task.id.as_str()))
                .chain(
                    fresh
                        .drafts
                        .iter()
                        .map(|entry| (entry.path.as_path(), entry.task.id.as_str())),
                )
                .collect();
            for (path, record_id) in records_to_clean {
                if let Some(cleaned_id) =
                    self.clean_vacated_links(path, record_id, &current.task.id, &create_lock)?
                {
                    cleaned_task_ids.push(cleaned_id);
                }
            }
            for lock in &task_locks {
                lock.check()
                    .map_err(|error| TaskStoreError::TaskLock(error.to_string()))?;
            }
            for lock in &draft_locks {
                lock.check()
                    .map_err(|error| TaskStoreError::TaskLock(error.to_string()))?;
            }
            create_lock
                .check()
                .map_err(|error| TaskStoreError::CreateLock(error.to_string()))?;
            fs::remove_file(&current.path)
                .map_err(|error| TaskStoreError::Storage(error.to_string()))?;
            return Ok(TaskDeletionResult {
                success: true,
                cleaned_task_ids,
            });
        }
        Err(TaskStoreError::TaskLock(format!(
            "Could not take a stable set of locks to clean references to {}. Retry once the tasks and drafts referencing it stop changing.",
            target.task.id
        )))
    }

    fn vacated_link_records(&self, target_id: &str) -> VacatedLinkRecords {
        let tasks: Vec<_> =
            self.list_tasks()
                .into_iter()
                .chain(self.list_archived_tasks())
                .filter(|entry| !task_ids_equal(&entry.task.id, target_id, &self.task_prefix))
                .filter(|entry| {
                    task_links_target(&entry.task, target_id, &self.task_prefix)
                        || entry.task.parent_task_id.as_deref().is_some_and(|parent| {
                            task_ids_equal(parent, target_id, &self.task_prefix)
                        })
                })
                .collect();
        let drafts = crate::draft_store::DraftStore::new(
            &self.project_root,
            &self.backlog_relative_directory,
        )
        .healthy_unique_drafts()
        .into_iter()
        .map(|(id, entry)| DraftFileEntry {
            id,
            task: entry.task,
            path: entry.path,
        })
        .filter(|entry| {
            task_links_target(&entry.task, target_id, &self.task_prefix)
                || entry
                    .task
                    .parent_task_id
                    .as_deref()
                    .is_some_and(|parent| task_ids_equal(parent, target_id, &self.task_prefix))
        })
        .collect();
        VacatedLinkRecords { tasks, drafts }
    }

    fn clean_vacated_links(
        &self,
        path: &Path,
        record_id: &str,
        target_id: &str,
        create_lock: &crate::storage::FileLock,
    ) -> Result<Option<String>, TaskStoreError> {
        let source = self
            .storage
            .read(path)
            .map_err(|error| TaskStoreError::Storage(error.to_string()))?;
        let record = records::parse_task(&source)
            .map_err(|error| TaskStoreError::Serialization(error.to_string()))?;
        let dependencies: Vec<_> = record
            .dependencies
            .iter()
            .filter(|dependency| !task_ids_equal(dependency, target_id, &self.task_prefix))
            .cloned()
            .collect();
        let references: Vec<_> = record
            .references
            .iter()
            .filter(|reference| !is_exact_task_reference(reference, target_id, &self.task_prefix))
            .cloned()
            .collect();
        if dependencies == record.dependencies && references == record.references {
            return Ok(None);
        }
        let changes = BTreeMap::from([
            (
                "dependencies".to_owned(),
                Some(serde_json::json!(dependencies)),
            ),
            ("references".to_owned(), Some(serde_json::json!(references))),
            (
                "updated_date".to_owned(),
                Some(serde_json::json!(utc_timestamp())),
            ),
        ]);
        let updated = crate::frontmatter::update_frontmatter(&source, &changes)
            .map_err(|error| TaskStoreError::Serialization(error.to_string()))?;
        create_lock
            .check()
            .map_err(|error| TaskStoreError::CreateLock(error.to_string()))?;
        self.storage
            .write_record(path, &updated, Some(path))
            .map_err(|error| TaskStoreError::Storage(error.to_string()))?;
        Ok(Some(record_id.to_owned()))
    }

    fn acquire_move_task_lock(
        &self,
        task: &TaskFileEntry,
    ) -> Result<crate::storage::FileLock, TaskStoreError> {
        self.storage
            .acquire_entity_lock("task", &task.task.id, &task.path)
            .map_err(|error| {
                TaskStoreError::TaskLock(if error.kind() == std::io::ErrorKind::WouldBlock {
                    format!(
                        "Task {} is being modified by another process; retry if appropriate.",
                        task.task.id
                    )
                } else {
                    error.to_string()
                })
            })
    }

    fn find_archived_task(&self, id: &str) -> Result<Option<TaskFileEntry>, TaskStoreError> {
        let matches: Vec<_> = self
            .list_archived_tasks()
            .into_iter()
            .filter(|entry| task_ids_equal(id, &entry.task.id, &self.task_prefix))
            .collect();
        if matches.len() > 1 {
            return Err(TaskStoreError::Ambiguous(AmbiguousTaskIdError::new(
                id,
                matches
                    .iter()
                    .map(|entry| entry.path.display().to_string())
                    .collect(),
            )));
        }
        Ok(matches.into_iter().next())
    }

    fn task_path(&self, id: &str, title: &str) -> PathBuf {
        self.backlog_directory.join("tasks").join(format!(
            "{} - {}.md",
            id.to_lowercase(),
            sanitize_filename(title)
        ))
    }

    fn branch_task_file_ids(&self, config: &Config) -> Vec<String> {
        if config.check_active_branches == Some(false) || config.filesystem_only == Some(true) {
            return Vec::new();
        }
        let backlog_directory = self
            .backlog_relative_directory
            .to_string_lossy()
            .replace('\\', "/");
        let backlog_directory = backlog_directory.trim_matches('/').trim_start_matches("./");
        if backlog_directory.is_empty() {
            return Vec::new();
        }

        let git = GitRepository::new(&self.project_root);
        let include_origin = config.remote_operations != Some(false);
        if include_origin {
            // Remote refresh failures are non-fatal; the TypeScript allocator uses
            // whatever refs are available when fetching cannot complete.
            git.fetch_origin();
        }
        let current_branch = git.current_branch();
        let mut ids = Vec::new();
        for tip in git.recent_branch_tips(config.active_branch_days.unwrap_or(30.0), include_origin)
        {
            if tip.current || tip.name == current_branch {
                continue;
            }
            for path in git.files_in_tree(&tip.commit, backlog_directory) {
                let relative = path
                    .strip_prefix(&format!("{backlog_directory}/"))
                    .unwrap_or(&path);
                let is_task_record = ["tasks/", "archive/tasks/", "completed/"]
                    .iter()
                    .any(|directory| relative.starts_with(directory));
                if !is_task_record {
                    continue;
                }
                let Some(filename) = relative.rsplit('/').next() else {
                    continue;
                };
                if let Some(id) = extract_id_from_filename(filename, &self.task_prefix) {
                    ids.push(id);
                }
            }
        }
        ids
    }

    pub fn list_tasks(&self) -> Vec<TaskFileEntry> {
        let entries =
            list_task_file_paths(&self.backlog_directory.join("tasks"), &self.task_prefix)
                .into_iter()
                .filter_map(|path| self.read_task(&path, true).ok().flatten())
                .collect();
        sort_entries(entries)
    }

    pub fn list_archived_tasks(&self) -> Vec<TaskFileEntry> {
        let mut entries = Vec::new();
        for directory in self.archived_task_directories() {
            entries.extend(
                list_task_file_paths(&directory, &self.task_prefix)
                    .into_iter()
                    .filter_map(|path| self.read_task(&path, false).ok().flatten()),
            );
        }
        sort_entries(entries)
    }

    pub fn load_task(&self, id: &str) -> Result<Option<TaskFileEntry>, TaskStoreError> {
        let active = self.list_tasks();
        let archived = self.list_archived_tasks();
        let identity_matches: Vec<&TaskFileEntry> = active
            .iter()
            .chain(&archived)
            .filter(|entry| task_ids_equal(id, &entry.task.id, "task"))
            .collect();

        if identity_matches.len() > 1 {
            return Err(TaskStoreError::Ambiguous(AmbiguousTaskIdError::new(
                id,
                identity_matches
                    .iter()
                    .map(|entry| entry.path.display().to_string())
                    .collect(),
            )));
        }
        if let Some(active_match) = active
            .iter()
            .find(|entry| task_ids_equal(id, &entry.task.id, "task"))
        {
            return Ok(Some(active_match.clone()));
        }
        if !identity_matches.is_empty() {
            return Ok(None);
        }

        let mut active_match = None;
        let mut candidates = Vec::new();
        for path in self.all_task_paths() {
            let filename = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            let Some(file_id) = extract_id_from_filename(filename, &self.task_prefix) else {
                continue;
            };
            if task_ids_equal(id, &file_id, "task") {
                active_match = active_match.or_else(|| {
                    path.starts_with(self.backlog_directory.join("tasks"))
                        .then(|| path.clone())
                });
                candidates.push(path.display().to_string());
            }
        }
        if candidates.len() > 1 {
            return Err(TaskStoreError::Ambiguous(AmbiguousTaskIdError::new(
                id, candidates,
            )));
        }
        let Some(path) = active_match else {
            return Ok(None);
        };
        self.read_task(&path, true)
    }

    fn all_task_paths(&self) -> Vec<PathBuf> {
        let mut paths =
            list_task_file_paths(&self.backlog_directory.join("tasks"), &self.task_prefix);
        for directory in self.archived_task_directories() {
            paths.extend(list_task_file_paths(&directory, &self.task_prefix));
        }
        paths
    }

    fn occupied_task_ids(&self, config: &Config) -> Vec<String> {
        let mut ids: Vec<String> = self
            .all_task_paths()
            .iter()
            .filter_map(|path| self.storage.read(path).ok())
            .filter_map(|source| records::parse_task(&source).ok())
            .map(|record| record.id)
            .collect();
        ids.extend(collect_task_file_ids(
            &self.project_root,
            &self.backlog_relative_directory,
            &self.task_prefix,
        ));
        ids.extend(self.branch_task_file_ids(config));
        ids
    }

    fn archived_task_directories(&self) -> Vec<PathBuf> {
        let mut directories = Vec::new();
        let mut real_directories = HashSet::new();
        for relative in ARCHIVED_TASK_DIRECTORIES {
            let directory = self.backlog_directory.join(relative);
            let Ok(real_directory) = fs::canonicalize(&directory) else {
                continue;
            };
            if real_directories.insert(real_directory) {
                directories.push(directory);
            }
        }
        directories
    }

    fn read_task(
        &self,
        path: &Path,
        normalize: bool,
    ) -> Result<Option<TaskFileEntry>, TaskStoreError> {
        let Ok(source) = self.storage.read(path) else {
            return Ok(None);
        };
        let mut task = match records::parse_task(&source) {
            Ok(task) => task,
            Err(error) if is_dependency_error(&error) => {
                return Err(TaskStoreError::Dependencies(error.to_string()));
            }
            Err(_) => return Ok(None),
        };
        if normalize {
            task.id = normalize_task_id(&task.id, "task");
            if let Some(parent) = task.parent_task_id.as_mut() {
                *parent = normalize_task_id(parent, "task");
            }
        }
        Ok(Some(TaskFileEntry {
            task,
            path: path.to_owned(),
        }))
    }
}

pub(crate) fn utc_timestamp() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = (seconds / 86_400) as i64;
    let day_seconds = seconds % 86_400;
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_part = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_part + 2) / 5 + 1;
    let month = month_part + if month_part < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:00Z",
        day_seconds / 3_600,
        (day_seconds % 3_600) / 60
    )
}

fn is_dependency_error(error: &RecordError) -> bool {
    error
        .to_string()
        .starts_with("Invalid dependencies in task ")
}

fn move_error(
    error: std::io::Error,
    task: &TaskFileEntry,
    operation: TaskMoveOperation,
    destination: &Path,
) -> TaskStoreError {
    if error.kind() == std::io::ErrorKind::AlreadyExists {
        let filename = destination
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        match operation {
            TaskMoveOperation::Archive => TaskStoreError::ArchiveFileExists {
                id: task.task.id.clone(),
                filename,
            },
            TaskMoveOperation::Restore => TaskStoreError::RestoreFileExists {
                id: task.task.id.clone(),
                filename,
            },
        }
    } else {
        TaskStoreError::Storage(error.to_string())
    }
}

fn sanitize_filename(title: &str) -> String {
    let sanitized: String = title
        .chars()
        .map(|character| {
            if "<>:\"/\\|?*".contains(character) {
                '-'
            } else if "'(),!@#$%^&+={}[];".contains(character) {
                '\0'
            } else if character.is_whitespace() {
                '-'
            } else {
                character
            }
        })
        .filter(|character| *character != '\0')
        .collect();
    let mut collapsed = String::new();
    for character in sanitized.chars() {
        if character != '-' || !collapsed.ends_with('-') {
            collapsed.push(character);
        }
    }
    let trimmed = collapsed.trim_matches('-');
    if trimmed.is_empty() {
        "untitled".into()
    } else {
        trimmed.into()
    }
}

fn sort_entries(mut entries: Vec<TaskFileEntry>) -> Vec<TaskFileEntry> {
    entries.sort_by(|left, right| compare_task_ids(&left.task.id, &right.task.id));
    entries
}

pub(crate) fn compare_task_ids(left: &str, right: &str) -> Ordering {
    let left_parts = sortable_id_parts(left);
    let right_parts = sortable_id_parts(right);
    for index in 0..left_parts.len().max(right_parts.len()) {
        let left_part = left_parts.get(index).map(String::as_str).unwrap_or("0");
        let right_part = right_parts.get(index).map(String::as_str).unwrap_or("0");
        let order = compare_decimal(left_part, right_part);
        if order != Ordering::Equal {
            return order;
        }
    }
    // Numeric locale comparison treats leading-zero and case-only spellings as equal; stable
    // sorting then retains their incoming order instead of imposing a lexical tie-breaker.
    if sortable_id_text(left) == sortable_id_text(right) {
        return Ordering::Equal;
    }
    left.to_lowercase()
        .cmp(&right.to_lowercase())
        .then_with(|| left.cmp(right))
}

fn sortable_id_text(id: &str) -> String {
    let mut result = String::new();
    let mut characters = id.chars().peekable();
    while let Some(character) = characters.next() {
        if character.is_ascii_digit() {
            let mut digits = String::from(character);
            while characters.peek().is_some_and(char::is_ascii_digit) {
                digits.push(characters.next().unwrap());
            }
            result.push_str(&canonical_decimal(&digits));
        } else {
            result.extend(character.to_lowercase());
        }
    }
    result
}

fn sortable_id_parts(id: &str) -> Vec<String> {
    let body = id
        .split_once('-')
        .filter(|(prefix, _)| {
            !prefix.is_empty() && prefix.bytes().all(|byte| byte.is_ascii_alphabetic())
        })
        .map_or(id, |(_, body)| body);
    let segments: Vec<&str> = body.split('.').collect();
    if segments
        .iter()
        .all(|segment| leading_digits(segment).is_some())
    {
        return segments
            .iter()
            .map(|segment| canonical_decimal(leading_digits(segment).unwrap_or("0")))
            .collect();
    }
    let trailing = body
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    vec![canonical_decimal(if trailing.is_empty() {
        "0"
    } else {
        &trailing
    })]
}

fn leading_digits(value: &str) -> Option<&str> {
    let end = value.bytes().take_while(u8::is_ascii_digit).count();
    (end > 0).then(|| &value[..end])
}

fn canonical_decimal(value: &str) -> String {
    let value = value.trim_start_matches('0');
    if value.is_empty() { "0" } else { value }.to_owned()
}

fn compare_decimal(left: &str, right: &str) -> Ordering {
    left.len().cmp(&right.len()).then_with(|| left.cmp(right))
}
