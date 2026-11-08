use backlog_md::{
    config::Config,
    records::{parse_markdown, parse_task},
    storage::Storage,
    task_store::{TaskStore, TaskStoreError},
};
use serde_json::{Value, json};
use std::{
    path::Path,
    process::{Command, Stdio},
    sync::{
        Arc, Barrier,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture(std::path::PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "backlog-task-store-mutations-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn reference(root: &Path) -> Value {
    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
const core = new Core(process.env.BACKLOG_MUTATION_ROOT);
const id = await core.generateNextId();
const task = {
  id, title: "A created task", status: "To Do", assignee: [], labels: [], dependencies: [],
  references: [], documentation: [], modifiedFiles: [], rawContent: "Unmanaged notes\n",
  createdDate: "2025-01-01", description: "Initial description", acceptanceCriteriaItems: [],
  definitionOfDoneItems: [],
};
const path = await core.filesystem.saveTask(task);
const created = await Bun.file(path).text();
const storedId = id.replace(/-(\d+)$/, (_, digits) => `-${digits.padStart(3, "0")}`).toLowerCase();
await Bun.write(path, created.replace(`id: ${id}`, `id: ${storedId}`).replace(`title: ${task.title}`, `title: ${task.title}\ncustom_metadata: preserve-me`));
const loaded = await core.filesystem.loadTask(id);
const taskLockError = await core.filesystem.withTaskLock(loaded, async () => {
  try { await core.filesystem.withTaskLock(loaded, async () => {}); return null; }
  catch (error) { return {name:error.name, message:error.message}; }
});
loaded.title = "Updated task";
loaded.description = "Updated description";
await core.filesystem.saveTask(loaded);
const updated = await Bun.file(path).text();
process.stdout.write(JSON.stringify({id, path, created, updated, taskLockError}));
"#;
    let bun = std::env::var_os("BACKLOG_REFERENCE_BUN").unwrap_or_else(|| "bun".into());
    let output = Command::new(bun)
        .args(["--eval", SCRIPT])
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
        .env("BACKLOG_MUTATION_ROOT", root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run live TypeScript task mutations");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn initial_task() -> backlog_md::records::TaskRecord {
    let mut task = parse_task(
        "---\nid: TASK-999\ntitle: A created task\nstatus: To Do\ncreated_date: '2025-01-01'\n---\nUnmanaged notes\n",
    )
    .unwrap();
    task.description = "Initial description".into();
    task
}

#[test]
fn create_and_update_match_live_typescript_files() {
    let fixture = Fixture::new();
    let typescript_root = fixture.0.join("typescript");
    let rust_root = fixture.0.join("rust");
    std::fs::create_dir_all(&typescript_root).unwrap();
    std::fs::create_dir_all(&rust_root).unwrap();
    for root in [&typescript_root, &rust_root] {
        let occupied = root.join("backlog/tasks/task-03 - Malformed.md");
        std::fs::create_dir_all(occupied.parent().unwrap()).unwrap();
        std::fs::write(occupied, "---\nid: [\n---\n").unwrap();
    }

    let expected = reference(&typescript_root);
    let store = TaskStore::new(&rust_root, "backlog", "task");
    let created = store
        .create_task(initial_task(), &Config::default())
        .unwrap();
    assert_eq!(created.task.id, expected["id"]);
    assert_eq!(
        created.path.to_string_lossy(),
        expected["path"].as_str().unwrap().replace(
            typescript_root.to_str().unwrap(),
            rust_root.to_str().unwrap()
        )
    );
    let expected_created = parse_task(expected["created"].as_str().unwrap()).unwrap();
    let created_content = std::fs::read_to_string(&created.path).unwrap();
    let actual_created = parse_task(&created_content).unwrap();
    assert_eq!(actual_created.id, expected_created.id);
    assert_eq!(actual_created.title, expected_created.title);
    assert_eq!(actual_created.raw_content, expected_created.raw_content);
    assert_eq!(
        normalize_source(&created_content),
        normalize_source(expected["created"].as_str().unwrap())
    );

    let content = std::fs::read_to_string(&created.path).unwrap();
    let stored_id = format!("task-{:0>3}", created.task.id.trim_start_matches("TASK-"));
    std::fs::write(
        &created.path,
        content
            .replace(
                &format!("id: {}", created.task.id),
                &format!("id: {stored_id}"),
            )
            .replace(
                "title: A created task\n",
                "title: A created task\ncustom_metadata: preserve-me\n",
            ),
    )
    .unwrap();
    let loaded = store.load_task(&created.task.id).unwrap().unwrap();
    let held_lock = Storage::new(rust_root.join("backlog"), None)
        .acquire_entity_lock("task", &loaded.task.id, &created.path)
        .unwrap();
    let blocked = store.update_task(loaded.task.clone()).unwrap_err();
    assert!(matches!(blocked, TaskStoreError::TaskLock(_)));
    assert_eq!(blocked.to_string(), expected["taskLockError"]["message"]);
    drop(held_lock);

    let mut updated = store.load_task(&created.task.id).unwrap().unwrap().task;
    updated.title = "Updated task".into();
    updated.description = "Updated description".into();
    let updated = store.update_task(updated).unwrap().unwrap();
    let expected_updated = parse_task(expected["updated"].as_str().unwrap()).unwrap();
    let actual_content = std::fs::read_to_string(&updated.path).unwrap();
    let actual_updated = parse_task(&actual_content).unwrap();
    assert_eq!(actual_updated.id, expected_updated.id);
    assert_eq!(actual_updated.title, expected_updated.title);
    assert_eq!(actual_updated.description, expected_updated.description);
    assert_eq!(updated.task.raw_content, expected_updated.raw_content);
    assert!(updated.task.raw_content.contains("Unmanaged notes"));
    let actual_content = std::fs::read_to_string(&updated.path).unwrap();
    let actual_markdown = parse_markdown(&actual_content).unwrap();
    let expected_markdown = parse_markdown(expected["updated"].as_str().unwrap()).unwrap();
    assert_eq!(actual_markdown.frontmatter, expected_markdown.frontmatter);
    assert_eq!(actual_markdown.content, expected_markdown.content);
    assert_eq!(
        json!({"id": updated.task.id}),
        json!({"id": expected_updated.id})
    );
}

fn normalize_source(source: &str) -> String {
    source
        .replace("created_date: '2025-01-01'", "created_date: 2025-01-01")
        .trim_end()
        .to_owned()
}

#[test]
fn update_resolution_errors_match_live_typescript() {
    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
const core = new Core(process.env.BACKLOG_MUTATION_ROOT);
const results = [];
for (const id of ["2", "3", "4", "99"]) {
  try { results.push({id, found: Boolean(await core.filesystem.loadTask(id))}); }
  catch (error) { results.push({id, error: {name:error.name, message:error.message, candidates:error.candidates}}); }
}
process.stdout.write(JSON.stringify(results));
"#;
    let fixture = Fixture::new();
    let backlog = fixture.0.join("backlog");
    let write = |relative: &str, contents: &str| {
        let path = backlog.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    };
    write(
        "archive/tasks/task-2 - Archived.md",
        "---\nid: TASK-2\ntitle: Archived\nstatus: Done\ncreated_date: '2025-01-01'\n---\nBody\n",
    );
    write(
        "tasks/task-3 - Invalid dependencies.md",
        "---\nid: TASK-3\ntitle: Invalid dependencies\nstatus: To Do\ncreated_date: '2025-01-01'\ndependencies:\n  invalid: mapping\n---\nBody\n",
    );
    for title in ["First duplicate", "Second duplicate"] {
        write(
            &format!("tasks/task-4 - {title}.md"),
            &format!(
                "---\nid: TASK-4\ntitle: {title}\nstatus: To Do\ncreated_date: '2025-01-01'\n---\nBody\n"
            ),
        );
    }
    let bun = std::env::var_os("BACKLOG_REFERENCE_BUN").unwrap_or_else(|| "bun".into());
    let output = Command::new(bun)
        .args(["--eval", SCRIPT])
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
        .env("BACKLOG_MUTATION_ROOT", &fixture.0)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run live TypeScript lookup errors");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected: Value = serde_json::from_slice(&output.stdout).unwrap();

    let store = TaskStore::new(&fixture.0, "backlog", "task");
    let actual: Vec<_> = ["2", "3", "4", "99"]
        .into_iter()
        .map(|id| {
            let mut task = initial_task();
            task.id = id.into();
            match store.update_task(task) {
                Ok(Some(_)) => json!({"id": id, "found": true}),
                Ok(None) => json!({"id": id, "found": false}),
                Err(TaskStoreError::Ambiguous(error)) => json!({"id": id, "error": {"name": error.name, "message": error.to_string(), "candidates": error.candidates}}),
                Err(TaskStoreError::Dependencies(message)) => json!({"id": id, "error": {"name": "TaskDependenciesParseError", "message": message}}),
                Err(error) => json!({"id": id, "error": {"name": "TaskStoreError", "message": error.to_string()}}),
            }
        })
        .collect();
    assert_eq!(Value::Array(actual), expected);
}

#[test]
fn concurrent_creates_allocate_distinct_task_ids() {
    let fixture = Fixture::new();
    let store = TaskStore::new(&fixture.0, "backlog", "task");
    let config = Config::default();
    let barrier = Arc::new(Barrier::new(3));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let store = store.clone();
            let barrier = Arc::clone(&barrier);
            let config = config.clone();
            thread::spawn(move || {
                barrier.wait();
                store.create_task(initial_task(), &config).unwrap().task.id
            })
        })
        .collect();
    barrier.wait();
    let mut ids: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    ids.sort();
    assert_eq!(ids, ["TASK-1", "TASK-2"]);
}

#[test]
fn task_creation_reserves_uncommitted_linked_worktree_filenames() {
    let fixture = Fixture::new();
    let typescript_root = fixture.0.join("typescript");
    let rust_root = fixture.0.join("rust");
    let typescript_side = fixture.0.join("typescript-side");
    let rust_side = fixture.0.join("rust-side");
    setup_linked_worktree(&typescript_root, &typescript_side);
    setup_linked_worktree(&rust_root, &rust_side);

    let expected = reference(&typescript_root);
    let store = TaskStore::new(&rust_root, "backlog", "task");
    let created = store
        .create_task(initial_task(), &Config::default())
        .unwrap();
    assert_eq!(created.task.id, expected["id"]);
    assert_eq!(created.task.id, "TASK-12");
}

#[test]
fn branch_snapshot_ids_match_live_typescript_settings() {
    let fixture = Fixture::new();
    for (index, enabled, stale, expected_id) in [
        (0, true, false, "TASK-16"),
        (1, false, false, "TASK-2"),
        (2, true, true, "TASK-2"),
    ] {
        let typescript_root = fixture.0.join(format!("typescript-{index}"));
        let rust_root = fixture.0.join(format!("rust-{index}"));
        setup_branch_snapshot(&typescript_root, stale);
        setup_branch_snapshot(&rust_root, stale);
        if !enabled {
            let config_path = typescript_root.join("backlog/config.yml");
            std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
            std::fs::write(&config_path, "check_active_branches: false\n").unwrap();
        }

        let expected = reference(&typescript_root);
        let store = TaskStore::new(&rust_root, "backlog", "task");
        let config = Config {
            check_active_branches: Some(enabled),
            active_branch_days: Some(30.0),
            ..Config::default()
        };
        let created = store.create_task(initial_task(), &config).unwrap();
        assert_eq!(expected["id"], expected_id);
        assert_eq!(created.task.id, expected_id);
    }
}

#[test]
fn remote_branch_ids_match_live_typescript_refresh_and_settings() {
    let fixture = Fixture::new();

    let typescript_root = fixture.0.join("typescript-remote");
    let rust_root = fixture.0.join("rust-remote");
    let source = setup_remote_snapshot(&fixture.0, &typescript_root, &rust_root, false);
    run_git(&source, &["switch", "-q", "feature"], None);
    let pushed_task = source.join("backlog/tasks/task-16 - Pushed after clone.md");
    std::fs::write(&pushed_task, "malformed after clone").unwrap();
    run_git(&source, &["add", "."], None);
    run_git(&source, &["commit", "-qm", "push after clone"], None);
    run_git(&source, &["push", "-q", "origin", "feature"], None);
    run_git(&source, &["switch", "-q", "main"], None);

    let expected = reference(&typescript_root);
    let store = TaskStore::new(&rust_root, "backlog", "task");
    let created = store
        .create_task(initial_task(), &Config::default())
        .unwrap();
    assert_eq!(expected["id"], "TASK-18");
    assert_eq!(created.task.id, "TASK-18");

    let disabled_typescript = fixture.0.join("typescript-disabled");
    let disabled_rust = fixture.0.join("rust-disabled");
    setup_remote_snapshot(&fixture.0, &disabled_typescript, &disabled_rust, false);
    let config_path = disabled_typescript.join("backlog/config.yml");
    std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    std::fs::write(&config_path, "remote_operations: false\n").unwrap();
    let expected = reference(&disabled_typescript);
    let config = Config {
        remote_operations: Some(false),
        ..Config::default()
    };
    let created = TaskStore::new(&disabled_rust, "backlog", "task")
        .create_task(initial_task(), &config)
        .unwrap();
    assert_eq!(expected["id"], "TASK-2");
    assert_eq!(created.task.id, "TASK-2");

    let stale_typescript = fixture.0.join("typescript-stale-remote");
    let stale_rust = fixture.0.join("rust-stale-remote");
    setup_remote_snapshot(&fixture.0, &stale_typescript, &stale_rust, true);
    let expected = reference(&stale_typescript);
    let config = Config {
        active_branch_days: Some(30.0),
        ..Config::default()
    };
    let created = TaskStore::new(&stale_rust, "backlog", "task")
        .create_task(initial_task(), &config)
        .unwrap();
    assert_eq!(expected["id"], "TASK-2");
    assert_eq!(created.task.id, "TASK-2");

    let filesystem_typescript = fixture.0.join("typescript-filesystem-only");
    let filesystem_rust = fixture.0.join("rust-filesystem-only");
    setup_remote_snapshot(&fixture.0, &filesystem_typescript, &filesystem_rust, false);
    let config_path = filesystem_typescript.join("backlog/config.yml");
    std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    std::fs::write(&config_path, "filesystem_only: true\n").unwrap();
    let expected = reference(&filesystem_typescript);
    let config = Config {
        filesystem_only: Some(true),
        ..Config::default()
    };
    let created = TaskStore::new(&filesystem_rust, "backlog", "task")
        .create_task(initial_task(), &config)
        .unwrap();
    assert_eq!(expected["id"], "TASK-2");
    assert_eq!(created.task.id, "TASK-2");
}

fn setup_remote_snapshot(
    parent: &Path,
    typescript_root: &Path,
    rust_root: &Path,
    stale: bool,
) -> std::path::PathBuf {
    let source = parent.join(format!(
        "remote-source-{}",
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let origin = parent.join(format!(
        "remote-origin-{}",
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&source).unwrap();
    let task = source.join("backlog/tasks/task-1 - Initial.md");
    std::fs::create_dir_all(task.parent().unwrap()).unwrap();
    std::fs::write(
        task,
        "---\nid: TASK-1\ntitle: Initial\nstatus: To Do\ncreated_date: '2025-01-01'\n---\nBody\n",
    )
    .unwrap();
    run_git(&source, &["init", "-q", "-b", "main"], None);
    run_git(&source, &["config", "user.email", "test@example.com"], None);
    run_git(&source, &["config", "user.name", "Task Store Test"], None);
    run_git(&source, &["add", "."], None);
    run_git(&source, &["commit", "-qm", "initial"], None);
    run_git(&source, &["switch", "-q", "-c", "feature"], None);
    std::fs::write(
        source.join("backlog/tasks/task-15 - Malformed remote task.md"),
        "---\nid: [\n---\n",
    )
    .unwrap();
    std::fs::write(
        source.join("backlog/tasks/task-015 - Duplicate remote claim.md"),
        "malformed duplicate filename claim",
    )
    .unwrap();
    let archived = source.join("backlog/archive/tasks/task-17 - Archived remote task.md");
    std::fs::create_dir_all(archived.parent().unwrap()).unwrap();
    std::fs::write(archived, "malformed archived filename claim").unwrap();
    run_git(&source, &["add", "."], None);
    run_git(
        &source,
        &["commit", "-qm", "remote task"],
        stale.then_some(("GIT_AUTHOR_DATE", "2000-01-01T00:00:00Z")),
    );
    run_git(
        &source,
        &["init", "--bare", "-q", origin.to_str().unwrap()],
        None,
    );
    run_git(
        &source,
        &["remote", "add", "origin", origin.to_str().unwrap()],
        None,
    );
    run_git(&source, &["push", "-q", "--all", "origin"], None);
    run_git(
        parent,
        &[
            "--git-dir",
            origin.to_str().unwrap(),
            "symbolic-ref",
            "HEAD",
            "refs/heads/main",
        ],
        None,
    );

    for clone in [typescript_root, rust_root] {
        run_git(
            parent,
            &[
                "clone",
                "-q",
                origin.to_str().unwrap(),
                clone.to_str().unwrap(),
            ],
            None,
        );
    }
    source
}

fn setup_branch_snapshot(root: &Path, stale: bool) {
    std::fs::create_dir_all(root).unwrap();
    let initial = root.join("backlog/tasks/task-1 - Initial.md");
    std::fs::create_dir_all(initial.parent().unwrap()).unwrap();
    std::fs::write(
        initial,
        "---\nid: TASK-1\ntitle: Initial\nstatus: To Do\ncreated_date: '2025-01-01'\n---\nBody\n",
    )
    .unwrap();
    run_git(root, &["init", "-q"], None);
    run_git(root, &["config", "user.email", "test@example.com"], None);
    run_git(root, &["config", "user.name", "Task Store Test"], None);
    run_git(root, &["add", "."], None);
    run_git(root, &["commit", "-qm", "initial"], None);
    let current_branch = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(current_branch.status.success());
    let current_branch = String::from_utf8_lossy(&current_branch.stdout)
        .trim()
        .to_owned();
    run_git(root, &["switch", "-q", "-c", "feature"], None);
    let branch_task = root.join("backlog/tasks/task-15 - Malformed branch task.md");
    std::fs::write(branch_task, "---\nid: [\n---\n").unwrap();
    let duplicate_task = root.join("backlog/tasks/task-015 - Duplicate branch claim.md");
    std::fs::write(duplicate_task, "malformed duplicate filename claim").unwrap();
    run_git(root, &["add", "."], None);
    let old_date = "2000-01-01T00:00:00Z";
    run_git(
        root,
        &["commit", "-qm", "branch task"],
        stale.then_some(("GIT_AUTHOR_DATE", old_date)),
    );
    run_git(root, &["switch", "-q", &current_branch], None);
}

fn run_git(root: &Path, arguments: &[&str], date: Option<(&str, &str)>) {
    let mut command = Command::new("git");
    command.args(arguments).current_dir(root);
    if let Some((name, value)) = date {
        command.env(name, value);
        if name == "GIT_AUTHOR_DATE" {
            command.env("GIT_COMMITTER_DATE", value);
        }
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn setup_linked_worktree(root: &Path, sibling: &Path) {
    std::fs::create_dir_all(root).unwrap();
    let initial = root.join("backlog/tasks/task-1 - Initial.md");
    std::fs::create_dir_all(initial.parent().unwrap()).unwrap();
    std::fs::write(
        initial,
        "---\nid: TASK-1\ntitle: Initial\nstatus: To Do\ncreated_date: '2025-01-01'\n---\nBody\n",
    )
    .unwrap();
    for arguments in [
        vec!["init", "-q"],
        vec!["config", "user.email", "test@example.com"],
        vec!["config", "user.name", "Task Store Test"],
        vec!["add", "."],
        vec!["commit", "-qm", "initial"],
    ] {
        let output = Command::new("git")
            .args(arguments)
            .current_dir(root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let output = Command::new("git")
        .args([
            "worktree",
            "add",
            "-q",
            "-b",
            "sibling",
            sibling.to_str().unwrap(),
        ])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let reserved = sibling.join("backlog/tasks/task-11 - Uncommitted.md");
    std::fs::create_dir_all(reserved.parent().unwrap()).unwrap();
    std::fs::write(reserved, "uncommitted filename reservation").unwrap();
}
