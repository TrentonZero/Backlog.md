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
            assert_eq!(
                String::from_utf8_lossy(&actual.stdout),
                String::from_utf8_lossy(&expected.stdout),
                "args: {args:?}"
            );
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
fn rust_task_view_plain_and_json_match_the_live_typescript_cli() {
    let fixture = fixture();
    for args in [
        &["task", "view", "task-1", "--plain"][..],
        &["task", "view", "1", "--json"][..],
    ] {
        let expected = typescript_cli(args, &fixture.0);
        let actual = rust_cli(args, &fixture.0);
        assert_eq!(
            actual.status.code(),
            expected.status.code(),
            "args: {args:?}"
        );
        assert_eq!(actual.stderr, expected.stderr, "args: {args:?}");
        if args.contains(&"--json") {
            let actual_json: Value = serde_json::from_slice(&actual.stdout).unwrap();
            let expected_json: Value = serde_json::from_slice(&expected.stdout).unwrap();
            assert_eq!(actual_json, expected_json, "args: {args:?}");
        } else {
            assert_eq!(
                String::from_utf8_lossy(&actual.stdout),
                String::from_utf8_lossy(&expected.stdout),
                "args: {args:?}"
            );
        }
    }
}

#[test]
fn rust_task_view_rich_details_match_the_live_typescript_cli() {
    let fixture = fixture();
    fixture.file(
        "backlog/tasks/task-1-first.md",
        "---\nid: task-1\ntitle: First task\nstatus: To Do\ntype: enhancement\npriority: high\nassignee:\n  - '@alex'\nreporter: '@sam'\ncreated_date: '2026-07-14 09:30'\nupdated_date: '2026-07-14 10:45'\ndue_date: 2026-08-10\nlabels:\n  - cli\n  - json\nmilestone: m-1\ndependencies:\n  - TASK-2\nreferences:\n  - https://example.com/issue\ndocumentation:\n  - doc-1\nmodified_files:\n  - src/cli.rs\nordinal: 1000\n---\n## Description\n\n<!-- SECTION:DESCRIPTION:BEGIN -->\nMachine-readable output\n<!-- SECTION:DESCRIPTION:END -->\n\n## Acceptance Criteria\n<!-- AC:BEGIN -->\n- [x] #1 Produces JSON\n<!-- AC:END -->\n\n## Definition of Done\n<!-- DOD:BEGIN -->\n- [ ] #1 Tests pass\n<!-- DOD:END -->\n\n## Implementation Plan\n<!-- SECTION:PLAN:BEGIN -->\n1. Add formatter\n<!-- SECTION:PLAN:END -->\n\n## Implementation Notes\n<!-- SECTION:NOTES:BEGIN -->\nFormatter added\n<!-- SECTION:NOTES:END -->\n\n## Comments\n<!-- COMMENTS:BEGIN -->\nauthor: @alex\ncreated: 2026-07-14 11:00\n\n---\n\nKeep this stable\n\n---\n<!-- COMMENTS:END -->\n\n## Final Summary\n<!-- SECTION:FINAL_SUMMARY:BEGIN -->\nReady for review\n<!-- SECTION:FINAL_SUMMARY:END -->\n",
    );
    fixture.file(
        "backlog/tasks/task-3-child.md",
        "---\nid: task-3\ntitle: Child task\nstatus: To Do\nparent_task_id: task-1\ncreated_date: 2026-07-14\n---\n",
    );
    let task_path = fixture.0.join("backlog/tasks/task-1-first.md");
    let task_source = std::fs::read_to_string(&task_path).unwrap();
    let task_source = task_source.replace(
        "Keep this stable\n\n---\n<!-- COMMENTS:END -->",
        "Keep this stable\n\n---\n\n---\n\nThis comment has no date\n\n---\n<!-- COMMENTS:END -->",
    );
    std::fs::write(task_path, task_source).unwrap();
    for args in [
        &["task", "view", "1", "--json"][..],
        &["task", "view", "task-1", "--plain"][..],
        &["task", "view", "task-2", "--json"][..],
        &["task", "view", "task-2", "--plain"][..],
    ] {
        let expected = typescript_cli(args, &fixture.0);
        let actual = rust_cli(args, &fixture.0);
        assert_eq!(
            actual.status.code(),
            expected.status.code(),
            "args: {args:?}"
        );
        assert_eq!(actual.stderr, expected.stderr, "args: {args:?}");
        if args.contains(&"--json") {
            let actual_json: Value = serde_json::from_slice(&actual.stdout).unwrap();
            let expected_json: Value = serde_json::from_slice(&expected.stdout).unwrap();
            assert_eq!(actual_json["kind"], expected_json["kind"]);
            for (key, expected_value) in expected_json["task"].as_object().unwrap() {
                assert_eq!(actual_json["task"][key], *expected_value, "field {key}");
            }
        } else {
            assert_eq!(
                String::from_utf8_lossy(&actual.stdout),
                String::from_utf8_lossy(&expected.stdout),
                "args: {args:?}"
            );
        }
    }
}

