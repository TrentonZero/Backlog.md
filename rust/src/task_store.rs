//! Local task record reads with active/archive identity semantics.

use crate::{
    identity::{canonical_task_id, extract_id_from_filename, normalize_task_id, task_ids_equal},
    records::{self, RecordError, TaskRecord},
    storage::Storage,
    task_file_ids::list_task_file_paths,
};
use std::{
    cmp::Ordering,
    collections::HashSet,
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
};

const ARCHIVED_TASK_DIRECTORIES: [&str; 2] = ["archive/tasks", "completed"];

#[derive(Clone, Debug, PartialEq)]
pub struct TaskFileEntry {
    pub task: TaskRecord,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AmbiguousTaskIdError {
    pub name: &'static str,
    pub id: String,
    pub candidates: Vec<String>,
    message: String,
}

impl AmbiguousTaskIdError {
    fn new(id: &str, mut candidates: Vec<String>) -> Self {
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
    Dependencies(String),
}

impl fmt::Display for TaskStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ambiguous(error) => error.fmt(formatter),
            Self::Dependencies(message) => formatter.write_str(message),
        }
    }
}

impl Error for TaskStoreError {}

#[derive(Clone, Debug)]
pub struct TaskStore {
    backlog_directory: PathBuf,
    task_prefix: String,
    storage: Storage,
}

impl TaskStore {
    pub fn new(backlog_directory: impl Into<PathBuf>, task_prefix: impl Into<String>) -> Self {
        let backlog_directory = backlog_directory.into();
        Self {
            storage: Storage::new(&backlog_directory, None),
            backlog_directory,
            task_prefix: task_prefix.into(),
        }
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

fn is_dependency_error(error: &RecordError) -> bool {
    error
        .to_string()
        .starts_with("Invalid dependencies in task ")
}

fn sort_entries(mut entries: Vec<TaskFileEntry>) -> Vec<TaskFileEntry> {
    entries.sort_by(|left, right| compare_task_ids(&left.task.id, &right.task.id));
    entries
}

fn compare_task_ids(left: &str, right: &str) -> Ordering {
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
    left.to_lowercase()
        .cmp(&right.to_lowercase())
        .then_with(|| left.cmp(right))
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
