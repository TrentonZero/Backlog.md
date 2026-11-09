use backlog_md::{
    config::Config, draft_store::DraftStore, records::parse_task, task_store::TaskStore,
};
use serde_json::{Value, json};
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
            "backlog-draft-lifecycle-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("backlog/drafts")).unwrap();
        std::fs::create_dir_all(root.join("backlog/tasks")).unwrap();
        Self(root)
    }

    fn file(&self, relative: &str, source: &str) -> PathBuf {
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

fn draft_source(id: &str, title: &str) -> String {
    format!(
        "---\nid: {id}\ntitle: {title}\nstatus: Draft\ncreated_date: '2025-01-01'\ncustom: keep\n---\nDraft body\n"
    )
}

#[test]
fn archives_draft_without_rewriting_source_and_returns_false_when_missing() {
    let fixture = Fixture::new();
    let source = draft_source("DRAFT-1", "Archive me");
    let original = fixture.file("backlog/drafts/draft-1 - Archive me.md", &source);
    let store = DraftStore::new(&fixture.0, "backlog");
    assert!(store.archive_draft("1").unwrap());
    let archived = fixture
        .0
        .join("backlog/archive/drafts/draft-1 - Archive me.md");
    assert_eq!(std::fs::read_to_string(archived).unwrap(), source);
    assert!(!original.exists());
    assert!(!store.archive_draft("1").unwrap());
}

#[test]
fn archive_destination_collision_matches_live_typescript_overwrite() {
    let rust_fixture = Fixture::new();
    let ts_fixture = Fixture::new();
    let source = draft_source("DRAFT-1", "Archive me");
    for fixture in [&rust_fixture, &ts_fixture] {
        fixture.file("backlog/drafts/draft-1 - Archive me.md", &source);
        fixture.file(
            "backlog/archive/drafts/draft-1 - Archive me.md",
            "existing archive\n",
        );
    }
    let store = DraftStore::new(&rust_fixture.0, "backlog");
    let archived = store.archive_draft("1").unwrap();
    let rust_result = json!({
        "archived": archived,
        "sourceExists": rust_fixture.0.join("backlog/drafts/draft-1 - Archive me.md").exists(),
        "archivedContents": std::fs::read_to_string(rust_fixture.0.join("backlog/archive/drafts/draft-1 - Archive me.md")).unwrap(),
    });
    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
import { readFile, stat } from "node:fs/promises";
const root = process.env.BACKLOG_DRAFT_LIFECYCLE_ROOT;
const core = new Core(root);
const archived = await core.archiveDraft("1", false);
let sourceExists = true;
try { await stat(root+"/backlog/drafts/draft-1 - Archive me.md"); } catch { sourceExists = false; }
const archivedContents = await readFile(root+"/backlog/archive/drafts/draft-1 - Archive me.md", "utf8");
process.stdout.write(JSON.stringify({archived, sourceExists, archivedContents}));
"#;
    let bun = std::env::var_os("BACKLOG_REFERENCE_BUN").unwrap_or_else(|| "bun".into());
    let output = Command::new(bun)
        .args(["--eval", SCRIPT])
        .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
        .env("BACKLOG_DRAFT_LIFECYCLE_ROOT", &ts_fixture.0)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(rust_result, expected);
}

#[cfg(unix)]
#[test]
fn archive_snapshots_symlink_content_without_moving_the_target() {
    let fixture = Fixture::new();
    let source_text = draft_source("DRAFT-7", "Linked");
    let target = fixture.file("external.md", &source_text);
    let source = fixture.0.join("backlog/drafts/draft-7 - Linked.md");
    std::os::unix::fs::symlink(&target, &source).unwrap();
    let store = DraftStore::new(&fixture.0, "backlog");
    assert!(store.archive_draft("7").unwrap());
    let archived = fixture.0.join("backlog/archive/drafts/draft-7 - Linked.md");
    assert!(
        !std::fs::symlink_metadata(&archived)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(std::fs::read_to_string(archived).unwrap(), source_text);
    assert_eq!(std::fs::read_to_string(target).unwrap(), source_text);
    assert!(!source.exists());
}

#[test]
fn promotes_draft_to_task_and_demotes_task_to_a_new_draft_id() {
    let fixture = Fixture::new();
    fixture.file(
        "backlog/drafts/draft-1 - Promote me.md",
        &draft_source("DRAFT-1", "Promote me"),
    );
    let config = Config {
        task_prefix: Some("task".into()),
        filesystem_only: Some(true),
        ..Config::default()
    };
    let tasks = TaskStore::new(&fixture.0, "backlog", "task");
    assert!(tasks.promote_draft("1", &config).unwrap());
    assert!(
        !fixture
            .0
            .join("backlog/drafts/draft-1 - Promote me.md")
            .exists()
    );
    let promoted_path = fixture.0.join("backlog/tasks/task-1 - Promote-me.md");
    let promoted_source = std::fs::read_to_string(&promoted_path).unwrap();
    let promoted = parse_task(&promoted_source).unwrap();
    assert_eq!(promoted.id, "TASK-1");
    assert_eq!(promoted.status, "To Do");
    assert!(promoted_source.contains("custom: keep"));
    assert!(promoted_source.contains("Draft body"));

    let demotion = tasks.demote_task("1", &config).unwrap();
    assert!(demotion.success);
    assert!(!promoted_path.exists());
    let draft_path = fixture.0.join("backlog/drafts/draft-1 - Promote-me.md");
    let demoted_source = std::fs::read_to_string(draft_path).unwrap();
    let demoted = parse_task(&demoted_source).unwrap();
    assert_eq!(demoted.id, "DRAFT-1");
    assert_eq!(demoted.status, "To Do");
    assert!(demoted_source.contains("custom: keep"));
    assert!(demoted_source.contains("Draft body"));
}

#[test]
fn archive_promotion_and_demotion_match_live_typescript() {
    let rust_fixture = Fixture::new();
    let ts_fixture = Fixture::new();
    for fixture in [&rust_fixture, &ts_fixture] {
        fixture.file("backlog/config.yml", "project_name: test\ndefault_status: To Do\nstatuses: [To Do, In Progress, Done]\ntask_prefix: task\nfilesystem_only: true\ncheck_active_branches: false\nremote_operations: false\n");
        fixture.file(
            "backlog/drafts/draft-1 - Archived.md",
            &draft_source("DRAFT-1", "Archived"),
        );
        fixture.file(
            "backlog/drafts/draft-2 - Promote me.md",
            &draft_source("DRAFT-2", "Promote me"),
        );
        fixture.file("backlog/drafts/draft-4 - Draft link.md", "---\nid: DRAFT-4\ntitle: Draft link\nstatus: Draft\ncreated_date: '2025-01-01'\ndependencies: [TASK-1]\nreferences: [task-1]\n---\nDraft body\n");
        fixture.file("backlog/tasks/task-1 - Demote me.md", "---\nid: TASK-1\ntitle: Demote me\nstatus: To Do\ncreated_date: '2025-01-01'\nupdated_date: '2025-01-02'\ndependencies: [TASK-1, TASK-99]\nreferences: [task-1, task-99]\n---\nTask body\n");
        fixture.file("backlog/tasks/task-3 - Task link.md", "---\nid: TASK-3\ntitle: Task link\nstatus: To Do\ncreated_date: '2025-01-01'\ndependencies: [TASK-1]\nreferences: [task-1]\n---\nLinked body\n");
    }

    let config = Config {
        task_prefix: Some("task".into()),
        filesystem_only: Some(true),
        check_active_branches: Some(false),
        remote_operations: Some(false),
        default_status: Some("To Do".into()),
        ..Config::default()
    };
    let drafts = DraftStore::new(&rust_fixture.0, "backlog");
    let tasks = TaskStore::new(&rust_fixture.0, "backlog", "task");
    let archived = drafts.archive_draft("1").unwrap();
    let promoted = tasks.promote_draft("2", &config).unwrap();
    let demoted = tasks.demote_task("1", &config).unwrap();
    let task3 = tasks.load_task("3").unwrap().unwrap().task;
    let task4 = tasks.load_task("4").unwrap().unwrap().task;
    let draft4 = drafts.load_draft("4").unwrap().unwrap().task;
    let draft5 = drafts.load_draft("5").unwrap().unwrap().task;
    let archived_source = std::fs::read_to_string(
        rust_fixture
            .0
            .join("backlog/archive/drafts/draft-1 - Archived.md"),
    )
    .unwrap();
    let rust_result = json!({
        "archived": archived,
        "archivedSource": archived_source,
        "promoted": promoted,
        "promotedId": task4.id,
        "promotedStatus": task4.status,
        "demoted": demoted.success,
        "demotedId": draft5.id,
        "demotedStatus": draft5.status,
        "demotedUpdatedDate": draft5.updated_date,
        "demotedDependencies": draft5.dependencies,
        "demotedReferences": draft5.references,
        "cleaned": {"task": task3.dependencies.is_empty() && task3.references.is_empty(), "draft": draft4.dependencies.is_empty() && draft4.references.is_empty()},
        "body": std::fs::read_to_string(rust_fixture.0.join("backlog/tasks/task-4 - Promote-me.md")).unwrap().contains("Draft body"),
    });

    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
import { readFile } from "node:fs/promises";
const root = process.env.BACKLOG_DRAFT_LIFECYCLE_ROOT;
const core = new Core(root);
await core.ensureConfigLoaded();
const archived = await core.archiveDraft("1", false);
const promoted = await core.promoteDraft("2", false);
const demoted = await core.demoteTask("1", false);
const task3 = await core.filesystem.loadTask("3");
const task4 = await core.filesystem.loadTask("4");
const draft4 = await core.filesystem.loadDraft("4");
const draft5 = await core.filesystem.loadDraft("5");
process.stdout.write(JSON.stringify({
  archived,
  archivedSource:await readFile(root+"/backlog/archive/drafts/draft-1 - Archived.md", "utf8"),
  promoted,
  promotedId:task4?.id,
  promotedStatus:task4?.status,
  demoted:demoted.success,
  demotedId:draft5?.id,
  demotedStatus:draft5?.status,
  demotedUpdatedDate:draft5?.updatedDate ?? null,
  demotedDependencies:draft5?.dependencies,
  demotedReferences:draft5?.references,
  cleaned:{task:task3?.dependencies?.length===0 && task3?.references?.length===0, draft:draft4?.dependencies?.length===0 && draft4?.references?.length===0},
  body:(await readFile(root+"/backlog/tasks/task-4 - Promote-me.md", "utf8")).includes("Draft body")
}));
"#;
    let bun = std::env::var_os("BACKLOG_REFERENCE_BUN").unwrap_or_else(|| "bun".into());
    let output = Command::new(bun)
        .args(["--eval", SCRIPT])
        .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
        .env("BACKLOG_DRAFT_LIFECYCLE_ROOT", &ts_fixture.0)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(rust_result, expected);
}

#[test]
fn missing_and_ambiguous_lifecycle_ids_match_live_typescript() {
    let fixture = Fixture::new();
    fixture.file("backlog/config.yml", "project_name: test\ndefault_status: To Do\nstatuses: [To Do, In Progress, Done]\ntask_prefix: task\nfilesystem_only: true\ncheck_active_branches: false\nremote_operations: false\n");
    fixture.file(
        "backlog/drafts/draft-1 - One.md",
        &draft_source("DRAFT-1", "One"),
    );
    fixture.file(
        "backlog/drafts/draft-01 - Twin.md",
        &draft_source("DRAFT-01", "Twin"),
    );
    fixture.file("backlog/archive/tasks/task-2 - Archived.md", "---\nid: TASK-2\ntitle: Archived\nstatus: Done\ncreated_date: '2025-01-01'\n---\nArchived body\n");
    let config = Config {
        task_prefix: Some("task".into()),
        filesystem_only: Some(true),
        check_active_branches: Some(false),
        remote_operations: Some(false),
        ..Config::default()
    };
    let drafts = DraftStore::new(&fixture.0, "backlog");
    let tasks = TaskStore::new(&fixture.0, "backlog", "task");
    let rust_result = json!({
        "missingArchive": drafts.archive_draft("99").unwrap(),
        "missingPromotion": tasks.promote_draft("99", &config).unwrap(),
        "missingDemotion": tasks.demote_task("99", &config).unwrap().success,
        "ambiguousArchive": drafts.archive_draft("1").is_err(),
        "ambiguousPromotion": tasks.promote_draft("1", &config).is_err(),
        "archivedDemotion": tasks.demote_task("2", &config).is_err(),
    });

    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
const core = new Core(process.env.BACKLOG_DRAFT_LIFECYCLE_ROOT);
await core.ensureConfigLoaded();
let ambiguousArchive = false;
let ambiguousPromotion = false;
try { await core.archiveDraft("1", false); } catch { ambiguousArchive = true; }
try { await core.promoteDraft("1", false); } catch { ambiguousPromotion = true; }
const missingArchive = await core.archiveDraft("99", false);
const missingPromotion = await core.promoteDraft("99", false);
const missingDemotion = await core.demoteTask("99", false);
let archivedDemotion = false;
try { await core.demoteTask("2", false); } catch { archivedDemotion = true; }
process.stdout.write(JSON.stringify({missingArchive, missingPromotion, missingDemotion:missingDemotion.success, ambiguousArchive, ambiguousPromotion, archivedDemotion}));
"#;
    let bun = std::env::var_os("BACKLOG_REFERENCE_BUN").unwrap_or_else(|| "bun".into());
    let output = Command::new(bun)
        .args(["--eval", SCRIPT])
        .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
        .env("BACKLOG_DRAFT_LIFECYCLE_ROOT", &fixture.0)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(rust_result, expected);
}

#[test]
fn promotion_and_demotion_destination_collisions_match_live_typescript() {
    let rust_fixture = Fixture::new();
    let ts_fixture = Fixture::new();
    for fixture in [&rust_fixture, &ts_fixture] {
        fixture.file("backlog/config.yml", "project_name: test\ndefault_status: To Do\nstatuses: [To Do, In Progress, Done]\ntask_prefix: task\nfilesystem_only: true\ncheck_active_branches: false\nremote_operations: false\n");
        fixture.file(
            "backlog/drafts/draft-1 - Promote me.md",
            &draft_source("DRAFT-1", "Promote me"),
        );
        fixture.file("backlog/tasks/task-2 - Demote me.md", "---\nid: TASK-2\ntitle: Demote me\nstatus: To Do\ncreated_date: '2025-01-01'\n---\nTask body\n");
        std::fs::create_dir_all(fixture.0.join("backlog/tasks/task-1 - Promote-me.md")).unwrap();
        std::fs::create_dir_all(fixture.0.join("backlog/drafts/draft-1 - Demote-me.md")).unwrap();
    }
    let config = Config {
        task_prefix: Some("task".into()),
        filesystem_only: Some(true),
        check_active_branches: Some(false),
        remote_operations: Some(false),
        ..Config::default()
    };
    let tasks = TaskStore::new(&rust_fixture.0, "backlog", "task");
    let promotion_failed = tasks.promote_draft("1", &config).is_err();
    let demotion_failed = tasks.demote_task("2", &config).is_err();
    let rust_result = json!({
        "promotionFailed": promotion_failed,
        "promotionSourceExists": rust_fixture.0.join("backlog/drafts/draft-1 - Promote me.md").exists(),
        "promotionDestinationIsDirectory": rust_fixture.0.join("backlog/tasks/task-1 - Promote-me.md").is_dir(),
        "demotionFailed": demotion_failed,
        "demotionSourceExists": rust_fixture.0.join("backlog/tasks/task-2 - Demote me.md").exists(),
        "demotionDestinationIsDirectory": rust_fixture.0.join("backlog/drafts/draft-1 - Demote-me.md").is_dir(),
    });

    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
import { mkdir, stat } from "node:fs/promises";
const root = process.env.BACKLOG_DRAFT_LIFECYCLE_ROOT;
const core = new Core(root);
await core.ensureConfigLoaded();
let promotionFailed = false;
let demotionFailed = false;
try { await core.promoteDraft("1", false); } catch { promotionFailed = true; }
try { await core.demoteTask("2", false); } catch { demotionFailed = true; }
const exists = async path => { try { await stat(path); return true; } catch { return false; } };
process.stdout.write(JSON.stringify({
  promotionFailed,
  promotionSourceExists:await exists(root+"/backlog/drafts/draft-1 - Promote me.md"),
  promotionDestinationIsDirectory:(await stat(root+"/backlog/tasks/task-1 - Promote-me.md")).isDirectory(),
  demotionFailed,
  demotionSourceExists:await exists(root+"/backlog/tasks/task-2 - Demote me.md"),
  demotionDestinationIsDirectory:(await stat(root+"/backlog/drafts/draft-1 - Demote-me.md")).isDirectory()
}));
"#;
    let bun = std::env::var_os("BACKLOG_REFERENCE_BUN").unwrap_or_else(|| "bun".into());
    let output = Command::new(bun)
        .args(["--eval", SCRIPT])
        .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
        .env("BACKLOG_DRAFT_LIFECYCLE_ROOT", &ts_fixture.0)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(rust_result, expected);
}
