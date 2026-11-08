use backlog_md::identity::{
    TaskIdResolution, decision_id_key, decision_ids_equal, document_id_key, entity_ids_equal,
    find_decision_by_id, find_document_by_id, normalize_document_id, resolve_task_by_id,
};
use serde_json::Value;
use std::process::{Command, Stdio};

#[derive(Debug)]
struct Record {
    id: String,
    path: String,
}

impl backlog_md::identity::HasEntityId for Record {
    fn entity_id(&self) -> &str {
        &self.id
    }
}

#[test]
fn shared_entity_identity_matches_live_typescript() {
    const REFERENCE: &str = r#"
import { documentIdKey, findDocumentById, normalizeDocumentId } from "./src/utils/document-id.ts";
import { decisionIdKey, decisionIdsEqual, findDecisionById } from "./src/utils/decision-id.ts";
import { resolveTaskById } from "./src/utils/task-id.ts";
const docs = [{id:"doc-01", path:"backlog/docs/z.md"}, {id:"DOC-1", path:"backlog/docs/a.md"}];
const task = id => ({id});
const result = {};
result.numericKey = documentIdKey("doc-0001");
result.emptyKey = documentIdKey("doc-  ");
result.textAlias = documentIdKey("DOC-Guide");
result.decisionKey = decisionIdKey("DECISION-0007");
result.decisionNumericEqual = decisionIdsEqual("decision-0007", "7");
result.decisionTextEqual = decisionIdsEqual("decision-Guide", "GUIDE");
result.normalized = normalizeDocumentId(" 01 ");
result.unique = findDocumentById([{id:"doc-2", path:"backlog/docs/unique.md"}], "2", item => item.path).path;
result.emptyEqual = Boolean(findDocumentById([{id:"doc-", path:"one.md"}], "", item => item.path));
try { findDocumentById(docs, "doc-1", item => item.path); result.duplicate = null; }
catch (error) { result.duplicate = {message:error.message, candidates:error.candidates}; }
result.decisionMissing = findDecisionById([{id:"decision-2", path:"d.md"}], "1", item => item.path);
result.tasks = [
  resolveTaskById([task("task-01")], "1").status,
  resolveTaskById([], "bad!").status,
  resolveTaskById([], "task-2").status,
  resolveTaskById([task("task-1"), task("TASK-001")], "1").status
];
process.stdout.write(JSON.stringify(result));
"#;
    let bun = std::env::var_os("BACKLOG_REFERENCE_BUN").unwrap_or_else(|| "bun".into());
    let output = Command::new(bun)
        .args(["--eval", REFERENCE])
        .current_dir(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap(),
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("live identity comparisons require Bun");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected: Value = serde_json::from_slice(&output.stdout).unwrap();

    let docs = [
        Record {
            id: "doc-01".into(),
            path: "backlog/docs/z.md".into(),
        },
        Record {
            id: "DOC-1".into(),
            path: "backlog/docs/a.md".into(),
        },
    ];
    let duplicate = find_document_by_id(&docs, "doc-1", |item| item.path.clone()).unwrap_err();
    let actual = serde_json::json!({
        "numericKey": document_id_key("doc-0001"),
        "emptyKey": document_id_key("doc-  "),
        "textAlias": document_id_key("DOC-Guide"),
        "normalized": normalize_document_id(" 01 "),
        "unique": find_document_by_id(&[Record { id: "doc-2".into(), path: "backlog/docs/unique.md".into() }], "2", |item| item.path.clone()).unwrap().map(|item| item.path.clone()),
        "emptyEqual": find_document_by_id(&[Record { id: "doc-".into(), path: "one.md".into() }], "", |item| item.path.clone()).unwrap().is_some(),
        "duplicate": { "message": duplicate.to_string(), "candidates": duplicate.candidates },
        "decisionKey": decision_id_key("DECISION-0007"),
        "decisionNumericEqual": decision_ids_equal("decision-0007", "7"),
        "decisionTextEqual": decision_ids_equal("decision-Guide", "GUIDE"),
        "decisionMissing": find_decision_by_id(&[Record { id: "decision-2".into(), path: "d.md".into() }], "1", |item| item.path.clone()).unwrap().map(|item| item.path.clone()),
        "tasks": [
            match resolve_task_by_id("1", &["task-01".into()]) { TaskIdResolution::Found(_) => "found", _ => "other" },
            match resolve_task_by_id("bad!", &[]) { TaskIdResolution::Invalid => "invalid", _ => "other" },
            match resolve_task_by_id("task-2", &[]) { TaskIdResolution::NotFound => "not-found", _ => "other" },
            match resolve_task_by_id("1", &["task-1".into(), "TASK-001".into()]) { TaskIdResolution::Ambiguous(_) => "ambiguous", _ => "other" }
        ]
    });
    assert_eq!(actual, expected);
    assert!(entity_ids_equal("decision", "DECISION-001", "1"));
}