#[test]
fn rust_task_view_archived_and_missing_results_match_the_live_typescript_cli() {
    let fixture = fixture();
    let active_path = fixture.0.join("backlog/tasks/task-1-first.md");
    let archived_path = fixture.0.join("backlog/archive/tasks/task-1-first.md");
    std::fs::create_dir_all(archived_path.parent().unwrap()).unwrap();
    std::fs::rename(active_path, &archived_path).unwrap();
    for args in [
        &["task", "view", "1", "--json"][..],
        &["task", "view", "task-1", "--plain"][..],
        &["task", "view", "999", "--plain"][..],
    ] {
        let expected = typescript_cli(args, &fixture.0);
        let actual = rust_cli(args, &fixture.0);
        assert_eq!(
            actual.status.code(),
            expected.status.code(),
            "args: {args:?}"
        );
        assert_eq!(actual.stderr, expected.stderr, "args: {args:?}");
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
fn rust_task_view_ambiguous_ids_match_the_live_typescript_cli() {
    let fixture = fixture();
    fixture.file(
        "backlog/tasks/task-01-duplicate.md",
        "---\nid: TASK-01\ntitle: Duplicate task\nstatus: To Do\ncreated_date: 2026-01-03\n---\n",
    );
    let args = ["task", "view", "1", "--json"];
    let expected = typescript_cli(&args, &fixture.0);
    let actual = rust_cli(&args, &fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stdout, expected.stdout);
    assert_eq!(actual.stderr, expected.stderr);
}

#[test]
fn rust_task_view_graph_text_matches_diamonds_cycles_and_missing_dependencies() {
    let fixture = fixture();
    fixture.file(
        "backlog/tasks/task-1-first.md",
        "---\nid: task-1\ntitle: Root task\nstatus: To Do\ncreated_date: 2026-01-01\ndependencies:\n  - TASK-2\n  - TASK-3\n  - TASK-999\n---\n",
    );
    fixture.file(
        "backlog/tasks/task-2-second.md",
        "---\nid: task-2\ntitle: Left task\nstatus: To Do\ncreated_date: 2026-01-02\ndependencies:\n  - TASK-4\n---\n",
    );
    fixture.file(
        "backlog/tasks/task-3-third.md",
        "---\nid: task-3\ntitle: Right task\nstatus: To Do\ncreated_date: 2026-01-03\ndependencies:\n  - TASK-4\n---\n",
    );
    fixture.file(
        "backlog/tasks/task-4-fourth.md",
        "---\nid: task-4\ntitle: Shared task\nstatus: To Do\ncreated_date: 2026-01-04\ndependencies:\n  - TASK-1\n---\n",
    );
    let args = ["task", "view", "task-1", "--plain"];
    let expected = typescript_cli(&args, &fixture.0);
    let actual = rust_cli(&args, &fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stderr, expected.stderr);
    assert_eq!(
        String::from_utf8_lossy(&actual.stdout),
        String::from_utf8_lossy(&expected.stdout)
    );
}

#[test]
fn rust_cli_help_and_version_are_available_without_a_project() {
    let fixture = Fixture::new();
    for args in [
        &["--help"][..],
        &["--version"][..],
        &["task", "list", "--help"][..],
        &["task", "view", "--help"][..],
    ] {
        let output = rust_cli(args, &fixture.0);
        assert!(output.status.success(), "args: {args:?}");
        assert!(output.stderr.is_empty(), "args: {args:?}");
        assert!(!output.stdout.is_empty(), "args: {args:?}");
        if args == ["task", "view", "--help"] {
            let help = String::from_utf8_lossy(&output.stdout);
            assert!(help.contains("--plain"));
            assert!(help.contains("--json"));
        }
    }
}
