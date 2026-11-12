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
fn rust_task_create_matches_live_typescript_for_default_output_and_record() {
    let expected_fixture = Fixture::new();
    let actual_fixture = Fixture::new();
    std::fs::create_dir_all(expected_fixture.0.join("backlog/tasks")).unwrap();
    std::fs::create_dir_all(actual_fixture.0.join("backlog/tasks")).unwrap();
    let args = ["task", "create", "First task"];
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stderr, expected.stderr);
    let actual_stdout = String::from_utf8_lossy(&actual.stdout)
        .replace(&actual_fixture.0.to_string_lossy().to_string(), "<ROOT>");
    let expected_stdout = String::from_utf8_lossy(&expected.stdout)
        .replace(&expected_fixture.0.to_string_lossy().to_string(), "<ROOT>");
    assert_eq!(actual_stdout, expected_stdout);
    let read_created_task = |fixture: &Fixture| {
        let path = std::fs::read_dir(fixture.0.join("backlog/tasks"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        std::fs::read_to_string(path).unwrap()
    };
    let created = read_created_task(&actual_fixture);
    let expected_task = read_created_task(&expected_fixture);
    let mut created_record = backlog_md::records::parse_task(&created).unwrap();
    let mut expected_record = backlog_md::records::parse_task(&expected_task).unwrap();
    created_record.created_date.clear();
    expected_record.created_date.clear();
    assert_eq!(created_record, expected_record);
}

#[test]
fn rust_task_create_populated_plain_result_matches_live_typescript() {
    let expected_fixture = fixture();
    let actual_fixture = fixture();
    for fixture in [&expected_fixture, &actual_fixture] {
        fixture.file(
            "backlog/milestones/m-1-release-x.md",
            "---\nid: m-1\ntitle: Release X\ncreated_date: 2026-01-01\n---\n",
        );
    }
    let args = [
        "task",
        "create",
        " Child task ",
        "--desc",
        "A multiline\ndescription",
        "--status",
        "done",
        "-a",
        "@alice,@bob",
        "-a",
        "@carol",
        "-l",
        "backend,api",
        "-l",
        "ui",
        "--priority",
        "HIGH",
        "--type",
        "BUG",
        "--milestone",
        "Release X",
        "--due-date",
        "2026-08-10",
        "--ac",
        "First criterion",
        "--acceptance-criteria",
        "Second criterion",
        "--dod",
        "Review",
        "--no-dod-defaults",
        "--plan",
        "Implementation plan",
        "--notes",
        "Implementation notes",
        "--final-summary",
        "Finished summary",
        "--parent",
        "1",
        "--depends-on",
        "2,,TASK-2",
        "--ref",
        "docs/design.md,,docs/other.md",
        "--doc",
        "docs/guide.md,,docs/reference.md",
        "--modified-file",
        "src/main.rs,,src/lib.rs",
        "--plain",
    ];
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stderr, expected.stderr);
    let actual_stdout = String::from_utf8_lossy(&actual.stdout)
        .replace(&actual_fixture.0.to_string_lossy().to_string(), "<ROOT>");
    let expected_stdout = String::from_utf8_lossy(&expected.stdout)
        .replace(&expected_fixture.0.to_string_lossy().to_string(), "<ROOT>");
    assert_eq!(actual_stdout, expected_stdout);

    let created_task = |fixture: &Fixture| {
        std::fs::read_dir(fixture.0.join("backlog/tasks"))
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .contains("Child-task")
            })
            .unwrap()
    };
    let mut created_record = backlog_md::records::parse_task(
        &std::fs::read_to_string(created_task(&actual_fixture)).unwrap(),
    )
    .unwrap();
    let mut expected_record = backlog_md::records::parse_task(
        &std::fs::read_to_string(created_task(&expected_fixture)).unwrap(),
    )
    .unwrap();
    created_record.created_date.clear();
    expected_record.created_date.clear();
    assert_eq!(created_record, expected_record);
}

