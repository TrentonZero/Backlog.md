use backlog_md::{
    config::Config, document_store::DocumentStore, records::DocumentRecord, storage::Storage,
};
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
            "backlog-document-lifecycle-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("backlog/docs")).unwrap();
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

fn document(id: &str, title: &str) -> DocumentRecord {
    DocumentRecord {
        id: id.into(),
        title: title.into(),
        document_type: "guide".into(),
        created_date: "2025-01-01".into(),
        updated_date: None,
        raw_content: "Body\n".into(),
        tags: None,
    }
}

fn reference_output(script: &str, root: &PathBuf) -> Value {
    let output =
        Command::new(std::env::var_os("BACKLOG_REFERENCE_BUN").unwrap_or_else(|| "bun".into()))
            .args(["--eval", script])
            .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
            .env("BACKLOG_DOCUMENT_ROOT", root)
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

fn lifecycle_fixture() -> (Fixture, String) {
    let fixture = Fixture::new();
    let source = "---\nid: DOC-1\ntitle: Field Guide\ntype: guide\ncreated_date: '2025-01-01'\ncustom: retained\n---\nOriginal body\n";
    fixture.write("backlog/docs/guides/doc-1 - Field-Guide.md", source);
    (fixture, source.into())
}

#[test]
fn archives_and_restores_documents_without_overwriting_or_rewriting_source() {
    let (fixture, source) = lifecycle_fixture();
    let reference = Fixture::new();
    reference.write("backlog/docs/guides/doc-1 - Field-Guide.md", &source);
    let store = DocumentStore::new(&fixture.0, "backlog", "task");
    let blocker = Storage::new(fixture.0.join("backlog"), None)
        .acquire_entity_lock(
            "document",
            "doc-1",
            &fixture.0.join("backlog/docs/guides/doc-1 - Field-Guide.md"),
        )
        .unwrap();
    assert!(store.archive_document("1").is_err());
    drop(blocker);
    let archived = store.archive_document("1").unwrap().unwrap();
    assert_eq!(archived.id, "DOC-1");
    assert_eq!(
        std::fs::read_to_string(
            fixture
                .0
                .join("backlog/archive/docs/guides/doc-1 - Field-Guide.md")
        )
        .unwrap(),
        source
    );
    assert!(
        !fixture
            .0
            .join("backlog/docs/guides/doc-1 - Field-Guide.md")
            .exists()
    );
    let archive_again = store.archive_document("1").unwrap_err().to_string();

    fixture.write(
        "backlog/docs/guides/doc-1 - Field-Guide.md",
        "---\nid: doc-9\ntitle: Collision\n---\nDo not replace\n",
    );
    let restore_collision = store.restore_document("1").unwrap_err().to_string();
    assert_eq!(
        std::fs::read_to_string(
            fixture
                .0
                .join("backlog/archive/docs/guides/doc-1 - Field-Guide.md")
        )
        .unwrap(),
        source
    );
    std::fs::remove_file(fixture.0.join("backlog/docs/guides/doc-1 - Field-Guide.md")).unwrap();
    let restored = store.restore_document("doc-001").unwrap().unwrap();
    assert_eq!(restored.id, "DOC-1");
    assert_eq!(
        std::fs::read_to_string(fixture.0.join("backlog/docs/guides/doc-1 - Field-Guide.md"))
            .unwrap(),
        source
    );
    let restore_again = store.restore_document("1").unwrap_err().to_string();
    let delete_active = store.delete_document("1").unwrap_err().to_string();
    assert!(store.archive_document("404").unwrap().is_none());

    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
import { readFile, writeFile, mkdir } from "node:fs/promises";
const root = process.env.BACKLOG_DOCUMENT_ROOT;
const core = new Core(root);
const source = await readFile(root+"/backlog/docs/guides/doc-1 - Field-Guide.md", "utf8");
const archived = await core.archiveContentRecord("document", "1", false);
let archiveAgain = "";
try { await core.archiveContentRecord("document", "1", false); } catch (error) { archiveAgain = error.message; }
await mkdir(root+"/backlog/docs/guides", {recursive:true});
await writeFile(root+"/backlog/docs/guides/doc-1 - Field-Guide.md", "---\\nid: doc-9\\ntitle: Collision\\n---\\nDo not replace\\n");
let restoreCollision = "";
try { await core.restoreContentRecord("document", "1", false); } catch (error) { restoreCollision = error.message; }
await (await import("node:fs/promises")).rm(root+"/backlog/docs/guides/doc-1 - Field-Guide.md");
const restored = await core.restoreContentRecord("document", "doc-001", false);
let restoreAgain = "";
let deleteActive = "";
try { await core.restoreContentRecord("document", "1", false); } catch (error) { restoreAgain = error.message; }
try { await core.deleteContentRecord("document", "1", false); } catch (error) { deleteActive = error.message; }
const missing = await core.archiveContentRecord("document", "404", false);
process.stdout.write(JSON.stringify({archived:archived.id, archiveAgain, restoreCollision, restored:restored.id, restoreAgain, deleteActive, missing:missing===null, source:await readFile(root+"/backlog/docs/guides/doc-1 - Field-Guide.md", "utf8")}));
"#;
    let mut expected = reference_output(SCRIPT, &reference.0);
    expected["source"] = serde_json::json!(source);
    assert_eq!(expected["archived"], "DOC-1");
    assert_eq!(expected["restored"], "DOC-1");
    assert_eq!(expected["archiveAgain"], archive_again);
    assert_eq!(expected["restoreCollision"], restore_collision);
    assert_eq!(expected["restoreAgain"], restore_again);
    assert_eq!(expected["deleteActive"], delete_active);
    assert_eq!(expected["missing"], true);
    assert_eq!(expected["source"], source);
}

#[cfg(unix)]
#[test]
fn deleting_an_archived_symlink_reports_backlinks_and_keeps_owners_unchanged() {
    let fixture = Fixture::new();
    let reference = Fixture::new();
    let source = "---\nid: doc-1\ntitle: Guide\n---\nOutside source\n";
    let rust_target = fixture.write("outside/guide.md", source);
    let reference_target = reference.write("outside/guide.md", source);
    let archived_path = fixture
        .0
        .join("backlog/archive/docs/guides/doc-1 - Guide.md");
    let reference_archived_path = reference
        .0
        .join("backlog/archive/docs/guides/doc-1 - Guide.md");
    std::fs::create_dir_all(archived_path.parent().unwrap()).unwrap();
    std::fs::create_dir_all(reference_archived_path.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&rust_target, &archived_path).unwrap();
    std::os::unix::fs::symlink(&reference_target, &reference_archived_path).unwrap();

    let task_active = "---\nid: TASK-1\ntitle: Active reference\nstatus: To Do\ncreated_date: '2025-01-01'\nlabels: []\ndependencies: []\ndocumentation:\n  - \"backlog/archive/docs/guides/doc-1 - Guide.md#section\"\n---\n";
    let task_archived = "---\nid: TASK-2\ntitle: Archived reference\nstatus: Done\ncreated_date: '2025-01-01'\nlabels: []\ndependencies: []\ndocumentation:\n  - \"./backlog/docs/guides/doc-1 - Guide.md\"\n---\n";
    let draft = "---\nid: DRAFT-1\ntitle: Draft reference\nstatus: Draft\ncreated_date: '2025-01-01'\nlabels: []\ndependencies: []\ndocumentation:\n  - \"backlog/archive/docs/guides/doc-1 - Guide.md\"\n---\n";
    for root in [&fixture, &reference] {
        root.write("backlog/tasks/task-1 - Active.md", task_active);
        root.write("backlog/archive/tasks/task-2 - Archived.md", task_archived);
        root.write("backlog/drafts/draft-1 - Draft.md", draft);
    }
    let owners_before = [task_active, task_archived, draft];
    let store = DocumentStore::new(&fixture.0, "backlog", "TASK");
    let removed = store.delete_document("DOC-01").unwrap().unwrap();
    assert_eq!(removed.id, "doc-1");
    assert_eq!(
        removed.listed_as_documentation_by,
        ["TASK-1", "TASK-2", "DRAFT-1"]
    );
    assert!(!archived_path.exists());
    assert_eq!(std::fs::read_to_string(rust_target).unwrap(), source);
    for (path, expected) in [
        ("backlog/tasks/task-1 - Active.md", owners_before[0]),
        (
            "backlog/archive/tasks/task-2 - Archived.md",
            owners_before[1],
        ),
        ("backlog/drafts/draft-1 - Draft.md", owners_before[2]),
    ] {
        assert_eq!(
            std::fs::read_to_string(fixture.0.join(path)).unwrap(),
            expected
        );
    }

    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
import { readFile } from "node:fs/promises";
const root = process.env.BACKLOG_DOCUMENT_ROOT;
const core = new Core(root);
const result = await core.deleteContentRecord("document", "DOC-01", false);
process.stdout.write(JSON.stringify({id:result.id, listed:result.listedAsDocumentationBy, linkExists:await Bun.file(root+"/backlog/archive/docs/guides/doc-1 - Guide.md").exists(), target:await readFile(root+"/outside/guide.md", "utf8"), task:await readFile(root+"/backlog/tasks/task-1 - Active.md", "utf8")}));
"#;
    let mut expected = reference_output(SCRIPT, &reference.0);
    expected["target"] = serde_json::json!(source);
    expected["task"] = serde_json::json!(task_active);
    assert_eq!(expected["id"], "doc-1");
    assert_eq!(
        expected["listed"],
        serde_json::json!(["TASK-1", "TASK-2", "DRAFT-1"])
    );
    assert_eq!(expected["linkExists"], false);
    assert_eq!(expected["target"], source);
    assert_eq!(expected["task"], task_active);
}

#[cfg(unix)]
#[test]
fn archive_and_restore_preserve_relative_document_symlinks_like_typescript() {
    let fixture = Fixture::new();
    let reference = Fixture::new();
    let source = "---\nid: doc-1\ntitle: Linked guide\n---\nLinked body\n";
    fixture.write("outside/guide.md", source);
    reference.write("outside/guide.md", source);
    let source_relative = "../../../outside/guide.md";
    for root in [&fixture, &reference] {
        let link = root.0.join("backlog/docs/guides/doc-1 - Linked-guide.md");
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(source_relative, link).unwrap();
    }
    let store = DocumentStore::new(&fixture.0, "backlog", "task");
    store.archive_document("1").unwrap().unwrap();
    let archived = fixture
        .0
        .join("backlog/archive/docs/guides/doc-1 - Linked-guide.md");
    let rust_archive_link = std::fs::read_link(&archived).unwrap();
    let rust_archive_content = std::fs::read_to_string(&archived).unwrap();
    store.restore_document("1").unwrap().unwrap();
    let restored = fixture
        .0
        .join("backlog/docs/guides/doc-1 - Linked-guide.md");
    let rust_restored_link = std::fs::read_link(&restored).unwrap();
    let rust_restored_content = std::fs::read_to_string(&restored).unwrap();

    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
import { readFile, readlink } from "node:fs/promises";
const root = process.env.BACKLOG_DOCUMENT_ROOT;
const core = new Core(root);
await core.archiveContentRecord("document", "1", false);
const archived = root+"/backlog/archive/docs/guides/doc-1 - Linked-guide.md";
const archiveLink = await readlink(archived);
const archiveContent = await readFile(archived, "utf8");
await core.restoreContentRecord("document", "1", false);
const restored = root+"/backlog/docs/guides/doc-1 - Linked-guide.md";
process.stdout.write(JSON.stringify({archiveLink, archiveContent, restoredLink:await readlink(restored), restoredContent:await readFile(restored, "utf8")}));
"#;
    let expected = reference_output(SCRIPT, &reference.0);
    assert_eq!(
        expected["archiveLink"],
        rust_archive_link.to_string_lossy().as_ref()
    );
    assert_eq!(expected["archiveContent"], rust_archive_content);
    assert_eq!(
        expected["restoredLink"],
        rust_restored_link.to_string_lossy().as_ref()
    );
    assert_eq!(expected["restoredContent"], rust_restored_content);
    assert_eq!(rust_restored_link, PathBuf::from(source_relative));
    assert_eq!(rust_restored_content, source);
}

#[test]
fn archive_restore_and_delete_fail_closed_on_duplicate_active_and_archived_ids() {
    let fixture = Fixture::new();
    let reference = Fixture::new();
    for root in [&fixture, &reference] {
        root.write(
            "backlog/docs/doc-1 - Active.md",
            "---\nid: doc-1\ntitle: Active\n---\nActive body\n",
        );
        root.write(
            "backlog/archive/docs/doc-9 - Archived.md",
            "---\nid: DOC-001\ntitle: Archived\n---\nArchived body\n",
        );
    }
    let store = DocumentStore::new(&fixture.0, "backlog", "task");
    let rust_archive = store.archive_document("1").unwrap_err().to_string();
    let rust_restore = store.restore_document("1").unwrap_err().to_string();
    let rust_delete = store.delete_document("1").unwrap_err().to_string();
    let rust_update = store
        .update_document(document("1", "Changed"))
        .unwrap_err()
        .to_string();
    assert!(store.archive_document("404").unwrap().is_none());
    assert!(store.restore_document("404").unwrap().is_none());
    assert!(store.delete_document("404").unwrap().is_none());

    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
const core = new Core(process.env.BACKLOG_DOCUMENT_ROOT);
const attempt = async operation => { try { await operation(); return ""; } catch (error) { return error.message; } };
const archive = await attempt(()=>core.archiveContentRecord("document", "1", false));
const restore = await attempt(()=>core.restoreContentRecord("document", "1", false));
const remove = await attempt(()=>core.deleteContentRecord("document", "1", false));
const update = await attempt(()=>core.updateDocumentFromInput({id:"1", title:"Changed"}, false));
process.stdout.write(JSON.stringify({archive, restore, remove, update}));
"#;
    let expected = reference_output(SCRIPT, &reference.0);
    assert_eq!(expected["archive"], rust_archive);
    assert_eq!(expected["restore"], rust_restore);
    assert_eq!(expected["remove"], rust_delete);
    assert_eq!(expected["update"], rust_update);
}

#[test]
fn document_ids_stay_reserved_while_archived_then_follow_remaining_filename_claims() {
    let archived = Fixture::new();
    archived.write(
        "backlog/archive/docs/doc-1 - Archived.md",
        "---\nid: doc-1\ntitle: Archived\n---\nBody\n",
    );
    let archived_store = DocumentStore::new(&archived.0, "backlog", "task");
    let while_archived = archived_store
        .create_document(document("doc-0", "Created"), &Config::default(), "")
        .unwrap();
    assert_eq!(while_archived.document.id, "doc-2");
    std::fs::remove_file(while_archived.path).unwrap();
    archived_store.delete_document("1").unwrap().unwrap();
    let after_delete = archived_store
        .create_document(document("doc-0", "Created again"), &Config::default(), "")
        .unwrap();
    assert_eq!(after_delete.document.id, "doc-1");

    let claimed = Fixture::new();
    claimed.write(
        "backlog/archive/docs/doc-1 - Archived.md",
        "---\nid: doc-1\ntitle: Archived\n---\nBody\n",
    );
    claimed.write(
        "backlog/docs/doc-1 - Drifted.md",
        "---\nid: doc-guide\ntitle: Drifted\n---\nBody\n",
    );
    let claimed_store = DocumentStore::new(&claimed.0, "backlog", "task");
    claimed_store.delete_document("1").unwrap().unwrap();
    let still_reserved = claimed_store
        .create_document(document("doc-0", "Next"), &Config::default(), "")
        .unwrap();
    assert_eq!(still_reserved.document.id, "doc-2");

    let reference = Fixture::new();
    reference.write(
        "backlog/archive/docs/doc-1 - Archived.md",
        "---\nid: doc-1\ntitle: Archived\n---\nBody\n",
    );
    let expected = reference_output(
        r#"
import { Core } from "./src/core/backlog.ts";
import { unlink } from "node:fs/promises";
const core = new Core(process.env.BACKLOG_DOCUMENT_ROOT);
const before = await core.createDocumentFromInput({title:"Created", content:"Body"}, false);
await unlink(core.filesystem.contentRecordFilePath("document", before));
await core.deleteContentRecord("document", "1", false);
const after = await core.createDocumentFromInput({title:"Created again", content:"Body"}, false);
process.stdout.write(JSON.stringify({before:before.id, after:after.id}));
"#,
        &reference.0,
    );
    assert_eq!(
        expected,
        serde_json::json!({"before":"doc-2", "after":"doc-1"})
    );

    let reference_claimed = Fixture::new();
    reference_claimed.write(
        "backlog/archive/docs/doc-1 - Archived.md",
        "---\nid: doc-1\ntitle: Archived\n---\nBody\n",
    );
    reference_claimed.write(
        "backlog/docs/doc-1 - Drifted.md",
        "---\nid: doc-guide\ntitle: Drifted\n---\nBody\n",
    );
    let expected = reference_output(
        r#"
import { Core } from "./src/core/backlog.ts";
const core = new Core(process.env.BACKLOG_DOCUMENT_ROOT);
await core.deleteContentRecord("document", "1", false);
const created = await core.createDocumentFromInput({title:"Next", content:"Body"}, false);
process.stdout.write(JSON.stringify({id:created.id}));
"#,
        &reference_claimed.0,
    );
    assert_eq!(expected, serde_json::json!({"id":"doc-2"}));
}

#[test]
fn archived_documents_keep_the_same_read_only_update_error_as_typescript() {
    let fixture = Fixture::new();
    let reference = Fixture::new();
    let source = "---\nid: doc-1\ntitle: Archived\n---\nBody\n";
    fixture.write("backlog/archive/docs/doc-1 - Archived.md", source);
    reference.write("backlog/archive/docs/doc-1 - Archived.md", source);
    let store = DocumentStore::new(&fixture.0, "backlog", "task");
    let rust_error = store
        .update_document(document("1", "Changed"))
        .unwrap_err()
        .to_string();

    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
const core = new Core(process.env.BACKLOG_DOCUMENT_ROOT);
let error = "";
try { await core.updateDocumentFromInput({id:"1", title:"Changed"}, false); } catch (caught) { error = caught.message; }
process.stdout.write(JSON.stringify({error}));
"#;
    let expected = reference_output(SCRIPT, &reference.0);
    assert_eq!(expected["error"], rust_error);
}
