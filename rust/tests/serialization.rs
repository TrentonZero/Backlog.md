use backlog_md::{
    frontmatter::parse_frontmatter,
    records::{parse_decision, parse_document, parse_task},
    serialization::{serialize_decision, serialize_document, serialize_task},
};
use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn task_serialization_updates_fields_and_managed_body_without_losing_unknown_metadata() {
    let source = "---\nid: TASK-7\ntitle: Original\nstatus: Todo\ncreated_date: 2026-10-19\nlabels: [one]\ncustom: retained\n---\n\n## Description\n\nOld text.\n\n## Acceptance Criteria\n\n- [ ] #1 First\n";
    let mut task = parse_task(source).unwrap();
    task.title = "Updated".into();
    task.description = "New text.".into();
    task.acceptance_criteria_items[0].checked = true;
    let output = serialize_task(&task, source).unwrap();
    let reparsed = parse_task(&output).unwrap();
    assert_eq!(reparsed.title, "Updated");
    assert_eq!(reparsed.description, "New text.");
    assert!(reparsed.acceptance_criteria_items[0].checked);
    assert_eq!(
        parse_frontmatter(&output).unwrap().data["custom"],
        "retained"
    );
}

#[test]
fn unchanged_task_serialization_keeps_original_source() {
    let source = "---\nid: TASK-8\ntitle: Keep\ncustom: [unusual, source]\n---\n\n## Description\n\n  Exact body  \n";
    let task = parse_task(source).unwrap();
    let output = serialize_task(&task, source).unwrap();
    assert_eq!(parse_task(&output).unwrap().description, "Exact body");
    assert!(output.contains("custom: [unusual, source]"));
}

#[test]
fn optional_empty_task_fields_clear_owned_metadata() {
    let source = "---\nid: TASK-12\ntitle: Optional\nstatus: Todo\nreporter: Alex\ndue_date: 2026-10-20\n---\nBody.\n";
    let mut task = parse_task(source).unwrap();
    task.reporter = Some(String::new());
    task.due_date = Some(String::new());
    let output = serialize_task(&task, source).unwrap();
    let metadata = parse_frontmatter(&output).unwrap().data;
    assert!(!metadata.contains_key("reporter"));
    assert!(!metadata.contains_key("due_date"));
}

#[test]
fn decision_serialization_preserves_crlf_body_endings() {
    let source = "---\r\nid: DEC-5\r\ntitle: EOL\r\nstatus: proposed\r\n---\r\n## Context\r\n\r\nBefore.\r\n";
    let mut decision = parse_decision(source).unwrap();
    decision.context = "After.".into();
    let output = serialize_decision(&decision, source).unwrap();
    assert!(output.contains("## Context\r\n"));
    assert!(!output.replace("\r\n", "").contains('\n'));
}

#[test]
fn decision_and_document_serialization_keep_unknown_metadata_and_update_owned_fields() {
    let decision_source = "---\nid: DEC-2\ntitle: Original\nstatus: proposed\ncustom: keep\n---\n## Context\n\nBefore.\n";
    let mut decision = parse_decision(decision_source).unwrap();
    decision.title = "Updated".into();
    decision.context = "After.".into();
    let decision_output = serialize_decision(&decision, decision_source).unwrap();
    let reparsed = parse_decision(&decision_output).unwrap();
    assert_eq!(reparsed.title, "Updated");
    assert_eq!(reparsed.context, "After.");
    assert_eq!(
        parse_frontmatter(&decision_output).unwrap().data["custom"],
        "keep"
    );

    let document_source = "---\nid: DOC-3\ntitle: Original\ntype: guide\ntags: [one]\ncustom: keep\n---\nOriginal body.\n";
    let mut document = parse_document(document_source).unwrap();
    document.title = "Updated".into();
    document.tags = None;
    document.raw_content = "New body.".into();
    let document_output = serialize_document(&document, document_source).unwrap();
    let reparsed = parse_document(&document_output).unwrap();
    assert_eq!(reparsed.title, "Updated");
    assert_eq!(reparsed.raw_content, "New body.");
    assert_eq!(reparsed.tags, None);
    assert_eq!(
        parse_frontmatter(&document_output).unwrap().data["custom"],
        "keep"
    );
}

