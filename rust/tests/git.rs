use backlog_md::git::GitRepository;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "backlog-git fixture spaces-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.0)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn reference(root: &Path, path_override: Option<&str>) -> Value {
    const SCRIPT: &str = r#"
import { GitOperations, isGitRepository } from "./src/git/operations.ts";
const root = process.env.BACKLOG_GIT_FIXTURE;
const operations = new GitOperations(root);
const result = {
  repository: await isGitRepository(root),
  root: await operations.getRepositoryRoot(),
  branch: await operations.getCurrentBranch(),
  worktrees: await operations.listWorktreePaths()
};
process.stdout.write(JSON.stringify(result));
"#;
    let bun = std::env::var_os("BACKLOG_REFERENCE_BUN").unwrap_or_else(|| "bun".into());
    let mut command = Command::new(bun);
    command
        .args(["--eval", SCRIPT])
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
        .env("BACKLOG_GIT_FIXTURE", root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(path) = path_override {
        command.env("PATH", path);
    }
    let output = command.output().expect("run live TypeScript Git reference");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn repository_root_branch_and_linked_worktrees_match_live_typescript() {
    let fixture = Fixture::new();
    fixture.git(&["init", "-b", "main"]);
    fixture.git(&["config", "user.name", "Git Test"]);
    fixture.git(&["config", "user.email", "git-test@example.com"]);
    std::fs::write(fixture.0.join("tracked file.txt"), "fixture\n").unwrap();
    fixture.git(&["add", "."]);
    fixture.git(&["commit", "-m", "Initial commit"]);
    let linked = fixture.0.join("linked worktree");
    Command::new("git")
        .args(["worktree", "add", "-b", "feature", linked.to_str().unwrap()])
        .current_dir(&fixture.0)
        .output()
        .unwrap()
        .status
        .success()
        .then_some(())
        .expect("create linked worktree");
    let detached = fixture.0.join("detached worktree");
    Command::new("git")
        .args([
            "worktree",
            "add",
            "--detach",
            detached.to_str().unwrap(),
            "HEAD",
        ])
        .current_dir(&fixture.0)
        .output()
        .unwrap()
        .status
        .success()
        .then_some(())
        .expect("create detached worktree");

    let expected = reference(&fixture.0, None);
    let repository = GitRepository::new(&fixture.0);
    let actual = serde_json::json!({
        "repository": repository.is_repository(),
        "root": repository.repository_root(),
        "branch": repository.current_branch(),
        "worktrees": repository.worktree_paths(),
    });
    assert_eq!(actual, expected);

    let expected_detached = reference(&detached, None);
    let actual_detached = GitRepository::new(&detached);
    assert_eq!(
        serde_json::json!({
            "repository": actual_detached.is_repository(),
            "root": actual_detached.repository_root(),
            "branch": actual_detached.current_branch(),
            "worktrees": actual_detached.worktree_paths(),
        }),
        expected_detached
    );
    assert_eq!(expected_detached["branch"], "");
}

#[test]
fn non_repository_and_missing_git_match_live_typescript() {
    let fixture = Fixture::new();
    let invalid_root = fixture.0.join("missing project");
    let expected_invalid = reference(&invalid_root, None);
    let invalid = GitRepository::new(&invalid_root);
    assert_eq!(
        serde_json::json!({
            "repository": invalid.is_repository(),
            "root": invalid.repository_root(),
            "branch": invalid.current_branch(),
            "worktrees": invalid.worktree_paths(),
        }),
        expected_invalid
    );

    let missing_git = GitRepository::with_executable(&fixture.0, "backlog-no-such-git-executable");
    let actual = serde_json::json!({
        "repository": missing_git.is_repository(),
        "root": missing_git.repository_root(),
        "branch": missing_git.current_branch(),
        "worktrees": missing_git.worktree_paths(),
    });
    let expected = reference(&fixture.0, Some("/nonexistent"));
    assert_eq!(actual, expected);
    assert_eq!(actual["repository"], false);
    assert_eq!(actual["root"], Value::Null);
    assert_eq!(actual["branch"], "");
    assert_eq!(actual["worktrees"], serde_json::json!([]));
}
