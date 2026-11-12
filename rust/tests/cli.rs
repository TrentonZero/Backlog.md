use serde_json::Value;
use std::{
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "backlog-rust-cli-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(
            path.join("backlog.config.yml"),
            "project_name: CLI Fixture\ntask_prefix: task\nstatuses:\n  - To Do\n  - Done\npriorities:\n  - High\n  - Low\n",
        )
        .unwrap();
        Self(path)
    }

    fn file(&self, path: &str, contents: &str) {
        let path = self.0.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn rust_cli(args: &[&str], cwd: &std::path::Path) -> Output {
    let binary = env!("CARGO_BIN_EXE_backlog");
    Command::new(binary)
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap()
}

fn typescript_cli(args: &[&str], cwd: &std::path::Path) -> Output {
    let bun = std::env::var_os("BACKLOG_REFERENCE_BUN").unwrap_or_else(|| "bun".into());
    let cli = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("src/cli.ts");
    Command::new(bun)
        .arg(cli)
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap()
}

fn fixture() -> Fixture {
    let fixture = Fixture::new();
    fixture.file(
        "backlog/tasks/task-1-first.md",
        "---\nid: task-1\ntitle: First task\nstatus: To Do\npriority: high\nlabels:\n  - backend\ndependencies:\n  - task-2\ncreated_date: 2026-01-01\n---\n",
    );
    fixture.file(
        "backlog/tasks/task-2-second.md",
        "---\nid: task-2\ntitle: Second task\nstatus: Done\npriority: low\ncreated_date: 2026-01-02\n---\n",
    );
    fixture
}

#[test]
fn rust_task_list_plain_and_json_match_the_live_typescript_cli() {
    let fixture = fixture();
    for args in [
        vec!["task", "list", "--plain"],
        vec![
            "task", "list", "--plain", "--status", "To Do", "--search", "backend",
        ],
        vec!["task", "list", "--plain", "--search", "task"],
        vec![
            "task",
            "list",
            "--plain",
            "--exclude-status",
            "Done",
            "--sort",
            "priority",
        ],
        vec![
            "task", "list", "--plain", "--labels", "backend", "--limit", "1",
        ],
        vec![
            "task", "list", "--plain", "--status", "Done", "--labels", "missing",
        ],
        vec!["task", "list", "--plain", "--status", "Unknown"],
        vec!["task", "list", "--json"],
        vec![
            "task", "list", "--json", "--status", "To Do", "--search", "backend",
        ],
        vec!["task", "list", "--json", "--search", "task"],
        vec![
            "task",
            "list",
            "--json",
            "--exclude-status",
            "Done",
            "--sort",
            "priority",
        ],
        vec![
            "task", "list", "--json", "--labels", "backend", "--limit", "1",
        ],
        vec![
            "task", "list", "--json", "--status", "Done", "--labels", "missing",
        ],
        vec!["task", "list", "--json", "--status", "Unknown"],
    ] {
        let expected = typescript_cli(&args, &fixture.0);
        let actual = rust_cli(&args, &fixture.0);
        assert_eq!(
            actual.status.code(),
            expected.status.code(),
            "args: {args:?}"
        );
        assert_eq!(
            String::from_utf8_lossy(&actual.stderr),
            String::from_utf8_lossy(&expected.stderr),
            "args: {args:?}"
        );
        if args.contains(&"--json") {
            let actual_json: Value = serde_json::from_slice(&actual.stdout).unwrap();
            let expected_json: Value = serde_json::from_slice(&expected.stdout).unwrap();
            assert_eq!(actual_json, expected_json, "args: {args:?}");
        } else {
            assert_eq!(actual.stdout, expected.stdout, "args: {args:?}");
        }
    }
}

#[test]
fn rust_task_list_empty_project_matches_the_live_typescript_cli() {
    let fixture = Fixture::new();
    std::fs::create_dir_all(fixture.0.join("backlog/tasks")).unwrap();
    for args in [
        &["task", "list", "--plain"][..],
        &["task", "list", "--json"][..],
    ] {
        let expected = typescript_cli(args, &fixture.0);
        let actual = rust_cli(args, &fixture.0);
        assert_eq!(actual.status.code(), expected.status.code());
        assert_eq!(actual.stderr, expected.stderr);
        if args.contains(&"--json") {
            let actual_json: Value = serde_json::from_slice(&actual.stdout).unwrap();
            let expected_json: Value = serde_json::from_slice(&expected.stdout).unwrap();
            assert_eq!(actual_json, expected_json);
        } else {
            assert_eq!(actual.stdout, expected.stdout);
        }
    }
}

#[test]
fn rust_cli_help_and_version_are_available_without_a_project() {
    let fixture = Fixture::new();
    for args in [
        &["--help"][..],
        &["--version"][..],
        &["task", "list", "--help"][..],
    ] {
        let output = rust_cli(args, &fixture.0);
        assert!(output.status.success(), "args: {args:?}");
        assert!(output.stderr.is_empty(), "args: {args:?}");
        assert!(!output.stdout.is_empty(), "args: {args:?}");
    }
}
