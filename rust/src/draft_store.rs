//! Local draft record reads and mutations.

use crate::{
    config::Config,
    git::GitRepository,
    identity::{canonical_task_id, extract_id_from_filename, generate_next_id, task_ids_equal},
    records::{self, TaskRecord},
    serialization,
    storage::Storage,
    task_file_ids::list_task_file_paths,
};
use std::{
    collections::HashMap,
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq)]
pub struct DraftFileEntry {
    pub task: TaskRecord,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DraftStoreError {
    Ambiguous { id: String, paths: Vec<String> },
    Lock(String),
    Storage(String),
    Serialization(String),
}

impl fmt::Display for DraftStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ambiguous { id, paths } => write!(
                f,
                "Draft ID {} is ambiguous; {} files match:\n{}",
                canonical_task_id(id, "draft"),
                paths.len(),
                paths
                    .iter()
                    .map(|path| format!("  - {path}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
            Self::Lock(message) | Self::Storage(message) | Self::Serialization(message) => {
                f.write_str(message)
            }
        }
    }
}
impl Error for DraftStoreError {}

#[derive(Clone, Debug)]
pub struct DraftStore {
    backlog_directory: PathBuf,
    storage: Storage,
}

impl DraftStore {
    pub fn new(
        project_root: impl Into<PathBuf>,
        backlog_relative_directory: impl Into<PathBuf>,
    ) -> Self {
        let project_root = project_root.into();
        let backlog_directory = project_root.join(backlog_relative_directory.into());
        let common_git_dir = GitRepository::new(&project_root).common_directory();
        Self {
            storage: Storage::new(&backlog_directory, common_git_dir),
            backlog_directory,
        }
    }

    pub fn list_drafts(&self) -> Vec<DraftFileEntry> {
        let mut entries: Vec<_> = self
            .paths()
            .into_iter()
            .filter_map(|path| self.read_entry(&path).ok())
            .collect();
        entries.sort_by(|a, b| crate::task_store::compare_task_ids(&a.task.id, &b.task.id));
        entries
    }

    pub fn load_draft(&self, id: &str) -> Result<Option<DraftFileEntry>, DraftStoreError> {
        let Some(path) = self.resolve_path(id)? else {
            return Ok(None);
        };
        match self.read_entry(&path) {
            Ok(entry) => Ok(Some(entry)),
            Err(_) => Ok(None),
        }
    }

    pub fn create_draft(
        &self,
        mut task: TaskRecord,
        config: &Config,
    ) -> Result<DraftFileEntry, DraftStoreError> {
        let lock = self
            .storage
            .acquire_create_lock()
            .map_err(|e| DraftStoreError::Lock(e.to_string()))?;
        let paths = self.paths();
        let mut ids: Vec<String> = paths
            .iter()
            .filter_map(|path| {
                path.file_name()?
                    .to_str()
                    .and_then(|name| extract_id_from_filename(name, "draft"))
            })
            .collect();
        ids.extend(
            paths
                .iter()
                .filter_map(|path| self.storage.read(path).ok())
                .filter_map(|source| records::parse_task(&source).ok())
                .map(|draft| draft.id),
        );
        let refs: Vec<_> = ids.iter().map(String::as_str).collect();
        let padding = config
            .zero_padded_ids
            .filter(|n| n.is_finite() && *n > 0.0)
            .map(|n| n as usize);
        task.id = generate_next_id(&refs, "draft", padding);
        task.status = "Draft".into();
        let path = self.draft_path(&task.id, &task.title);
        let source = serialization::serialize_task(&task, "")
            .map_err(|e| DraftStoreError::Serialization(e.to_string()))?;
        lock.check()
            .map_err(|e| DraftStoreError::Lock(e.to_string()))?;
        self.storage
            .write_record(&path, &source, None)
            .map_err(|e| DraftStoreError::Storage(e.to_string()))?;
        self.read_entry(&path)
    }

