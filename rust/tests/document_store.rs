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
            "backlog-document-store-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("backlog/docs")).unwrap();
        Self(root)
    }

    fn file(&self, path: &str, source: &str) {
        let path = self.0.join("backlog/docs").join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, source).unwrap();
    }

    fn archived_file(&self, path: &str, source: &str) {
        let path = self.0.join("backlog/archive/docs").join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, source).unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn document(id: &str, title: &str, content: &str) -> DocumentRecord {
    DocumentRecord {
        id: id.into(),
        title: title.into(),
        document_type: "other".into(),
        created_date: "2025-01-01".into(),
        updated_date: None,
        raw_content: content.into(),
        tags: None,
    }
}

#[test]
fn lists_and_resolves_documents_like_typescript() {
    let fixture = Fixture::new();
    fixture.file(
        "doc-10 - Alpha.md",
        "---\nid: DOC-10\ntitle: Alpha\n---\nA\n",
    );
    fixture.file(
        "nested/doc-2 - Beta.md",
        "---\nid: doc-2\ntitle: Beta\n---\nB\n",
    );
    fixture.file("doc-4 - alpha.md", "---\nid: doc-4\ntitle: alpha\n---\na\n");
    fixture.file(
        "A-folder/doc-5 - Same.md",
        "---\nid: doc-6\ntitle: Same\n---\nA\n",
    );
    fixture.file(
        "a-folder/doc-5 - Same.md",
        "---\nid: doc-5\ntitle: Same\n---\na\n",
    );
    fixture.file("README.md", "---\nid: doc-3\ntitle: README\n---\nIgnore\n");
    fixture.file("broken.md", "---\nid: [\n---\n");
    let store = DocumentStore::new(&fixture.0, "backlog");
    let documents = store.list_documents();
    assert_eq!(
        documents
            .iter()
            .map(|entry| entry.document.title.as_str())
            .collect::<Vec<_>>(),
        ["alpha", "Alpha", "Beta", "Same", "Same"]
    );
    let same_paths: Vec<_> = documents
        .iter()
        .filter(|entry| entry.document.title == "Same")
        .map(|entry| {
            entry
                .path
                .strip_prefix(fixture.0.join("backlog/docs"))
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(
        same_paths,
        ["a-folder/doc-5 - Same.md", "A-folder/doc-5 - Same.md"]
    );
    assert_eq!(store.load_document("2").unwrap().document.title, "Beta");
    assert_eq!(
        store.load_document("99").unwrap_err().to_string(),
        "Document not found: 99"
    );

    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
const core = new Core(process.env.BACKLOG_DOCUMENT_ROOT);
const docs = await core.filesystem.listDocuments();
const found = await core.filesystem.loadDocument("2");
let missing = null;
try { missing = (await core.filesystem.loadDocument("99")).title; } catch {}
process.stdout.write(JSON.stringify({titles:docs.map(doc=>doc.title), samePaths:docs.filter(doc=>doc.title==="Same").map(doc=>doc.path), found:found.title, missing}));
"#;
    let output =
        Command::new(std::env::var_os("BACKLOG_REFERENCE_BUN").unwrap_or_else(|| "bun".into()))
            .args(["--eval", SCRIPT])
            .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
            .env("BACKLOG_DOCUMENT_ROOT", &fixture.0)
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
        serde_json::json!({"titles":["alpha", "Alpha", "Beta", "Same", "Same"], "samePaths":["a-folder/doc-5 - Same.md", "A-folder/doc-5 - Same.md"], "found":"Beta", "missing":null})
    );
}

#[test]
fn duplicate_identity_fails_and_create_update_preserve_source() {
    let fixture = Fixture::new();
    fixture.file("doc-001 - Broken.md", "---\nid: [\n---\n");
    fixture.file("doc-004 - Existing.md", "---\nid: DOC-006\ntitle: Existing\ntype: guide\ncreated_date: '2025-01-01'\ncustom: keep\n---\nOriginal body\n");
    fixture.file("guides/doc-007 - New-doc.md", "Existing collision bytes\n");
    fixture.file("README.md", "---\nid: doc-100\ntitle: Instructions\n---\n");
    std::fs::write(
        fixture.0.join("backlog/config.yml"),
        "project_name: test\nzero_padded_ids: 3\n",
    )
    .unwrap();
    let store = DocumentStore::new(&fixture.0, "backlog");
    let config = Config {
        zero_padded_ids: Some(3.0),
        ..Config::default()
    };
    let created = store
        .create_document(document("doc-0", "New doc", "Created\n"), &config, "guides")
        .unwrap();
    assert_eq!(created.document.id, "doc-008");
    assert_eq!(
        std::fs::read_to_string(fixture.0.join("backlog/docs/guides/doc-007 - New-doc.md"))
            .unwrap(),
        "Existing collision bytes\n"
    );
    let mut updated = store.load_document("6").unwrap().document;
    updated.title = "Renamed doc".into();
    updated.raw_content = "Changed\n".into();
    let blocker = Storage::new(fixture.0.join("backlog"), None)
        .acquire_entity_lock(
            "document",
            "doc-6",
            &fixture.0.join("backlog/docs/doc-004 - Existing.md"),
        )
        .unwrap();
    assert!(store.update_document(updated.clone()).is_err());
    drop(blocker);
    let saved = store.update_document(updated).unwrap();
    assert_eq!(saved.document.id, "doc-006");
    assert!(saved.path.ends_with("doc-006 - Renamed-doc.md"));
    let saved_source = std::fs::read_to_string(&saved.path).unwrap();
    assert!(saved_source.contains("custom: keep"));
    assert!(saved_source.contains("Changed"));

    fixture.file(
        "doc-8 - Duplicate.md",
        "---\nid: DOC-006\ntitle: Duplicate\n---\n",
    );
    let rust_ambiguity = store.load_document("6").unwrap_err().to_string();

    let reference = Fixture::new();
    reference.file("doc-001 - Broken.md", "---\nid: [\n---\n");
    reference.file("doc-004 - Existing.md", "---\nid: DOC-006\ntitle: Existing\ntype: guide\ncreated_date: '2025-01-01'\ncustom: keep\n---\nOriginal body\n");
    reference.file("guides/doc-007 - New-doc.md", "Existing collision bytes\n");
    reference.file("README.md", "---\nid: doc-100\ntitle: Instructions\n---\n");
    std::fs::write(
        reference.0.join("backlog/config.yml"),
        "project_name: test\nzero_padded_ids: 3\n",
    )
    .unwrap();
    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
import { readFile, writeFile } from "node:fs/promises";
import { basename } from "node:path";
const core = new Core(process.env.BACKLOG_DOCUMENT_ROOT);
await core.ensureConfigLoaded();
const created = await core.createDocumentFromInput({title:"New doc", content:"Created\\n", path:"guides"}, false);
const existing = await core.filesystem.loadDocument("6");
await core.updateDocumentFromInput({id:existing.id, title:"Renamed doc", content:"Changed\\n"}, false);
const saved = await core.filesystem.loadDocument("6");
const source = await readFile(core.filesystem.contentRecordFilePath("document", saved), "utf8");
await writeFile(process.env.BACKLOG_DOCUMENT_ROOT+"/backlog/docs/doc-8 - Duplicate.md", `---\nid: DOC-006\ntitle: Duplicate\n---\n`);
let ambiguous = "";
try { await core.filesystem.loadDocument("6"); } catch (error) { ambiguous = error.message; }
process.stdout.write(JSON.stringify({id:created.id, filename:basename(saved.path), custom:source.includes("custom: keep"), body:source.includes("Changed\\n"), ambiguous, collision:await readFile(process.env.BACKLOG_DOCUMENT_ROOT+"/backlog/docs/guides/doc-007 - New-doc.md", "utf8")}));
"#;
    let output =
        Command::new(std::env::var_os("BACKLOG_REFERENCE_BUN").unwrap_or_else(|| "bun".into()))
            .args(["--eval", SCRIPT])
            .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
            .env("BACKLOG_DOCUMENT_ROOT", &reference.0)
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
        serde_json::json!({"id":"doc-008", "filename":"doc-006 - Renamed-doc.md", "custom":true, "body":true, "ambiguous":rust_ambiguity, "collision":"Existing collision bytes\n"})
    );
}