#[test]
fn rust_task_create_missing_dependency_matches_live_typescript_and_writes_nothing() {
    let expected_fixture = fixture();
    let actual_fixture = fixture();
    let args = ["task", "create", "Refused", "--dep", "999"];
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stdout, expected.stdout);
    assert_eq!(actual.stderr, expected.stderr);
    let count_tasks = |fixture: &Fixture| {
        std::fs::read_dir(fixture.0.join("backlog/tasks"))
            .unwrap()
            .count()
    };
    assert_eq!(count_tasks(&actual_fixture), 2);
    assert_eq!(count_tasks(&expected_fixture), 2);
}

#[test]
fn rust_task_create_rejects_empty_dependency_values_like_live_typescript() {
    for args in [
        vec!["task", "create", "Refused", "--dep", ""],
        vec!["task", "create", "Refused", "--depends-on", ""],
        vec!["task", "create", "Refused", "--ref", ""],
        vec!["task", "create", "Refused", "--doc", ""],
    ] {
        let expected_fixture = fixture();
        let actual_fixture = fixture();
        let expected = typescript_cli(&args, &expected_fixture.0);
        let actual = rust_cli(&args, &actual_fixture.0);
        assert_eq!(
            actual.status.code(),
            expected.status.code(),
            "args: {args:?}"
        );
        assert_eq!(actual.stdout, expected.stdout, "args: {args:?}");
        assert_eq!(actual.stderr, expected.stderr, "args: {args:?}");
        let count_tasks = |fixture: &Fixture| {
            std::fs::read_dir(fixture.0.join("backlog/tasks"))
                .unwrap()
                .count()
        };
        assert_eq!(count_tasks(&actual_fixture), 2, "args: {args:?}");
    }
}

#[test]
fn rust_task_create_missing_title_error_matches_live_typescript() {
    let fixture = Fixture::new();
    let args = ["task", "create"];
    let expected = typescript_cli(&args, &fixture.0);
    let actual = rust_cli(&args, &fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stdout, expected.stdout);
    assert_eq!(actual.stderr, expected.stderr);
}

#[test]
fn rust_task_create_matches_configured_assignee_dod_and_explicit_clear_defaults() {
    for args in [
        vec!["task", "create", "Defaulted"],
        vec![
            "task",
            "create",
            "Explicitly unassigned",
            "-a",
            "",
            "--no-dod-defaults",
        ],
    ] {
        let expected_fixture = fixture();
        let actual_fixture = fixture();
        let config = "project_name: CLI Fixture\ntask_prefix: task\nstatuses:\n  - To Do\n  - Done\npriorities:\n  - High\n  - Low\ndefault_assignee:\n  - '@default'\ndefinition_of_done:\n  - Check documentation\n  - Run tests\n";
        expected_fixture.file("backlog.config.yml", config);
        actual_fixture.file("backlog.config.yml", config);
        let expected = typescript_cli(&args, &expected_fixture.0);
        let actual = rust_cli(&args, &actual_fixture.0);
        assert_eq!(
            actual.status.code(),
            expected.status.code(),
            "args: {args:?}"
        );
        assert_eq!(actual.stderr, expected.stderr, "args: {args:?}");
        let find_task = |fixture: &Fixture| {
            std::fs::read_dir(fixture.0.join("backlog/tasks"))
                .unwrap()
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .find(|path| {
                    std::fs::read_to_string(path).is_ok_and(|source| {
                        backlog_md::records::parse_task(&source)
                            .is_ok_and(|record| record.title == args[2])
                    })
                })
                .unwrap()
        };
        let mut actual_record = backlog_md::records::parse_task(
            &std::fs::read_to_string(find_task(&actual_fixture)).unwrap(),
        )
        .unwrap();
        let mut expected_record = backlog_md::records::parse_task(
            &std::fs::read_to_string(find_task(&expected_fixture)).unwrap(),
        )
        .unwrap();
        actual_record.created_date.clear();
        expected_record.created_date.clear();
        assert_eq!(actual_record, expected_record, "args: {args:?}");
    }
}

