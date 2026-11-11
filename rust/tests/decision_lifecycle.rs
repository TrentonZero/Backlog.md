use backlog_md::{config::Config, decision_store::DecisionStore, storage::Storage};
use serde_json::Value;
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "backlog-decision-lifecycle-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("backlog/decisions")).unwrap();
        Self(root)
    }

    fn write(&self, relative: &str, source: &str) -> PathBuf {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, source).unwrap();
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn reference(script: &str, root: &Fixture) -> Value {
    let output =
        Command::new(std::env::var_os("BACKLOG_REFERENCE_BUN").unwrap_or_else(|| "bun".into()))
            .args(["--eval", script])
            .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
            .env("BACKLOG_DECISION_ROOT", &root.0)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn git(root: &PathBuf, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn setup_branch_decision_claim(root: &Fixture) {
    initial_decision(root);
    git(&root.0, &["init", "-q", "-b", "main"]);
    git(&root.0, &["config", "user.email", "test@example.com"]);
    git(&root.0, &["config", "user.name", "Decision Lifecycle Test"]);
    git(&root.0, &["add", "."]);
    git(&root.0, &["commit", "-qm", "initial"]);
    git(&root.0, &["switch", "-q", "-c", "retain-id"]);
    root.write(
        "backlog/archive/decisions/decision-2 - Retained.md",
        "---\nid: decision-2\ntitle: Retained\ndate: 2025-01-01\n---\nBranch claim\n",
    );
    git(&root.0, &["add", "."]);
    git(&root.0, &["commit", "-qm", "retain decision id"]);
    git(&root.0, &["switch", "-q", "main"]);
    std::fs::write(
        root.0.join("backlog/config.yml"),
        "remote_operations: false\n",
    )
    .unwrap();
}

fn initial_decision(root: &Fixture) -> String {
    let source = "---\nid: decision-1\ntitle: Field Guide\ndate: 2025-01-01\ncustom: retained\n---\n## Decision\nOriginal choice\n";
    root.write("backlog/decisions/decision-1 - Field-Guide.md", source);
    source.into()
}

#[test]
fn archives_and_restores_decisions_byte_for_byte_with_live_typescript_parity() {
    let fixture = Fixture::new();
    let reference_fixture = Fixture::new();
    let source = initial_decision(&fixture);
    initial_decision(&reference_fixture);
    let store = DecisionStore::new(&fixture.0, "backlog", "task");
    let archived = store.archive_decision("1").unwrap().unwrap();
    assert_eq!(archived.id, "decision-1");
    assert_eq!(
        std::fs::read_to_string(
            fixture
                .0
                .join("backlog/archive/decisions/decision-1 - Field-Guide.md")
        )
        .unwrap(),
        source
    );
    let restored = store.restore_decision("decision-001").unwrap().unwrap();
    assert_eq!(restored.id, "decision-1");
    assert_eq!(
        std::fs::read_to_string(
            fixture
                .0
                .join("backlog/decisions/decision-1 - Field-Guide.md")
        )
        .unwrap(),
        source
    );

    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
const core = new Core(process.env.BACKLOG_DECISION_ROOT);
const archived = await core.archiveContentRecord("decision", "1", false);
const restored = await core.restoreContentRecord("decision", "decision-001", false);
process.stdout.write(JSON.stringify({archived:archived.id, restored:restored.id, source:await Bun.file(process.env.BACKLOG_DECISION_ROOT+"/backlog/decisions/decision-1 - Field-Guide.md").text()}));
"#;
    let expected = reference(SCRIPT, &reference_fixture);
    assert_eq!(expected["archived"], archived.id);
    assert_eq!(expected["restored"], restored.id);
    assert_eq!(expected["source"], source);
}

#[test]
fn wrong_state_missing_and_ambiguous_lifecycle_results_match_typescript() {
    let fixture = Fixture::new();
    let reference_fixture = Fixture::new();
    initial_decision(&fixture);
    initial_decision(&reference_fixture);
    let store = DecisionStore::new(&fixture.0, "backlog", "task");
    store.archive_decision("1").unwrap();
    let archive_again = store.archive_decision("1").unwrap_err().to_string();
    store.restore_decision("1").unwrap();
    let restore_active = store.restore_decision("1").unwrap_err().to_string();
    let delete_active = store.delete_decision("1").unwrap_err().to_string();
    assert!(store.archive_decision("404").unwrap().is_none());

    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
const core = new Core(process.env.BACKLOG_DECISION_ROOT);
await core.archiveContentRecord("decision", "1", false);
let archiveAgain = "";
try { await core.archiveContentRecord("decision", "1", false); } catch (error) { archiveAgain = error.message; }
await core.restoreContentRecord("decision", "1", false);
let restoreActive = "";
let deleteActive = "";
try { await core.restoreContentRecord("decision", "1", false); } catch (error) { restoreActive = error.message; }
try { await core.deleteContentRecord("decision", "1", false); } catch (error) { deleteActive = error.message; }
const missing = await core.archiveContentRecord("decision", "404", false);
process.stdout.write(JSON.stringify({archiveAgain, restoreActive, deleteActive, missing:missing===null}));
"#;
    let expected = reference(SCRIPT, &reference_fixture);
    assert_eq!(expected["archiveAgain"], archive_again);
    assert_eq!(expected["restoreActive"], restore_active);
    assert_eq!(expected["deleteActive"], delete_active);
    assert_eq!(expected["missing"], true);

    let ambiguous_fixture = Fixture::new();
    let ambiguous_reference = Fixture::new();
    let archived = "---\nid: decision-1\ntitle: Field Guide\ndate: 2025-01-01\n---\n## Decision\nOriginal choice\n";
    ambiguous_fixture.write(
        "backlog/archive/decisions/decision-1 - Field-Guide.md",
        archived,
    );
    ambiguous_fixture.write("backlog/decisions/decision-1 - Field-Guide.md", archived);
    ambiguous_reference.write(
        "backlog/archive/decisions/decision-1 - Field-Guide.md",
        archived,
    );
    ambiguous_reference.write("backlog/decisions/decision-1 - Field-Guide.md", archived);
    let error = DecisionStore::new(&ambiguous_fixture.0, "backlog", "task")
        .archive_decision("1")
        .unwrap_err()
        .to_string();
    const AMBIGUOUS_SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
const core = new Core(process.env.BACKLOG_DECISION_ROOT);
let error = "";
try { await core.archiveContentRecord("decision", "1", false); } catch (failure) { error = failure.message; }
process.stdout.write(JSON.stringify({error}));
"#;
    assert_eq!(
        reference(AMBIGUOUS_SCRIPT, &ambiguous_reference)["error"],
        error
    );
}

#[test]
fn archive_and_restore_collisions_and_entity_lock_match_typescript() {
    let active_source = "---\nid: decision-1\ntitle: Field Guide\ndate: 2025-01-01\n---\nChoice\n";
    let collision_source =
        "---\nid: decision-9\ntitle: Collision\ndate: 2025-01-01\n---\nKeep this\n";
    let archive_fixture = Fixture::new();
    let archive_reference = Fixture::new();
    for root in [&archive_fixture, &archive_reference] {
        root.write(
            "backlog/decisions/decision-1 - Field-Guide.md",
            active_source,
        );
        root.write(
            "backlog/archive/decisions/decision-1 - Field-Guide.md",
            collision_source,
        );
    }
    let archive_error = DecisionStore::new(&archive_fixture.0, "backlog", "task")
        .archive_decision("1")
        .unwrap_err()
        .to_string();
    const ARCHIVE_SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
const core = new Core(process.env.BACKLOG_DECISION_ROOT);
let error = "";
try { await core.archiveContentRecord("decision", "1", false); } catch (failure) { error = failure.message; }
process.stdout.write(JSON.stringify({error}));
"#;
    assert_eq!(
        reference(ARCHIVE_SCRIPT, &archive_reference)["error"],
        archive_error
    );
    assert_eq!(
        std::fs::read_to_string(
            archive_fixture
                .0
                .join("backlog/decisions/decision-1 - Field-Guide.md")
        )
        .unwrap(),
        active_source
    );
    assert_eq!(
        std::fs::read_to_string(
            archive_fixture
                .0
                .join("backlog/archive/decisions/decision-1 - Field-Guide.md")
        )
        .unwrap(),
        collision_source
    );

    let restore_fixture = Fixture::new();
    let restore_reference = Fixture::new();
    for root in [&restore_fixture, &restore_reference] {
        root.write(
            "backlog/archive/decisions/decision-1 - Field-Guide.md",
            active_source,
        );
        root.write(
            "backlog/decisions/decision-1 - Field-Guide.md",
            collision_source,
        );
    }
    let restore_error = DecisionStore::new(&restore_fixture.0, "backlog", "task")
        .restore_decision("1")
        .unwrap_err()
        .to_string();
    const RESTORE_SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
const core = new Core(process.env.BACKLOG_DECISION_ROOT);
let error = "";
try { await core.restoreContentRecord("decision", "1", false); } catch (failure) { error = failure.message; }
process.stdout.write(JSON.stringify({error}));
"#;
    assert_eq!(
        reference(RESTORE_SCRIPT, &restore_reference)["error"],
        restore_error
    );

    let lock_fixture = Fixture::new();
    let lock_reference = Fixture::new();
    lock_fixture.write(
        "backlog/decisions/decision-1 - Field-Guide.md",
        active_source,
    );
    lock_reference.write(
        "backlog/decisions/decision-1 - Field-Guide.md",
        active_source,
    );
    let path = lock_fixture
        .0
        .join("backlog/decisions/decision-1 - Field-Guide.md");
    let blocker = Storage::new(lock_fixture.0.join("backlog"), None)
        .acquire_entity_lock("decision", "decision-1", &path)
        .unwrap();
    let rust_refused = DecisionStore::new(&lock_fixture.0, "backlog", "task")
        .archive_decision("1")
        .is_err();
    drop(blocker);
    const LOCK_SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
const root = process.env.BACKLOG_DECISION_ROOT;
const core = new Core(root);
const record = await core.getDecision("1");
let refused = false;
try { await core.filesystem.withContentRecordLock("decision", record, () => core.archiveContentRecord("decision", "1", false)); } catch { refused = true; }
process.stdout.write(JSON.stringify({refused, activeExists:await Bun.file(root+"/backlog/decisions/decision-1 - Field-Guide.md").exists(), archivedExists:await Bun.file(root+"/backlog/archive/decisions/decision-1 - Field-Guide.md").exists()}));
"#;
    let expected = reference(LOCK_SCRIPT, &lock_reference);
    assert!(rust_refused);
    assert_eq!(expected["refused"], true);
    assert_eq!(expected["activeExists"], true);
    assert_eq!(expected["archivedExists"], false);
}

#[cfg(unix)]
#[test]
fn deletes_only_archived_decision_symlinks_and_reports_backlinks_like_typescript() {
    let fixture = Fixture::new();
    let reference_fixture = Fixture::new();
    let source = "---\nid: decision-1\ntitle: Linked\ndate: 2025-01-01\n---\nLinked choice\n";
    let rust_target = fixture.write("outside/decision.md", source);
    let reference_target = reference_fixture.write("outside/decision.md", source);
    let rust_link = fixture.0.join("backlog/decisions/decision-1 - Linked.md");
    let reference_link = reference_fixture
        .0
        .join("backlog/decisions/decision-1 - Linked.md");
    std::fs::create_dir_all(rust_link.parent().unwrap()).unwrap();
    std::fs::create_dir_all(reference_link.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&rust_target, &rust_link).unwrap();
    std::os::unix::fs::symlink(reference_target, &reference_link).unwrap();
    let active_task = "---\nid: TASK-1\ntitle: Active\nstatus: To Do\ncreated_date: '2025-01-01'\nlabels: []\ndependencies: []\ndocumentation:\n  - backlog/archive/decisions/decision-1 - Linked.md\n---\n";
    let archived_task = "---\nid: TASK-2\ntitle: Archived\nstatus: Done\ncreated_date: '2025-01-01'\nlabels: []\ndependencies: []\ndocumentation:\n  - backlog/decisions/decision-1 - Linked.md\n---\n";
    let draft = "---\nid: DRAFT-1\ntitle: Draft\nstatus: Draft\ncreated_date: '2025-01-01'\nlabels: []\ndependencies: []\ndocumentation:\n  - backlog/archive/decisions/decision-1 - Linked.md\n---\n";
    for root in [&fixture, &reference_fixture] {
        root.write("backlog/tasks/task-1 - Active.md", active_task);
        root.write("backlog/archive/tasks/task-2 - Archived.md", archived_task);
        root.write("backlog/drafts/draft-1 - Draft.md", draft);
    }
    let store = DecisionStore::new(&fixture.0, "backlog", "task");
    store.archive_decision("1").unwrap();
    let archived_link = fixture
        .0
        .join("backlog/archive/decisions/decision-1 - Linked.md");
    assert!(
        std::fs::symlink_metadata(&archived_link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    store.restore_decision("1").unwrap();
    assert!(
        std::fs::symlink_metadata(&rust_link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    store.archive_decision("1").unwrap();
    let removed = store.delete_decision("DECISION-01").unwrap().unwrap();
    assert_eq!(removed.id, "decision-1");
    assert_eq!(
        removed.listed_as_documentation_by,
        ["TASK-1", "TASK-2", "DRAFT-1"]
    );
    assert!(!rust_link.exists());
    assert!(rust_target.exists());
    assert_eq!(std::fs::read_to_string(rust_target).unwrap(), source);
    assert_eq!(
        std::fs::read_to_string(fixture.0.join("backlog/tasks/task-1 - Active.md")).unwrap(),
        active_task
    );
    assert_eq!(
        std::fs::read_to_string(fixture.0.join("backlog/archive/tasks/task-2 - Archived.md"))
            .unwrap(),
        archived_task
    );
    assert_eq!(
        std::fs::read_to_string(fixture.0.join("backlog/drafts/draft-1 - Draft.md")).unwrap(),
        draft
    );

    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
const root = process.env.BACKLOG_DECISION_ROOT;
const core = new Core(root);
await core.archiveContentRecord("decision", "1", false);
await core.restoreContentRecord("decision", "1", false);
await core.archiveContentRecord("decision", "1", false);
const result = await core.deleteContentRecord("decision", "DECISION-01", false);
process.stdout.write(JSON.stringify({id:result.id, listed:result.listedAsDocumentationBy, linkExists:await Bun.file(root+"/backlog/archive/decisions/decision-1 - Linked.md").exists(), target:await Bun.file(root+"/outside/decision.md").text(), task:await Bun.file(root+"/backlog/tasks/task-1 - Active.md").text()}));
"#;
    let expected = reference(SCRIPT, &reference_fixture);
    assert_eq!(expected["id"], removed.id);
    assert_eq!(
        expected["listed"],
        serde_json::json!(removed.listed_as_documentation_by)
    );
    assert_eq!(expected["linkExists"], false);
    assert_eq!(expected["target"], source);
    assert_eq!(expected["task"], active_task);
}

#[test]
fn archive_reserves_id_and_delete_releases_it_when_no_file_claims_it() {
    let fixture = Fixture::new();
    let reference_fixture = Fixture::new();
    initial_decision(&fixture);
    initial_decision(&reference_fixture);
    let store = DecisionStore::new(&fixture.0, "backlog", "task");
    let archived = store.archive_decision("1").unwrap().unwrap();
    let next = store
        .create_decision(
            backlog_md::decision_store::DecisionCreateInput {
                title: "Next".into(),
                status: None,
                context: String::new(),
                decision: String::new(),
                consequences: String::new(),
                alternatives: None,
            },
            &Config::default(),
        )
        .unwrap();
    assert_eq!(next.decision.id, "decision-2");
    store.archive_decision("2").unwrap();
    let deleted = store.delete_decision("2").unwrap().unwrap();
    let reused = store
        .create_decision(
            backlog_md::decision_store::DecisionCreateInput {
                title: "Reused".into(),
                status: None,
                context: String::new(),
                decision: String::new(),
                consequences: String::new(),
                alternatives: None,
            },
            &Config {
                remote_operations: Some(false),
                ..Config::default()
            },
        )
        .unwrap();
    assert_eq!(reused.decision.id, "decision-2");

    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
const core = new Core(process.env.BACKLOG_DECISION_ROOT);
const archived = await core.archiveContentRecord("decision", "1", false);
const next = await core.createDecisionFromInput({title:"Next"}, false);
await core.archiveContentRecord("decision", "2", false);
const deleted = await core.deleteContentRecord("decision", "2", false);
const reused = await core.createDecisionFromInput({title:"Reused"}, false);
process.stdout.write(JSON.stringify({archived:archived.id, next:next.id, deleted:deleted.id, reused:reused.id}));
"#;
    let expected = reference(SCRIPT, &reference_fixture);
    assert_eq!(expected["archived"], archived.id);
    assert_eq!(expected["next"], next.decision.id);
    assert_eq!(expected["deleted"], deleted.id);
    assert_eq!(expected["reused"], reused.decision.id);
}

#[test]
fn committed_branch_filename_keeps_a_deleted_decision_id_reserved_like_typescript() {
    let fixture = Fixture::new();
    let reference_fixture = Fixture::new();
    setup_branch_decision_claim(&fixture);
    setup_branch_decision_claim(&reference_fixture);
    for root in [&fixture, &reference_fixture] {
        root.write(
            "backlog/archive/decisions/decision-2 - Local copy.md",
            "---\nid: decision-2\ntitle: Local copy\ndate: 2025-01-01\n---\nLocal claim\n",
        );
    }
    let store = DecisionStore::new(&fixture.0, "backlog", "task");
    store.delete_decision("2").unwrap().unwrap();
    let created = store
        .create_decision(
            backlog_md::decision_store::DecisionCreateInput {
                title: "After deletion".into(),
                status: None,
                context: String::new(),
                decision: String::new(),
                consequences: String::new(),
                alternatives: None,
            },
            &Config {
                remote_operations: Some(false),
                ..Config::default()
            },
        )
        .unwrap();
    assert_eq!(created.decision.id, "decision-3");

    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
const core = new Core(process.env.BACKLOG_DECISION_ROOT);
await core.deleteContentRecord("decision", "2", false);
const created = await core.createDecisionFromInput({title:"After deletion"}, false);
process.stdout.write(JSON.stringify({created:created.id}));
"#;
    assert_eq!(
        reference(SCRIPT, &reference_fixture)["created"],
        created.decision.id
    );
}