#[test]
fn decision_serialization_distinguishes_whitespace_updates_from_unchanged_fields() {
    let source = "---\nid: DEC-4\ntitle: Empty\nstatus: proposed\n---\n";
    let mut decision = parse_decision(source).unwrap();
    decision.context = "  ".into();
    let output = serialize_decision(&decision, source).unwrap();
    assert!(output.contains("## Context"));
    assert!(output.contains("<!-- SECTION:CONTEXT:BEGIN -->"));
}

#[test]
fn record_serializers_match_live_typescript_for_supported_changes() {
    const REFERENCE: &str = r#"
import { parseTask, parseDecision, parseDocument } from "./src/markdown/parser.ts";
import { serializeTask, serializeDecision, serializeDocument } from "./src/markdown/serializer.ts";
const values = JSON.parse(await Bun.stdin.text());
const result = values.map(({kind, source}) => {
  try {
    const family = kind.replace("new", "").replace("empty", "").replace("legacy", "").replace("crlf", "").replace("mixed", "").replace("headerless", "").replace("ordinal", "");
    const record = family === "task" ? parseTask(source) : family === "decision" || kind === "unsafe" || kind === "duplicate" ? parseDecision(source) : parseDocument(source);
    if (kind === "unsafe") { record.context = "<!-- SECTION:CONTEXT:BEGIN -->"; return {output: serializeDecision(record, source)}; }
    if (kind === "duplicate" || kind === "legacydecision") return {output: serializeDecision(record, source)};
    if (kind === "emptydecision") return {output: serializeDecision(record)};
    if (kind === "ordinaltask") record.ordinal = Number.NaN;
    if (kind === "decisionheaderless") { record.context = "After."; return {output: serializeDecision(record, source)}; }
    record.title = "Updated";
    if (family === "task") { record.description = "Changed body."; if (record.acceptanceCriteriaItems[0]) record.acceptanceCriteriaItems[0].checked = true; if (record.definitionOfDoneItems[0]) record.definitionOfDoneItems[0].checked = true; if (record.acceptanceCriteriaItems.length) record.comments = [{index: 1, body: "Reviewed.", createdDate: "2026-11-08T00:00:00Z", author: "Alex"}]; }
    if (family === "decision") record.context = "Changed context.";
    if (family === "document") { record.rawContent = "Changed document."; record.tags = undefined; }
    const original = kind.startsWith("new") || kind === "emptydecision" ? [] : [source];
    return {output: family === "task" ? serializeTask(record, ...original) : family === "decision" ? serializeDecision(record, ...original) : serializeDocument(record, ...original)};
  } catch (error) { return {error: error.message}; }
});
process.stdout.write(JSON.stringify(result));
"#;
    let sources = [
        (
            "task",
            "---\nid: TASK-9\ntitle: Original\nstatus: Todo\ncustom: keep\n---\n## Description\n\nOld.\n## Acceptance Criteria\n\n- [ ] First\n## Definition of Done\n\n- [ ] Verify\n",
        ),
        (
            "decision",
            "---\nid: DEC-9\ntitle: Original\nstatus: proposed\ncustom: keep\n---\n## Context\n\nOld.\n",
        ),
        (
            "document",
            "---\nid: DOC-9\ntitle: Original\ntype: guide\ntags: [one]\ncustom: keep\n---\nOld document.\n",
        ),
        (
            "unsafe",
            "---\nid: DEC-10\ntitle: Original\nstatus: proposed\n---\n## Context\n\nOld.\n",
        ),
        (
            "newtask",
            "---\nid: TASK-11\ntitle: Original\nstatus: Todo\n---\n## Description\n\nOld.\n## Acceptance Criteria\n\n- [ ] First\n## Definition of Done\n\n- [ ] Verify\n",
        ),
        (
            "newdecision",
            "---\nid: DEC-11\ntitle: Original\nstatus: proposed\n---\n## Context\n\nOld.\n",
        ),
        (
            "newdocument",
            "---\nid: DOC-11\ntitle: Original\ntype: guide\n---\nOld document.\n",
        ),
        (
            "legacydecision",
            "---\nid: DEC-12\ntitle: Legacy\nstatus: proposed\n---\n## Context\nOld context.\n## Decision\nProceed.\n",
        ),
        (
            "emptydecision",
            "---\nid: DEC-13\ntitle: Empty\nstatus: proposed\n---\n",
        ),
        (
            "crlfdecision",
            "---\r\nid: DEC-14\r\ntitle: EOL\r\nstatus: proposed\r\n---\r\n",
        ),
        (
            "duplicate",
            "---\nid: DEC-15\ntitle: Duplicate\nstatus: proposed\n---\n## Context\nOne.\n## Context\nTwo.\n",
        ),
        (
            "decisionmixed",
            "---\nid: DEC-16\ntitle: Mixed\nstatus: proposed\n---\n\r\n## Context\r\n\r\nOld.\r\n",
        ),
        (
            "ordinaltask",
            "---\nid: TASK-17\ntitle: Ordinal\nstatus: Todo\nordinal: bad\n---\n",
        ),
        ("decisionheaderless", "## Context\r\n\r\nBefore.\r\n"),
    ];
    let input: Vec<_> = sources
        .iter()
        .map(|(kind, source)| json!({"kind":kind,"source":source}))
        .collect();
    let bun = std::env::var_os("BACKLOG_REFERENCE_BUN").unwrap_or_else(|| "bun".into());
    let mut child = Command::new(bun)
        .args(["--eval", REFERENCE])
        .current_dir(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap(),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("actual TypeScript comparisons require Bun");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(serde_json::to_string(&input).unwrap().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let reference: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();

    let mut task = parse_task(sources[0].1).unwrap();
    task.title = "Updated".into();
    task.description = "Changed body.".into();
    task.acceptance_criteria_items[0].checked = true;
    task.definition_of_done_items[0].checked = true;
    task.comments = vec![backlog_md::records::Comment {
        index: 1,
        body: "Reviewed.".into(),
        created_date: "2026-11-08T00:00:00Z".into(),
        author: Some("Alex".into()),
    }];
    let rust_task = serialize_task(&task, sources[0].1).unwrap();
    let expected = parse_task(reference[0]["output"].as_str().unwrap()).unwrap();
    let actual = parse_task(&rust_task).unwrap();
    assert_eq!(
        (
            actual.title.as_str(),
            actual.description.as_str(),
            actual.acceptance_criteria_items[0].checked,
            actual.definition_of_done_items[0].checked,
            actual.comments[0].body.as_str(),
            actual.raw_content.as_str()
        ),
        (
            expected.title.as_str(),
            expected.description.as_str(),
            expected.acceptance_criteria_items[0].checked,
            expected.definition_of_done_items[0].checked,
            expected.comments[0].body.as_str(),
            expected.raw_content.as_str()
        )
    );

    let mut decision = parse_decision(sources[1].1).unwrap();
    decision.title = "Updated".into();
    decision.context = "Changed context.".into();
    let rust_decision = serialize_decision(&decision, sources[1].1).unwrap();
    let expected = parse_decision(reference[1]["output"].as_str().unwrap()).unwrap();
    let actual = parse_decision(&rust_decision).unwrap();
    assert_eq!(
        (
            actual.title.as_str(),
            actual.context.as_str(),
            actual.raw_content.as_str()
        ),
        (
            expected.title.as_str(),
            expected.context.as_str(),
            expected.raw_content.as_str()
        )
    );

    let mut document = parse_document(sources[2].1).unwrap();
    document.title = "Updated".into();
    document.raw_content = "Changed document.".into();
    document.tags = None;
    let rust_document = serialize_document(&document, sources[2].1).unwrap();
    let expected = parse_document(reference[2]["output"].as_str().unwrap()).unwrap();
    let actual = parse_document(&rust_document).unwrap();
    assert_eq!(
        (
            actual.title.as_str(),
            actual.raw_content.as_str(),
            actual.tags.as_ref()
        ),
        (
            expected.title.as_str(),
            expected.raw_content.as_str(),
            expected.tags.as_ref()
        )
    );
    let mut unsafe_decision = parse_decision(sources[3].1).unwrap();
    unsafe_decision.context = "<!-- SECTION:CONTEXT:BEGIN -->".into();
    assert!(serialize_decision(&unsafe_decision, sources[3].1).is_err());
    assert!(reference[3]["error"].is_string());

    let mut task = parse_task(sources[4].1).unwrap();
    task.title = "Updated".into();
    task.description = "Changed body.".into();
    task.acceptance_criteria_items[0].checked = true;
    task.definition_of_done_items[0].checked = true;
    task.comments = vec![backlog_md::records::Comment {
        index: 1,
        body: "Reviewed.".into(),
        created_date: "2026-11-08T00:00:00Z".into(),
        author: Some("Alex".into()),
    }];
    let expected = parse_task(reference[4]["output"].as_str().unwrap()).unwrap();
    let actual = parse_task(&serialize_task(&task, "").unwrap()).unwrap();
    assert_eq!(
        (
            actual.title.as_str(),
            actual.description.as_str(),
            actual.acceptance_criteria_items[0].checked,
            actual.definition_of_done_items[0].checked,
            actual.comments[0].body.as_str()
        ),
        (
            expected.title.as_str(),
            expected.description.as_str(),
            expected.acceptance_criteria_items[0].checked,
            expected.definition_of_done_items[0].checked,
            expected.comments[0].body.as_str()
        )
    );

    let mut decision = parse_decision(sources[5].1).unwrap();
    decision.title = "Updated".into();
    decision.context = "Changed context.".into();
    let expected = parse_decision(reference[5]["output"].as_str().unwrap()).unwrap();
    let actual = parse_decision(&serialize_decision(&decision, "").unwrap()).unwrap();
    assert_eq!(
        (actual.title.as_str(), actual.context.as_str()),
        (expected.title.as_str(), expected.context.as_str())
    );

    let mut document = parse_document(sources[6].1).unwrap();
    document.title = "Updated".into();
    document.raw_content = "Changed document.".into();
    let expected = parse_document(reference[6]["output"].as_str().unwrap()).unwrap();
    let actual = parse_document(&serialize_document(&document, "").unwrap()).unwrap();
    assert_eq!(
        (actual.title.as_str(), actual.raw_content.as_str()),
        (expected.title.as_str(), expected.raw_content.as_str())
    );

    let legacy = parse_decision(sources[7].1).unwrap();
    let actual = parse_decision(&serialize_decision(&legacy, sources[7].1).unwrap()).unwrap();
    let expected = parse_decision(reference[7]["output"].as_str().unwrap()).unwrap();
    assert!(
        actual
            .raw_content
            .contains("<!-- SECTION:CONTEXT:BEGIN -->")
    );
    assert_eq!(actual.raw_content, expected.raw_content);

    let empty = parse_decision(sources[8].1).unwrap();
    let actual = parse_decision(&serialize_decision(&empty, "").unwrap()).unwrap();
    let expected = parse_decision(reference[8]["output"].as_str().unwrap()).unwrap();
    assert_eq!(actual.raw_content, expected.raw_content);

    let mut crlf = parse_decision(sources[9].1).unwrap();
    crlf.title = "Updated".into();
    crlf.context = "Changed context.".into();
    let actual = parse_decision(&serialize_decision(&crlf, sources[9].1).unwrap()).unwrap();
    let expected = parse_decision(reference[9]["output"].as_str().unwrap()).unwrap();
    assert_eq!(actual.raw_content, expected.raw_content);
    assert!(actual.raw_content.contains("\r\n"));

    let duplicate = parse_decision(sources[10].1).unwrap();
    assert!(serialize_decision(&duplicate, sources[10].1).is_err());
    assert!(reference[10]["error"].is_string());

    let mut mixed = parse_decision(sources[11].1).unwrap();
    mixed.context = "Changed context.".into();
    let mixed_output = serialize_decision(&mixed, sources[11].1).unwrap();
    let expected = parse_decision(reference[11]["output"].as_str().unwrap()).unwrap();
    assert_eq!(
        parse_decision(&mixed_output).unwrap().raw_content,
        expected.raw_content
    );

    let mut ordinal = parse_task(sources[12].1).unwrap();
    ordinal.title = "Updated".into();
    ordinal.description = "Changed body.".into();
    ordinal.ordinal = Some(f64::NAN);
    let ordinal_output = serialize_task(&ordinal, sources[12].1).unwrap();
    assert!(ordinal_output.contains("ordinal: .nan"));
    assert!(
        parse_task(&ordinal_output)
            .unwrap()
            .ordinal
            .unwrap()
            .is_nan()
    );
    assert!(reference[12]["output"].is_string(), "{:#?}", reference[12]);
    assert!(
        reference[12]["output"]
            .as_str()
            .unwrap()
            .contains("ordinal: .nan")
    );

    let mut headerless = parse_decision(sources[13].1).unwrap();
    headerless.context = "After.".into();
    let headerless_output = serialize_decision(&headerless, sources[13].1).unwrap();
    assert!(reference[13]["output"].is_string(), "{:#?}", reference[13]);
    let expected = parse_decision(reference[13]["output"].as_str().unwrap()).unwrap();
    assert_eq!(
        parse_decision(&headerless_output).unwrap().raw_content,
        expected.raw_content
    );
    assert!(headerless_output.contains("## Context\r\n"));
}