#[test]
fn rust_task_edit_core_fields_and_plain_output_match_live_typescript() {
    let expected_fixture = fixture();
    let actual_fixture = fixture();
    for fixture in [&expected_fixture, &actual_fixture] {
        fixture.file(
            "backlog/tasks/task-1-first.md",
            "---\nid: task-1\ntitle: First task\nstatus: To Do\npriority: high\nlabels:\n  - backend\ndependencies:\n  - task-2\ncustom: retained\ncreated_date: 2026-01-01\n---\n## Notes\n\nKeep this body.\n",
        );
        fixture.file(
            "backlog.config.yml",
            "project_name: CLI Fixture\ntask_prefix: task\nstatuses:\n  - To Do\n  - Done\npriorities:\n  - High\n  - Low\ntypes:\n  - bug\nprojects:\n  - Alpha\n",
        );
        fixture.file(
            "backlog/milestones/m-1-release-x.md",
            "---\nid: m-1\ntitle: Release X\ncreated_date: 2026-01-01\n---\n",
        );
    }
    let args = [
        "task",
        "edit",
        "1",
        "--title",
        "Renamed task",
        "--desc",
        "Updated description",
        "--status",
        "done",
        "-a",
        "@alice,@bob",
        "-l",
        "ui,bug",
        "--priority",
        "LOW",
        "--type",
        "BUG",
        "--project",
        "alpha",
        "--due-date",
        "2026-08-10",
        "--ordinal",
        "2500",
        "--milestone",
        "Release X",
        "--plain",
    ];
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stderr, expected.stderr);
    let actual_stdout = String::from_utf8_lossy(&actual.stdout)
        .replace(&actual_fixture.0.to_string_lossy().to_string(), "<ROOT>");
    let expected_stdout = String::from_utf8_lossy(&expected.stdout)
        .replace(&expected_fixture.0.to_string_lossy().to_string(), "<ROOT>");
    assert_eq!(actual_stdout, expected_stdout);
    let read_task = |fixture: &Fixture| {
        let path = std::fs::read_dir(fixture.0.join("backlog/tasks"))
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                std::fs::read_to_string(path).is_ok_and(|source| {
                    backlog_md::records::parse_task(&source)
                        .is_ok_and(|task| task.id.eq_ignore_ascii_case("task-1"))
                })
            })
            .unwrap();
        (
            path.file_name().unwrap().to_string_lossy().into_owned(),
            std::fs::read_to_string(path).unwrap(),
        )
    };
    let (actual_filename, actual_source) = read_task(&actual_fixture);
    let (expected_filename, expected_source) = read_task(&expected_fixture);
    assert_eq!(actual_filename, expected_filename);
    for preserved in ["custom: retained", "## Notes", "Keep this body."] {
        assert!(
            actual_source.contains(preserved),
            "lost source text: {preserved}"
        );
        assert!(
            expected_source.contains(preserved),
            "reference lacks source text: {preserved}"
        );
    }
    let mut actual_task = backlog_md::records::parse_task(&actual_source).unwrap();
    let mut expected_task = backlog_md::records::parse_task(&expected_source).unwrap();
    actual_task.updated_date = None;
    expected_task.updated_date = None;
    assert_eq!(actual_task, expected_task);
}

#[test]
fn rust_task_edit_conflicting_label_flags_match_live_typescript_without_writes() {
    let expected_fixture = fixture();
    let actual_fixture = fixture();
    let args = ["task", "edit", "1", "--label", "new", "--add-label", "more"];
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stdout, expected.stdout);
    assert_eq!(actual.stderr, expected.stderr);
    let read_task = |fixture: &Fixture| {
        std::fs::read_to_string(fixture.0.join("backlog/tasks/task-1-first.md")).unwrap()
    };
    assert_eq!(read_task(&actual_fixture), read_task(&expected_fixture));
}