    pub fn update_draft(
        &self,
        mut task: TaskRecord,
    ) -> Result<Option<DraftFileEntry>, DraftStoreError> {
        let Some(current) = self.load_draft(&task.id)? else {
            return Ok(None);
        };
        let filename_id = self.filename_id(&current.path)?;
        if !task_ids_equal(&current.task.id, &filename_id, "draft") {
            return Err(DraftStoreError::Serialization(
                "Draft ID does not match its filename".into(),
            ));
        }
        let lock = self
            .storage
            .acquire_entity_lock("draft", &filename_id, &current.path)
            .map_err(|e| DraftStoreError::Lock(e.to_string()))?;
        let Some(after_lock) = self.load_draft(&filename_id)? else {
            return Ok(None);
        };
        if after_lock.path != current.path {
            return Err(DraftStoreError::Lock(format!(
                "Draft {} was moved or removed by another process.",
                current.task.id
            )));
        }
        if !task_ids_equal(&after_lock.task.id, &filename_id, "draft") {
            return Err(DraftStoreError::Serialization(
                "Draft ID does not match its filename".into(),
            ));
        }
        let original = self
            .storage
            .read(&current.path)
            .map_err(|e| DraftStoreError::Storage(e.to_string()))?;
        task.id = filename_id;
        task.status = "Draft".into();
        let source = serialization::serialize_task(&task, &original)
            .map_err(|e| DraftStoreError::Serialization(e.to_string()))?;
        let target = self.draft_path(&task.id, &task.title);
        lock.check()
            .map_err(|e| DraftStoreError::Lock(e.to_string()))?;
        self.storage
            .write_record(&target, &source, Some(&current.path))
            .map_err(|e| DraftStoreError::Storage(e.to_string()))?;
        self.read_entry(&target).map(Some)
    }

    pub(crate) fn healthy_unique_drafts(&self) -> Vec<(String, DraftFileEntry)> {
        let mut counts = HashMap::<String, usize>::new();
        let parsed: Vec<_> = self
            .paths()
            .into_iter()
            .filter_map(|path| {
                let filename = path.file_name()?.to_str()?;
                let filename_id = extract_id_from_filename(filename, "draft")?;
                let key = canonical_task_id(&filename_id, "draft");
                *counts.entry(key.clone()).or_default() += 1;
                let entry = self.read_entry(&path).ok()?;
                task_ids_equal(&entry.task.id, &filename_id, "draft").then_some((key, entry))
            })
            .collect();
        parsed
            .into_iter()
            .filter(|(key, _)| counts.get(key) == Some(&1))
            .collect()
    }

    fn paths(&self) -> Vec<PathBuf> {
        list_task_file_paths(&self.backlog_directory.join("drafts"), "draft")
    }

    fn resolve_path(&self, id: &str) -> Result<Option<PathBuf>, DraftStoreError> {
        let mut matches: Vec<_> = self
            .paths()
            .into_iter()
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .and_then(|name| extract_id_from_filename(name, "draft"))
                    .is_some_and(|file_id| task_ids_equal(&file_id, id, "draft"))
            })
            .collect();
        matches.sort();
        if matches.len() > 1 {
            return Err(DraftStoreError::Ambiguous {
                id: id.into(),
                paths: matches
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect(),
            });
        }
        Ok(matches.pop())
    }

    fn read_entry(&self, path: &Path) -> Result<DraftFileEntry, DraftStoreError> {
        let source = self
            .storage
            .read(path)
            .map_err(|e| DraftStoreError::Storage(e.to_string()))?;
        let task = records::parse_task(&source)
            .map_err(|e| DraftStoreError::Serialization(e.to_string()))?;
        Ok(DraftFileEntry {
            task,
            path: path.to_path_buf(),
        })
    }

    fn filename_id(&self, path: &Path) -> Result<String, DraftStoreError> {
        path.file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| extract_id_from_filename(name, "draft"))
            .ok_or_else(|| DraftStoreError::Serialization("Draft filename has no valid ID".into()))
    }

    fn draft_path(&self, id: &str, title: &str) -> PathBuf {
        self.backlog_directory.join("drafts").join(format!(
            "{} - {}.md",
            id.to_ascii_lowercase(),
            sanitize_filename(title)
        ))
    }
}

fn sanitize_filename(title: &str) -> String {
    let sanitized: String = title
        .chars()
        .map(|c| {
            if "<>:\"/\\|?*".contains(c) {
                '-'
            } else if "'(),!@#$%^&+={}[];".contains(c) {
                '\0'
            } else if c.is_whitespace() {
                '-'
            } else {
                c
            }
        })
        .filter(|c| *c != '\0')
        .collect();
    let mut collapsed = String::new();
    for c in sanitized.chars() {
        if c != '-' || !collapsed.ends_with('-') {
            collapsed.push(c);
        }
    }
    let trimmed = collapsed.trim_matches('-');
    if trimmed.is_empty() {
        "untitled".into()
    } else {
        trimmed.into()
    }
}
