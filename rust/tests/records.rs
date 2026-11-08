use backlog_md::records::{
    parse_decision, parse_document, parse_markdown, parse_milestone, parse_task,
};
use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn markdown_parser_trims_the_body_and_keeps_date_spellings() {
    let parsed =
        parse_markdown("---\ncreated_date: 2026-10-19T00:00:00Z\n---\n\n  body  \n").unwrap();
    assert_eq!(parsed.frontmatter["created_date"], "2026-10-19T00:00:00Z");
    assert_eq!(parsed.content, "body");
    let legacy = parse_markdown("---\ncreated_date: 01-02-26\n---\n").unwrap();
    assert_eq!(legacy.frontmatter["created_date"], "01-02-26");
}

#[test]
fn task_parser_applies_defaults_and_parses_managed_content() {
    let task = parse_task(
        "---\nid: TASK-1\ntitle: Example\nstatus: Todo\npriority: '  HIGH   PRIORITY '\nassignee: [alice, bob]\ncreated_date: 01-02-26\n---\n## Description\n\nA task.\n\n## Acceptance Criteria\n\n- [x] First\n- [ ] Second\n",
    )
    .unwrap();
    assert_eq!(task.id, "TASK-1");
    assert_eq!(task.title, "Example");
    assert_eq!(task.assignee, ["alice", "bob"]);
    assert_eq!(task.created_date, "2026-02-01");
    assert_eq!(task.priority.as_deref(), Some("high priority"));
    assert_eq!(task.description, "A task.");
    assert_eq!(task.acceptance_criteria_items.len(), 2);
    assert!(task.acceptance_criteria_items[0].checked);
    assert!(!task.acceptance_criteria_items[1].checked);
}

#[test]
fn dependency_shapes_are_refused_with_a_helpful_error() {
    let error =
        parse_task("---\nid: TASK-2\ndependencies: [TASK-1, {id: TASK-3}]\n---\n").unwrap_err();
    assert_eq!(
        error.to_string(),
        "Invalid dependencies in task TASK-2: entry 2 contains a mapping or nested list. Use a list of task IDs in the task Markdown."
    );
}

#[test]
fn decision_document_and_milestone_parsers_keep_their_defaults() {
    let decision = parse_decision("---\nid: DEC-1\n---\n## Context\n\nBecause.\n").unwrap();
    assert_eq!(decision.status, "proposed");
    assert_eq!(decision.context, "Because.");
    let document = parse_document("---\nid: DOC-1\n---\nBody\n").unwrap();
    assert_eq!(document.document_type, "other");
    assert_eq!(document.raw_content, "Body");
    let milestone = parse_milestone("---\nid: MS-1\n---\n## Description\n\nSoon.\n").unwrap();
    assert_eq!(milestone.description, "Soon.");
}

#[test]
fn due_dates_validate_the_complete_time_and_offset_and_keep_the_written_day() {
    for valid in [
        "2026-02-28",
        "2024-02-29T23:59:59.123456789-08:00",
        "2026-01-01 00:00Z",
        "2026-01-01T00:00+1400",
    ] {
        let source = format!("---\ndue_date: {valid}\n---\n");
        assert_eq!(
            parse_task(&source).unwrap().due_date.as_deref(),
            Some(&valid[..10])
        );
    }
    for invalid in [
        "2026-02-29",
        "2026-01-01T24:00",
        "2026-01-01T12:60",
        "2026-01-01T12:00+14:01",
        "2026-01-01T12:00.1Z",
        "x",
    ] {
        let source = format!("---\ndue_date: '{invalid}'\n---\n");
        assert!(parse_task(&source).is_err(), "{invalid}");
    }
    assert_eq!(
        parse_task("---\ndue_date: '  '\n---\n").unwrap().due_date,
        None
    );
    for scalar in ["false", "0"] {
        let source = format!("---\n{{due_date: {scalar}}}\n---\n");
        assert!(parse_task(&source).is_err(), "{scalar}");
    }
}