#[test]
fn rust_task_edit_invalid_fields_match_live_typescript_without_writes() {
    for args in [
        vec!["task", "edit", "1", "--status", "Unknown"],
        vec!["task", "edit", "1", "--title", ""],
        vec!["task", "edit", "1", "--due-date", "2026-02-30"],
        vec![
            "task",
            "edit",
            "1",
            "--due-date",
            "2026-08-10",
            "--clear-due-date",
        ],
    ] {
        let expected_fixture = fixture();
        let actual_fixture = fixture();
        let expected = typescript_cli(&args, &expected_fixture.0);
        let actual = rust_cli(&args, &actual_fixture.0);
        assert_eq!(actual.status.code(), expected.status.code(), "{args:?}");
        assert_eq!(actual.stdout, expected.stdout, "{args:?}");
        assert_eq!(actual.stderr, expected.stderr, "{args:?}");
        for path in ["task-1-first.md", "task-2-second.md"] {
            let relative = format!("backlog/tasks/{path}");
            assert_eq!(
                std::fs::read(actual_fixture.0.join(&relative)).unwrap(),
                std::fs::read(expected_fixture.0.join(&relative)).unwrap(),
                "{args:?}"
            );
        }
    }
}

#[test]
fn rust_task_edit_identity_refusals_match_live_typescript_without_writes() {
    for identity in ["missing", "archived", "ambiguous"] {
        let expected_fixture = fixture();
        let actual_fixture = fixture();
        if identity != "missing" {
            for fixture in [&expected_fixture, &actual_fixture] {
                let task_source =
                    std::fs::read_to_string(fixture.0.join("backlog/tasks/task-1-first.md"))
                        .unwrap();
                let archive_path = fixture.0.join("backlog/archive/tasks/task-1-first.md");
                std::fs::create_dir_all(archive_path.parent().unwrap()).unwrap();
                std::fs::write(&archive_path, &task_source).unwrap();
                if identity == "archived" {
                    std::fs::remove_file(fixture.0.join("backlog/tasks/task-1-first.md")).unwrap();
                }
            }
        }
        let task_id = if identity == "missing" { "404" } else { "1" };
        let args = ["task", "edit", task_id, "--title", "Changed"];
        let expected = typescript_cli(&args, &expected_fixture.0);
        let actual = rust_cli(&args, &actual_fixture.0);
        assert_eq!(actual.status.code(), expected.status.code(), "{identity}");
        assert_eq!(actual.stdout, expected.stdout, "{identity}");
        let actual_stderr = String::from_utf8_lossy(&actual.stderr)
            .replace(&actual_fixture.0.to_string_lossy().to_string(), "<ROOT>");
        let expected_stderr = String::from_utf8_lossy(&expected.stderr)
            .replace(&expected_fixture.0.to_string_lossy().to_string(), "<ROOT>");
        assert_eq!(actual_stderr, expected_stderr, "{identity}");
        let collect = |fixture: &Fixture| {
            [
                "backlog/tasks/task-1-first.md",
                "backlog/archive/tasks/task-1-first.md",
            ]
            .into_iter()
            .filter_map(|path| {
                std::fs::read(fixture.0.join(path))
                    .ok()
                    .map(|bytes| (path, bytes))
            })
            .collect::<Vec<_>>()
        };
        assert_eq!(
            collect(&actual_fixture),
            collect(&expected_fixture),
            "{identity}"
        );
    }
}

#[test]
fn rust_task_edit_help_documents_supported_core_fields() {
    let fixture = Fixture::new();
    let output = rust_cli(&["task", "edit", "--help"], &fixture.0);
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);
    for option in [
        "--title",
        "--description",
        "--desc",
        "--assignee",
        "--status",
        "--label",
        "-l",
        "--add-label",
        "--remove-label",
        "--clear-labels",
        "--priority",
        "--type",
        "--project",
        "--due-date",
        "--clear-due-date",
        "--ordinal",
        "--milestone",
        "--clear-milestone",
        "--plain",
    ] {
        assert!(
            help.contains(option),
            "missing {option} from task edit help"
        );
    }
}

