//! Filename-based task ID reservations from this project and its sibling worktrees.

use crate::{git::GitRepository, identity::extract_id_from_filename};
use std::{fs, path::Path};

const TASK_DIRECTORIES: [&str; 3] = ["tasks", "archive/tasks", "completed"];

/// Collect IDs claimed by task filenames without reading record contents.
///
/// Active and archived files reserve their filename ID even when their frontmatter is malformed or
/// claims a different ID. In a Git repository, the same project-relative path is scanned in each
/// linked worktree so uncommitted files reserve IDs too.
pub fn collect_task_file_ids(
    project_root: &Path,
    backlog_directory: &Path,
    task_prefix: &str,
) -> Vec<String> {
    let mut ids = Vec::new();
    scan_project(project_root, backlog_directory, task_prefix, &mut ids);

    let git = GitRepository::new(project_root);
    let Some(repository_root) = git.repository_root() else {
        return ids;
    };
    let Ok(project_relative) = project_root.strip_prefix(&repository_root) else {
        return ids;
    };

    for worktree_root in git.worktree_paths() {
        let worktree_project = worktree_root.join(project_relative);
        if worktree_project != project_root {
            scan_project(&worktree_project, backlog_directory, task_prefix, &mut ids);
        }
    }
    ids
}

fn scan_project(
    project_root: &Path,
    backlog_directory: &Path,
    task_prefix: &str,
    ids: &mut Vec<String>,
) {
    for directory in TASK_DIRECTORIES {
        let path = project_root.join(backlog_directory).join(directory);
        scan_directory(&path, task_prefix, ids);
    }
}

fn scan_directory(directory: &Path, task_prefix: &str, ids: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let pattern_prefix = format!("{}-", task_prefix.to_ascii_lowercase());
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let Some(filename) = path.file_name().and_then(|filename| filename.to_str()) else {
            continue;
        };
        let prefix_matches = if cfg!(windows) {
            filename
                .get(..pattern_prefix.len())
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(&pattern_prefix))
        } else {
            filename.starts_with(&pattern_prefix)
        };
        let file_matches = file_type.is_file()
            || (file_type.is_symlink()
                && fs::metadata(&path).is_ok_and(|metadata| metadata.is_file()));
        if file_matches
            && path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
            && prefix_matches
            && let Some(id) = extract_id_from_filename(filename, task_prefix)
        {
            ids.push(id);
        }
    }
}
