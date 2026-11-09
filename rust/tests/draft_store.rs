use backlog_md::draft_store::DraftStore;
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
            "backlog-draft-store-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("backlog/drafts")).unwrap();
        Self(root)
    }

    fn file(&self, name: &str, id: &str, title: &str) {
        std::fs::write(self.0.join("backlog/drafts").join(name), format!("---\nid: {id}\ntitle: {title}\nstatus: Draft\ncreated_date: '2025-01-01'\n---\nBody\n")).unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn lists_drafts_in_numeric_id_order_and_resolves_filename_identity() {
    let fixture = Fixture::new();
    fixture.file("draft-10 - Ten.md", "DRAFT-10", "Ten");
    fixture.file("draft-2 - Two.md", "DRAFT-2", "Two");
    let store = DraftStore::new(&fixture.0, "backlog");
    let drafts = store.list_drafts();
    assert_eq!(
        drafts
            .iter()
            .map(|entry| entry.task.id.as_str())
            .collect::<Vec<_>>(),
        ["DRAFT-2", "DRAFT-10"]
    );
    assert_eq!(store.load_draft("2").unwrap().unwrap().task.title, "Two");
    assert!(store.load_draft("99").unwrap().is_none());

    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
const core = new Core(process.env.BACKLOG_DRAFT_ROOT);
const drafts = await core.filesystem.listDrafts();
const found = await core.filesystem.loadDraft("2");
process.stdout.write(JSON.stringify({ids:drafts.map(draft=>draft.id), titles:drafts.map(draft=>draft.title), found:found?.title ?? null}));
"#;
    let bun = std::env::var_os("BACKLOG_REFERENCE_BUN").unwrap_or_else(|| "bun".into());
    let output = Command::new(bun)
        .args(["--eval", SCRIPT])
        .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
        .env("BACKLOG_DRAFT_ROOT", &fixture.0)
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
    assert_eq!(
        expected,
        serde_json::json!({"ids":["DRAFT-2", "DRAFT-10"], "titles":["Two", "Ten"], "found":"Two"})
    );
}

#[test]
fn creates_with_padding_and_updates_without_losing_unmanaged_content() {
    let fixture = Fixture::new();
    let reference_fixture = Fixture::new();
    std::fs::write(
        fixture.0.join("backlog/drafts/draft-002 - Broken.md"),
        "---\nid: [\n---\n",
    )
    .unwrap();
    fixture.file("draft-004 - Drifted.md", "DRAFT-006", "Drifted");
    std::fs::write(
        fixture.0.join("backlog/config.yml"),
        "project_name: test\nstatuses: [To Do, In Progress, Done]\nzero_padded_ids: 3\n",
    )
    .unwrap();
    std::fs::write(
        reference_fixture
            .0
            .join("backlog/drafts/draft-002 - Broken.md"),
        "---\nid: [\n---\n",
    )
    .unwrap();
    reference_fixture.file("draft-004 - Drifted.md", "DRAFT-006", "Drifted");
    std::fs::write(
        reference_fixture.0.join("backlog/config.yml"),
        "project_name: test\nstatuses: [To Do, In Progress, Done]\nzero_padded_ids: 3\n",
    )
    .unwrap();
    let store = DraftStore::new(&fixture.0, "backlog");
    let config = backlog_md::config::Config {
        zero_padded_ids: Some(3.0),
        ..backlog_md::config::Config::default()
    };
    let mut task = backlog_md::records::parse_task("---\nid: DRAFT-0\ntitle: First title\nstatus: To Do\ncreated_date: '2025-01-01'\ncustom: retained\n---\nOriginal body\n").unwrap();
    let created = store.create_draft(task.clone(), &config).unwrap();
    assert_eq!(created.task.id, "DRAFT-007");
    assert_eq!(created.task.status, "Draft");
    std::fs::write(&created.path, "---\nid: DRAFT-007\ntitle: First title\nstatus: Draft\ncreated_date: '2025-01-01'\ncustom: retained\n---\nOriginal body\n").unwrap();
    task = store.load_draft("7").unwrap().unwrap().task;
    task.title = "Renamed title".into();
    task.status = "Done".into();
    let updated = store.update_draft(task).unwrap().unwrap();
    assert_eq!(updated.task.status, "Draft");
    assert!(updated.path.ends_with("draft-007 - Renamed-title.md"));
    let source = std::fs::read_to_string(&updated.path).unwrap();
    assert!(source.contains("custom: retained"));
    assert!(source.contains("Original body"));
    assert!(
        !fixture
            .0
            .join("backlog/drafts/draft-007 - First-title.md")
            .exists()
    );

    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
import { readFile, writeFile } from "node:fs/promises";
import { join, basename } from "node:path";
const root = process.env.BACKLOG_DRAFT_ROOT;
const core = new Core(root);
await core.ensureConfigLoaded();
const {task, filePath} = await core.createTaskFromInput({title:"First title", status:"Draft"});
await writeFile(filePath, `---\nid: ${task.id}\ntitle: First title\nstatus: Draft\ncreated_date: '2025-01-01'\ncustom: retained\n---\nOriginal body\n`);
const draft = await core.filesystem.loadDraft(task.id);
draft.title = "Renamed title";
draft.status = "Done";
const updatedPath = await core.updateDraft(draft, false);
const source = await readFile(updatedPath, "utf8");
process.stdout.write(JSON.stringify({id:task.id, filename:basename(updatedPath), status:(await core.filesystem.loadDraft(task.id)).status, custom:source.includes("custom: retained"), body:source.includes("Original body")}));
"#;
    let bun = std::env::var_os("BACKLOG_REFERENCE_BUN").unwrap_or_else(|| "bun".into());
    let output = Command::new(bun)
        .args(["--eval", SCRIPT])
        .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
        .env("BACKLOG_DRAFT_ROOT", &reference_fixture.0)
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
    assert_eq!(expected["id"], created.task.id);
    assert_eq!(
        expected["filename"],
        updated.path.file_name().unwrap().to_string_lossy().as_ref()
    );
    assert_eq!(expected["status"], "Draft");
    assert_eq!(expected["custom"], true);
    assert_eq!(expected["body"], true);
}

#[test]
fn duplicate_filename_ids_fail_closed_and_drifted_frontmatter_matches_live_typescript() {
    let fixture = Fixture::new();
    fixture.file("draft-01 - First.md", "DRAFT-01", "First");
    fixture.file("draft-1 - Duplicate.md", "DRAFT-1", "Duplicate");
    let store = DraftStore::new(&fixture.0, "backlog");
    assert!(store.load_draft("1").is_err());

    fixture.file("draft-2 - Drift.md", "DRAFT-3", "Drift");
    let drifted = store.load_draft("2").unwrap().unwrap();
    assert_eq!(drifted.task.id, "DRAFT-3");
    let mut attempted_edit = drifted.task;
    attempted_edit.id = "DRAFT-2".into();
    assert!(store.update_draft(attempted_edit).is_err());

    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
const core = new Core(process.env.BACKLOG_DRAFT_ROOT);
let ambiguous = false;
try { await core.filesystem.loadDraft("1"); } catch { ambiguous = true; }
const drifted = await core.filesystem.loadDraft("2");
const missing = await core.filesystem.loadDraft("99");
process.stdout.write(JSON.stringify({ambiguous, drifted:drifted?.id ?? null, missing:missing?.id ?? null}));
"#;
    let bun = std::env::var_os("BACKLOG_REFERENCE_BUN").unwrap_or_else(|| "bun".into());
    let output = Command::new(bun)
        .args(["--eval", SCRIPT])
        .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
        .env("BACKLOG_DRAFT_ROOT", &fixture.0)
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
    assert_eq!(
        expected,
        serde_json::json!({"ambiguous":true, "drifted":"DRAFT-3", "missing":null})
    );
}