#[test]
fn representative_records_match_the_actual_typescript_parsers() {
    const REFERENCE: &str = r#"
import { parseTask, parseDecision, parseDocument, parseMilestone } from "./src/markdown/parser.ts";
const records = JSON.parse(await Bun.stdin.text());
const result = records.map(({kind, source}) => {
  try { return {ok: kind === "task" ? parseTask(source) : kind === "decision" ? parseDecision(source) : kind === "document" ? parseDocument(source) : parseMilestone(source)}; }
  catch (error) { return {error: error.message}; }
});
process.stdout.write(JSON.stringify(result));
"#;
    let sources = [
        (
            "task",
            "---\nid: TASK-7\ntitle: A task\nstatus: Todo\npriority: ' HIGH   PRIORITY '\nassignee: [alice, bob]\ncreated_date: 01-02-26\ndue_date: 2026-02-28T23:59:59-08:00\n---\n## Description\n\nBody.\n## Acceptance Criteria\n\n- [x] Keep\n",
        ),
        (
            "decision",
            "---\nid: DEC-7\n---\n## Context\n\nNeed.\n## Decision\n\nProceed.\n",
        ),
        ("document", "---\nid: DOC-7\ntags: [one, 2]\n---\nText.\n"),
        (
            "milestone",
            "---\nid: MS-7\n---\n## Description\n\nSoon.\n## Other\n\nLater.\n",
        ),
    ];
    let refusal_sources = [
        (
            "task",
            "---\nid: TASK-8\ndependencies: [TASK-1, {id: TASK-2}]\n---\n",
        ),
        ("task", "---\nid: TASK-9\ndue_date: bad\n---\n"),
        ("task", "---\n{due_date: false}\n---\n"),
        ("task", "---\n{due_date: 0}\n---\n"),
    ];
    let mut cases: Vec<_> = sources
        .iter()
        .map(|(kind, source)| json!({"kind":kind,"source":source}))
        .collect();
    cases.extend(
        refusal_sources
            .iter()
            .map(|(kind, source)| json!({"kind":kind,"source":source})),
    );
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
        .write_all(serde_json::to_string(&cases).unwrap().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let reference: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    let task = parse_task(sources[0].1).unwrap();
    assert_eq!(reference[0]["ok"]["id"], task.id);
    assert_eq!(reference[0]["ok"]["createdDate"], task.created_date);
    assert_eq!(reference[0]["ok"]["dueDate"], json!(task.due_date));
    assert_eq!(reference[0]["ok"]["priority"], json!(task.priority));
    assert_eq!(
        reference[0]["ok"]["acceptanceCriteriaItems"][0]["text"],
        task.acceptance_criteria_items[0].text
    );
    let decision = parse_decision(sources[1].1).unwrap();
    assert_eq!(reference[1]["ok"]["decision"], decision.decision);
    let document = parse_document(sources[2].1).unwrap();
    assert_eq!(reference[2]["ok"]["tags"], json!(document.tags));
    let milestone = parse_milestone(sources[3].1).unwrap();
    assert_eq!(reference[3]["ok"]["description"], milestone.description);
    assert_eq!(
        reference[4]["error"],
        parse_task(refusal_sources[0].1).unwrap_err().to_string()
    );
    assert_eq!(
        reference[5]["error"],
        parse_task(refusal_sources[1].1).unwrap_err().to_string()
    );
    assert_eq!(
        reference[6]["error"],
        parse_task(refusal_sources[2].1).unwrap_err().to_string()
    );
    assert_eq!(
        reference[7]["error"],
        parse_task(refusal_sources[3].1).unwrap_err().to_string()
    );
}

#[test]
fn task_ordinal_uses_javascript_number_coercion() {
    assert_eq!(
        parse_task("---\nordinal: []\n---\n").unwrap().ordinal,
        Some(0.0)
    );
    assert_eq!(
        parse_task("---\nordinal: '0x10'\n---\n").unwrap().ordinal,
        Some(16.0)
    );
}
