//! Read task and draft documentation backlinks for moved or removed content records.

use crate::{draft_store::DraftStore, task_store::TaskStore};
use std::path::Path;

pub(crate) fn list_documentation_references(
    project_root: &Path,
    backlog_directory: &Path,
    task_prefix: &str,
    paths: &[String],
) -> Vec<String> {
    let backlog_relative_directory = backlog_directory
        .strip_prefix(project_root)
        .unwrap_or(backlog_directory);
    let task_store = TaskStore::new(project_root, backlog_relative_directory, task_prefix);
    let draft_store = DraftStore::new(project_root, backlog_relative_directory);
    let matches = |task: &crate::records::TaskRecord| {
        task.documentation.iter().any(|entry| {
            let trimmed = entry.trim();
            let path = trimmed.strip_prefix("./").unwrap_or(trimmed);
            paths.iter().any(|project_path| {
                path == project_path || path.starts_with(&format!("{project_path}#"))
            })
        })
    };
    let mut seen = std::collections::HashSet::new();
    task_store
        .list_tasks()
        .into_iter()
        .chain(task_store.list_archived_tasks())
        .map(|entry| entry.task)
        .chain(
            draft_store
                .list_drafts()
                .into_iter()
                .map(|entry| entry.task),
        )
        .filter(|task| matches(task) && seen.insert(task.id.clone()))
        .map(|task| task.id)
        .collect()
}
