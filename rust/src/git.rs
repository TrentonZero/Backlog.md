//! Read-only repository and linked-worktree information from the installed Git executable.

use std::path::PathBuf;
use std::process::{Command, Output};

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