#[test]
fn rust_task_edit_default_confirmation_matches_live_typescript() {
    let expected_fixture = fixture();
    let actual_fixture = fixture();
    let args = ["task", "edit", "1", "--status", "Done"];
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stdout, expected.stdout);
    assert_eq!(actual.stderr, expected.stderr);
    let parse_task = |fixture: &Fixture| {
        let path = fixture.0.join("backlog/tasks/task-1-first.md");
        backlog_md::records::parse_task(&std::fs::read_to_string(path).unwrap()).unwrap()
    };
    let mut actual_task = parse_task(&actual_fixture);
    let mut expected_task = parse_task(&expected_fixture);
    actual_task.updated_date = None;
    expected_task.updated_date = None;
    assert_eq!(actual_task, expected_task);
}

#[test]
fn rust_task_edit_explicit_clears_match_live_typescript() {
    let expected_fixture = fixture();
    let actual_fixture = fixture();
    for fixture in [&expected_fixture, &actual_fixture] {
        fixture.file(
            "backlog.config.yml",
            "project_name: CLI Fixture\ntask_prefix: task\nstatuses:\n  - To Do\n  - Done\npriorities:\n  - High\n  - Low\ntypes:\n  - bug\nprojects:\n  - Alpha\n",
        );
        fixture.file(
            "backlog/milestones/m-1-release-x.md",
            "---\nid: m-1\ntitle: Release X\ncreated_date: 2026-01-01\n---\n",
        );
        fixture.file(
            "backlog/tasks/task-1-first.md",
            "---\nid: task-1\ntitle: First task\nstatus: To Do\npriority: high\nlabels:\n  - backend\nassignee:\n  - '@alice'\ntype: bug\nproject: Alpha\ndue_date: 2026-08-10\nmilestone: m-1\ncreated_date: 2026-01-01\n---\n",
        );
    }
    let args = [
        "task",
        "edit",
        "1",
        "--clear-labels",
        "--assignee",
        "",
        "--type",
        "",
        "--project",
        "",
        "--clear-due-date",
        "--clear-milestone",
    ];
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stdout, expected.stdout);
    assert_eq!(actual.stderr, expected.stderr);
    let parse_task = |fixture: &Fixture| {
        let path = fixture.0.join("backlog/tasks/task-1-first.md");
        backlog_md::records::parse_task(&std::fs::read_to_string(path).unwrap()).unwrap()
    };
    let mut actual_task = parse_task(&actual_fixture);
    let mut expected_task = parse_task(&expected_fixture);
    actual_task.updated_date = None;
    expected_task.updated_date = None;
    assert_eq!(actual_task, expected_task);
}

#[test]
fn rust_cli_help_and_version_are_available_without_a_project() {
    let fixture = Fixture::new();
    for args in [
        &["--help"][..],
        &["--version"][..],
        &["task", "list", "--help"][..],
        &["task", "view", "--help"][..],
        &["task", "create", "--help"][..],
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
        if args == ["task", "create", "--help"] {
            let help = String::from_utf8_lossy(&output.stdout);
            for option in [
                "--description",
                "--desc",
                "--assignee",
                "--labels",
                "--priority",
                "--type",
                "--project",
                "--due-date",
                "--milestone",
                "--ac",
                "--acceptance-criteria",
                "--dod",
                "--no-dod-defaults",
                "--plan",
                "--notes",
                "--final-summary",
                "--ordinal",
                "--parent",
                "--depends-on",
                "--dep",
                "--ref",
                "--doc",
                "--modified-file",
                "--plain",
            ] {
                assert!(
                    help.contains(option),
                    "missing {option} from task create help"
                );
            }
        }
    }
}