#[cfg(unix)]
#[test]
fn skips_directory_aliases_that_point_inside_the_document_tree() {
    let fixture = Fixture::new();
    fixture.file(
        "z-real/doc-1 - One.md",
        "---\nid: doc-1\ntitle: One\n---\nBody\n",
    );
    std::os::unix::fs::symlink(
        fixture.0.join("backlog/docs/z-real"),
        fixture.0.join("backlog/docs/a-alias"),
    )
    .unwrap();
    let store = DocumentStore::new(&fixture.0, "backlog");
    let rust_path = store.list_documents()[0]
        .path
        .strip_prefix(fixture.0.join("backlog/docs"))
        .unwrap()
        .to_string_lossy()
        .into_owned();

    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
const core = new Core(process.env.BACKLOG_DOCUMENT_ROOT);
const docs = await core.filesystem.listDocuments();
process.stdout.write(JSON.stringify(docs.map(doc=>doc.path)));
"#;
    let output =
        Command::new(std::env::var_os("BACKLOG_REFERENCE_BUN").unwrap_or_else(|| "bun".into()))
            .args(["--eval", SCRIPT])
            .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
            .env("BACKLOG_DOCUMENT_ROOT", &fixture.0)
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
    assert_eq!(rust_path, "z-real/doc-1 - One.md");
    assert_eq!(expected, serde_json::json!([rust_path]));
}

#[test]
fn updates_refuse_archived_documents_and_active_archive_duplicates() {
    let fixture = Fixture::new();
    fixture.archived_file(
        "doc-9 - Archived.md",
        "---\nid: doc-9\ntitle: Archived\n---\nBody\n",
    );
    let store = DocumentStore::new(&fixture.0, "backlog");
    let error = store
        .update_document(document("doc-9", "Changed", "Body\n"))
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Document doc-9 is archived. Archived documents are read-only. Restore it first: backlog doc restore doc-9"
    );

    fixture.file(
        "doc-10 - Active.md",
        "---\nid: doc-10\ntitle: Active\n---\nBody\n",
    );
    fixture.archived_file(
        "doc-10 - Archived.md",
        "---\nid: DOC-010\ntitle: Archived twin\n---\nBody\n",
    );
    assert!(
        store
            .update_document(document("10", "Changed", "Body\n"))
            .unwrap_err()
            .to_string()
            .contains("Document ID doc-10 is ambiguous")
    );
}
