//! Read-only repository and linked-worktree information from the installed Git executable.

use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitBranchTip {
    pub name: String,
    pub commit: String,
    pub current: bool,
    pub committed_at: i64,
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
