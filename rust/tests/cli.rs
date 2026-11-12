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

fn assert_cli_process_parity(
    args: &[&str],
    reference: &Fixture,
    candidate: &Fixture,
) -> (Output, Output) {
    let expected = typescript_cli(args, &reference.0);
    let actual = rust_cli(args, &candidate.0);
    assert_eq!(
        actual.status.code(),
        expected.status.code(),
        "args: {args:?}"
    );
    assert_eq!(
        actual.stdout,
        expected.stdout,
        "args: {args:?}\nactual stdout: {:?}\nexpected stdout: {:?}",
        String::from_utf8_lossy(&actual.stdout),
        String::from_utf8_lossy(&expected.stdout)
    );
    assert_eq!(actual.stderr, expected.stderr, "args: {args:?}");
    (expected, actual)
}

fn git(cwd: &std::path::Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn initialize_git_fixture(fixture: &Fixture) {
    git(&fixture.0, &["init", "-b", "main"]);
    git(&fixture.0, &["config", "user.name", "Backlog Test"]);
    git(
        &fixture.0,
        &["config", "user.email", "backlog-test@example.com"],
    );
    git(&fixture.0, &["add", "."]);
    git(&fixture.0, &["commit", "-m", "Baseline"]);
    fixture.file("UNRELATED.txt", "keep staged\n");
    git(&fixture.0, &["add", "UNRELATED.txt"]);
}

fn selected_commit_result(fixture: &Fixture) -> (String, String, String) {
    let message = git(&fixture.0, &["log", "-1", "--format=%s"]);
    let files = git(
        &fixture.0,
        &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"],
    );
    let staged = git(&fixture.0, &["diff", "--cached", "--name-only"]);
    (message, files, staged)
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
fn rust_task_create_auto_commit_matches_typescript_and_preserves_unrelated_index_entries() {
    let expected_fixture = Fixture::new();
    let actual_fixture = Fixture::new();
    for fixture in [&expected_fixture, &actual_fixture] {
        fixture.file(
            "backlog.config.yml",
            "project_name: CLI Fixture\ntask_prefix: task\nstatuses:\n  - To Do\n  - Done\nauto_commit: true\n",
        );
        std::fs::create_dir_all(fixture.0.join("backlog/tasks")).unwrap();
        initialize_git_fixture(fixture);
    }
    let args = ["task", "create", "Committed from Rust"];
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert!(
        expected.status.success(),
        "{}",
        String::from_utf8_lossy(&expected.stderr)
    );
    assert!(
        actual.status.success(),
        "{}",
        String::from_utf8_lossy(&actual.stderr)
    );
    assert_eq!(actual.status.code(), expected.status.code());
    let actual_stdout = String::from_utf8_lossy(&actual.stdout)
        .replace(&actual_fixture.0.to_string_lossy().to_string(), "<ROOT>");
    let expected_stdout = String::from_utf8_lossy(&expected.stdout)
        .replace(&expected_fixture.0.to_string_lossy().to_string(), "<ROOT>");
    assert_eq!(actual_stdout, expected_stdout);
    assert_eq!(actual.stderr, expected.stderr);
    assert_eq!(
        selected_commit_result(&actual_fixture),
        selected_commit_result(&expected_fixture)
    );
    let (message, committed_files, staged_files) = selected_commit_result(&actual_fixture);
    assert_eq!(message, "Create task TASK-1");
    assert!(committed_files.contains("backlog/tasks/"));
    assert_eq!(staged_files, "UNRELATED.txt");
}

#[test]
fn rust_task_create_auto_commit_is_a_noop_outside_git_repositories() {
    let expected_fixture = Fixture::new();
    let actual_fixture = Fixture::new();
    for fixture in [&expected_fixture, &actual_fixture] {
        fixture.file(
            "backlog.config.yml",
            "project_name: CLI Fixture\ntask_prefix: task\nstatuses:\n  - To Do\n  - Done\nauto_commit: true\n",
        );
        std::fs::create_dir_all(fixture.0.join("backlog/tasks")).unwrap();
    }
    let args = ["task", "create", "No repository"];
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(
        String::from_utf8_lossy(&actual.stdout)
            .replace(&actual_fixture.0.to_string_lossy().to_string(), "<ROOT>"),
        String::from_utf8_lossy(&expected.stdout)
            .replace(&expected_fixture.0.to_string_lossy().to_string(), "<ROOT>")
    );
    assert_eq!(actual.stderr, expected.stderr);
    assert_eq!(
        std::fs::read_dir(actual_fixture.0.join("backlog/tasks"))
            .unwrap()
            .count(),
        1
    );
}

#[cfg(unix)]
#[test]
fn rust_task_create_commits_to_a_symlinked_backlog_repository() {
    use std::os::unix::fs::symlink;

    let expected_project = Fixture::new();
    let expected_backlog = Fixture::new();
    let actual_project = Fixture::new();
    let actual_backlog = Fixture::new();
    for (project, backlog) in [
        (&expected_project, &expected_backlog),
        (&actual_project, &actual_backlog),
    ] {
        std::fs::remove_file(project.0.join("backlog.config.yml")).unwrap();
        git(&backlog.0, &["init", "-b", "main"]);
        git(&backlog.0, &["config", "user.name", "Backlog Test"]);
        git(
            &backlog.0,
            &["config", "user.email", "backlog-test@example.com"],
        );
        backlog.file(
            "config.yml",
            "project_name: Symlinked Backlog\nstatuses:\n  - To Do\nauto_commit: true\n",
        );
        std::fs::create_dir_all(backlog.0.join("tasks")).unwrap();
        std::fs::create_dir_all(backlog.0.join("drafts")).unwrap();
        backlog.file("README.md", "# Backlog\n");
        git(&backlog.0, &["add", "."]);
        git(&backlog.0, &["commit", "-m", "Baseline"]);
        backlog.file("UNRELATED.txt", "keep staged\n");
        git(&backlog.0, &["add", "UNRELATED.txt"]);
        symlink(&backlog.0, project.0.join("backlog")).unwrap();
    }
    let args = ["task", "create", "Symlinked repository task"];
    let expected = typescript_cli(&args, &expected_project.0);
    let actual = rust_cli(&args, &actual_project.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert!(
        actual.status.success(),
        "Rust: {}\nTypeScript: {}",
        String::from_utf8_lossy(&actual.stderr),
        String::from_utf8_lossy(&expected.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&actual.stdout)
            .replace(&actual_project.0.to_string_lossy().to_string(), "<ROOT>"),
        String::from_utf8_lossy(&expected.stdout)
            .replace(&expected_project.0.to_string_lossy().to_string(), "<ROOT>")
    );
    assert_eq!(actual.stderr, expected.stderr);
    assert_eq!(
        selected_commit_result(&actual_backlog),
        selected_commit_result(&expected_backlog)
    );
    assert_eq!(
        git(&actual_backlog.0, &["log", "-1", "--format=%s"]),
        "Create task TASK-1"
    );
    assert_eq!(
        git(&actual_backlog.0, &["diff", "--cached", "--name-only"]),
        "UNRELATED.txt"
    );
}

#[cfg(unix)]
#[test]
fn rust_task_create_hook_failure_matches_typescript_file_and_index_state() {
    use std::os::unix::fs::PermissionsExt;

    let expected_fixture = Fixture::new();
    let actual_fixture = Fixture::new();
    for fixture in [&expected_fixture, &actual_fixture] {
        fixture.file(
            "backlog.config.yml",
            "project_name: CLI Fixture\ntask_prefix: task\nstatuses:\n  - To Do\n  - Done\nauto_commit: true\n",
        );
        std::fs::create_dir_all(fixture.0.join("backlog/tasks")).unwrap();
        initialize_git_fixture(fixture);
        let hook = fixture.0.join(".git/hooks/pre-commit");
        std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
        let mut permissions = std::fs::metadata(&hook).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(hook, permissions).unwrap();
    }
    let args = ["task", "create", "Rejected by hook"];
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    let actual_stderr = String::from_utf8_lossy(&actual.stderr)
        .replace(&actual_fixture.0.to_string_lossy().to_string(), "<ROOT>");
    let expected_stderr = String::from_utf8_lossy(&expected.stderr)
        .replace(&expected_fixture.0.to_string_lossy().to_string(), "<ROOT>");
    assert!(
        expected_stderr.contains("commit task file"),
        "{expected_stderr}"
    );
    assert!(
        actual_stderr.contains("Selected-path Git commit failed"),
        "{actual_stderr}"
    );
    assert!(actual_stderr.contains("previous index entries were restored"));
    assert!(!actual_stderr.contains("remain in the working tree"));
    assert_eq!(
        git(&actual_fixture.0, &["log", "-1", "--format=%s"]),
        "Baseline"
    );
    assert_eq!(
        git(&expected_fixture.0, &["log", "-1", "--format=%s"]),
        "Baseline"
    );
    assert_eq!(
        git(&actual_fixture.0, &["diff", "--cached", "--name-only"]),
        git(&expected_fixture.0, &["diff", "--cached", "--name-only"])
    );
    assert_eq!(
        std::fs::read_dir(actual_fixture.0.join("backlog/tasks"))
            .unwrap()
            .count(),
        std::fs::read_dir(expected_fixture.0.join("backlog/tasks"))
            .unwrap()
            .count()
    );
    assert_eq!(
        git(&actual_fixture.0, &["diff", "--cached", "--name-only"]),
        "UNRELATED.txt"
    );
}

#[cfg(unix)]
#[test]
fn rust_task_create_bypasses_hooks_when_configured() {
    use std::os::unix::fs::PermissionsExt;

    let expected_fixture = Fixture::new();
    let actual_fixture = Fixture::new();
    for fixture in [&expected_fixture, &actual_fixture] {
        fixture.file(
            "backlog.config.yml",
            "project_name: CLI Fixture\ntask_prefix: task\nstatuses:\n  - To Do\n  - Done\nauto_commit: true\nbypass_git_hooks: true\n",
        );
        std::fs::create_dir_all(fixture.0.join("backlog/tasks")).unwrap();
        initialize_git_fixture(fixture);
        let hook = fixture.0.join(".git/hooks/pre-commit");
        std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
        let mut permissions = std::fs::metadata(&hook).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(hook, permissions).unwrap();
    }
    let args = ["task", "create", "Hooks are bypassed"];
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert!(actual.status.success());
    assert_eq!(
        selected_commit_result(&actual_fixture),
        selected_commit_result(&expected_fixture)
    );
}

#[cfg(unix)]
#[test]
fn rust_selected_path_hooks_see_only_the_record_being_committed() {
    use std::os::unix::fs::PermissionsExt;

    let expected_fixture = Fixture::new();
    let actual_fixture = Fixture::new();
    for fixture in [&expected_fixture, &actual_fixture] {
        fixture.file(
            "backlog.config.yml",
            "project_name: CLI Fixture\ntask_prefix: task\nstatuses:\n  - To Do\n  - Done\nauto_commit: true\n",
        );
        std::fs::create_dir_all(fixture.0.join("backlog/tasks")).unwrap();
        initialize_git_fixture(fixture);
        let hook = fixture.0.join(".git/hooks/pre-commit");
        std::fs::write(
            &hook,
            "#!/bin/sh\ngit diff --cached --name-only > hook-index.txt\n",
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&hook).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(hook, permissions).unwrap();
    }
    let args = ["task", "create", "Hook index scope"];
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert!(actual.status.success());
    assert_eq!(
        std::fs::read_to_string(actual_fixture.0.join("hook-index.txt")).unwrap(),
        std::fs::read_to_string(expected_fixture.0.join("hook-index.txt")).unwrap()
    );
    assert!(
        !std::fs::read_to_string(actual_fixture.0.join("hook-index.txt"))
            .unwrap()
            .contains("UNRELATED.txt")
    );
}

#[cfg(unix)]
#[test]
fn rust_selected_path_commit_honors_git_signing_configuration() {
    use std::os::unix::fs::PermissionsExt;

    let expected_fixture = Fixture::new();
    let actual_fixture = Fixture::new();
    for fixture in [&expected_fixture, &actual_fixture] {
        fixture.file(
            "backlog.config.yml",
            "project_name: CLI Fixture\ntask_prefix: task\nstatuses:\n  - To Do\n  - Done\nauto_commit: true\n",
        );
        std::fs::create_dir_all(fixture.0.join("backlog/tasks")).unwrap();
        initialize_git_fixture(fixture);
        let signer = fixture.0.join("fake-gpg");
        std::fs::write(
            &signer,
            "#!/bin/sh\ncat >/dev/null\nprintf '%s\\n' '-----BEGIN PGP SIGNATURE-----' '' 'dGVzdA==' '-----END PGP SIGNATURE-----'\n",
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&signer).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&signer, permissions).unwrap();
        git(&fixture.0, &["config", "commit.gpgsign", "true"]);
        git(&fixture.0, &["config", "user.signingkey", "fixture-key"]);
        git(
            &fixture.0,
            &["config", "gpg.program", signer.to_str().unwrap()],
        );
    }
    let args = ["task", "create", "Signed selected-path commit"];
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert_eq!(
        actual.status.code(),
        expected.status.code(),
        "Rust: {}\nTypeScript: {}",
        String::from_utf8_lossy(&actual.stderr),
        String::from_utf8_lossy(&expected.stderr)
    );
    assert!(!actual.status.success());
    assert!(!expected.status.success());
    assert!(String::from_utf8_lossy(&actual.stderr).contains("gpg failed to sign the data"));
    assert!(String::from_utf8_lossy(&expected.stderr).contains("gpg failed to sign the data"));
    assert_eq!(
        selected_commit_result(&actual_fixture),
        selected_commit_result(&expected_fixture)
    );
    assert_eq!(
        git(&actual_fixture.0, &["log", "-1", "--format=%s"]),
        "Baseline"
    );
    assert_eq!(
        git(&actual_fixture.0, &["diff", "--cached", "--name-only"]),
        "UNRELATED.txt"
    );
    assert_eq!(
        std::fs::read_dir(actual_fixture.0.join("backlog/tasks"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn rust_draft_create_auto_commit_matches_typescript_and_filesystem_only_disables_it() {
    for filesystem_only in [false, true] {
        let expected_fixture = Fixture::new();
        let actual_fixture = Fixture::new();
        for fixture in [&expected_fixture, &actual_fixture] {
            fixture.file(
                "backlog.config.yml",
                &format!(
                    "project_name: CLI Fixture\ntask_prefix: task\nstatuses:\n  - To Do\n  - Done\nauto_commit: true\nfilesystem_only: {filesystem_only}\n"
                ),
            );
            std::fs::create_dir_all(fixture.0.join("backlog/tasks")).unwrap();
            initialize_git_fixture(fixture);
        }
        let args = ["draft", "create", "A draft"];
        let expected = typescript_cli(&args, &expected_fixture.0);
        let actual = rust_cli(&args, &actual_fixture.0);
        assert_eq!(actual.status.code(), expected.status.code());
        assert_eq!(
            String::from_utf8_lossy(&actual.stdout)
                .replace(&actual_fixture.0.to_string_lossy().to_string(), "<ROOT>"),
            String::from_utf8_lossy(&expected.stdout)
                .replace(&expected_fixture.0.to_string_lossy().to_string(), "<ROOT>")
        );
        assert_eq!(
            selected_commit_result(&actual_fixture),
            selected_commit_result(&expected_fixture)
        );
        if filesystem_only {
            assert_eq!(
                git(&actual_fixture.0, &["log", "-1", "--format=%s"]),
                "Baseline"
            );
            assert_eq!(
                git(&actual_fixture.0, &["diff", "--cached", "--name-only"]),
                "UNRELATED.txt"
            );
        } else {
            assert_eq!(
                git(&actual_fixture.0, &["log", "-1", "--format=%s"]),
                "DRAFT-1 - Create draft DRAFT-1"
            );
        }
    }
}

#[test]
fn rust_task_archive_and_restore_auto_commit_only_the_moved_record() {
    let expected_fixture = Fixture::new();
    let actual_fixture = Fixture::new();
    for fixture in [&expected_fixture, &actual_fixture] {
        fixture.file(
            "backlog.config.yml",
            "project_name: CLI Fixture\ntask_prefix: task\nstatuses:\n  - To Do\n  - Done\nauto_commit: true\n",
        );
        fixture.file(
            "backlog/tasks/task-1-first.md",
            "---\nid: task-1\ntitle: First task\nstatus: To Do\ncreated_date: 2026-01-01\n---\n",
        );
        initialize_git_fixture(fixture);
    }
    for args in [
        &["task", "archive", "task-1"][..],
        &["task", "restore", "task-1"][..],
        &["task", "archive", "task-1"][..],
        &["task", "delete", "task-1"][..],
    ] {
        let expected = typescript_cli(args, &expected_fixture.0);
        let actual = rust_cli(args, &actual_fixture.0);
        assert_eq!(
            actual.status.code(),
            expected.status.code(),
            "{args:?}\nRust: {}\nTypeScript: {}",
            String::from_utf8_lossy(&actual.stderr),
            String::from_utf8_lossy(&expected.stderr)
        );
        assert_eq!(actual.stdout, expected.stdout, "{args:?}");
        assert_eq!(actual.stderr, expected.stderr, "{args:?}");
        assert_eq!(
            selected_commit_result(&actual_fixture),
            selected_commit_result(&expected_fixture),
            "{args:?}"
        );
        assert_eq!(
            git(&actual_fixture.0, &["diff", "--cached", "--name-only"]),
            "UNRELATED.txt"
        );
    }
}

#[cfg(unix)]
#[test]
fn rust_task_lifecycle_commits_symlink_paths_without_committing_their_targets() {
    for target_inside_repository in [true, false] {
        let expected_fixture = Fixture::new();
        let actual_fixture = Fixture::new();
        let expected_outside = Fixture::new();
        let actual_outside = Fixture::new();
        for (fixture, outside) in [
            (&expected_fixture, &expected_outside),
            (&actual_fixture, &actual_outside),
        ] {
            fixture.file(
                "backlog.config.yml",
                "project_name: CLI Fixture\ntask_prefix: task\nstatuses:\n  - To Do\n  - Done\nauto_commit: true\n",
            );
            let task_path = fixture.0.join("backlog/tasks/task-1-first.md");
            fixture.file(
                "backlog/tasks/task-1-first.md",
                "---\nid: task-1\ntitle: First task\nstatus: To Do\ncreated_date: 2026-01-01\n---\n",
            );
            let target = if target_inside_repository {
                fixture.0.join("notes/task.md")
            } else {
                outside.0.join("task.md")
            };
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            let contents = std::fs::read(&task_path).unwrap();
            std::fs::write(&target, contents).unwrap();
            std::fs::remove_file(&task_path).unwrap();
            std::os::unix::fs::symlink(&target, &task_path).unwrap();
            initialize_git_fixture(fixture);
        }
        for args in [
            &["task", "archive", "task-1"][..],
            &["task", "restore", "task-1"][..],
            &["task", "archive", "task-1"][..],
            &["task", "delete", "task-1"][..],
        ] {
            let expected = typescript_cli(args, &expected_fixture.0);
            let actual = rust_cli(args, &actual_fixture.0);
            assert_eq!(
                actual.status.code(),
                expected.status.code(),
                "inside={target_inside_repository} {args:?}\nRust: {}\nTypeScript: {}",
                String::from_utf8_lossy(&actual.stderr),
                String::from_utf8_lossy(&expected.stderr)
            );
            assert_eq!(actual.stdout, expected.stdout, "{args:?}");
            assert_eq!(actual.stderr, expected.stderr, "{args:?}");
            assert_eq!(
                selected_commit_result(&actual_fixture),
                selected_commit_result(&expected_fixture),
                "inside={target_inside_repository} {args:?}"
            );
        }
        assert_eq!(
            git(&actual_fixture.0, &["status", "--porcelain"]),
            "A  UNRELATED.txt"
        );
        let target = if target_inside_repository {
            actual_fixture.0.join("notes/task.md")
        } else {
            actual_outside.0.join("task.md")
        };
        assert!(target.is_file());
    }
}

#[test]
fn rust_task_edit_auto_commit_matches_typescript_and_keeps_unrelated_staged_work() {
    let expected_fixture = Fixture::new();
    let actual_fixture = Fixture::new();
    for fixture in [&expected_fixture, &actual_fixture] {
        fixture.file(
            "backlog.config.yml",
            "project_name: CLI Fixture\ntask_prefix: task\nstatuses:\n  - To Do\n  - Done\nauto_commit: true\n",
        );
        fixture.file(
            "backlog/tasks/task-1-first.md",
            "---\nid: task-1\ntitle: First task\nstatus: To Do\ncreated_date: 2026-01-01\n---\n",
        );
        initialize_git_fixture(fixture);
    }
    let args = ["task", "edit", "task-1", "--title", "Edited task"];
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stdout, expected.stdout);
    assert_eq!(actual.stderr, expected.stderr);
    assert_eq!(
        selected_commit_result(&actual_fixture),
        selected_commit_result(&expected_fixture)
    );
    assert_eq!(
        git(&actual_fixture.0, &["log", "-1", "--format=%s"]),
        "Update task TASK-1"
    );
    assert_eq!(
        git(&actual_fixture.0, &["diff", "--cached", "--name-only"]),
        "UNRELATED.txt"
    );
}

#[cfg(unix)]
#[test]
fn rust_task_edit_through_a_committed_symlink_updates_and_commits_its_target() {
    use std::os::unix::fs::symlink;

    let expected_fixture = Fixture::new();
    let actual_fixture = Fixture::new();
    for fixture in [&expected_fixture, &actual_fixture] {
        fixture.file(
            "backlog.config.yml",
            "project_name: CLI Fixture\ntask_prefix: task\nstatuses:\n  - To Do\n  - Done\nauto_commit: true\n",
        );
        let task_path = fixture.0.join("backlog/tasks/task-1-linked.md");
        let source =
            "---\nid: task-1\ntitle: Linked task\nstatus: To Do\ncreated_date: 2026-01-01\n---\n";
        fixture.file("backlog/tasks/task-1-linked.md", source);
        let target = fixture.0.join("notes/task.md");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, source).unwrap();
        std::fs::remove_file(&task_path).unwrap();
        symlink(&target, &task_path).unwrap();
        initialize_git_fixture(fixture);
    }
    let args = [
        "task",
        "edit",
        "task-1",
        "--description",
        "Edited through the link",
    ];
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stdout, expected.stdout);
    assert_eq!(actual.stderr, expected.stderr);
    assert!(
        std::fs::symlink_metadata(actual_fixture.0.join("backlog/tasks/task-1-linked.md"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        selected_commit_result(&actual_fixture),
        selected_commit_result(&expected_fixture)
    );
    assert_eq!(
        git(&actual_fixture.0, &["diff", "--cached", "--name-only"]),
        "UNRELATED.txt"
    );
}

#[cfg(unix)]
#[test]
fn rust_task_edit_through_an_uncommitted_symlink_matches_typescript() {
    use std::os::unix::fs::symlink;

    let expected_fixture = Fixture::new();
    let actual_fixture = Fixture::new();
    for fixture in [&expected_fixture, &actual_fixture] {
        fixture.file(
            "backlog.config.yml",
            "project_name: CLI Fixture\ntask_prefix: task\nstatuses:\n  - To Do\n  - Done\nauto_commit: true\n",
        );
        let task_path = fixture.0.join("backlog/tasks/task-1-linked.md");
        let contents =
            "---\nid: task-1\ntitle: Linked task\nstatus: To Do\ncreated_date: 2026-01-01\n---\n";
        fixture.file("backlog/tasks/task-1-linked.md", contents);
        initialize_git_fixture(fixture);
        let target = fixture.0.join("notes/task.md");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::rename(&task_path, &target).unwrap();
        symlink(&target, &task_path).unwrap();
    }

    for (index, edit) in ["First edit", "Second edit"].iter().enumerate() {
        let args = ["task", "edit", "task-1", "--description", edit];
        let expected = typescript_cli(&args, &expected_fixture.0);
        let actual = rust_cli(&args, &actual_fixture.0);
        assert_eq!(
            actual.status.code(),
            expected.status.code(),
            "Rust stdout: {}\nRust stderr: {}\nTypeScript stdout: {}\nTypeScript stderr: {}",
            String::from_utf8_lossy(&actual.stdout),
            String::from_utf8_lossy(&actual.stderr),
            String::from_utf8_lossy(&expected.stdout),
            String::from_utf8_lossy(&expected.stderr)
        );
        assert_eq!(actual.stdout, expected.stdout);
        assert_eq!(actual.stderr, expected.stderr);
        assert_eq!(
            selected_commit_result(&actual_fixture),
            selected_commit_result(&expected_fixture)
        );
        if index == 0 {
            assert_eq!(
                git(
                    &actual_fixture.0,
                    &["diff-tree", "--no-commit-id", "--name-status", "-r", "HEAD"]
                ),
                "T\tbacklog/tasks/task-1-linked.md"
            );
            assert_eq!(
                git(&actual_fixture.0, &["status", "--porcelain"]),
                "A  UNRELATED.txt\n?? notes/"
            );
        } else {
            assert_eq!(
                git(
                    &actual_fixture.0,
                    &["diff-tree", "--no-commit-id", "--name-status", "-r", "HEAD"]
                ),
                "A\tnotes/task.md"
            );
            assert_eq!(
                git(&actual_fixture.0, &["status", "--porcelain"]),
                "A  UNRELATED.txt"
            );
        }
    }
}

#[test]
fn rust_draft_view_and_id_shorthand_match_typescript_plain_output() {
    let expected_fixture = Fixture::new();
    let actual_fixture = Fixture::new();
    for fixture in [&expected_fixture, &actual_fixture] {
        fixture.file(
            "backlog/drafts/draft-1-first.md",
            "---\nid: draft-1\ntitle: First draft\nstatus: Draft\npriority: high\ncreated_date: 2026-01-01\n---\nDraft body.\n",
        );
    }

    for args in [
        &["draft", "view", "draft-1", "--plain"][..],
        &["draft", "draft-1", "--plain"][..],
    ] {
        let expected = typescript_cli(args, &expected_fixture.0);
        let actual = rust_cli(args, &actual_fixture.0);
        assert_eq!(actual.status.code(), expected.status.code(), "{args:?}");
        assert_eq!(
            String::from_utf8_lossy(&actual.stdout)
                .replace(&actual_fixture.0.to_string_lossy().to_string(), "<ROOT>"),
            String::from_utf8_lossy(&expected.stdout)
                .replace(&expected_fixture.0.to_string_lossy().to_string(), "<ROOT>"),
            "{args:?}"
        );
        assert_eq!(actual.stderr, expected.stderr, "{args:?}");
    }
}

#[test]
fn rust_draft_edit_help_lists_supported_field_operations() {
    let fixture = Fixture::new();
    let output = rust_cli(&["draft", "edit", "--help"], &fixture.0);
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);
    for option in [
        "draft edit [OPTIONS]",
        "--status <STATUS>",
        "draft edits only accept Draft",
        "--plan <PLAN>",
        "--append-notes <APPEND_NOTES>",
        "--comment <COMMENT>",
        "--final-summary <FINAL_SUMMARY>",
        "--modified-file <MODIFIED_FILES>",
        "--check-ac <CHECK_AC>",
        "--check-dod <CHECK_DOD>",
    ] {
        assert!(help.contains(option), "missing {option} in draft edit help");
    }
}

#[test]
fn rust_draft_edit_matches_typescript_and_preserves_unmanaged_source() {
    let expected_fixture = Fixture::new();
    let actual_fixture = Fixture::new();
    for fixture in [&expected_fixture, &actual_fixture] {
        fixture.file(
            "backlog.config.yml",
            "project_name: CLI Fixture\ntask_prefix: task\nstatuses:\n  - To Do\n  - Done\npriorities:\n  - High\n  - Low\n",
        );
        fixture.file(
            "backlog/drafts/draft-1-first.md",
            "---\nid: draft-1\ntitle: First draft\nstatus: Draft\ncreated_date: 2026-01-01\ncustom_field: retain me\n---\nOriginal body.\n",
        );
    }
    let args = [
        "draft",
        "edit",
        "draft-1",
        "--title",
        "Renamed draft",
        "--description",
        "New body",
        "--assignee",
        "@alice",
        "--label",
        "ui",
        "--priority",
        "high",
        "--plain",
    ];
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(
        String::from_utf8_lossy(&actual.stdout)
            .replace(&actual_fixture.0.to_string_lossy().to_string(), "<ROOT>"),
        String::from_utf8_lossy(&expected.stdout)
            .replace(&expected_fixture.0.to_string_lossy().to_string(), "<ROOT>")
    );
    assert_eq!(actual.stderr, expected.stderr);

    for fixture in [&expected_fixture, &actual_fixture] {
        let path = fixture.0.join("backlog/drafts/draft-1 - Renamed-draft.md");
        let contents = std::fs::read_to_string(path).unwrap();
        assert!(contents.contains("custom_field: retain me"));
        assert!(contents.contains("New body"));
        assert!(contents.contains("Original body."));
    }
    let actual = backlog_md::records::parse_task(
        &std::fs::read_to_string(
            actual_fixture
                .0
                .join("backlog/drafts/draft-1 - Renamed-draft.md"),
        )
        .unwrap(),
    )
    .unwrap();
    let expected = backlog_md::records::parse_task(
        &std::fs::read_to_string(
            expected_fixture
                .0
                .join("backlog/drafts/draft-1 - Renamed-draft.md"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn rust_draft_edit_updates_plan_notes_comments_summary_and_modified_files() {
    let expected_fixture = Fixture::new();
    let actual_fixture = Fixture::new();
    for fixture in [&expected_fixture, &actual_fixture] {
        fixture.file(
            "backlog/drafts/draft-1-fields.md",
            "---\nid: draft-1\ntitle: Field draft\nstatus: Draft\ncreated_date: 2026-01-01\n---\n",
        );
    }
    let args = [
        "draft",
        "edit",
        "draft-1",
        "--plan",
        "Implementation plan",
        "--append-plan",
        "Next step",
        "--notes",
        "Draft notes",
        "--append-notes",
        "More notes",
        "--comment",
        "Reviewed by the team",
        "--comment-author",
        "Alex",
        "--final-summary",
        "Summary",
        "--append-final-summary",
        "Further summary",
        "--modified-file",
        "src/file.rs",
        "--plain",
    ];
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(
        String::from_utf8_lossy(&actual.stdout)
            .replace(&actual_fixture.0.to_string_lossy().to_string(), "<ROOT>"),
        String::from_utf8_lossy(&expected.stdout)
            .replace(&expected_fixture.0.to_string_lossy().to_string(), "<ROOT>")
    );
    assert_eq!(actual.stderr, expected.stderr);
    let actual = backlog_md::records::parse_task(
        &std::fs::read_to_string(
            actual_fixture
                .0
                .join("backlog/drafts/draft-1 - Field-draft.md"),
        )
        .unwrap(),
    )
    .unwrap();
    let expected = backlog_md::records::parse_task(
        &std::fs::read_to_string(
            expected_fixture
                .0
                .join("backlog/drafts/draft-1 - Field-draft.md"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(
        actual.implementation_plan.as_deref(),
        Some("Implementation plan\n\nNext step")
    );
    assert_eq!(
        actual.implementation_notes.as_deref(),
        Some("Draft notes\n\nMore notes")
    );
    assert_eq!(actual.comments[0].body, "Reviewed by the team");
    assert_eq!(actual.comments[0].author.as_deref(), Some("Alex"));
    assert_eq!(
        actual.final_summary.as_deref(),
        Some("Summary\n\nFurther summary")
    );
    assert_eq!(actual.modified_files, ["src/file.rs"]);
}

#[test]
fn rust_draft_edit_title_rename_auto_commits_both_paths_and_keeps_unrelated_staged_work() {
    let expected_fixture = Fixture::new();
    let actual_fixture = Fixture::new();
    for fixture in [&expected_fixture, &actual_fixture] {
        fixture.file(
            "backlog.config.yml",
            "project_name: CLI Fixture\ntask_prefix: task\nstatuses:\n  - To Do\n  - Done\nauto_commit: true\n",
        );
        fixture.file(
            "backlog/drafts/draft-1-first.md",
            "---\nid: draft-1\ntitle: First draft\nstatus: Draft\ncreated_date: 2026-01-01\n---\n",
        );
        initialize_git_fixture(fixture);
    }
    let args = ["draft", "edit", "draft-1", "--title", "Renamed draft"];
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stdout, expected.stdout);
    assert_eq!(actual.stderr, expected.stderr);
    assert_eq!(
        selected_commit_result(&actual_fixture),
        selected_commit_result(&expected_fixture)
    );
    assert_eq!(
        git(&actual_fixture.0, &["diff", "--cached", "--name-only"]),
        "UNRELATED.txt"
    );
    assert_eq!(
        git(&actual_fixture.0, &["status", "--porcelain"]),
        "A  UNRELATED.txt"
    );
}

#[test]
fn rust_draft_edit_status_refusal_matches_typescript_without_mutation() {
    let expected_fixture = Fixture::new();
    let actual_fixture = Fixture::new();
    for fixture in [&expected_fixture, &actual_fixture] {
        fixture.file(
            "backlog/drafts/draft-1-first.md",
            "---\nid: draft-1\ntitle: First draft\nstatus: Draft\ncreated_date: 2026-01-01\n---\nOriginal body.\n",
        );
    }
    let path = "backlog/drafts/draft-1-first.md";
    let expected_before = std::fs::read(expected_fixture.0.join(path)).unwrap();
    let actual_before = std::fs::read(actual_fixture.0.join(path)).unwrap();
    let args = ["draft", "edit", "draft-1", "--status", "To Do"];
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stdout, expected.stdout);
    assert_eq!(actual.stderr, expected.stderr);
    assert_eq!(
        std::fs::read(expected_fixture.0.join(path)).unwrap(),
        expected_before
    );
    assert_eq!(
        std::fs::read(actual_fixture.0.join(path)).unwrap(),
        actual_before
    );
}

#[test]
fn rust_draft_edit_checklist_operations_match_typescript() {
    let expected_fixture = Fixture::new();
    let actual_fixture = Fixture::new();
    for fixture in [&expected_fixture, &actual_fixture] {
        fixture.file(
            "backlog/drafts/draft-1-checklists.md",
            "---\nid: draft-1\ntitle: Checklist draft\nstatus: Draft\ncreated_date: 2026-01-01\n---\n## Acceptance Criteria\n\n- [ ] Preserve checked state.\n\n## Definition of Done\n\n- [ ] Keep unmanaged text.\n",
        );
    }
    let args = [
        "draft",
        "edit",
        "draft-1",
        "--check-ac",
        "1",
        "--uncheck-dod",
        "1",
        "--plain",
    ];
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(
        String::from_utf8_lossy(&actual.stdout)
            .replace(&actual_fixture.0.to_string_lossy().to_string(), "<ROOT>"),
        String::from_utf8_lossy(&expected.stdout)
            .replace(&expected_fixture.0.to_string_lossy().to_string(), "<ROOT>")
    );
    assert_eq!(actual.stderr, expected.stderr);
    let actual = backlog_md::records::parse_task(
        &std::fs::read_to_string(
            actual_fixture
                .0
                .join("backlog/drafts/draft-1 - Checklist-draft.md"),
        )
        .unwrap(),
    )
    .unwrap();
    let expected = backlog_md::records::parse_task(
        &std::fs::read_to_string(
            expected_fixture
                .0
                .join("backlog/drafts/draft-1 - Checklist-draft.md"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(actual, expected);
    assert!(actual.acceptance_criteria_items[0].checked);
    assert!(!actual.definition_of_done_items[0].checked);
}

#[test]
fn rust_draft_edit_frontmatter_drift_matches_typescript_without_mutation() {
    let expected_fixture = Fixture::new();
    let actual_fixture = Fixture::new();
    for fixture in [&expected_fixture, &actual_fixture] {
        fixture.file(
            "backlog/drafts/draft-1-drifted.md",
            "---\nid: draft-99\ntitle: Drifted draft\nstatus: Draft\ncreated_date: 2026-01-01\n---\nDo not change this file.\n",
        );
    }
    let before =
        std::fs::read(expected_fixture.0.join("backlog/drafts/draft-1-drifted.md")).unwrap();
    let args = ["draft", "edit", "draft-1", "--title", "Changed"];
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stdout, expected.stdout);
    assert_eq!(
        String::from_utf8_lossy(&actual.stderr)
            .replace(&actual_fixture.0.to_string_lossy().to_string(), "<ROOT>"),
        String::from_utf8_lossy(&expected.stderr)
            .replace(&expected_fixture.0.to_string_lossy().to_string(), "<ROOT>")
    );
    assert_eq!(
        std::fs::read(expected_fixture.0.join("backlog/drafts/draft-1-drifted.md")).unwrap(),
        before
    );
    assert_eq!(
        std::fs::read(actual_fixture.0.join("backlog/drafts/draft-1-drifted.md")).unwrap(),
        before
    );
}

#[test]
fn rust_draft_edit_parse_failure_matches_typescript_without_mutation() {
    let expected_fixture = Fixture::new();
    let actual_fixture = Fixture::new();
    for fixture in [&expected_fixture, &actual_fixture] {
        fixture.file(
            "backlog/drafts/draft-1-broken.md",
            "---\nid: [unclosed\ntitle: Broken draft\nstatus: Draft\n---\nOriginal bytes.\n",
        );
    }
    let path = "backlog/drafts/draft-1-broken.md";
    let before = std::fs::read(expected_fixture.0.join(path)).unwrap();
    let args = ["draft", "edit", "draft-1", "--title", "Changed"];
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stdout, expected.stdout);
    assert_eq!(
        String::from_utf8_lossy(&actual.stderr)
            .replace(&actual_fixture.0.to_string_lossy().to_string(), "<ROOT>"),
        String::from_utf8_lossy(&expected.stderr)
            .replace(&expected_fixture.0.to_string_lossy().to_string(), "<ROOT>")
    );
    assert_eq!(
        std::fs::read(expected_fixture.0.join(path)).unwrap(),
        before
    );
    assert_eq!(std::fs::read(actual_fixture.0.join(path)).unwrap(), before);
}

#[test]
fn rust_draft_edit_checklist_errors_use_draft_guidance() {
    let expected_fixture = Fixture::new();
    let actual_fixture = Fixture::new();
    for fixture in [&expected_fixture, &actual_fixture] {
        fixture.file(
            "backlog/drafts/draft-1-first.md",
            "---\nid: draft-1\ntitle: First draft\nstatus: Draft\ncreated_date: 2026-01-01\n---\n",
        );
    }
    let args = ["draft", "edit", "draft-1", "--check-ac", "7"];
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stdout, expected.stdout);
    assert_eq!(
        String::from_utf8_lossy(&actual.stderr)
            .replace(&actual_fixture.0.to_string_lossy().to_string(), "<ROOT>"),
        String::from_utf8_lossy(&expected.stderr)
            .replace(&expected_fixture.0.to_string_lossy().to_string(), "<ROOT>")
    );
}

#[test]
fn rust_draft_view_and_edit_identity_errors_match_typescript() {
    let expected_fixture = Fixture::new();
    let actual_fixture = Fixture::new();
    for fixture in [&expected_fixture, &actual_fixture] {
        fixture.file(
            "backlog/drafts/draft-1-alpha.md",
            "---\nid: draft-1\ntitle: Alpha\nstatus: Draft\ncreated_date: 2026-01-01\n---\n",
        );
        fixture.file(
            "backlog/drafts/draft-01-beta.md",
            "---\nid: draft-01\ntitle: Beta\nstatus: Draft\ncreated_date: 2026-01-02\n---\n",
        );
    }

    for args in [
        &["draft", "view", "1", "--plain"][..],
        &["draft", "edit", "1", "--title", "Changed"][..],
        &["draft", "view", "DRAFT-99", "--plain"][..],
        &["draft", "edit", "DRAFT-99", "--title", "Changed"][..],
    ] {
        let expected = typescript_cli(args, &expected_fixture.0);
        let actual = rust_cli(args, &actual_fixture.0);
        assert_eq!(actual.status.code(), expected.status.code(), "{args:?}");
        assert_eq!(actual.stdout, expected.stdout, "{args:?}");
        assert_eq!(
            String::from_utf8_lossy(&actual.stderr)
                .replace(&actual_fixture.0.to_string_lossy().to_string(), "<ROOT>"),
            String::from_utf8_lossy(&expected.stderr)
                .replace(&expected_fixture.0.to_string_lossy().to_string(), "<ROOT>"),
            "{args:?}"
        );
    }
}

#[test]
fn rust_draft_list_matches_typescript_for_plain_json_and_windows() {
    let empty = Fixture::new();
    std::fs::create_dir_all(empty.0.join("backlog/drafts")).unwrap();
    for args in [
        &["draft", "list", "--plain"][..],
        &["draft", "list", "--json"][..],
    ] {
        let expected = typescript_cli(args, &empty.0);
        let actual = rust_cli(args, &empty.0);
        assert_eq!(actual.status.code(), expected.status.code(), "{args:?}");
        assert_eq!(actual.stderr, expected.stderr, "{args:?}");
        if args.contains(&"--json") {
            assert_eq!(
                serde_json::from_slice::<Value>(&actual.stdout).unwrap(),
                serde_json::from_slice::<Value>(&expected.stdout).unwrap()
            );
        } else {
            assert_eq!(actual.stdout, expected.stdout, "{args:?}");
        }
    }

    let populated = fixture();
    populated.file("backlog/drafts/draft-1-first.md", "---\nid: draft-1\ntitle: First draft\nstatus: Draft\npriority: high\ncreated_date: 2026-01-01\nordinal: 2\n---\n");
    populated.file("backlog/drafts/draft-2-second.md", "---\nid: draft-2\ntitle: Second draft\nstatus: Draft\npriority: low\ncreated_date: 2026-01-02\nordinal: 1\n---\n");
    populated.file("backlog/drafts/draft-03-third.md", "---\nid: draft-03\ntitle: Third draft\nstatus: Draft\npriority: medium\ncreated_date: 2026-01-03\nordinal: 3\n---\n");
    let original_first =
        std::fs::read(populated.0.join("backlog/drafts/draft-1-first.md")).unwrap();
    let original_second =
        std::fs::read(populated.0.join("backlog/drafts/draft-2-second.md")).unwrap();
    let original_third =
        std::fs::read(populated.0.join("backlog/drafts/draft-03-third.md")).unwrap();
    for args in [
        &["draft", "list", "--plain"][..],
        &["draft", "list", "--json"][..],
        &["draft", "list", "--sort", "id", "--max-count", "1"][..],
        &["draft", "list", "--max-count", " 1 ", "--skip", " 0 "][..],
        &["draft", "list", "--sort", "ordinal", "--max-count", "1"][..],
        &["draft", "list", "--json", "--skip", "1"][..],
        &[
            "draft",
            "list",
            "--json",
            "--max-count",
            "999999999999999999999999999999",
        ][..],
        &["draft", "list", "--skip", "999999999999999999999999999999"][..],
        &["draft", "list", "--count"][..],
        &["draft", "list", "--count", "--json"][..],
        &["draft", "list", "--sort", "unknown"][..],
        &["draft", "list", "--max-count", "0"][..],
        &["draft", "list", "--max-count", "-1"][..],
        &["draft", "list", "--max-count", "+1"][..],
        &["draft", "list", "--skip", "-1"][..],
        &["draft", "list", "--plain", "--json"][..],
    ] {
        let expected = typescript_cli(args, &populated.0);
        let actual = rust_cli(args, &populated.0);
        assert_eq!(actual.status.code(), expected.status.code(), "{args:?}");
        assert_eq!(actual.stderr, expected.stderr, "{args:?}");
        if args.contains(&"--json") && expected.status.success() {
            assert_eq!(
                serde_json::from_slice::<Value>(&actual.stdout).unwrap(),
                serde_json::from_slice::<Value>(&expected.stdout).unwrap(),
                "{args:?}"
            );
        } else {
            assert_eq!(actual.stdout, expected.stdout, "{args:?}");
        }
        assert_eq!(
            std::fs::read(populated.0.join("backlog/drafts/draft-1-first.md")).unwrap(),
            original_first
        );
        assert_eq!(
            std::fs::read(populated.0.join("backlog/drafts/draft-2-second.md")).unwrap(),
            original_second
        );
        assert_eq!(
            std::fs::read(populated.0.join("backlog/drafts/draft-03-third.md")).unwrap(),
            original_third
        );
    }
    let help = rust_cli(&["draft", "list", "--help"], &populated.0);
    assert!(help.status.success());
    let help = String::from_utf8_lossy(&help.stdout);
    for option in [
        "--plain",
        "--json",
        "--sort",
        "--max-count",
        "--skip",
        "--count",
    ] {
        assert!(help.contains(option), "missing {option} in draft list help");
    }
}

#[test]
fn rust_draft_create_matches_typescript_defaults_values_and_markdown() {
    let args = [
        "draft",
        "create",
        "  First draft  ",
        "--desc",
        "A multiline\ndescription",
        "-a",
        "@alice,@bob",
        "-a",
        "@carol",
        "-l",
        "backend,api",
        "-l",
        "ui",
        "-s",
        "Done",
    ];
    let config = "project_name: CLI Fixture\ntask_prefix: task\nstatuses:\n  - To Do\n  - Done\npriorities:\n  - High\n  - Low\ndefault_assignee:\n  - '@default'\ndefinition_of_done:\n  - Review\n  - Publish\n";
    let expected_fixture = fixture();
    let actual_fixture = fixture();
    for fixture in [&expected_fixture, &actual_fixture] {
        fixture.file("backlog.config.yml", config);
        std::fs::remove_dir_all(fixture.0.join("backlog/drafts")).ok();
    }
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stderr, expected.stderr);
    let actual_stdout = String::from_utf8_lossy(&actual.stdout)
        .replace(&actual_fixture.0.to_string_lossy().to_string(), "<ROOT>");
    let expected_stdout = String::from_utf8_lossy(&expected.stdout)
        .replace(&expected_fixture.0.to_string_lossy().to_string(), "<ROOT>");
    assert_eq!(actual_stdout, expected_stdout);
    let parse_created = |fixture: &Fixture| {
        let path = std::fs::read_dir(fixture.0.join("backlog/drafts"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let mut task =
            backlog_md::records::parse_task(&std::fs::read_to_string(path).unwrap()).unwrap();
        task.created_date.clear();
        task
    };
    assert_eq!(
        parse_created(&actual_fixture),
        parse_created(&expected_fixture)
    );

    for args in [
        &[
            "draft",
            "create",
            "Description precedence",
            "--description",
            "Primary",
            "--desc",
            "Fallback",
        ][..],
        &["draft", "create", "Default draft"][..],
        &["draft", "create", "Unassigned draft", "-a", ""][..],
    ] {
        let expected_fixture = fixture();
        let actual_fixture = fixture();
        for fixture in [&expected_fixture, &actual_fixture] {
            fixture.file("backlog.config.yml", config);
            std::fs::remove_dir_all(fixture.0.join("backlog/drafts")).ok();
        }
        let expected = typescript_cli(args, &expected_fixture.0);
        let actual = rust_cli(args, &actual_fixture.0);
        assert_eq!(actual.status.code(), expected.status.code(), "{args:?}");
        assert_eq!(actual.stderr, expected.stderr, "{args:?}");
        let actual_stdout = String::from_utf8_lossy(&actual.stdout)
            .replace(&actual_fixture.0.to_string_lossy().to_string(), "<ROOT>");
        let expected_stdout = String::from_utf8_lossy(&expected.stdout)
            .replace(&expected_fixture.0.to_string_lossy().to_string(), "<ROOT>");
        assert_eq!(actual_stdout, expected_stdout, "{args:?}");
        assert_eq!(
            parse_created(&actual_fixture),
            parse_created(&expected_fixture),
            "{args:?}"
        );
    }
}

#[test]
fn rust_draft_create_empty_title_refuses_without_creating_storage() {
    let args = ["draft", "create", ""];
    let expected_fixture = fixture();
    let actual_fixture = fixture();
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stdout, expected.stdout);
    assert_eq!(actual.stderr, expected.stderr);
    assert!(!expected_fixture.0.join("backlog/drafts").exists());
    assert!(!actual_fixture.0.join("backlog/drafts").exists());
    let help = rust_cli(&["draft", "create", "--help"], &actual_fixture.0);
    assert!(help.status.success());
    let help = String::from_utf8_lossy(&help.stdout);
    for option in [
        "--description",
        "--desc",
        "--assignee",
        "--status",
        "--labels",
    ] {
        assert!(
            help.contains(option),
            "missing {option} in draft create help"
        );
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
fn rust_task_id_shorthand_and_tasks_alias_match_the_live_typescript_cli() {
    let fixture = fixture();
    for args in [
        &["task", "1", "--plain"][..],
        &["task", "--json", "1"][..],
        &["task", "1"][..],
        &["tasks", "task-1", "--json"][..],
        &["task", "999", "--plain"][..],
        &["task", "complete"][..],
        &["task", "complete", "foo"][..],
        &["task", "complete", "--json"][..],
        &["task", "complete", "foo", "--json"][..],
        &["task", "--json", "complete"][..],
        &["task", "view", "1", "--json"][..],
        &["task", "list", "--plain"][..],
        &["task", "1", "--plain", "--json"][..],
        &["task"][..],
    ] {
        let expected = typescript_cli(args, &fixture.0);
        let actual = rust_cli(args, &fixture.0);
        assert_eq!(
            actual.status.code(),
            expected.status.code(),
            "args: {args:?}"
        );
        assert_eq!(actual.stderr, expected.stderr, "args: {args:?}");
        if args.contains(&"--json") && actual.status.success() {
            let actual_json: Value = serde_json::from_slice(&actual.stdout)
                .unwrap_or_else(|error| panic!("Rust JSON parse failed for {args:?}: {error}"));
            let expected_json: Value =
                serde_json::from_slice(&expected.stdout).unwrap_or_else(|error| {
                    panic!("TypeScript JSON parse failed for {args:?}: {error}")
                });
            assert_eq!(actual_json, expected_json, "args: {args:?}");
        } else if args == ["task"] {
            let actual_help = String::from_utf8_lossy(&actual.stdout);
            let expected_help = String::from_utf8_lossy(&expected.stdout);
            assert!(
                actual_help.contains("Usage:"),
                "Rust should print task help"
            );
            assert!(
                !expected_help.is_empty(),
                "TypeScript should print task help"
            );
            for subcommand in ["create", "list", "view"] {
                assert!(
                    actual_help.contains(subcommand),
                    "missing {subcommand} in task help"
                );
            }
        } else {
            assert_eq!(actual.stdout, expected.stdout, "args: {args:?}");
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
        let actual_stdout = String::from_utf8_lossy(&actual.stdout)
            .replace(&actual_fixture.0.to_string_lossy().to_string(), "<ROOT>");
        let expected_stdout = String::from_utf8_lossy(&expected.stdout)
            .replace(&expected_fixture.0.to_string_lossy().to_string(), "<ROOT>");
        assert_eq!(actual_stdout, expected_stdout, "{args:?}");
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
        "--depends-on",
        "--dep",
        "--clear-deps",
        "--ref",
        "--add-ref",
        "--remove-ref",
        "--clear-refs",
        "--doc",
        "--add-doc",
        "--remove-doc",
        "--clear-docs",
        "--acceptance-criteria",
        "--ac",
        "--clear-ac",
        "--remove-ac",
        "--check-ac",
        "--uncheck-ac",
        "--dod",
        "--remove-dod",
        "--check-dod",
        "--uncheck-dod",
        "--plain",
    ] {
        assert!(
            help.contains(option),
            "missing {option} from task edit help"
        );
    }
}

#[test]
fn rust_task_edit_checklists_match_live_typescript_and_preserve_checked_items() {
    let args = [
        "task",
        "edit",
        "task-1",
        "--acceptance-criteria",
        "Keep, comma",
        "--acceptance-criteria",
        "New item",
        "--dod",
        "Ship it",
        "--check-dod",
        "2",
    ];
    let ts_fixture = fixture();
    ts_fixture.file("backlog/tasks/task-1-first.md", "---\nid: task-1\ntitle: First task\nstatus: To Do\npriority: high\ncreated_date: 2026-01-01\n---\n\n## Acceptance Criteria\n<!-- AC:BEGIN -->\n- [x] #1 Keep, comma\n- [ ] #2 Remove me\n<!-- AC:END -->\n\n## Definition of Done\n<!-- DOD:BEGIN -->\n- [ ] #1 Check me\n<!-- DOD:END -->\n");
    let original = ts_fixture.0.join("backlog/tasks/task-1-first.md");
    let expected = typescript_cli(&args, &ts_fixture.0);
    let expected_source = std::fs::read(&original).unwrap();
    let rust_fixture = fixture();
    rust_fixture.file("backlog/tasks/task-1-first.md", "---\nid: task-1\ntitle: First task\nstatus: To Do\npriority: high\ncreated_date: 2026-01-01\n---\n\n## Acceptance Criteria\n<!-- AC:BEGIN -->\n- [x] #1 Keep, comma\n- [ ] #2 Remove me\n<!-- AC:END -->\n\n## Definition of Done\n<!-- DOD:BEGIN -->\n- [ ] #1 Check me\n<!-- DOD:END -->\n");
    let actual = rust_cli(&args, &rust_fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stderr, expected.stderr);
    assert_eq!(actual.stdout, expected.stdout);
    let actual_source =
        std::fs::read_to_string(rust_fixture.0.join("backlog/tasks/task-1-first.md")).unwrap();
    let expected_source = String::from_utf8(expected_source).unwrap();
    assert_eq!(
        actual_source.splitn(3, "---").nth(2).unwrap().trim_end(),
        expected_source.splitn(3, "---").nth(2).unwrap().trim_end()
    );
}

#[test]
fn rust_task_edit_checklist_refusals_match_live_typescript_without_writes() {
    for args in [
        vec!["task", "edit", "task-1", "--clear-ac", "--ac", "Extra"],
        vec!["task", "edit", "task-1", "--check-ac", "0"],
    ] {
        let ts_fixture = fixture();
        ts_fixture.file("backlog/tasks/task-1-first.md", "---\nid: task-1\ntitle: First task\nstatus: To Do\npriority: high\ncreated_date: 2026-01-01\n---\n\n## Acceptance Criteria\n<!-- AC:BEGIN -->\n- [ ] #1 Existing\n<!-- AC:END -->\n");
        let original = std::fs::read(ts_fixture.0.join("backlog/tasks/task-1-first.md")).unwrap();
        let expected = typescript_cli(&args, &ts_fixture.0);
        assert!(!expected.status.success());
        assert_eq!(
            std::fs::read(ts_fixture.0.join("backlog/tasks/task-1-first.md")).unwrap(),
            original
        );

        let rust_fixture = fixture();
        rust_fixture.file("backlog/tasks/task-1-first.md", "---\nid: task-1\ntitle: First task\nstatus: To Do\npriority: high\ncreated_date: 2026-01-01\n---\n\n## Acceptance Criteria\n<!-- AC:BEGIN -->\n- [ ] #1 Existing\n<!-- AC:END -->\n");
        let original = std::fs::read(rust_fixture.0.join("backlog/tasks/task-1-first.md")).unwrap();
        let actual = rust_cli(&args, &rust_fixture.0);
        assert_eq!(
            actual.status.code(),
            expected.status.code(),
            "args: {args:?}"
        );
        assert_eq!(actual.stdout, expected.stdout, "args: {args:?}");
        assert_eq!(actual.stderr, expected.stderr, "args: {args:?}");
        assert_eq!(
            std::fs::read(rust_fixture.0.join("backlog/tasks/task-1-first.md")).unwrap(),
            original
        );
    }
}

#[test]
fn rust_task_edit_malformed_checklists_refuse_indexed_edits_without_writes() {
    let args = ["task", "edit", "task-1", "--check-ac", "1"];
    let contents = "---\nid: task-1\ntitle: First task\nstatus: To Do\npriority: high\ncreated_date: 2026-01-01\n---\n\n## Acceptance Criteria\n<!-- AC:BEGIN -->\n- [?] Broken mark\n<!-- AC:END -->\n";
    let ts_fixture = fixture();
    ts_fixture.file("backlog/tasks/task-1-first.md", contents);
    let expected = typescript_cli(&args, &ts_fixture.0);
    let ts_path = ts_fixture.0.join("backlog/tasks/task-1-first.md");
    let original = std::fs::read(&ts_path).unwrap();
    let rust_fixture = fixture();
    rust_fixture.file("backlog/tasks/task-1-first.md", contents);
    let actual = rust_cli(&args, &rust_fixture.0);
    assert!(!actual.status.success());
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stderr, expected.stderr);
    assert_eq!(
        std::fs::read(rust_fixture.0.join("backlog/tasks/task-1-first.md")).unwrap(),
        original
    );
}

#[test]
fn rust_task_edit_noop_indexed_checklist_refuses_malformed_marks() {
    let args = ["task", "edit", "task-1", "--uncheck-ac", "2"];
    let contents = "---\nid: task-1\ntitle: First task\nstatus: To Do\npriority: high\ncreated_date: 2026-01-01\n---\n\n## Acceptance Criteria\n<!-- AC:BEGIN -->\n- [x] #1 Checked\n- [ ] #2 Already unchecked\n- [?] #3 Invalid\n<!-- AC:END -->\n";
    let ts_fixture = fixture();
    ts_fixture.file("backlog/tasks/task-1-first.md", contents);
    let expected = typescript_cli(&args, &ts_fixture.0);
    let rust_fixture = fixture();
    rust_fixture.file("backlog/tasks/task-1-first.md", contents);
    let actual = rust_cli(&args, &rust_fixture.0);
    assert!(!actual.status.success());
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stderr, expected.stderr);
    assert_eq!(actual.stdout, expected.stdout);
    assert_eq!(
        std::fs::read(rust_fixture.0.join("backlog/tasks/task-1-first.md")).unwrap(),
        contents.as_bytes()
    );
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
fn rust_task_edit_dependencies_references_and_docs_match_live_typescript() {
    let expected_fixture = fixture();
    let actual_fixture = fixture();
    for fixture in [&expected_fixture, &actual_fixture] {
        fixture.file(
            "backlog/tasks/task-1-first.md",
            "---\nid: task-1\ntitle: First task\nstatus: To Do\npriority: high\nlabels:\n  - backend\ndependencies:\n  - task-2\ncustom: retained\ncreated_date: 2026-01-01\n---\n## Notes\n\nKeep this body.\n",
        );
    }
    for args in [
        vec![
            "task",
            "edit",
            "1",
            "--depends-on",
            "2,2",
            "--dep",
            "2",
            "--ref",
            "issue:1,https://example.test/a",
            "--ref",
            "issue:2",
            "--doc",
            "docs/one.md,https://example.test/doc",
            "--plain",
        ],
        vec![
            "task",
            "edit",
            "1",
            "--add-ref",
            "issue:3,issue:4",
            "--remove-ref",
            "issue:1",
            "--add-doc",
            "docs/two.md",
            "--remove-doc",
            "docs/one.md",
            "--plain",
        ],
        vec!["task", "edit", "1", "--ref", "", "--doc", "", "--plain"],
        vec!["task", "edit", "1", "--add-doc", "docs/default.md"],
        vec![
            "task",
            "edit",
            "1",
            "--clear-deps",
            "--clear-refs",
            "--clear-docs",
            "--dep",
            "",
            "--ref",
            "",
            "--doc",
            "",
            "--plain",
        ],
    ] {
        let expected = typescript_cli(&args, &expected_fixture.0);
        let actual = rust_cli(&args, &actual_fixture.0);
        assert_eq!(actual.status.code(), expected.status.code(), "{args:?}");
        let actual_stdout = String::from_utf8_lossy(&actual.stdout)
            .replace(&actual_fixture.0.to_string_lossy().to_string(), "<ROOT>");
        let expected_stdout = String::from_utf8_lossy(&expected.stdout)
            .replace(&expected_fixture.0.to_string_lossy().to_string(), "<ROOT>");
        assert_eq!(actual_stdout, expected_stdout, "{args:?}");
        assert_eq!(actual.stderr, expected.stderr, "{args:?}");
        let read = |fixture: &Fixture| {
            let source =
                std::fs::read_to_string(fixture.0.join("backlog/tasks/task-1-first.md")).unwrap();
            backlog_md::records::parse_task(&source).unwrap()
        };
        let mut actual_task = read(&actual_fixture);
        let mut expected_task = read(&expected_fixture);
        actual_task.updated_date = None;
        expected_task.updated_date = None;
        assert_eq!(actual_task, expected_task, "{args:?}");
    }
    let source =
        std::fs::read_to_string(actual_fixture.0.join("backlog/tasks/task-1-first.md")).unwrap();
    assert!(source.contains("custom: retained"));
    assert!(source.contains("## Notes\n\nKeep this body."));
}

#[test]
fn rust_task_edit_invalid_relationship_inputs_match_live_typescript_without_writes() {
    for args in [
        vec!["task", "edit", "1", "--depends-on", "404"],
        vec!["task", "edit", "1", "--depends-on", "1"],
        vec!["task", "edit", "1", "--clear-deps", "--dep", "2"],
        vec!["task", "edit", "1", "--ref", "only", "--add-ref", "more"],
        vec!["task", "edit", "1", "--add-ref", ""],
        vec!["task", "edit", "1", "--remove-ref", ""],
        vec!["task", "edit", "1", "--clear-refs", "--add-ref", "more"],
        vec!["task", "edit", "1", "--doc", "only", "--remove-doc", "old"],
        vec!["task", "edit", "1", "--add-doc", ""],
        vec!["task", "edit", "1", "--remove-doc", ""],
        vec!["task", "edit", "1", "--clear-docs", "--remove-doc", "old"],
    ] {
        let expected_fixture = fixture();
        let actual_fixture = fixture();
        let expected = typescript_cli(&args, &expected_fixture.0);
        let actual = rust_cli(&args, &actual_fixture.0);
        assert_eq!(actual.status.code(), expected.status.code(), "{args:?}");
        assert_eq!(actual.stdout, expected.stdout, "{args:?}");
        assert_eq!(actual.stderr, expected.stderr, "{args:?}");
        for path in ["task-1-first.md", "task-2-second.md"] {
            let path = format!("backlog/tasks/{path}");
            assert_eq!(
                std::fs::read(actual_fixture.0.join(&path)).unwrap(),
                std::fs::read(expected_fixture.0.join(&path)).unwrap(),
                "{args:?}"
            );
        }
    }
}

#[test]
fn rust_task_edit_dependency_cycle_refusal_matches_live_typescript_without_writes() {
    let expected_fixture = fixture();
    let actual_fixture = fixture();
    for fixture in [&expected_fixture, &actual_fixture] {
        fixture.file(
            "backlog/tasks/task-2-second.md",
            "---\nid: task-2\ntitle: Second task\nstatus: Done\ndependencies:\n  - task-1\ncreated_date: 2026-01-02\n---\n",
        );
    }
    let args = ["task", "edit", "1", "--depends-on", "2"];
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stdout, expected.stdout);
    assert_eq!(actual.stderr, expected.stderr);
    for path in ["task-1-first.md", "task-2-second.md"] {
        let path = format!("backlog/tasks/{path}");
        assert_eq!(
            std::fs::read(actual_fixture.0.join(&path)).unwrap(),
            std::fs::read(expected_fixture.0.join(&path)).unwrap()
        );
    }
}

#[test]
fn rust_task_archive_restore_help_and_transitions_match_live_typescript() {
    let expected_fixture = fixture();
    let actual_fixture = fixture();
    for (args, action) in [
        (&["task", "archive", "1"][..], "archive"),
        (&["task", "restore", "1"][..], "restore"),
    ] {
        let expected = typescript_cli(args, &expected_fixture.0);
        let actual = rust_cli(args, &actual_fixture.0);
        assert_eq!(actual.status.code(), expected.status.code(), "{action}");
        assert_eq!(actual.stdout, expected.stdout, "{action}");
        assert_eq!(actual.stderr, expected.stderr, "{action}");
        let expected_dir = if action == "archive" {
            "backlog/archive/tasks"
        } else {
            "backlog/tasks"
        };
        let actual_dir = actual_fixture.0.join(expected_dir);
        let expected_dir = expected_fixture.0.join(expected_dir);
        let actual_file = std::fs::read_dir(actual_dir)
            .unwrap()
            .filter_map(Result::ok)
            .find(|entry| entry.file_name().to_string_lossy().contains("task-1"))
            .unwrap()
            .path();
        let expected_file = std::fs::read_dir(expected_dir)
            .unwrap()
            .filter_map(Result::ok)
            .find(|entry| entry.file_name().to_string_lossy().contains("task-1"))
            .unwrap()
            .path();
        assert_eq!(
            std::fs::read(actual_file).unwrap(),
            std::fs::read(expected_file).unwrap(),
            "{action}"
        );
        if action == "archive" {
            let repeated_args = ["task", "archive", "1"];
            let expected = typescript_cli(&repeated_args, &expected_fixture.0);
            let actual = rust_cli(&repeated_args, &actual_fixture.0);
            assert_eq!(actual.status.code(), expected.status.code());
            assert_eq!(actual.stdout, expected.stdout);
            assert_eq!(actual.stderr, expected.stderr);
        }
    }
    for command in ["archive", "restore", "delete"] {
        let output = rust_cli(&["task", command, "--help"], &actual_fixture.0);
        assert!(output.status.success(), "{command}");
        let help = String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        let required_copy: &[&str] = match command {
            "archive" => &[
                "id stays reserved",
                "dependencies",
                "no other task is changed",
            ],
            "restore" => &["same id", "status it had", "archived tasks are read-only"],
            "delete" => &["other tasks and drafts", "subtasks", "no prompt"],
            _ => unreachable!(),
        };
        for phrase in required_copy {
            assert!(help.contains(phrase), "{command} help lacks {phrase}");
        }
    }
}

#[test]
fn rust_task_delete_cleans_task_links_and_matches_live_typescript() {
    let expected_fixture = fixture();
    let actual_fixture = fixture();
    for fixture in [&expected_fixture, &actual_fixture] {
        let task_path = fixture.0.join("backlog/tasks/task-1-first.md");
        let task_source = std::fs::read_to_string(&task_path).unwrap();
        std::fs::create_dir_all(fixture.0.join("backlog/archive/tasks")).unwrap();
        std::fs::write(
            fixture.0.join("backlog/archive/tasks/task-1-first.md"),
            task_source,
        )
        .unwrap();
        std::fs::remove_file(task_path).unwrap();
        fixture.file(
            "backlog/tasks/task-2-second.md",
            "---\nid: task-2\ntitle: Second task\nstatus: Done\ndependencies:\n  - task-1\ncreated_date: 2026-01-02\n---\n",
        );
        fixture.file(
            "backlog/drafts/draft-1 - Draft.md",
            "---\nid: DRAFT-1\ntitle: Draft\nstatus: Draft\ncreated_date: 2026-01-03\ndependencies:\n  - task-1\n---\nDraft body\n",
        );
    }
    let args = ["task", "delete", "1"];
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stdout, expected.stdout);
    assert_eq!(actual.stderr, expected.stderr);
    assert_eq!(
        std::fs::read_dir(actual_fixture.0.join("backlog/archive/tasks"))
            .unwrap()
            .count(),
        0
    );
    let actual_dependent =
        std::fs::read_to_string(actual_fixture.0.join("backlog/tasks/task-2-second.md")).unwrap();
    let expected_dependent =
        std::fs::read_to_string(expected_fixture.0.join("backlog/tasks/task-2-second.md")).unwrap();
    assert_eq!(
        backlog_md::records::parse_task(&actual_dependent)
            .unwrap()
            .dependencies,
        backlog_md::records::parse_task(&expected_dependent)
            .unwrap()
            .dependencies
    );
    let actual_draft =
        std::fs::read_to_string(actual_fixture.0.join("backlog/drafts/draft-1 - Draft.md"))
            .unwrap();
    assert!(actual_draft.contains("dependencies: []"));
    assert!(actual_draft.contains("Draft body"));
}

#[test]
fn rust_task_lifecycle_wrong_state_and_missing_errors_match_live_typescript() {
    for args in [
        vec!["task", "archive", "404"],
        vec!["task", "restore", "404"],
        vec!["task", "restore", "1"],
        vec!["task", "delete", "1"],
        vec!["task", "delete", "404"],
    ] {
        let expected_fixture = fixture();
        let actual_fixture = fixture();
        let expected = typescript_cli(&args, &expected_fixture.0);
        let actual = rust_cli(&args, &actual_fixture.0);
        assert_eq!(actual.status.code(), expected.status.code(), "{args:?}");
        assert_eq!(actual.stdout, expected.stdout, "{args:?}");
        assert_eq!(actual.stderr, expected.stderr, "{args:?}");
        assert_eq!(
            std::fs::read(expected_fixture.0.join("backlog/tasks/task-1-first.md")).unwrap(),
            std::fs::read(actual_fixture.0.join("backlog/tasks/task-1-first.md")).unwrap(),
            "{args:?}"
        );
    }
}

#[test]
fn rust_draft_archive_and_promote_match_live_typescript() {
    for (action, expected_directory) in [
        ("archive", "backlog/archive/drafts"),
        ("promote", "backlog/tasks"),
    ] {
        let expected_fixture = Fixture::new();
        let actual_fixture = Fixture::new();
        for fixture in [&expected_fixture, &actual_fixture] {
            fixture.file(
                "backlog/drafts/draft-1-first.md",
                "---\nid: draft-1\ntitle: First draft\nstatus: Draft\ncreated_date: 2026-01-01\ncustom_field: preserve\n---\nDraft body.\n",
            );
        }
        let args = ["draft", action, "1"];
        let expected = typescript_cli(&args, &expected_fixture.0);
        let actual = rust_cli(&args, &actual_fixture.0);
        assert_eq!(actual.status.code(), expected.status.code(), "{action}");
        assert_eq!(actual.stdout, expected.stdout, "{action}");
        assert_eq!(actual.stderr, expected.stderr, "{action}");
        assert!(
            !actual_fixture
                .0
                .join("backlog/drafts/draft-1-first.md")
                .exists()
        );
        let actual_path = std::fs::read_dir(actual_fixture.0.join(expected_directory))
            .unwrap()
            .filter_map(Result::ok)
            .next()
            .unwrap()
            .path();
        let expected_path = std::fs::read_dir(expected_fixture.0.join(expected_directory))
            .unwrap()
            .filter_map(Result::ok)
            .next()
            .unwrap()
            .path();
        if action == "archive" {
            assert_eq!(
                std::fs::read(actual_path).unwrap(),
                std::fs::read(expected_path).unwrap(),
                "{action}"
            );
        } else {
            let actual =
                backlog_md::records::parse_task(&std::fs::read_to_string(actual_path).unwrap())
                    .unwrap();
            let expected =
                backlog_md::records::parse_task(&std::fs::read_to_string(expected_path).unwrap())
                    .unwrap();
            assert_eq!(actual, expected, "{action}");
        }
    }
}

#[test]
fn rust_task_demote_matches_live_typescript_and_cleans_links() {
    let expected_fixture = fixture();
    let actual_fixture = fixture();
    for fixture in [&expected_fixture, &actual_fixture] {
        fixture.file(
            "backlog/tasks/task-2-second.md",
            "---\nid: task-2\ntitle: Second task\nstatus: To Do\ndependencies:\n  - task-1\nreferences:\n  - task-1\ncreated_date: 2026-01-02\n---\nDependent body.\n",
        );
    }
    let args = ["task", "demote", "1"];
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stdout, expected.stdout);
    assert_eq!(actual.stderr, expected.stderr);
    assert!(
        !actual_fixture
            .0
            .join("backlog/tasks/task-1-first.md")
            .exists()
    );
    let demoted_path = std::fs::read_dir(actual_fixture.0.join("backlog/drafts"))
        .unwrap()
        .filter_map(Result::ok)
        .next()
        .unwrap()
        .path();
    let expected_demoted_path = std::fs::read_dir(expected_fixture.0.join("backlog/drafts"))
        .unwrap()
        .filter_map(Result::ok)
        .next()
        .unwrap()
        .path();
    let actual_demoted =
        backlog_md::records::parse_task(&std::fs::read_to_string(demoted_path).unwrap()).unwrap();
    let expected_demoted =
        backlog_md::records::parse_task(&std::fs::read_to_string(expected_demoted_path).unwrap())
            .unwrap();
    assert_eq!(actual_demoted, expected_demoted);
    let actual_dependent = backlog_md::records::parse_task(
        &std::fs::read_to_string(actual_fixture.0.join("backlog/tasks/task-2-second.md")).unwrap(),
    )
    .unwrap();
    let expected_dependent = backlog_md::records::parse_task(
        &std::fs::read_to_string(expected_fixture.0.join("backlog/tasks/task-2-second.md"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(actual_dependent, expected_dependent);
    assert!(actual_dependent.dependencies.is_empty());
    assert!(actual_dependent.references.is_empty());
}

#[test]
fn rust_draft_archive_promote_and_task_demote_auto_commit_only_changed_paths() {
    for (command, record) in [
        (vec!["draft", "archive", "1"], "draft"),
        (vec!["draft", "promote", "1"], "draft"),
        (vec!["task", "demote", "1"], "task"),
    ] {
        let expected_fixture = Fixture::new();
        let actual_fixture = Fixture::new();
        for fixture in [&expected_fixture, &actual_fixture] {
            let config = "project_name: CLI Fixture\ntask_prefix: task\nstatuses:\n  - To Do\n  - Done\npriorities:\n  - High\n  - Low\nauto_commit: true\n";
            fixture.file("backlog.config.yml", config);
            fixture.file(
                "backlog/drafts/draft-1-first.md",
                "---\nid: draft-1\ntitle: First draft\nstatus: Draft\ncreated_date: 2026-01-01\n---\nDraft body.\n",
            );
            if record == "task" {
                fixture.file(
                    "backlog/tasks/task-1-first.md",
                    "---\nid: task-1\ntitle: First task\nstatus: To Do\ncreated_date: 2026-01-01\n---\nTask body.\n",
                );
                fixture.file(
                    "backlog/tasks/task-2-second.md",
                    "---\nid: task-2\ntitle: Second task\nstatus: To Do\ndependencies:\n  - task-1\nreferences:\n  - task-1\ncreated_date: 2026-01-02\n---\nDependent body.\n",
                );
            }
            initialize_git_fixture(fixture);
        }
        let expected = typescript_cli(&command, &expected_fixture.0);
        let actual = rust_cli(&command, &actual_fixture.0);
        assert_eq!(actual.status.code(), expected.status.code(), "{command:?}");
        assert_eq!(actual.stdout, expected.stdout, "{command:?}");
        assert_eq!(actual.stderr, expected.stderr, "{command:?}");
        assert_eq!(
            selected_commit_result(&actual_fixture),
            selected_commit_result(&expected_fixture),
            "{command:?}"
        );
        assert_eq!(
            git(&actual_fixture.0, &["diff", "--cached", "--name-only"]),
            "UNRELATED.txt",
            "{command:?}"
        );
    }
}

#[test]
fn rust_draft_lifecycle_identity_and_missing_errors_match_live_typescript() {
    for args in [
        &["draft", "archive", "404"][..],
        &["draft", "promote", "404"][..],
        &["task", "demote", "404"][..],
    ] {
        let expected_fixture = Fixture::new();
        let actual_fixture = Fixture::new();
        let expected = typescript_cli(args, &expected_fixture.0);
        let actual = rust_cli(args, &actual_fixture.0);
        assert_eq!(actual.status.code(), expected.status.code(), "{args:?}");
        assert_eq!(actual.stdout, expected.stdout, "{args:?}");
        assert_eq!(actual.stderr, expected.stderr, "{args:?}");
    }

    let expected_fixture = fixture();
    let actual_fixture = fixture();
    for fixture in [&expected_fixture, &actual_fixture] {
        let source = fixture.0.join("backlog/tasks/task-1-first.md");
        let archived = fixture.0.join("backlog/archive/tasks/task-1-first.md");
        std::fs::create_dir_all(archived.parent().unwrap()).unwrap();
        std::fs::rename(source, archived).unwrap();
    }
    let args = ["task", "demote", "1"];
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stdout, expected.stdout);
    assert_eq!(actual.stderr, expected.stderr);
    assert!(
        actual_fixture
            .0
            .join("backlog/archive/tasks/task-1-first.md")
            .exists()
    );
    assert!(!actual_fixture.0.join("backlog/drafts").exists());

    for action in ["archive", "promote"] {
        let expected_fixture = Fixture::new();
        let actual_fixture = Fixture::new();
        for fixture in [&expected_fixture, &actual_fixture] {
            fixture.file(
                "backlog/drafts/draft-1-drifted.md",
                "---\nid: draft-99\ntitle: Drifted draft\nstatus: Draft\ncreated_date: 2026-01-01\n---\nKeep this draft.\n",
            );
        }
        let args = ["draft", action, "1"];
        let expected = typescript_cli(&args, &expected_fixture.0);
        let actual = rust_cli(&args, &actual_fixture.0);
        assert_eq!(actual.status.code(), expected.status.code(), "{action}");
        assert_eq!(actual.stdout, expected.stdout, "{action}");
        assert_eq!(actual.stderr, expected.stderr, "{action}");
    }

    for action in ["archive", "promote"] {
        let expected_fixture = Fixture::new();
        let actual_fixture = Fixture::new();
        for fixture in [&expected_fixture, &actual_fixture] {
            fixture.file(
                "backlog/drafts/draft-1-alpha.md",
                "---\nid: draft-1\ntitle: Alpha\nstatus: Draft\ncreated_date: 2026-01-01\n---\n",
            );
            fixture.file(
                "backlog/drafts/draft-01-beta.md",
                "---\nid: draft-01\ntitle: Beta\nstatus: Draft\ncreated_date: 2026-01-02\n---\n",
            );
        }
        let args = ["draft", action, "1"];
        let expected = typescript_cli(&args, &expected_fixture.0);
        let actual = rust_cli(&args, &actual_fixture.0);
        assert_eq!(actual.status.code(), expected.status.code(), "{action}");
        assert_eq!(actual.stdout, expected.stdout, "{action}");
        assert_eq!(
            String::from_utf8_lossy(&actual.stderr)
                .replace(&actual_fixture.0.to_string_lossy().to_string(), "<ROOT>"),
            String::from_utf8_lossy(&expected.stderr)
                .replace(&expected_fixture.0.to_string_lossy().to_string(), "<ROOT>"),
            "{action}"
        );
        for fixture in [&actual_fixture, &expected_fixture] {
            assert!(fixture.0.join("backlog/drafts/draft-1-alpha.md").exists());
            assert!(fixture.0.join("backlog/drafts/draft-01-beta.md").exists());
        }
    }
}

#[test]
fn rust_task_delete_subtask_guard_matches_live_typescript() {
    let expected_fixture = fixture();
    let actual_fixture = fixture();
    for fixture in [&expected_fixture, &actual_fixture] {
        fixture.file(
            "backlog/tasks/task-2-second.md",
            "---\nid: task-2\ntitle: Second task\nstatus: Done\nparent_task_id: task-1\ncreated_date: 2026-01-02\n---\n",
        );
        let source =
            std::fs::read_to_string(fixture.0.join("backlog/tasks/task-1-first.md")).unwrap();
        fixture.file("backlog/archive/tasks/task-1-first.md", &source);
        std::fs::remove_file(fixture.0.join("backlog/tasks/task-1-first.md")).unwrap();
    }
    let args = ["task", "delete", "1"];
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stdout, expected.stdout);
    assert_eq!(actual.stderr, expected.stderr);
    assert_eq!(
        std::fs::read(
            actual_fixture
                .0
                .join("backlog/archive/tasks/task-1-first.md")
        )
        .unwrap(),
        std::fs::read(
            expected_fixture
                .0
                .join("backlog/archive/tasks/task-1-first.md")
        )
        .unwrap()
    );
}

#[test]
fn rust_task_archive_refuses_ambiguous_identity_without_writes() {
    let expected_fixture = fixture();
    let actual_fixture = fixture();
    for fixture in [&expected_fixture, &actual_fixture] {
        let active = fixture.0.join("backlog/tasks/task-1-first.md");
        let archived = fixture.0.join("backlog/archive/tasks/task-1-first.md");
        std::fs::create_dir_all(archived.parent().unwrap()).unwrap();
        std::fs::copy(active, archived).unwrap();
    }
    let args = ["task", "archive", "1"];
    let expected = typescript_cli(&args, &expected_fixture.0);
    let actual = rust_cli(&args, &actual_fixture.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stdout, expected.stdout);
    let actual_stderr = String::from_utf8_lossy(&actual.stderr)
        .replace(&actual_fixture.0.to_string_lossy().to_string(), "<ROOT>");
    let expected_stderr = String::from_utf8_lossy(&expected.stderr)
        .replace(&expected_fixture.0.to_string_lossy().to_string(), "<ROOT>");
    assert_eq!(actual_stderr, expected_stderr);
    for relative in [
        "backlog/tasks/task-1-first.md",
        "backlog/archive/tasks/task-1-first.md",
    ] {
        assert_eq!(
            std::fs::read(actual_fixture.0.join(relative)).unwrap(),
            std::fs::read(expected_fixture.0.join(relative)).unwrap()
        );
    }
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

#[test]
fn rust_document_list_matches_typescript_order_windows_and_errors() {
    let project = fixture();
    project.file(
        "backlog/docs/doc-10 - Zebra.md",
        "---\nid: doc-10\ntitle: Zebra\n---\nZebra body\n",
    );
    project.file(
        "backlog/docs/z/doc-4 - Guide.md",
        "---\nid: doc-4\ntitle: Guide\n---\nGuide Z\n",
    );
    project.file(
        "backlog/docs/a/doc-3 - Guide.md",
        "---\nid: doc-3\ntitle: Guide\n---\nGuide A\n",
    );
    project.file(
        "backlog/docs/doc-2 - Alpha.md",
        "---\nid: doc-2\ntitle: Alpha\n---\nAlpha body\n",
    );
    project.file(
        "backlog/docs/README.md",
        "---\nid: doc-99\ntitle: README\n---\nIgnore\n",
    );
    project.file(
        "backlog/archive/docs/doc-11 - Archived.md",
        "---\nid: doc-11\ntitle: Archived\n---\nIgnore\n",
    );

    for args in [
        &["doc", "list"][..],
        &["doc", "list", "--plain"][..],
        &["doc", "list", "--plain", "--max-count", "2"][..],
        &["doc", "list", "--plain", "--max-count", "2", "--skip", "2"][..],
        &["doc", "list", "--plain", "--skip", "2"][..],
        &["doc", "list", "--plain", "--count"][..],
        &[
            "doc",
            "list",
            "--plain",
            "--count",
            "--max-count",
            "2",
            "--skip",
            "1",
        ][..],
        &["doc", "list", "--plain", "--max-count", "0"][..],
        &["doc", "list", "--plain", "--max-count", "-1"][..],
        &["doc", "list", "--plain", "--skip", "-1"][..],
        &["doc", "list", "--plain", "--skip", "invalid"][..],
    ] {
        let expected = typescript_cli(args, &project.0);
        let actual = rust_cli(args, &project.0);
        assert_eq!(
            actual.status.code(),
            expected.status.code(),
            "args: {args:?}"
        );
        assert_eq!(actual.stdout, expected.stdout, "args: {args:?}");
        assert_eq!(actual.stderr, expected.stderr, "args: {args:?}");
    }

    let empty = fixture();
    let args = ["doc", "list", "--plain"];
    let expected = typescript_cli(&args, &empty.0);
    let actual = rust_cli(&args, &empty.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stdout, expected.stdout);
    assert_eq!(actual.stderr, expected.stderr);
}

#[test]
fn rust_document_view_matches_typescript_ids_archives_and_errors() {
    let project = fixture();
    project.file(
        "backlog/docs/doc-7 - Active Guide.md",
        "---\nid: DOC-7\ntitle: Active Guide\ntype: guide\ncustom: keep\n---\n\nStored active markdown.\n",
    );
    project.file(
        "backlog/archive/docs/doc-8 - Archived Guide.md",
        "---\nid: doc-8\ntitle: Archived Guide\ntype: guide\n---\n\nStored archived markdown.\n",
    );
    project.file(
        "backlog/docs/doc-9 - Conflict.md",
        "---\nid: doc-9\ntitle: Active conflict\n---\nActive claimant.\n",
    );
    project.file(
        "backlog/archive/docs/doc-9 - Conflict.md",
        "---\nid: DOC-009\ntitle: Archived conflict\n---\nArchived claimant.\n",
    );

    for args in [
        &["doc", "view", "7", "--plain"][..],
        &["doc", "view", "DOC-007"][..],
        &["doc", "view", "8", "--plain"][..],
        &["doc", "view", "doc-008"][..],
        &["doc", "view", "doc-999", "--plain"][..],
        &["doc", "view", "9", "--plain"][..],
    ] {
        let expected = typescript_cli(args, &project.0);
        let actual = rust_cli(args, &project.0);
        assert_eq!(
            actual.status.code(),
            expected.status.code(),
            "args: {args:?}"
        );
        assert_eq!(actual.stdout, expected.stdout, "args: {args:?}");
        assert_eq!(actual.stderr, expected.stderr, "args: {args:?}");
    }
}

fn normalize_document_timestamps(source: &str) -> String {
    let mut in_frontmatter = false;
    let mut normalized = String::new();
    for line in source.split_inclusive('\n') {
        if line.trim_end_matches('\n') == "---" {
            in_frontmatter = !in_frontmatter;
        }
        if in_frontmatter
            && (line.starts_with("created_date:") || line.starts_with("updated_date:"))
        {
            normalized.push_str(&format!(
                "{}: <generated>\n",
                line.split(':').next().unwrap()
            ));
        } else {
            normalized.push_str(line);
        }
    }
    normalized
}

fn document_update_fixture() -> Fixture {
    let fixture = fixture();
    fixture.file(
        "backlog/docs/guides/doc-1 - Setup-Guide.md",
        "---\nid: doc-1\ntitle: Setup Guide\ntype: guide\ncreated_date: 2026-01-01\ntags:\n  - setup\ncustom_field: keep me\n---\n# Original body\n\nPreserve this text.\n",
    );
    fixture.file(
        "backlog/archive/docs/doc-2 - Archived-Guide.md",
        "---\nid: doc-2\ntitle: Archived Guide\ntype: guide\ncreated_date: 2026-01-01\n---\nArchived body.\n",
    );
    fixture
}

const LIFECYCLE_DOCUMENT_PATH: &str = "backlog/docs/guides/doc-1 - Lifecycle-Guide.md";
const ARCHIVED_LIFECYCLE_DOCUMENT_PATH: &str =
    "backlog/archive/docs/guides/doc-1 - Lifecycle-Guide.md";
const LIFECYCLE_DOCUMENT_SOURCE: &str = "---\nid: doc-1\ntitle: Lifecycle Guide\ntype: guide\ncreated_date: '2026-01-01'\ncustom: keep exactly\n---\nOriginal body\n";

fn document_lifecycle_process_fixture() -> Fixture {
    let fixture = fixture();
    fixture.file(LIFECYCLE_DOCUMENT_PATH, LIFECYCLE_DOCUMENT_SOURCE);
    fixture.file(
        "backlog/tasks/task-1-first.md",
        "---\nid: task-1\ntitle: First task\nstatus: To Do\npriority: high\ncreated_date: 2026-01-01\ndocumentation:\n  - backlog/docs/guides/doc-1 - Lifecycle-Guide.md\n  - ./backlog/docs/guides/doc-1 - Lifecycle-Guide.md\n  - backlog/docs/guides/doc-1 - Lifecycle-Guide.md#section\n  - backlog/archive/docs/guides/doc-1 - Lifecycle-Guide.md\n---\n",
    );
    fixture.file(
        "backlog/archive/tasks/task-3-archived.md",
        "---\nid: task-3\ntitle: Archived task\nstatus: Done\npriority: low\ncreated_date: 2026-01-03\ndocumentation:\n  - backlog/docs/guides/doc-1 - Lifecycle-Guide.md\n  - backlog/archive/docs/guides/doc-1 - Lifecycle-Guide.md#archived\n---\n",
    );
    fixture.file(
        "backlog/drafts/draft-1 - Linked Draft.md",
        "---\nid: DRAFT-1\ntitle: Linked Draft\nstatus: Draft\ncreated_date: 2026-01-04\ndocumentation:\n  - ./backlog/docs/guides/doc-1 - Lifecycle-Guide.md#heading\n  - backlog/archive/docs/guides/doc-1 - Lifecycle-Guide.md\n---\n",
    );
    fixture
}

#[test]
fn rust_document_lifecycle_matches_typescript_state_changes_backlinks_and_id_release() {
    let reference = document_lifecycle_process_fixture();
    let candidate = document_lifecycle_process_fixture();
    let references_before = [
        "backlog/tasks/task-1-first.md",
        "backlog/archive/tasks/task-3-archived.md",
        "backlog/drafts/draft-1 - Linked Draft.md",
    ]
    .map(|path| std::fs::read_to_string(candidate.0.join(path)).unwrap());

    for args in [
        vec!["doc", "archive", "doc-001"],
        vec!["doc", "archive", "1"],
        vec!["doc", "restore", "DOC-001"],
        vec!["doc", "restore", "doc-1"],
        vec!["doc", "delete", "1"],
        vec!["doc", "archive", "404"],
        vec!["doc", "restore", "404"],
        vec!["doc", "delete", "404"],
        vec!["doc", "archive", "doc-1"],
        vec!["doc", "delete", "DOC-001"],
        vec!["doc", "delete", "1"],
    ] {
        let expected = typescript_cli(&args, &reference.0);
        let actual = rust_cli(&args, &candidate.0);
        assert_eq!(
            actual.status.code(),
            expected.status.code(),
            "args: {args:?}"
        );
        assert_eq!(actual.stdout, expected.stdout, "args: {args:?}");
        assert_eq!(actual.stderr, expected.stderr, "args: {args:?}");
    }

    assert!(!reference.0.join(LIFECYCLE_DOCUMENT_PATH).exists());
    assert!(!candidate.0.join(LIFECYCLE_DOCUMENT_PATH).exists());
    assert!(!reference.0.join(ARCHIVED_LIFECYCLE_DOCUMENT_PATH).exists());
    assert!(!candidate.0.join(ARCHIVED_LIFECYCLE_DOCUMENT_PATH).exists());
    for (index, path) in [
        "backlog/tasks/task-1-first.md",
        "backlog/archive/tasks/task-3-archived.md",
        "backlog/drafts/draft-1 - Linked Draft.md",
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            std::fs::read_to_string(candidate.0.join(path)).unwrap(),
            references_before[index]
        );
    }
    let recreated = typescript_cli(&["doc", "create", "Replacement"], &reference.0);
    let actual_recreated = rust_cli(&["doc", "create", "Replacement"], &candidate.0);
    assert_eq!(actual_recreated.status.code(), recreated.status.code());
    assert_eq!(actual_recreated.stdout, recreated.stdout);
    assert_eq!(actual_recreated.stderr, recreated.stderr);
    assert!(String::from_utf8_lossy(&actual_recreated.stdout).contains("Created document doc-1"));
}

#[test]
fn rust_document_lifecycle_matches_collisions_and_ambiguous_ids_without_writes() {
    let reference = document_lifecycle_process_fixture();
    let candidate = document_lifecycle_process_fixture();
    for fixture in [&reference, &candidate] {
        fixture.file(
            ARCHIVED_LIFECYCLE_DOCUMENT_PATH,
            "---\nid: doc-9\ntitle: Existing archive file\ntype: other\ncreated_date: 2026-01-01\n---\nKeep this file.\n",
        );
    }
    let collision_before =
        std::fs::read(reference.0.join(ARCHIVED_LIFECYCLE_DOCUMENT_PATH)).unwrap();
    let active_before = std::fs::read(candidate.0.join(LIFECYCLE_DOCUMENT_PATH)).unwrap();
    assert_cli_process_parity(&["doc", "archive", "doc-1"], &reference, &candidate);
    assert_eq!(
        std::fs::read(candidate.0.join(LIFECYCLE_DOCUMENT_PATH)).unwrap(),
        active_before
    );
    assert_eq!(
        std::fs::read(candidate.0.join(ARCHIVED_LIFECYCLE_DOCUMENT_PATH)).unwrap(),
        collision_before
    );

    for fixture in [&reference, &candidate] {
        std::fs::remove_file(fixture.0.join(ARCHIVED_LIFECYCLE_DOCUMENT_PATH)).unwrap();
        std::fs::rename(
            fixture.0.join(LIFECYCLE_DOCUMENT_PATH),
            fixture.0.join(ARCHIVED_LIFECYCLE_DOCUMENT_PATH),
        )
        .unwrap();
        fixture.file(
            LIFECYCLE_DOCUMENT_PATH,
            "---\nid: doc-9\ntitle: Existing active file\ntype: other\ncreated_date: 2026-01-01\n---\nKeep this file.\n",
        );
    }
    let active_before = std::fs::read(candidate.0.join(LIFECYCLE_DOCUMENT_PATH)).unwrap();
    let archived_before =
        std::fs::read(candidate.0.join(ARCHIVED_LIFECYCLE_DOCUMENT_PATH)).unwrap();
    assert_cli_process_parity(&["doc", "restore", "doc-1"], &reference, &candidate);
    assert_eq!(
        std::fs::read(candidate.0.join(LIFECYCLE_DOCUMENT_PATH)).unwrap(),
        active_before
    );
    assert_eq!(
        std::fs::read(candidate.0.join(ARCHIVED_LIFECYCLE_DOCUMENT_PATH)).unwrap(),
        archived_before
    );

    for fixture in [&reference, &candidate] {
        fixture.file(LIFECYCLE_DOCUMENT_PATH, LIFECYCLE_DOCUMENT_SOURCE);
    }
    let active_before = std::fs::read(candidate.0.join(LIFECYCLE_DOCUMENT_PATH)).unwrap();
    let archived_before =
        std::fs::read(candidate.0.join(ARCHIVED_LIFECYCLE_DOCUMENT_PATH)).unwrap();
    assert_cli_process_parity(&["doc", "archive", "1"], &reference, &candidate);
    assert_eq!(
        std::fs::read(candidate.0.join(LIFECYCLE_DOCUMENT_PATH)).unwrap(),
        active_before
    );
    assert_eq!(
        std::fs::read(candidate.0.join(ARCHIVED_LIFECYCLE_DOCUMENT_PATH)).unwrap(),
        archived_before
    );
}

#[cfg(unix)]
#[test]
fn rust_document_lifecycle_moves_and_deletes_symlinks_without_changing_targets() {
    use std::os::unix::fs::symlink;

    let reference = document_lifecycle_process_fixture();
    let candidate = document_lifecycle_process_fixture();
    for fixture in [&reference, &candidate] {
        let target = fixture.0.join("outside/lifecycle.md");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::rename(fixture.0.join(LIFECYCLE_DOCUMENT_PATH), &target).unwrap();
        symlink(
            "../../../outside/lifecycle.md",
            fixture.0.join(LIFECYCLE_DOCUMENT_PATH),
        )
        .unwrap();
    }
    for (args, old_path, new_path) in [
        (
            &["doc", "archive", "doc-1"][..],
            LIFECYCLE_DOCUMENT_PATH,
            ARCHIVED_LIFECYCLE_DOCUMENT_PATH,
        ),
        (
            &["doc", "restore", "1"][..],
            ARCHIVED_LIFECYCLE_DOCUMENT_PATH,
            LIFECYCLE_DOCUMENT_PATH,
        ),
        (
            &["doc", "archive", "1"][..],
            LIFECYCLE_DOCUMENT_PATH,
            ARCHIVED_LIFECYCLE_DOCUMENT_PATH,
        ),
    ] {
        assert_cli_process_parity(args, &reference, &candidate);
        for fixture in [&reference, &candidate] {
            assert!(!fixture.0.join(old_path).exists());
            assert!(
                std::fs::symlink_metadata(fixture.0.join(new_path))
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
            assert_eq!(
                std::fs::canonicalize(fixture.0.join(new_path)).unwrap(),
                std::fs::canonicalize(fixture.0.join("outside/lifecycle.md")).unwrap()
            );
        }
    }
    assert_cli_process_parity(&["doc", "delete", "doc-1"], &reference, &candidate);
    for fixture in [&reference, &candidate] {
        assert!(!fixture.0.join(ARCHIVED_LIFECYCLE_DOCUMENT_PATH).exists());
        assert_eq!(
            std::fs::read(fixture.0.join("outside/lifecycle.md")).unwrap(),
            LIFECYCLE_DOCUMENT_SOURCE.as_bytes()
        );
    }
}

#[test]
fn rust_document_lifecycle_auto_commit_matches_typescript_selected_paths() {
    let reference = document_lifecycle_process_fixture();
    let candidate = document_lifecycle_process_fixture();
    for fixture in [&reference, &candidate] {
        let configured = typescript_cli(&["config", "set", "autoCommit", "true"], &fixture.0);
        assert!(
            configured.status.success(),
            "{}",
            String::from_utf8_lossy(&configured.stderr)
        );
        initialize_git_fixture(fixture);
        fixture.file("STAGED.txt", "staged by the user\n");
        git(&fixture.0, &["add", "STAGED.txt"]);
        fixture.file("UNTRACKED.txt", "untracked user work\n");
    }

    for (args, message) in [
        (
            &["doc", "archive", "1"][..],
            "backlog: Archive document doc-1",
        ),
        (
            &["doc", "restore", "doc-1"][..],
            "backlog: Restore document doc-1",
        ),
        (
            &["doc", "archive", "doc-1"][..],
            "backlog: Archive document doc-1",
        ),
        (
            &["doc", "delete", "1"][..],
            "backlog: Delete document doc-1",
        ),
    ] {
        assert_cli_process_parity(args, &reference, &candidate);
        let expected_commit = selected_commit_result(&reference);
        let actual_commit = selected_commit_result(&candidate);
        assert_eq!(actual_commit, expected_commit, "args: {args:?}");
        assert_eq!(actual_commit.0, message, "args: {args:?}");
        assert_eq!(actual_commit.2, "STAGED.txt\nUNRELATED.txt");
        assert_eq!(
            git(&candidate.0, &["status", "--porcelain"]),
            git(&reference.0, &["status", "--porcelain"])
        );
    }
}

#[test]
fn rust_document_lifecycle_preserves_applied_change_when_commit_fails() {
    for (args, initial_archived, expected_path) in [
        (
            &["doc", "archive", "doc-1"][..],
            false,
            ARCHIVED_LIFECYCLE_DOCUMENT_PATH,
        ),
        (
            &["doc", "restore", "doc-1"][..],
            true,
            LIFECYCLE_DOCUMENT_PATH,
        ),
        (&["doc", "delete", "doc-1"][..], true, ""),
    ] {
        let reference = document_lifecycle_process_fixture();
        let candidate = document_lifecycle_process_fixture();
        for fixture in [&reference, &candidate] {
            let configured = typescript_cli(&["config", "set", "autoCommit", "true"], &fixture.0);
            assert!(
                configured.status.success(),
                "{}",
                String::from_utf8_lossy(&configured.stderr)
            );
            if initial_archived {
                std::fs::create_dir_all(
                    fixture
                        .0
                        .join(ARCHIVED_LIFECYCLE_DOCUMENT_PATH)
                        .parent()
                        .unwrap(),
                )
                .unwrap();
                std::fs::rename(
                    fixture.0.join(LIFECYCLE_DOCUMENT_PATH),
                    fixture.0.join(ARCHIVED_LIFECYCLE_DOCUMENT_PATH),
                )
                .unwrap();
            }
            initialize_git_fixture(fixture);
            std::fs::write(fixture.0.join(".git/index.lock"), "").unwrap();
        }
        let expected = typescript_cli(args, &reference.0);
        let actual = rust_cli(args, &candidate.0);
        assert_eq!(
            actual.status.code(),
            expected.status.code(),
            "args: {args:?}"
        );
        assert_eq!(actual.stdout, expected.stdout, "args: {args:?}");
        assert!(
            String::from_utf8_lossy(&expected.stderr).contains("committing the change failed"),
            "{}",
            String::from_utf8_lossy(&expected.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&actual.stderr)
                .replace(&candidate.0.to_string_lossy().to_string(), "<ROOT>"),
            String::from_utf8_lossy(&expected.stderr)
                .replace(&reference.0.to_string_lossy().to_string(), "<ROOT>"),
            "args: {args:?}"
        );
        let path = if expected_path.is_empty() {
            None
        } else {
            Some(expected_path)
        };
        for fixture in [&reference, &candidate] {
            match path {
                Some(path) => assert_eq!(
                    std::fs::read(fixture.0.join(path)).unwrap(),
                    LIFECYCLE_DOCUMENT_SOURCE.as_bytes(),
                    "args: {args:?}"
                ),
                None => assert!(!fixture.0.join(ARCHIVED_LIFECYCLE_DOCUMENT_PATH).exists()),
            }
        }
    }
}

#[test]
fn rust_document_update_matches_typescript_files_output_moves_and_refusals() {
    let success_cases: [(&[&str], &str); 3] = [
        (
            &[
                "doc",
                "update",
                "doc-1",
                "--title",
                "Install Runbook",
                "--content",
                "# Updated body\n\nRun install steps.",
                "--type",
                "specification",
                "--tags",
                "ops,runbook",
                "--path",
                "runbooks",
            ],
            "backlog/docs/runbooks/doc-1 - Install-Runbook.md",
        ),
        (
            &["doc", "update", "doc-1", "--title", "Setup Handbook"],
            "backlog/docs/guides/doc-1 - Setup-Handbook.md",
        ),
        (
            &["doc", "update", "doc-1", "--tags", "ops,ops"],
            "backlog/docs/guides/doc-1 - Setup-Guide.md",
        ),
    ];
    for (args, relative_path) in success_cases {
        let reference = document_update_fixture();
        let candidate = document_update_fixture();
        let expected = typescript_cli(args, &reference.0);
        let actual = rust_cli(args, &candidate.0);
        assert_eq!(
            actual.status.code(),
            expected.status.code(),
            "args: {args:?}; rust stderr: {}",
            String::from_utf8_lossy(&actual.stderr)
        );
        assert_eq!(actual.stdout, expected.stdout, "args: {args:?}");
        assert_eq!(actual.stderr, expected.stderr, "args: {args:?}");
        let expected_path = reference.0.join(relative_path);
        let actual_path = candidate.0.join(relative_path);
        assert_eq!(
            normalize_document_timestamps(&std::fs::read_to_string(actual_path).unwrap()),
            normalize_document_timestamps(&std::fs::read_to_string(expected_path).unwrap()),
            "args: {args:?}"
        );
        if relative_path != "backlog/docs/guides/doc-1 - Setup-Guide.md" {
            assert!(
                !candidate
                    .0
                    .join("backlog/docs/guides/doc-1 - Setup-Guide.md")
                    .exists(),
                "old source remained after moving/renaming: {args:?}"
            );
        }
    }

    let absolute_path = std::env::temp_dir().join(format!(
        "backlog-rust-doc-update-outside-{}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
    ));
    let absolute_path = absolute_path.to_string_lossy().into_owned();
    let refusal_cases = [
        vec!["doc", "update", "doc-1", "--title", "  "],
        vec!["doc", "update", "doc-1", "--type", "invalid"],
        vec!["doc", "update", "doc-1", "--path", "../outside"],
        vec!["doc", "update", "doc-1", "--path", &absolute_path],
        vec!["doc", "update", "doc-404", "--content", "Nope"],
        vec!["doc", "update", "doc-2", "--content", "Nope"],
    ];
    for args in refusal_cases {
        let reference = document_update_fixture();
        let candidate = document_update_fixture();
        let original = std::fs::read_to_string(
            candidate
                .0
                .join("backlog/docs/guides/doc-1 - Setup-Guide.md"),
        )
        .unwrap();
        let archived_original = std::fs::read_to_string(
            candidate
                .0
                .join("backlog/archive/docs/doc-2 - Archived-Guide.md"),
        )
        .unwrap();
        let expected = typescript_cli(&args, &reference.0);
        let actual = rust_cli(&args, &candidate.0);
        assert_eq!(
            actual.status.code(),
            expected.status.code(),
            "args: {args:?}"
        );
        assert_eq!(actual.stdout, expected.stdout, "args: {args:?}");
        assert_eq!(actual.stderr, expected.stderr, "args: {args:?}");
        assert_eq!(
            std::fs::read_to_string(
                candidate
                    .0
                    .join("backlog/docs/guides/doc-1 - Setup-Guide.md")
            )
            .unwrap(),
            original,
            "refused update changed source: {args:?}"
        );
        assert!(
            !candidate.0.join("backlog/outside").exists(),
            "refused update wrote outside docs: {args:?}"
        );
        assert_eq!(
            std::fs::read_to_string(
                candidate
                    .0
                    .join("backlog/archive/docs/doc-2 - Archived-Guide.md")
            )
            .unwrap(),
            archived_original,
            "refused update changed archived source: {args:?}"
        );
    }
    assert!(
        !std::path::Path::new(&absolute_path).exists(),
        "refused update wrote to the absolute path"
    );
}

#[test]
fn rust_document_update_retains_written_file_when_auto_commit_fails() {
    let reference = document_update_fixture();
    let candidate = document_update_fixture();
    for fixture in [&reference, &candidate] {
        fixture.file(
            "backlog.config.yml",
            "project_name: CLI Fixture\ntask_prefix: task\nstatuses:\n  - To Do\n  - Done\nauto_commit: true\n",
        );
        initialize_git_fixture(fixture);
        std::fs::write(fixture.0.join(".git/index.lock"), "").unwrap();
    }

    let args = ["doc", "update", "doc-1", "--content", "# Applied update\n"];
    let expected = typescript_cli(&args, &reference.0);
    let actual = rust_cli(&args, &candidate.0);
    assert_eq!(expected.status.code(), Some(1));
    assert_eq!(actual.status.code(), Some(1));
    assert!(expected.stdout.is_empty());
    assert!(actual.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&actual.stderr)
            .replace(&candidate.0.to_string_lossy().to_string(), "<ROOT>"),
        String::from_utf8_lossy(&expected.stderr)
            .replace(&reference.0.to_string_lossy().to_string(), "<ROOT>")
    );

    let relative_path = "backlog/docs/guides/doc-1 - Setup-Guide.md";
    let expected_path = reference.0.join(relative_path);
    let actual_path = candidate.0.join(relative_path);
    assert!(
        expected_path.exists(),
        "TypeScript removed the applied update"
    );
    assert!(
        actual_path.exists(),
        "Rust removed the applied update after commit failure: {}",
        String::from_utf8_lossy(&actual.stderr)
    );
    assert_eq!(
        normalize_document_timestamps(&std::fs::read_to_string(actual_path).unwrap()),
        normalize_document_timestamps(&std::fs::read_to_string(expected_path).unwrap())
    );
}

#[test]
fn rust_document_create_matches_typescript_content_output_and_refusals() {
    for (args, relative_path) in [
        (
            &["doc", "create", "  Quick Start  "][..],
            "backlog/docs/doc-1 - Quick-Start.md",
        ),
        (
            &[
                "doc",
                "create",
                "API Guidelines",
                "--type",
                "specification",
                "--path",
                "guides/api",
                "--plain",
            ][..],
            "backlog/docs/guides/api/doc-1 - API-Guidelines.md",
        ),
        (
            &["doc", "create", "EmptyType", "--type", ""][..],
            "backlog/docs/doc-1 - EmptyType.md",
        ),
    ] {
        let reference = fixture();
        let candidate = fixture();
        let expected = typescript_cli(args, &reference.0);
        let actual = rust_cli(args, &candidate.0);
        assert_eq!(
            actual.status.code(),
            expected.status.code(),
            "args: {args:?}"
        );
        assert_eq!(actual.stdout, expected.stdout, "args: {args:?}");
        assert_eq!(actual.stderr, expected.stderr, "args: {args:?}");

        let reference_path = reference.0.join(relative_path);
        let candidate_path = candidate.0.join(relative_path);
        let expected_markdown = std::fs::read_to_string(reference_path).unwrap();
        let actual_markdown = std::fs::read_to_string(candidate_path).unwrap();
        assert_eq!(
            normalize_document_timestamps(&actual_markdown),
            normalize_document_timestamps(&expected_markdown),
            "args: {args:?}"
        );
    }

    let absolute_path = std::env::temp_dir().join(format!(
        "backlog-rust-doc-create-outside-{}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
    ));
    assert!(!absolute_path.exists());
    let absolute_path = absolute_path.to_string_lossy().into_owned();
    let invalid_cases = [
        vec!["doc", "create", ""],
        vec!["doc", "create", "Unsafe", "--type", "invalid"],
        vec!["doc", "create", "Unsafe", "--path", &absolute_path],
        vec!["doc", "create", "Unsafe", "--path", "../outside"],
        vec!["doc", "create", "Unsafe", "--path", "..\\outside"],
    ];
    for args in invalid_cases {
        let reference = fixture();
        let candidate = fixture();
        let expected = typescript_cli(&args, &reference.0);
        let actual = rust_cli(&args, &candidate.0);
        assert_eq!(
            actual.status.code(),
            expected.status.code(),
            "args: {args:?}"
        );
        assert_eq!(actual.stdout, expected.stdout, "args: {args:?}");
        assert_eq!(actual.stderr, expected.stderr, "args: {args:?}");
        assert!(
            !candidate.0.join("backlog/docs").exists(),
            "invalid create wrote a docs directory: {args:?}"
        );
        assert!(
            !reference.0.join("backlog/docs").exists(),
            "invalid reference create wrote a docs directory: {args:?}"
        );
        assert!(
            !candidate.0.join("outside").exists(),
            "invalid create wrote outside docs: {args:?}"
        );
        assert!(
            !reference.0.join("outside").exists(),
            "invalid reference create wrote outside docs: {args:?}"
        );
    }
    assert!(!std::path::Path::new(&absolute_path).exists());
}

fn decision_fixture() -> Fixture {
    let fixture = Fixture::new();
    fixture.file(
        "backlog/decisions/decision-1 - Alpha.md",
        "---\nid: decision-1\ntitle: Alpha\ndate: 2026-01-01\nstatus: accepted\nsuperseded_by: decision-2\n---\n\n## Context\n\nAlpha context.\n\n## Decision\n\n<!-- SECTION:DECISION:BEGIN -->\nChoose alpha.\n<!-- SECTION:DECISION:END -->\n",
    );
    fixture.file(
        "backlog/decisions/decision-2 - Beta.md",
        "---\nid: decision-2\ntitle: Beta\ndate: 2026-01-02\nstatus: proposed\n---\n\nBeta content.\n",
    );
    fixture.file(
        "backlog/archive/decisions/decision-3 - Archived.md",
        "---\nid: decision-3\ntitle: Archived\ndate: 2026-01-03\nupdated_date: 2026-02-03\nstatus: rejected\n---\n\nArchived content.\n",
    );
    fixture
}

#[test]
fn rust_decision_list_matches_typescript_plain_json_windows_and_empty() {
    let reference = decision_fixture();
    let candidate = decision_fixture();
    for args in [
        vec!["decision", "list", "--plain"],
        vec!["decision", "list", "--json"],
        vec!["decision", "list", "--max-count", "1"],
        vec!["decision", "list", "--skip", "1", "--max-count", "1"],
        vec![
            "decision",
            "list",
            "--count",
            "--skip",
            "1",
            "--max-count",
            "1",
        ],
        vec![
            "decision",
            "list",
            "--json",
            "--skip",
            "1",
            "--max-count",
            "1",
        ],
        vec!["decision", "list", "--json", "--count"],
        vec!["decision", "list", "--plain", "--json"],
    ] {
        assert_cli_process_parity(&args, &reference, &candidate);
    }

    let empty_reference = Fixture::new();
    let empty_candidate = Fixture::new();
    assert_cli_process_parity(
        &["decision", "list", "--plain"],
        &empty_reference,
        &empty_candidate,
    );
    assert_cli_process_parity(
        &["decision", "list", "--json"],
        &empty_reference,
        &empty_candidate,
    );
}

#[test]
fn rust_decision_view_matches_typescript_active_archived_json_and_identity_errors() {
    let reference = decision_fixture();
    let candidate = decision_fixture();
    for args in [
        vec!["decision", "view", "1", "--plain"],
        vec!["decision", "view", "decision-001", "--json"],
        vec!["decision", "view", "3", "--plain"],
        vec!["decision", "view", "decision-003", "--json"],
        vec!["decision", "view", "99", "--plain"],
        vec!["decision", "view", "1", "--plain", "--json"],
    ] {
        assert_cli_process_parity(&args, &reference, &candidate);
    }

    let ambiguous_reference = decision_fixture();
    let ambiguous_candidate = decision_fixture();
    for fixture in [&ambiguous_reference, &ambiguous_candidate] {
        fixture.file(
            "backlog/archive/decisions/decision-1 - Duplicate.md",
            "---\nid: decision-1\ntitle: Duplicate\ndate: 2026-03-01\nstatus: proposed\n---\n",
        );
    }
    assert_cli_process_parity(
        &["decision", "view", "1", "--plain"],
        &ambiguous_reference,
        &ambiguous_candidate,
    );
}

#[test]
fn rust_decision_view_preserves_unmanaged_markers_and_crlf_content() {
    let reference = decision_fixture();
    let candidate = decision_fixture();
    for fixture in [&reference, &candidate] {
        let path = fixture.0.join("backlog/decisions/decision-1 - Alpha.md");
        let source = std::fs::read_to_string(&path).unwrap();
        let crlf = source.replace('\n', "\r\n");
        let crlf = format!(
            "{crlf}\r\n## Extra Notes\r\n\r\n<!-- SECTION:NOTES:BEGIN -->\r\nKeep this marker block.\r\n<!-- SECTION:NOTES:END -->\r\n\r\n<!-- SECTION:DESCRIPTION:BEGIN -->\r\nKeep this too.\r\n<!-- SECTION:DESCRIPTION:END -->"
        );
        std::fs::write(path, crlf).unwrap();
    }
    assert_cli_process_parity(
        &["decision", "view", "1", "--plain"],
        &reference,
        &candidate,
    );
}

#[test]
fn rust_decision_create_matches_typescript_sections_defaults_and_refusals() {
    let reference = Fixture::new();
    let candidate = Fixture::new();
    std::fs::create_dir_all(reference.0.join("backlog")).unwrap();
    std::fs::create_dir_all(candidate.0.join("backlog")).unwrap();
    let args = [
        "decision",
        "create",
        "Choice",
        "--status",
        "accepted",
        "--context",
        "First line\nSecond line",
        "--decision",
        "Use the shared store",
        "--plain",
    ];
    assert_cli_process_parity(&args, &reference, &candidate);
    let expected =
        std::fs::read_to_string(reference.0.join("backlog/decisions/decision-1 - Choice.md"))
            .unwrap();
    let actual =
        std::fs::read_to_string(candidate.0.join("backlog/decisions/decision-1 - Choice.md"))
            .unwrap();
    let without_date = |source: &str| {
        source
            .lines()
            .map(|line| {
                if line.starts_with("date:") {
                    "date: <created>"
                } else {
                    line
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(without_date(&actual), without_date(&expected));
    assert!(actual.contains("status: accepted"));
    assert!(actual.contains("First line\nSecond line"));
    assert!(actual.contains("## Consequences"));

    let default_reference = Fixture::new();
    let default_candidate = Fixture::new();
    std::fs::create_dir_all(default_reference.0.join("backlog")).unwrap();
    std::fs::create_dir_all(default_candidate.0.join("backlog")).unwrap();
    assert_cli_process_parity(
        &["decision", "create", "Default", "--context", "Only context"],
        &default_reference,
        &default_candidate,
    );
    let default_source = std::fs::read_to_string(
        default_candidate
            .0
            .join("backlog/decisions/decision-1 - Default.md"),
    )
    .unwrap();
    assert!(default_source.contains("status: proposed"));
    assert!(default_source.contains("Only context"));
    assert!(default_source.contains(
        "## Decision\n\n<!-- SECTION:DECISION:BEGIN -->\n<!-- SECTION:DECISION:END -->\n\n## Consequences"
    ));

    for args in [
        vec!["decision", "create", "   "],
        vec![
            "decision",
            "create",
            "Unsafe",
            "--context",
            "<!-- SECTION:CONTEXT:END -->",
        ],
    ] {
        let reference = Fixture::new();
        let candidate = Fixture::new();
        std::fs::create_dir_all(reference.0.join("backlog")).unwrap();
        std::fs::create_dir_all(candidate.0.join("backlog")).unwrap();
        assert_cli_process_parity(&args, &reference, &candidate);
        assert!(!candidate.0.join("backlog/decisions").exists());
    }
}

#[test]
fn rust_milestone_list_matches_typescript_aliases_sections_and_windows() {
    let reference = Fixture::new();
    let candidate = Fixture::new();
    for fixture in [&reference, &candidate] {
        fixture.file(
            "backlog/milestones/m-0 - Active Release.md",
            "---\nid: m-0\ntitle: Active Release\ndue_date: 2026-08-15\n---\n\n## Description\n\nActive milestone.\n",
        );
        fixture.file(
            "backlog/milestones/m-1 - In Progress.md",
            "---\nid: m-1\ntitle: In Progress\n---\n",
        );
        fixture.file(
            "backlog/milestones/m-2 - Completed.md",
            "---\nid: m-2\ntitle: Completed\n---\n",
        );
        fixture.file(
            "backlog/tasks/task-1-active.md",
            "---\nid: task-1\ntitle: Active task\nstatus: To Do\nmilestone: Active Release\n---\n",
        );
        fixture.file(
            "backlog/tasks/task-2-progress.md",
            "---\nid: task-2\ntitle: Progress task\nstatus: In Progress\nmilestone: m-1\n---\n",
        );
        fixture.file(
            "backlog/tasks/task-3-done.md",
            "---\nid: task-3\ntitle: Completed task\nstatus: Done\nmilestone: Completed\n---\n",
        );
    }

    for args in [
        vec!["milestone", "list", "--plain"],
        vec!["milestones", "list", "--show-completed", "--plain"],
        vec!["milestone", "list", "--max-count", "1"],
        vec![
            "milestone",
            "list",
            "--show-completed",
            "--max-count",
            "1",
            "--skip",
            "2",
        ],
        vec!["milestone", "list", "--count"],
        vec!["milestone", "list", "--show-completed", "--count"],
        vec!["milestone", "list", "--skip", "99", "--max-count", "1"],
        vec!["milestone", "list", "--max-count", "0"],
    ] {
        assert_cli_process_parity(&args, &reference, &candidate);
    }

    let empty_reference = Fixture::new();
    let empty_candidate = Fixture::new();
    for fixture in [&empty_reference, &empty_candidate] {
        std::fs::create_dir_all(fixture.0.join("backlog/milestones")).unwrap();
        std::fs::create_dir_all(fixture.0.join("backlog/tasks")).unwrap();
    }
    let (expected, _) = assert_cli_process_parity(
        &["milestone", "list", "--plain"],
        &empty_reference,
        &empty_candidate,
    );
    assert_eq!(
        String::from_utf8(expected.stdout).unwrap(),
        "Active milestones (0):\n  (none)\n\nCompleted milestones (0):\n  (none)\n"
    );
}

#[test]
fn rust_milestone_add_matches_typescript_trim_validation_and_files() {
    let reference = Fixture::new();
    let candidate = Fixture::new();
    for fixture in [&reference, &candidate] {
        std::fs::create_dir_all(fixture.0.join("backlog")).unwrap();
    }
    let args = [
        "milestone",
        "add",
        "  Ship V1  ",
        "--description",
        "Release scope\n\n### Checks\n\nShip it.",
        "--due-date",
        "2026-08-15",
    ];
    assert_cli_process_parity(&args, &reference, &candidate);
    let expected =
        std::fs::read_to_string(reference.0.join("backlog/milestones/m-0 - ship-v1.md")).unwrap();
    let actual =
        std::fs::read_to_string(candidate.0.join("backlog/milestones/m-0 - ship-v1.md")).unwrap();
    assert_eq!(actual, expected);
    assert!(actual.contains("due_date: \"2026-08-15\""));
    assert!(actual.contains("Release scope\n\n### Checks\n\nShip it."));

    let duplicate_reference = Fixture::new();
    let duplicate_candidate = Fixture::new();
    for fixture in [&duplicate_reference, &duplicate_candidate] {
        fixture.file(
            "backlog/milestones/m-4 - Existing.md",
            "---\nid: m-4\ntitle: Existing\n---\n",
        );
    }
    for args in [
        vec!["milestone", "add", "   "],
        vec!["milestone", "add", " existing "],
        vec![
            "milestone",
            "add",
            "Invalid date",
            "--due-date",
            "2026-02-30",
        ],
    ] {
        assert_cli_process_parity(&args, &duplicate_reference, &duplicate_candidate);
        let entries = std::fs::read_dir(duplicate_candidate.0.join("backlog/milestones"))
            .unwrap()
            .count();
        assert_eq!(entries, 1, "invalid create wrote a milestone: {args:?}");
    }

    let archived_reference = Fixture::new();
    let archived_candidate = Fixture::new();
    for fixture in [&archived_reference, &archived_candidate] {
        fixture.file(
            "backlog/archive/milestones/m-0 - Existing.md",
            "---\nid: m-0\ntitle: Existing\n---\n",
        );
    }
    assert_cli_process_parity(
        &["milestone", "add", "Existing"],
        &archived_reference,
        &archived_candidate,
    );
    assert!(
        archived_candidate
            .0
            .join("backlog/milestones/m-1 - existing.md")
            .exists()
    );

    let default_reference = Fixture::new();
    let default_candidate = Fixture::new();
    for fixture in [&default_reference, &default_candidate] {
        std::fs::create_dir_all(fixture.0.join("backlog")).unwrap();
    }
    assert_cli_process_parity(
        &["milestone", "add", "Default"],
        &default_reference,
        &default_candidate,
    );
    let default_record = std::fs::read_to_string(
        default_candidate
            .0
            .join("backlog/milestones/m-0 - default.md"),
    )
    .unwrap();
    assert!(default_record.contains("Milestone: Default"));
}

#[test]
fn rust_milestone_add_auto_commits_only_the_created_file() {
    let reference = Fixture::new();
    let candidate = Fixture::new();
    for fixture in [&reference, &candidate] {
        let config_path = fixture.0.join("backlog.config.yml");
        let config = std::fs::read_to_string(&config_path).unwrap();
        std::fs::write(
            &config_path,
            config.replace("statuses:\n", "auto_commit: true\nstatuses:\n"),
        )
        .unwrap();
        std::fs::create_dir_all(fixture.0.join("backlog/milestones")).unwrap();
        std::fs::create_dir_all(fixture.0.join("backlog/tasks")).unwrap();
        initialize_git_fixture(fixture);
    }
    assert_cli_process_parity(
        &["milestone", "add", "Committed Release"],
        &reference,
        &candidate,
    );
    assert_eq!(
        selected_commit_result(&candidate),
        (
            "backlog: Add milestone m-0".into(),
            "backlog/milestones/m-0 - committed-release.md".into(),
            "UNRELATED.txt".into(),
        )
    );
}

#[test]
fn rust_milestone_group_invocation_matches_typescript_help_channel_and_status() {
    for alias in ["milestone", "milestones"] {
        let fixture = Fixture::new();
        let expected = typescript_cli(&[alias], &fixture.0);
        let actual = rust_cli(&[alias], &fixture.0);
        assert_eq!(actual.status.code(), expected.status.code(), "{alias}");
        assert!(
            expected.stdout.is_empty(),
            "{alias}: TS unexpectedly printed stdout"
        );
        assert!(
            actual.stdout.is_empty(),
            "{alias}: Rust printed help to stdout"
        );
        let expected_help = String::from_utf8_lossy(&expected.stderr);
        let actual_help = String::from_utf8_lossy(&actual.stderr);
        assert!(!expected_help.is_empty(), "{alias}: TS did not print help");
        for command in ["list", "add", "rename", "archive", "remove"] {
            assert!(
                expected_help.contains(command),
                "{alias}: TS help missing {command}"
            );
            assert!(
                actual_help.contains(command),
                "{alias}: Rust help missing {command}"
            );
        }
    }
}

#[test]
fn rust_milestone_rename_auto_commits_only_the_moved_record_and_matching_tasks() {
    let reference = Fixture::new();
    let candidate = Fixture::new();
    for fixture in [&reference, &candidate] {
        fixture.file(
            "backlog.config.yml",
            "project_name: CLI Fixture\ntask_prefix: task\nstatuses:\n  - To Do\n  - Done\nauto_commit: true\n",
        );
        fixture.file(
            "backlog/milestones/m-0 - release-a.md",
            "---\nid: m-0\ntitle: Release A\n---\n",
        );
        fixture.file(
            "backlog/tasks/task-1.md",
            "---\nid: task-1\ntitle: Task A\nstatus: To Do\nmilestone: Release A\n---\n",
        );
        initialize_git_fixture(fixture);
    }
    let args = ["milestone", "rename", "Release A", "Release Prime"];
    let expected = typescript_cli(&args, &reference.0);
    let actual = rust_cli(&args, &candidate.0);
    assert_eq!(actual.status.code(), expected.status.code());
    let expected_stdout = String::from_utf8(expected.stdout)
        .unwrap()
        .replace(&reference.0.display().to_string(), "FIXTURE_ROOT");
    let actual_stdout = String::from_utf8(actual.stdout)
        .unwrap()
        .replace(&candidate.0.display().to_string(), "FIXTURE_ROOT");
    assert_eq!(actual_stdout, expected_stdout);
    assert_eq!(actual.stderr, expected.stderr);
    assert_eq!(
        selected_commit_result(&candidate),
        selected_commit_result(&reference)
    );
    let (message, committed_files, staged_files) = selected_commit_result(&candidate);
    assert_eq!(message, "backlog: Rename milestone m-0");
    assert!(committed_files.contains("backlog/milestones/"));
    assert!(committed_files.contains("backlog/tasks/task-1.md"));
    assert_eq!(staged_files, "UNRELATED.txt");
}

#[cfg(unix)]
#[test]
fn rust_milestone_rename_reports_original_commit_error_and_file_rollback_failure() {
    let setup = |fixture: &Fixture| {
        fixture.file(
            "backlog.config.yml",
            "project_name: CLI Fixture\ntask_prefix: task\nstatuses:\n  - To Do\n  - Done\nauto_commit: true\n",
        );
        let relative_path = "backlog/milestones/m-0 - release-a.md";
        fixture.file(
            relative_path,
            "---\nid: m-0\ntitle: Release A\n---\n\nDescription.\n",
        );
        initialize_git_fixture(fixture);

        let old_path = fixture.0.join(relative_path).to_string_lossy().to_string();
        let quoted_path = format!("'{}'", old_path.replace('\'', "'\\''"));
        let hook = format!(
            "#!/bin/sh\nrm -f {quoted_path}\nmkdir {quoted_path}\nprintf 'forced commit hook failure\\n' >&2\nexit 1\n"
        );
        let hook_path = fixture.0.join(".git/hooks/pre-commit");
        std::fs::write(&hook_path, hook).unwrap();
        let mut permissions = std::fs::metadata(&hook_path).unwrap().permissions();
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(0o755);
        std::fs::set_permissions(&hook_path, permissions).unwrap();
    };

    let reference = Fixture::new();
    let candidate = Fixture::new();
    setup(&reference);
    setup(&candidate);
    let args = ["milestone", "rename", "Release A", "Release Prime"];
    let expected = typescript_cli(&args, &reference.0);
    let actual = rust_cli(&args, &candidate.0);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stdout, expected.stdout);
    assert!(
        String::from_utf8_lossy(&expected.stderr)
            .contains("failed to rollback milestone file rename")
    );
    let actual_error = String::from_utf8_lossy(&actual.stderr);
    assert!(actual_error.contains("forced commit hook failure"));
    assert!(actual_error.contains("failed to rollback milestone file rename"));
    let expected_renamed_path = reference
        .0
        .join("backlog/milestones/m-0 - release-prime.md");
    assert!(expected_renamed_path.exists());
    let expected_renamed_bytes = std::fs::read(expected_renamed_path).unwrap();
    let actual_renamed_path = candidate
        .0
        .join("backlog/milestones/m-0 - release-prime.md");
    assert!(actual_renamed_path.exists());
    let actual_renamed_bytes = std::fs::read(actual_renamed_path).unwrap();
    assert_eq!(actual_renamed_bytes, expected_renamed_bytes);
}

#[cfg(unix)]
#[test]
fn rust_milestone_rename_restores_original_file_after_commit_failure() {
    let source = "---\nid: m-0\ntitle: Release A\nowner: platform\n---\n\n## Description\n\nOriginal milestone body.\n";
    let setup = |fixture: &Fixture| {
        fixture.file(
            "backlog.config.yml",
            "project_name: CLI Fixture\ntask_prefix: task\nstatuses:\n  - To Do\n  - Done\nauto_commit: true\n",
        );
        fixture.file("backlog/milestones/m-0 - release-a.md", source);
        initialize_git_fixture(fixture);
        let hook_path = fixture.0.join(".git/hooks/pre-commit");
        std::fs::write(
            &hook_path,
            "#!/bin/sh\nprintf 'forced commit hook failure\\n' >&2\nexit 1\n",
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&hook_path).unwrap().permissions();
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(0o755);
        std::fs::set_permissions(&hook_path, permissions).unwrap();
    };
    let reference = Fixture::new();
    let candidate = Fixture::new();
    setup(&reference);
    setup(&candidate);
    let args = ["milestone", "rename", "Release A", "Release Prime"];
    let expected = typescript_cli(&args, &reference.0);
    let actual = rust_cli(&args, &candidate.0);
    assert_eq!(actual.status.code(), expected.status.code());
    let original_path = candidate.0.join("backlog/milestones/m-0 - release-a.md");
    let renamed_path = candidate
        .0
        .join("backlog/milestones/m-0 - release-prime.md");
    assert!(original_path.exists());
    assert!(!renamed_path.exists());
    let restored = std::fs::read(original_path).unwrap();
    assert_eq!(restored, source.as_bytes());
}

#[test]
fn rust_milestone_rename_matches_typescript_task_updates_and_due_dates() {
    let source = "---\nid: m-0\ntitle: Release A\ndue_date: \"2026-08-15\"\nowner: platform # preserve\n---\n\n## Description\n\nMilestone: Release A\n\nKeep this custom description.\n";
    let setup = |fixture: &Fixture| {
        fixture.file("backlog/milestones/m-0 - release-a.md", source);
        fixture.file(
            "backlog/tasks/task-1-title.md",
            "---\nid: task-1\ntitle: Title reference\nstatus: To Do\nmilestone: Release A\nowner: platform\n---\n\nTask body stays.\n",
        );
        fixture.file(
            "backlog/tasks/task-2-id.md",
            "---\nid: task-2\ntitle: ID reference\nstatus: In Progress\nmilestone: m-0\n---\n",
        );
    };
    let reference = Fixture::new();
    let candidate = Fixture::new();
    setup(&reference);
    setup(&candidate);

    let assert_rename_parity = |args: &[&str], reference: &Fixture, candidate: &Fixture| {
        let expected = typescript_cli(args, &reference.0);
        let actual = rust_cli(args, &candidate.0);
        assert_eq!(
            actual.status.code(),
            expected.status.code(),
            "args: {args:?}"
        );
        let expected_stdout = String::from_utf8(expected.stdout)
            .unwrap()
            .replace(&reference.0.display().to_string(), "FIXTURE_ROOT");
        let actual_stdout = String::from_utf8(actual.stdout)
            .unwrap()
            .replace(&candidate.0.display().to_string(), "FIXTURE_ROOT");
        assert_eq!(actual_stdout, expected_stdout, "args: {args:?}");
        assert_eq!(actual.stderr, expected.stderr, "args: {args:?}");
    };
    assert_rename_parity(
        &["milestone", "rename", "Release A", "Release Prime"],
        &reference,
        &candidate,
    );
    let renamed = std::fs::read_to_string(
        candidate
            .0
            .join("backlog/milestones/m-0 - release-prime.md"),
    )
    .unwrap();
    assert!(renamed.contains("due_date: \"2026-08-15\""));
    assert!(renamed.contains("Milestone: Release A"));
    assert!(renamed.contains("Keep this custom description."));
    for path in ["task-1-title.md", "task-2-id.md"] {
        let task = std::fs::read_to_string(candidate.0.join("backlog/tasks").join(path)).unwrap();
        assert!(task.contains("milestone: m-0"));
    }
    assert!(
        std::fs::read_to_string(candidate.0.join("backlog/tasks/task-1-title.md"))
            .unwrap()
            .contains("Task body stays.")
    );

    let no_update_reference = Fixture::new();
    let no_update_candidate = Fixture::new();
    setup(&no_update_reference);
    setup(&no_update_candidate);
    assert_rename_parity(
        &[
            "milestone",
            "rename",
            "m-0",
            "Release Prime",
            "--no-update-tasks",
        ],
        &no_update_reference,
        &no_update_candidate,
    );
    assert!(
        std::fs::read_to_string(no_update_candidate.0.join("backlog/tasks/task-1-title.md"))
            .unwrap()
            .contains("milestone: Release A")
    );

    for (args, expected_due) in [
        (
            vec![
                "milestone",
                "rename",
                "m-0",
                "Release Prime",
                "--due-date",
                "2026-09-01",
            ],
            Some("due_date: \"2026-09-01\""),
        ),
        (
            vec![
                "milestone",
                "rename",
                "m-0",
                "Release Prime",
                "--clear-due-date",
            ],
            None,
        ),
    ] {
        let reference = Fixture::new();
        let candidate = Fixture::new();
        for fixture in [&reference, &candidate] {
            fixture.file(
                "backlog/milestones/m-0 - release-prime.md",
                &source.replace("Release A", "Release Prime"),
            );
        }
        assert_cli_process_parity(&args, &reference, &candidate);
        let contents = std::fs::read_to_string(
            candidate
                .0
                .join("backlog/milestones/m-0 - release-prime.md"),
        )
        .unwrap();
        assert_eq!(contents.contains("due_date:"), expected_due.is_some());
        if let Some(expected_due) = expected_due {
            assert!(contents.contains(expected_due));
        }
    }

    let reference = Fixture::new();
    let candidate = Fixture::new();
    for fixture in [&reference, &candidate] {
        fixture.file("backlog/milestones/m-0 - release-a.md", source);
    }
    assert_cli_process_parity(
        &[
            "milestone",
            "rename",
            "Release A",
            "Release Prime",
            "--due-date",
            "2026-09-01",
            "--clear-due-date",
        ],
        &reference,
        &candidate,
    );
    assert!(
        candidate
            .0
            .join("backlog/milestones/m-0 - release-a.md")
            .exists()
    );
}

#[test]
fn rust_milestone_archive_matches_typescript_moves_and_identity_errors() {
    let source = "---\nid: m-0\ntitle: Release A\nowner: platform\n---\n\nCustom milestone text.\n";
    let reference = Fixture::new();
    let candidate = Fixture::new();
    for fixture in [&reference, &candidate] {
        fixture.file("backlog/milestones/m-0 - release-a.md", source);
    }
    assert_cli_process_parity(
        &["milestone", "archive", "Release A"],
        &reference,
        &candidate,
    );
    assert!(
        !candidate
            .0
            .join("backlog/milestones/m-0 - release-a.md")
            .exists()
    );
    assert_eq!(
        std::fs::read_to_string(
            candidate
                .0
                .join("backlog/archive/milestones/m-0 - release-a.md"),
        )
        .unwrap(),
        source
    );
    assert_cli_process_parity(
        &["milestone", "archive", "Release A"],
        &reference,
        &candidate,
    );

    let missing_reference = Fixture::new();
    let missing_candidate = Fixture::new();
    assert_cli_process_parity(
        &["milestone", "archive", "missing"],
        &missing_reference,
        &missing_candidate,
    );

    let ambiguous_reference = Fixture::new();
    let ambiguous_candidate = Fixture::new();
    for fixture in [&ambiguous_reference, &ambiguous_candidate] {
        fixture.file(
            "backlog/milestones/m-0 - shared.md",
            "---\nid: m-0\ntitle: Shared\n---\n",
        );
        fixture.file(
            "backlog/milestones/m-1 - shared-other.md",
            "---\nid: m-1\ntitle: Shared\n---\n",
        );
    }
    assert_cli_process_parity(
        &["milestone", "archive", "Shared"],
        &ambiguous_reference,
        &ambiguous_candidate,
    );
    assert!(
        ambiguous_candidate
            .0
            .join("backlog/milestones/m-0 - shared.md")
            .exists()
    );
}

#[test]
fn rust_milestone_remove_matches_typescript_task_handling_and_refusals() {
    let setup = |fixture: &Fixture| {
        fixture.file(
            "backlog/milestones/m-0 - release-a.md",
            "---\nid: m-0\ntitle: Release A\nowner: platform\n---\n",
        );
        fixture.file(
            "backlog/milestones/m-1 - release-b.md",
            "---\nid: m-1\ntitle: Release B\n---\n",
        );
        fixture.file(
            "backlog/tasks/task-1.md",
            "---\nid: task-1\ntitle: Task A\nstatus: To Do\nmilestone: Release A\nowner: keep\n---\n\nTask body.\n",
        );
        fixture.file(
            "backlog/tasks/task-2.md",
            "---\nid: task-2\ntitle: Task B\nstatus: Done\nmilestone: m-0\n---\n",
        );
    };
    for (args, handling) in [
        (vec!["milestone", "remove", "Release A"], "clear"),
        (
            vec![
                "milestone",
                "remove",
                "Release A",
                "--task-handling",
                "keep",
            ],
            "keep",
        ),
        (
            vec![
                "milestone",
                "remove",
                "Release A",
                "--task-handling",
                "reassign",
                "--reassign-to",
                "Release B",
            ],
            "reassign",
        ),
    ] {
        let reference = Fixture::new();
        let candidate = Fixture::new();
        setup(&reference);
        setup(&candidate);
        assert_cli_process_parity(&args, &reference, &candidate);
        for path in ["task-1.md", "task-2.md"] {
            let task =
                std::fs::read_to_string(candidate.0.join("backlog/tasks").join(path)).unwrap();
            assert!(task.contains("owner: keep") || path == "task-2.md");
            assert!(task.contains("Task body.") || path == "task-2.md");
            match handling {
                "clear" => assert!(!task.contains("milestone:")),
                "keep" => {
                    let expected = if path == "task-1.md" {
                        "Release A"
                    } else {
                        "m-0"
                    };
                    assert!(task.contains(&format!("milestone: {expected}")));
                }
                "reassign" => assert!(task.contains("milestone: m-1")),
                _ => unreachable!(),
            }
        }
        assert!(
            candidate
                .0
                .join("backlog/archive/milestones/m-0 - release-a.md")
                .exists()
        );
    }

    let invalid_reference = Fixture::new();
    let invalid_candidate = Fixture::new();
    for fixture in [&invalid_reference, &invalid_candidate] {
        fixture.file(
            "backlog/milestones/m-0 - release-a.md",
            "---\nid: m-0\ntitle: Release A\n---\n",
        );
        fixture.file(
            "backlog/archive/milestones/m-1 - old.md",
            "---\nid: m-1\ntitle: Old\n---\n",
        );
    }
    for args in [
        vec![
            "milestone",
            "remove",
            "Release A",
            "--task-handling",
            "invalid",
        ],
        vec![
            "milestone",
            "remove",
            "Release A",
            "--task-handling",
            "reassign",
        ],
        vec![
            "milestone",
            "remove",
            "Release A",
            "--task-handling",
            "reassign",
            "--reassign-to",
            "m-1",
        ],
        vec![
            "milestone",
            "remove",
            "Release A",
            "--task-handling",
            "reassign",
            "--reassign-to",
            "m-0",
        ],
    ] {
        assert_cli_process_parity(&args, &invalid_reference, &invalid_candidate);
        assert!(
            invalid_candidate
                .0
                .join("backlog/milestones/m-0 - release-a.md")
                .exists()
        );
    }
}

#[test]
fn rust_decision_edit_matches_typescript_source_preservation_and_refusals() {
    let source = "---\nid: decision-1\ntitle: Alpha\ndate: 2026-01-01\nstatus: accepted\nsuperseded_by: decision-2\nowner: platform # keep\n---\n\n## Context\n\nOld context.\n\n## Decision\n\nOld choice.\n\n## Consequences\n\nKeep consequences.\n\n## Alternatives\n\nKeep alternatives.\n\n## Revisit trigger\n\nReview next release.\n";
    let setup = |fixture: &Fixture| {
        fixture.file("backlog/decisions/decision-1 - Alpha.md", source);
        fixture.file(
            "backlog/decisions/decision-2 - Beta.md",
            "---\nid: decision-2\ntitle: Beta\ndate: 2026-01-02\nstatus: proposed\n---\n",
        );
        fixture.file(
            "backlog/archive/decisions/decision-3 - Archived.md",
            "---\nid: decision-3\ntitle: Archived\ndate: 2026-01-03\nstatus: rejected\n---\n",
        );
    };
    let reference = Fixture::new();
    let candidate = Fixture::new();
    setup(&reference);
    setup(&candidate);

    for args in [
        vec![
            "decision",
            "edit",
            "1",
            "--context",
            "New context\nwith a second line",
            "--alternatives",
            "",
        ],
        vec![
            "decision",
            "edit",
            "1",
            "--title",
            "Revised",
            "--status",
            "Under Review",
        ],
    ] {
        assert_cli_process_parity(&args, &reference, &candidate);
    }

    let current_path = candidate
        .0
        .join("backlog/decisions/decision-1 - Revised.md");
    assert_cli_process_parity(
        &["decision", "edit", "1", "--superseded-by", "decision-2"],
        &reference,
        &candidate,
    );
    let after_link = std::fs::read_to_string(&current_path).unwrap();
    assert!(after_link.contains("status: Under Review"));
    assert_cli_process_parity(
        &["decision", "edit", "1", "--superseded-by", ""],
        &reference,
        &candidate,
    );
    let before_noop = std::fs::read_to_string(&current_path).unwrap();
    assert_cli_process_parity(
        &["decision", "edit", "1", "--status", "Under Review"],
        &reference,
        &candidate,
    );
    let after_noop = std::fs::read_to_string(&current_path).unwrap();
    assert_eq!(after_noop, before_noop);

    assert_cli_process_parity(
        &["decision", "edit", "1", "--status", "accepted"],
        &reference,
        &candidate,
    );
    let before_identity_errors = std::fs::read_to_string(&current_path).unwrap();
    for args in [
        vec!["decision", "edit", "1", "--superseded-by", "1"],
        vec!["decision", "edit", "1", "--superseded-by", "999"],
        vec!["decision", "edit", "1"],
        vec![
            "decision",
            "edit",
            "1",
            "--context",
            "<!-- SECTION:CONTEXT:END -->",
        ],
    ] {
        assert_cli_process_parity(&args, &reference, &candidate);
    }

    for args in [
        vec!["decision", "edit", "999", "--context", "changed"],
        vec!["decision", "edit", "3", "--context", "changed"],
    ] {
        assert_cli_process_parity(&args, &reference, &candidate);
    }
    assert_eq!(
        std::fs::read_to_string(
            candidate
                .0
                .join("backlog/decisions/decision-1 - Revised.md"),
        )
        .unwrap(),
        before_identity_errors
    );

    let ambiguous_reference = Fixture::new();
    let ambiguous_candidate = Fixture::new();
    setup(&ambiguous_reference);
    setup(&ambiguous_candidate);
    for fixture in [&ambiguous_reference, &ambiguous_candidate] {
        fixture.file("backlog/decisions/decision-001 - Duplicate.md", source);
    }
    assert_cli_process_parity(
        &["decision", "edit", "1", "--context", "changed"],
        &ambiguous_reference,
        &ambiguous_candidate,
    );

    let current = std::fs::read_to_string(
        candidate
            .0
            .join("backlog/decisions/decision-1 - Revised.md"),
    )
    .unwrap();
    assert!(current.contains("owner: platform # keep"));
    assert!(current.contains("## Revisit trigger\n\nReview next release."));
    assert!(current.contains("New context\nwith a second line"));
    assert!(current.contains("status: accepted"));
    assert!(!current.contains("superseded_by:"));
    let expected = std::fs::read_to_string(
        reference
            .0
            .join("backlog/decisions/decision-1 - Revised.md"),
    )
    .unwrap();
    let current_frontmatter = backlog_md::frontmatter::parse_frontmatter(&current).unwrap();
    let expected_frontmatter = backlog_md::frontmatter::parse_frontmatter(&expected).unwrap();
    assert_eq!(current_frontmatter.content, expected_frontmatter.content);
    for key in ["id", "title", "status", "owner"] {
        assert_eq!(
            current_frontmatter.data[key],
            expected_frontmatter.data[key]
        );
    }
}

#[test]
fn rust_decision_lifecycle_matches_typescript_changes_backlinks_and_id_release() {
    const ACTIVE: &str = "backlog/decisions/decision-1 - Field-Guide.md";
    const ARCHIVED: &str = "backlog/archive/decisions/decision-1 - Field-Guide.md";
    const SOURCE: &str = "---\nid: decision-1\ntitle: Field Guide\ndate: 2026-01-01\ncustom: retained\n---\nOriginal decision body\n";
    const TASK: &str = "backlog/tasks/task-1-first.md";
    const DRAFT: &str = "backlog/drafts/draft-1 - Linked-Draft.md";
    let reference = Fixture::new();
    let candidate = Fixture::new();
    for fixture in [&reference, &candidate] {
        fixture.file(ACTIVE, SOURCE);
        fixture.file(
            TASK,
            "---\nid: task-1\ntitle: First task\nstatus: To Do\npriority: high\ncreated_date: 2026-01-01\ndocumentation:\n  - backlog/decisions/decision-1 - Field-Guide.md\n  - backlog/archive/decisions/decision-1 - Field-Guide.md\n---\n",
        );
        fixture.file(
            DRAFT,
            "---\nid: DRAFT-1\ntitle: Linked Draft\nstatus: Draft\ncreated_date: 2026-01-04\ndocumentation:\n  - ./backlog/decisions/decision-1 - Field-Guide.md#heading\n---\n",
        );
    }
    let task_before = std::fs::read(candidate.0.join(TASK)).unwrap();
    let draft_before = std::fs::read(candidate.0.join(DRAFT)).unwrap();

    for (args, old_path, new_path) in [
        (&["decision", "archive", "1"][..], ACTIVE, ARCHIVED),
        (
            &["decision", "restore", "decision-001"][..],
            ARCHIVED,
            ACTIVE,
        ),
        (&["decision", "archive", "decision-1"][..], ACTIVE, ARCHIVED),
        (&["decision", "delete", "1"][..], ARCHIVED, ""),
    ] {
        let (expected, actual) = assert_cli_process_parity(args, &reference, &candidate);
        assert_eq!(actual.status.code(), expected.status.code());
        assert!(!candidate.0.join(old_path).exists(), "args: {args:?}");
        if new_path.is_empty() {
            assert!(!candidate.0.join(ARCHIVED).exists());
        } else {
            assert_eq!(
                std::fs::read(candidate.0.join(new_path)).unwrap(),
                SOURCE.as_bytes()
            );
        }
    }
    assert_eq!(std::fs::read(candidate.0.join(TASK)).unwrap(), task_before);
    assert_eq!(
        std::fs::read(candidate.0.join(DRAFT)).unwrap(),
        draft_before
    );
    assert_cli_process_parity(
        &["decision", "create", "Replacement after deletion"],
        &reference,
        &candidate,
    );
    assert!(
        candidate
            .0
            .join("backlog/decisions/decision-1 - Replacement-after-deletion.md")
            .exists()
    );
}

#[test]
fn rust_decision_lifecycle_refuses_wrong_state_missing_ambiguous_and_colliding_paths() {
    const ACTIVE: &str = "backlog/decisions/decision-1 - Field-Guide.md";
    const ARCHIVED: &str = "backlog/archive/decisions/decision-1 - Field-Guide.md";
    const SOURCE: &str =
        "---\nid: decision-1\ntitle: Field Guide\ndate: 2026-01-01\n---\nOriginal body\n";
    let reference = Fixture::new();
    let candidate = Fixture::new();
    for fixture in [&reference, &candidate] {
        fixture.file(ACTIVE, SOURCE);
    }
    for args in [
        &["decision", "delete", "1"][..],
        &["decision", "restore", "1"][..],
        &["decision", "archive", "404"][..],
        &["decision", "restore", "404"][..],
        &["decision", "delete", "404"][..],
    ] {
        assert_cli_process_parity(args, &reference, &candidate);
    }

    for fixture in [&reference, &candidate] {
        fixture.file(ARCHIVED, SOURCE);
    }
    for args in [
        &["decision", "archive", "1"][..],
        &["decision", "restore", "1"][..],
        &["decision", "delete", "1"][..],
    ] {
        assert_cli_process_parity(args, &reference, &candidate);
    }

    let collision = SOURCE.replace("decision-1", "decision-9");
    for fixture in [&reference, &candidate] {
        std::fs::remove_file(fixture.0.join(ARCHIVED)).unwrap();
        fixture.file(ARCHIVED, &collision);
    }
    assert_cli_process_parity(&["decision", "archive", "1"], &reference, &candidate);

    for fixture in [&reference, &candidate] {
        std::fs::remove_file(fixture.0.join(ARCHIVED)).unwrap();
    }
    assert_cli_process_parity(&["decision", "archive", "1"], &reference, &candidate);
    for fixture in [&reference, &candidate] {
        fixture.file(ACTIVE, &collision);
    }
    assert_cli_process_parity(&["decision", "restore", "1"], &reference, &candidate);

    let ambiguous_reference = Fixture::new();
    let ambiguous_candidate = Fixture::new();
    for fixture in [&ambiguous_reference, &ambiguous_candidate] {
        fixture.file(ACTIVE, SOURCE);
        fixture.file(
            "backlog/archive/decisions/decision-001 - Duplicate.md",
            SOURCE,
        );
    }
    for args in [
        &["decision", "archive", "1"][..],
        &["decision", "restore", "1"][..],
        &["decision", "delete", "1"][..],
    ] {
        assert_cli_process_parity(args, &ambiguous_reference, &ambiguous_candidate);
    }
}

#[cfg(unix)]
#[test]
fn rust_decision_lifecycle_moves_and_deletes_symlinks_without_changing_targets() {
    use std::os::unix::fs::symlink;

    const ACTIVE: &str = "backlog/decisions/decision-1 - Linked.md";
    const ARCHIVED: &str = "backlog/archive/decisions/decision-1 - Linked.md";
    const TARGET: &str = "outside/decision.md";
    const SOURCE: &str =
        "---\nid: decision-1\ntitle: Linked\ndate: 2026-01-01\n---\nSymlink target\n";
    let reference = Fixture::new();
    let candidate = Fixture::new();
    for fixture in [&reference, &candidate] {
        fixture.file(TARGET, SOURCE);
        let link = fixture.0.join(ACTIVE);
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        symlink("../../outside/decision.md", &link).unwrap();
    }
    for (args, old_path, new_path) in [
        (&["decision", "archive", "1"][..], ACTIVE, ARCHIVED),
        (&["decision", "restore", "1"][..], ARCHIVED, ACTIVE),
        (&["decision", "archive", "1"][..], ACTIVE, ARCHIVED),
    ] {
        assert_cli_process_parity(args, &reference, &candidate);
        for fixture in [&reference, &candidate] {
            assert!(!fixture.0.join(old_path).exists());
            assert!(
                std::fs::symlink_metadata(fixture.0.join(new_path))
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
            assert_eq!(
                std::fs::read(fixture.0.join(TARGET)).unwrap(),
                SOURCE.as_bytes()
            );
        }
    }
    assert_cli_process_parity(&["decision", "delete", "1"], &reference, &candidate);
    for fixture in [&reference, &candidate] {
        assert!(std::fs::symlink_metadata(fixture.0.join(ARCHIVED)).is_err());
        assert_eq!(
            std::fs::read(fixture.0.join(TARGET)).unwrap(),
            SOURCE.as_bytes()
        );
    }
}

#[test]
fn rust_decision_lifecycle_auto_commit_stages_only_selected_paths() {
    const ACTIVE: &str = "backlog/decisions/decision-1 - Field-Guide.md";
    const ARCHIVED: &str = "backlog/archive/decisions/decision-1 - Field-Guide.md";
    const SOURCE: &str =
        "---\nid: decision-1\ntitle: Field Guide\ndate: 2026-01-01\n---\nOriginal body\n";
    let reference = Fixture::new();
    let candidate = Fixture::new();
    for fixture in [&reference, &candidate] {
        fixture.file(ACTIVE, SOURCE);
        let configured = typescript_cli(&["config", "set", "autoCommit", "true"], &fixture.0);
        assert!(configured.status.success());
        initialize_git_fixture(fixture);
        fixture.file("STAGED.txt", "staged user change\n");
        git(&fixture.0, &["add", "STAGED.txt"]);
        fixture.file("UNTRACKED.txt", "untracked user change\n");
    }

    for (args, message, paths) in [
        (
            &["decision", "archive", "1"][..],
            "backlog: Archive decision decision-1",
            format!("{ARCHIVED}\n{ACTIVE}"),
        ),
        (
            &["decision", "restore", "decision-1"][..],
            "backlog: Restore decision decision-1",
            format!("{ARCHIVED}\n{ACTIVE}"),
        ),
        (
            &["decision", "archive", "decision-1"][..],
            "backlog: Archive decision decision-1",
            format!("{ARCHIVED}\n{ACTIVE}"),
        ),
        (
            &["decision", "delete", "1"][..],
            "backlog: Delete decision decision-1",
            ARCHIVED.to_string(),
        ),
    ] {
        assert_cli_process_parity(args, &reference, &candidate);
        let expected = selected_commit_result(&reference);
        let actual = selected_commit_result(&candidate);
        assert_eq!(actual, expected, "args: {args:?}");
        assert_eq!(actual.0, message, "args: {args:?}");
        assert_eq!(actual.1, paths, "args: {args:?}");
        assert_eq!(actual.2, "STAGED.txt\nUNRELATED.txt", "args: {args:?}");
        assert_eq!(
            git(&candidate.0, &["status", "--porcelain"]),
            git(&reference.0, &["status", "--porcelain"]),
            "args: {args:?}"
        );
    }
}

#[test]
fn rust_decision_lifecycle_keeps_applied_change_when_auto_commit_fails() {
    const ACTIVE: &str = "backlog/decisions/decision-1 - Field-Guide.md";
    const ARCHIVED: &str = "backlog/archive/decisions/decision-1 - Field-Guide.md";
    const SOURCE: &str =
        "---\nid: decision-1\ntitle: Field Guide\ndate: 2026-01-01\n---\nOriginal body\n";
    for (args, initially_archived, expected_path) in [
        (&["decision", "archive", "1"][..], false, ARCHIVED),
        (&["decision", "restore", "1"][..], true, ACTIVE),
        (&["decision", "delete", "1"][..], true, ""),
    ] {
        let reference = Fixture::new();
        let candidate = Fixture::new();
        for fixture in [&reference, &candidate] {
            fixture.file(if initially_archived { ARCHIVED } else { ACTIVE }, SOURCE);
            let configured = typescript_cli(&["config", "set", "autoCommit", "true"], &fixture.0);
            assert!(configured.status.success());
            initialize_git_fixture(fixture);
            std::fs::create_dir_all(fixture.0.join(".git")).unwrap();
            std::fs::write(fixture.0.join(".git/index.lock"), "").unwrap();
        }
        let expected = typescript_cli(args, &reference.0);
        let actual = rust_cli(args, &candidate.0);
        assert_eq!(
            actual.status.code(),
            expected.status.code(),
            "args: {args:?}"
        );
        assert_eq!(actual.stdout, expected.stdout, "args: {args:?}");
        assert_eq!(
            String::from_utf8_lossy(&actual.stderr)
                .replace(&candidate.0.to_string_lossy().to_string(), "<ROOT>"),
            String::from_utf8_lossy(&expected.stderr)
                .replace(&reference.0.to_string_lossy().to_string(), "<ROOT>"),
            "args: {args:?}"
        );
        assert!(
            String::from_utf8_lossy(&expected.stderr).contains("committing the change failed"),
            "{}",
            String::from_utf8_lossy(&expected.stderr)
        );
        assert!(
            String::from_utf8_lossy(&actual.stderr).contains("committing the change failed"),
            "{}",
            String::from_utf8_lossy(&actual.stderr)
        );
        if expected_path.is_empty() {
            assert!(!reference.0.join(ARCHIVED).exists());
            assert!(!candidate.0.join(ARCHIVED).exists());
        } else {
            assert_eq!(
                std::fs::read(candidate.0.join(expected_path)).unwrap(),
                SOURCE.as_bytes(),
                "args: {args:?}"
            );
        }
    }
}
