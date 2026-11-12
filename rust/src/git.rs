//! Repository information and selected-path commits through the installed Git executable.

use crate::config::Config;
use std::process::{Command, Output};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitBranchTip {
    pub name: String,
    pub commit: String,
    pub current: bool,
    pub committed_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GitIndexEntry {
    mode: String,
    object_id: String,
    stage: String,
}

#[derive(Clone, Debug)]
pub struct GitRepository {
    project_root: PathBuf,
    executable: PathBuf,
}

impl GitRepository {
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        Self::with_executable(project_root, "git")
    }

    pub fn with_executable(
        project_root: impl Into<PathBuf>,
        executable: impl Into<PathBuf>,
    ) -> Self {
        Self {
            project_root: project_root.into(),
            executable: executable.into(),
        }
    }

    pub fn is_repository(&self) -> bool {
        self.run(&["rev-parse", "--git-dir"])
            .is_some_and(|output| output.status.success())
    }

    pub fn repository_root(&self) -> Option<PathBuf> {
        let output = self.run(&["rev-parse", "--show-toplevel"])?;
        if !output.status.success() {
            return None;
        }
        let root = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        (!root.is_empty()).then(|| PathBuf::from(root))
    }

    /// Return Git's shared directory for lock files coordinated across linked worktrees.
    pub fn common_directory(&self) -> Option<PathBuf> {
        let output = self.run(&["rev-parse", "--git-common-dir"])?;
        if !output.status.success() {
            return None;
        }
        let directory = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if directory.is_empty() {
            return None;
        }
        let path = PathBuf::from(directory);
        Some(if path.is_absolute() {
            path
        } else {
            self.project_root.join(path)
        })
    }

    /// Refresh the configured origin using the repository's Git transport settings.
    pub fn fetch_origin(&self) -> bool {
        let Some(remote) = self.run(&["remote", "get-url", "origin"]) else {
            return false;
        };
        if !remote.status.success() || String::from_utf8_lossy(&remote.stdout).trim().is_empty() {
            return false;
        }
        let Ok(mut child) = Command::new(&self.executable)
            .args([
                "fetch",
                "origin",
                "--prune",
                "--quiet",
                "--no-recurse-submodules",
            ])
            .current_dir(&self.project_root)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GCM_INTERACTIVE", "Never")
            .stdin(std::process::Stdio::null())
            .spawn()
        else {
            return false;
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return status.success(),
                Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return false;
                }
                Ok(None) => {}
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    /// List current and recent local and configured origin branches in a stable order.
    pub fn recent_branch_tips(
        &self,
        active_branch_days: f64,
        include_origin: bool,
    ) -> Vec<GitBranchTip> {
        let mut args = vec![
            "for-each-ref",
            "--format=%(HEAD)%00%(refname:short)%00%(objectname)%00%(committerdate:unix)",
            "refs/heads",
        ];
        if include_origin {
            args.push("refs/remotes/origin");
        }
        let Some(output) = self.run(&args) else {
            return Vec::new();
        };
        if !output.status.success() {
            return Vec::new();
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        let cutoff = now - active_branch_days * 86_400.0;
        let mut tips: Vec<_> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| {
                let mut fields = line.split('\0');
                let head = fields.next()?;
                let name = fields.next()?.trim();
                let commit = fields.next()?.trim();
                let committed_at = fields.next()?.trim().parse::<i64>().ok()?;
                let current = head.trim() == "*";
                (!name.is_empty()
                    && !commit.is_empty()
                    && name != "origin/HEAD"
                    && (current || committed_at as f64 >= cutoff))
                    .then(|| GitBranchTip {
                        name: name.to_owned(),
                        commit: commit.to_owned(),
                        current,
                        committed_at,
                    })
            })
            .collect();
        tips.sort_by(|left, right| left.name.cmp(&right.name));
        tips
    }

    /// List paths in a committed tree beneath the given project-relative directory.
    pub fn files_in_tree(&self, reference: &str, path: &str) -> Vec<String> {
        let Some(output) = self.run(&["ls-tree", "-r", "--name-only", "-z", reference, "--", path])
        else {
            return Vec::new();
        };
        if !output.status.success() {
            return Vec::new();
        }
        String::from_utf8_lossy(&output.stdout)
            .split('\0')
            .filter(|path| !path.is_empty())
            .map(str::to_owned)
            .collect()
    }

    pub fn current_branch(&self) -> String {
        if !self.is_repository() {
            return String::new();
        }
        self.run(&["branch", "--show-current"])
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
            .unwrap_or_default()
    }

    /// Commit only the Backlog files selected by a completed write operation.
    /// Git's `--only` path mode keeps unrelated staged work out of the commit.
    pub fn commit_selected_paths(
        &self,
        message: &str,
        paths: &[PathBuf],
        config: &Config,
    ) -> Result<(), String> {
        if config.auto_commit != Some(true)
            || config.filesystem_only == Some(true)
            || paths.is_empty()
        {
            return Ok(());
        }
        let mut paths_by_repository = BTreeMap::<PathBuf, Vec<String>>::new();
        for path in paths {
            let path = if path.is_absolute() {
                path.clone()
            } else {
                self.project_root.join(path)
            };
            let resolved = self.selected_git_path(&path).ok_or_else(|| {
                format!(
                    "Cannot commit {}: the path cannot be resolved.",
                    path.display()
                )
            })?;
            let repository_root = resolved
                .parent()
                .and_then(|parent| {
                    Self::with_executable(parent, &self.executable).repository_root()
                })
                .or_else(|| self.repository_root());
            let Some(repository_root) = repository_root else {
                continue;
            };
            let relative = resolved.strip_prefix(&repository_root).map_err(|_| {
                format!(
                    "Cannot commit {}: the path cannot be addressed in repository {}.",
                    path.display(),
                    repository_root.display()
                )
            })?;
            let relative = relative.to_str().ok_or_else(|| {
                format!(
                    "Cannot commit {}: Git paths must be valid UTF-8.",
                    path.display()
                )
            })?;
            if !resolved.exists() && !self.is_tracked_path(&repository_root, relative) {
                continue;
            }
            if !relative.is_empty() {
                paths_by_repository
                    .entry(repository_root)
                    .or_default()
                    .push(relative.to_owned());
            }
        }
        let mut committed_repositories = Vec::new();
        for (repository_root, mut relative_paths) in paths_by_repository {
            relative_paths.sort();
            relative_paths.dedup();
            if relative_paths.is_empty() {
                continue;
            }
            match self.commit_repository_paths(message, &repository_root, &relative_paths, config) {
                Ok(true) => committed_repositories.push(repository_root),
                Ok(false) => {}
                Err(error) => {
                    return if committed_repositories.is_empty() {
                        Err(error)
                    } else {
                        Err(format!(
                            "Selected-path commit stopped after earlier repositories were committed ({}): {error}",
                            committed_repositories
                                .iter()
                                .map(|path: &PathBuf| path.display().to_string())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ))
                    };
                }
            }
        }
        Ok(())
    }

    fn commit_repository_paths(
        &self,
        message: &str,
        repository_root: &Path,
        relative_paths: &[String],
        config: &Config,
    ) -> Result<bool, String> {
        let path_args = relative_paths
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let original_entries = self.selected_index_entries(repository_root, &path_args)?;
        let add_args = [
            vec!["--literal-pathspecs", "add", "-A", "--"],
            path_args.clone(),
        ]
        .concat();
        if let Err(error) = self.run_checked(&add_args, repository_root, "stage selected files") {
            let staged_entries = self.selected_index_entries(repository_root, &path_args)?;
            if !self.restore_selected_index(
                repository_root,
                &path_args,
                &staged_entries,
                &original_entries,
            )? {
                return Err(format!(
                    "{error} The Git index changed concurrently, so its selected entries were preserved for manual review."
                ));
            }
            return Err(error);
        }
        let staged_entries = self.selected_index_entries(repository_root, &path_args)?;
        if !self.has_cached_changes(repository_root, &path_args)? {
            return Ok(false);
        }

        let mut commit_args = vec!["--literal-pathspecs", "commit", "--only", "-m", message];
        if config.bypass_git_hooks == Some(true) {
            commit_args.push("--no-verify");
        }
        commit_args.push("--");
        commit_args.extend(path_args);
        if let Err(error) = self.run_checked(&commit_args, repository_root, "commit selected files")
        {
            if !self.restore_selected_index(
                repository_root,
                &relative_paths
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                &staged_entries,
                &original_entries,
            )? {
                return Err(format!(
                    "{error} The Git index changed concurrently, so its selected entries were preserved for manual review."
                ));
            }
            return Err(format!(
                "Selected-path Git commit failed for {}. The files remain in the working tree; their previous index entries were restored. {error}",
                relative_paths.join(", ")
            ));
        }
        Ok(true)
    }

    fn selected_git_path(&self, path: &Path) -> Option<PathBuf> {
        let link_path = resolve_git_path(path)?;
        let is_file_link = std::fs::symlink_metadata(path)
            .is_ok_and(|metadata| metadata.file_type().is_symlink())
            && !std::fs::metadata(path).is_ok_and(|metadata| metadata.is_dir());
        if !is_file_link {
            return Some(
                if std::fs::metadata(path).is_ok_and(|metadata| metadata.is_dir()) {
                    path.canonicalize().unwrap_or(link_path)
                } else {
                    link_path
                },
            );
        }
        let Some(repository_root) = link_path
            .parent()
            .and_then(|parent| Self::with_executable(parent, &self.executable).repository_root())
        else {
            return Some(path.canonicalize().unwrap_or(link_path));
        };
        let relative = link_path.strip_prefix(&repository_root).ok()?.to_str()?;
        let changed_link = Command::new(&self.executable)
            .args([
                "--literal-pathspecs",
                "status",
                "--porcelain",
                "--untracked-files=all",
                "--",
                relative,
            ])
            .current_dir(&repository_root)
            .stdin(std::process::Stdio::null())
            .output()
            .is_ok_and(|output| output.status.success() && !output.stdout.is_empty());
        if changed_link {
            Some(link_path)
        } else {
            Some(path.canonicalize().unwrap_or(link_path))
        }
    }

    fn selected_index_entries(
        &self,
        cwd: &Path,
        paths: &[&str],
    ) -> Result<Vec<Vec<GitIndexEntry>>, String> {
        paths
            .iter()
            .map(|path| {
                let output = Command::new(&self.executable)
                    .args(["--literal-pathspecs", "ls-files", "-s", "-z", "--", path])
                    .current_dir(cwd)
                    .stdin(std::process::Stdio::null())
                    .output()
                    .map_err(|error| {
                        format!("Could not read Git index entry for {path}: {error}")
                    })?;
                if !output.status.success() {
                    return Err(format!(
                        "Could not read Git index entry for {path}: {}",
                        String::from_utf8_lossy(&output.stderr).trim()
                    ));
                }
                Ok(parse_index_entries(&output.stdout))
            })
            .collect()
    }

    fn is_tracked_path(&self, cwd: &Path, path: &str) -> bool {
        let indexed = Command::new(&self.executable)
            .args([
                "--literal-pathspecs",
                "ls-files",
                "--error-unmatch",
                "--",
                path,
            ])
            .current_dir(cwd)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        indexed
            || Command::new(&self.executable)
                .args(["--literal-pathspecs", "ls-tree", "-r", "HEAD", "--", path])
                .current_dir(cwd)
                .stdin(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output()
                .is_ok_and(|output| output.status.success() && !output.stdout.is_empty())
    }

    fn has_cached_changes(&self, cwd: &Path, paths: &[&str]) -> Result<bool, String> {
        let output = Command::new(&self.executable)
            .args(["--literal-pathspecs", "diff", "--cached", "--quiet", "--"])
            .args(paths)
            .current_dir(cwd)
            .stdin(std::process::Stdio::null())
            .output()
            .map_err(|error| format!("Could not inspect selected Git changes: {error}"))?;
        match output.status.code() {
            Some(0) => Ok(false),
            Some(1) => Ok(true),
            code => Err(format!(
                "Could not inspect selected Git changes (exit code {}). {}",
                code.unwrap_or(-1),
                String::from_utf8_lossy(&output.stderr).trim()
            )),
        }
    }

    fn restore_selected_index(
        &self,
        cwd: &Path,
        paths: &[&str],
        expected: &[Vec<GitIndexEntry>],
        restore: &[Vec<GitIndexEntry>],
    ) -> Result<bool, String> {
        let current = self.selected_index_entries(cwd, paths)?;
        if current != expected {
            return Ok(false);
        }
        if current == restore {
            return Ok(true);
        }
        let mut records = Vec::new();
        for (path, entries) in paths.iter().zip(restore) {
            let object_id_length = expected
                .iter()
                .flatten()
                .next()
                .or_else(|| entries.first())
                .map_or(40, |entry| entry.object_id.len());
            records.extend_from_slice(
                format!("0 {}\t{path}\0", "0".repeat(object_id_length)).as_bytes(),
            );
            for entry in entries {
                records.extend_from_slice(
                    format!(
                        "{} {} {}\t{path}\0",
                        entry.mode, entry.object_id, entry.stage
                    )
                    .as_bytes(),
                );
            }
        }
        let output = Command::new(&self.executable)
            .args(["update-index", "-z", "--index-info"])
            .current_dir(cwd)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                child
                    .stdin
                    .take()
                    .expect("piped Git stdin")
                    .write_all(&records)?;
                child.wait_with_output()
            })
            .map_err(|error| format!("Could not restore selected Git index entries: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "Could not restore selected Git index entries: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(true)
    }

    fn run_checked(&self, args: &[&str], cwd: &Path, operation: &str) -> Result<(), String> {
        let output = Command::new(&self.executable)
            .args(args)
            .current_dir(cwd)
            .stdin(std::process::Stdio::null())
            .output()
            .map_err(|error| format!("Could not {operation}: {error}"))?;
        if output.status.success() {
            return Ok(());
        }
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        Err(format!(
            "Git {operation} failed (exit code {}). Command: git {}.{}",
            output.status.code().unwrap_or(-1),
            args.join(" "),
            if detail.is_empty() {
                String::new()
            } else {
                format!("\n{detail}")
            }
        ))
    }

    pub fn worktree_paths(&self) -> Vec<PathBuf> {
        if !self.is_repository() {
            return Vec::new();
        }
        let Some(output) = self.run(&["worktree", "list", "--porcelain"]) else {
            return Vec::new();
        };
        if !output.status.success() {
            return Vec::new();
        }
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim_end)
            .filter_map(|line| line.strip_prefix("worktree "))
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .collect()
    }

    fn run(&self, args: &[&str]) -> Option<Output> {
        Command::new(&self.executable)
            .args(args)
            .current_dir(&self.project_root)
            .stdin(std::process::Stdio::null())
            .output()
            .ok()
    }
}

fn parse_index_entries(output: &[u8]) -> Vec<GitIndexEntry> {
    output
        .split(|byte| *byte == 0)
        .filter_map(|record| {
            let tab = record.iter().position(|byte| *byte == b'\t')?;
            let header = std::str::from_utf8(&record[..tab]).ok()?;
            let mut fields = header.split_whitespace();
            Some(GitIndexEntry {
                mode: fields.next()?.to_owned(),
                object_id: fields.next()?.to_owned(),
                stage: fields.next()?.to_owned(),
            })
        })
        .collect()
}

fn resolve_git_path(path: &Path) -> Option<PathBuf> {
    let mut parent = path.parent()?;
    loop {
        if let Ok(resolved_parent) = parent.canonicalize() {
            let suffix = path.strip_prefix(parent).ok()?;
            return Some(resolved_parent.join(suffix));
        }
        let next = parent.parent()?;
        parent = next;
    }
}
