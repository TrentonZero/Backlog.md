use std::collections::BTreeMap;

use backlog_md::sections::{SectionKey::*, extract_section, update_sections};

fn changes(
    key: backlog_md::sections::SectionKey,
    value: Option<&str>,
) -> BTreeMap<backlog_md::sections::SectionKey, Option<String>> {
    BTreeMap::from([(key, value.map(str::to_owned))])
}

#[test]
fn task_sections_use_canonical_blocks_and_preserve_omitted_values() {
    let original = "## Description\n\nOld description\n\n## Acceptance Criteria\n- [ ] Keep\n\n## Implementation Notes\n\nOld notes";
    let updated =
        update_sections(original, &changes(Description, Some("New description"))).unwrap();
    assert_eq!(
        updated,
        "## Description\n\n<!-- SECTION:DESCRIPTION:BEGIN -->\nNew description\n<!-- SECTION:DESCRIPTION:END -->\n\n## Acceptance Criteria\n- [ ] Keep\n\n## Implementation Notes\n\n<!-- SECTION:NOTES:BEGIN -->\nOld notes\n<!-- SECTION:NOTES:END -->"
    );
    assert_eq!(
        extract_section(&updated, ImplementationNotes).as_deref(),
        Some("Old notes")
    );
    assert!(
        !update_sections(&updated, &changes(Description, None))
            .unwrap()
            .contains("New description")
    );
    assert_eq!(
        update_sections(original, &BTreeMap::new()).unwrap(),
        original
    );
}

#[test]
fn task_legacy_ambiguity_refuses_replacement_but_retains_safe_appends() {
    let original = "## Implementation Notes\n\nOld\n\n## Verification\n\nPROOF";
    let error = update_sections(original, &changes(ImplementationNotes, Some("New")))
        .unwrap_err()
        .to_string();
    assert!(error.contains("may be a separate section"));
    assert!(error.contains("<!-- SECTION:NOTES:BEGIN -->"));
    let appended = update_sections(
        original,
        &changes(
            ImplementationNotes,
            Some("Old\n\n## Verification\n\nPROOF\n\nAppended"),
        ),
    )
    .unwrap();
    assert!(!appended.contains("SECTION:NOTES"));
    assert!(appended.contains("PROOF\n\nAppended"));
}

#[test]
fn task_marker_pairs_are_whole_lines_and_depth_counted() {
    let original = "## Implementation Notes\n\n<!-- SECTION:NOTES:BEGIN -->\n<!-- SECTION:NOTES:BEGIN -->\nFIRST\n<!-- SECTION:NOTES:END -->\nSECOND\n<!-- SECTION:NOTES:END -->";
    assert_eq!(
        extract_section(original, ImplementationNotes).as_deref(),
        Some("<!-- SECTION:NOTES:BEGIN -->\nFIRST\n<!-- SECTION:NOTES:END -->\nSECOND")
    );
    let updated = update_sections(
        original,
        &changes(
            ImplementationNotes,
            Some("End is <!-- SECTION:NOTES:END --> inline\nTail"),
        ),
    )
    .unwrap();
    assert_eq!(
        extract_section(&updated, ImplementationNotes).as_deref(),
        Some("End is <!-- SECTION:NOTES:END --> inline\nTail")
    );
    assert!(!updated.contains("SECOND"));
}

#[test]
fn legacy_ambiguity_uses_markdown_context_and_setext_headings() {
    for body in [
        "```md\n## Example\n```",
        "    ## Code",
        "> ## Quote",
        "- item\n  ## Nested",
        "<!--\n## HTML\n-->",
        "<div>\n## HTML\n</div>",
    ] {
        let original = format!("## Implementation Notes\n\n{body}");
        assert!(
            update_sections(&original, &changes(ImplementationNotes, Some("New"))).is_ok(),
            "{body}"
        );
    }
    for body in [
        "Old\n\nVerification\n---\nPROOF",
        "Old\n   ## Verification\nPROOF",
        "Old\n##\nPROOF",
        "Old\n<widget>\n## Verification\nPROOF",
    ] {
        let original = format!("## Implementation Notes\n\n{body}");
        assert!(
            update_sections(&original, &changes(ImplementationNotes, None)).is_err(),
            "{body}"
        );
    }
}

#[test]
fn decision_updates_migrate_legacy_in_place_and_keep_unmanaged_source() {
    let original =
        "Preamble\n\n## Context\n\nOld\n\n## Custom\n\nKeep  \n\n## Decision\n\nChoice\n";
    let updated = update_sections(original, &changes(Context, Some("New"))).unwrap();
    assert_eq!(
        updated,
        "Preamble\n\n## Context\n<!-- SECTION:CONTEXT:BEGIN -->\nNew\n<!-- SECTION:CONTEXT:END -->\n## Custom\n\nKeep  \n\n## Decision\n<!-- SECTION:DECISION:BEGIN -->\n\nChoice\n<!-- SECTION:DECISION:END -->\n"
    );
    assert_eq!(
        extract_section(&updated, Decision).as_deref(),
        Some("Choice")
    );
    let cleared = update_sections(&updated, &changes(Context, Some(""))).unwrap();
    assert_eq!(extract_section(&cleared, Context).as_deref(), Some(""));
}

#[test]
fn decision_marked_bodies_own_unfinished_fences_and_duplicate_headings_refuse_writes() {
    let original = "## Context\n\n<!-- SECTION:CONTEXT:BEGIN -->\n```md\nExample\n<!-- SECTION:CONTEXT:END -->\n\n## Decision\n\n<!-- SECTION:DECISION:BEGIN -->\nChoice\n<!-- SECTION:DECISION:END -->";
    assert_eq!(
        extract_section(original, Decision).as_deref(),
        Some("Choice")
    );
    let updated = update_sections(original, &changes(Decision, Some("New"))).unwrap();
    assert!(updated.contains("```md\nExample"));
    assert_eq!(extract_section(&updated, Decision).as_deref(), Some("New"));
    assert!(
        update_sections(
            "## Context\nOne\n\n## Context\nTwo",
            &changes(Context, Some("New"))
        )
        .unwrap_err()
        .to_string()
        .contains("more than one Context")
    );
}

use backlog_md::sections::{
    ChecklistKind, Comment, Criterion, assert_section_input_safe, assert_valid_checklist_marks,
    parse_comments, parse_criteria, update_comments, update_criteria,
    validate_decision_editor_structure,
};

fn criterion(index: usize, text: &str, checked: bool) -> Criterion {
    Criterion {
        index,
        text: text.to_owned(),
        checked,
    }
}
fn comment(index: usize, body: &str, date: &str, author: Option<&str>) -> Comment {
    Comment {
        index,
        body: body.to_owned(),
        created_date: date.to_owned(),
        author: author.map(str::to_owned),
    }
}
fn checklist_cases() -> [(ChecklistKind, &'static str, &'static str); 2] {
    [
        (
            ChecklistKind::AcceptanceCriteria,
            "Acceptance Criteria",
            "AC",
        ),
        (ChecklistKind::DefinitionOfDone, "Definition of Done", "DOD"),
    ]
}

#[test]
fn checklist_reads_sequential_indices_and_rewrites_sorted_items_with_prose() {
    for (kind, title, marker) in checklist_cases() {
        let source = format!(
            "## {title}\n<!-- {marker}:BEGIN -->\n### Critical\n- [ ] #7 First\n  continuation\n- [x] #3 Second\n<!-- {marker}:END -->\n\n## Custom\nKeep"
        );
        assert_eq!(
            parse_criteria(&source, kind),
            vec![criterion(1, "First", false), criterion(2, "Second", true)]
        );
        let updated = update_criteria(
            &source,
            kind,
            &[
                criterion(2, "Second", true),
                criterion(1, "First", true),
                criterion(3, "Third", false),
            ],
        )
        .unwrap();
        assert_eq!(
            updated,
            format!(
                "## Custom\nKeep\n\n## {title}\n<!-- {marker}:BEGIN -->\n### Critical\n- [x] #1 First\n  continuation\n- [x] #2 Second\n- [ ] #3 Third\n<!-- {marker}:END -->"
            )
        );
        assert_eq!(
            update_criteria(&updated, kind, &parse_criteria(&updated, kind)).unwrap(),
            updated
        );
    }
}

#[test]
fn checklist_foreign_blocks_are_opaque_even_with_nested_or_unmatched_target_markers() {
    for (kind, title, marker) in checklist_cases() {
        let source = format!(
            "## Comments\n<!-- COMMENTS:BEGIN -->\n<!-- {marker}:END -->\n## {title}\n<!-- {marker}:BEGIN -->\n- [ ] #1 Fake\n<!-- {marker}:END -->\n<!-- COMMENTS:END -->\n\n## {title}\n<!-- {marker}:BEGIN -->\n- [ ] #8 Real\n<!-- SECTION:CUSTOM:BEGIN -->\n- [ ] Opaque\n<!-- {marker}:END -->\n<!-- SECTION:CUSTOM:END -->\n<!-- {marker}:END -->"
        );
        assert_eq!(
            parse_criteria(&source, kind),
            vec![criterion(1, "Real", false)]
        );
        let updated = update_criteria(&source, kind, &[criterion(1, "Replacement", true)]).unwrap();
        assert!(updated.contains("- [ ] Opaque\n"));
        assert!(updated.contains("- [ ] #1 Fake"));
        assert_eq!(
            parse_criteria(&updated, kind),
            vec![criterion(1, "Replacement", true)]
        );
    }
}

#[test]
fn malformed_checklist_markers_refuse_updates_and_hide_reads() {
    for (kind, title, marker) in checklist_cases() {
        for (body, detail) in [
            (format!("<!-- {marker}:END -->"), "without a preceding"),
            (
                format!("<!-- {marker}:BEGIN -->\n- [ ] #1 Lost"),
                "without a following",
            ),
            (
                format!(
                    "<!-- {marker}:BEGIN -->\n<!-- {marker}:BEGIN -->\n<!-- {marker}:END -->\n<!-- {marker}:END -->"
                ),
                "found a second",
            ),
        ] {
            let source = format!("## {title}\n{body}");
            assert!(parse_criteria(&source, kind).is_empty());
            let error = update_criteria(&source, kind, &[]).unwrap_err().to_string();
            assert!(
                error.contains(&format!("Malformed {title} markers")),
                "{error}"
            );
            assert!(error.contains(detail), "{error}");
        }
    }
}

#[test]
fn checklist_clear_preserves_residual_shell_and_noop_preserves_absent_prose_source() {
    for (kind, title, marker) in checklist_cases() {
        let source = format!(
            "## Description\r\n\r\nDetails\r\n\r\n## {title}\r\n<!-- {marker}:BEGIN -->\r\n- [ ] #1 Old\r\nKeep prose.\r\n<!-- {marker}:END -->\r\n\r\n## Implementation Plan\r\n\r\nPlan"
        );
        let cleared = update_criteria(&source, kind, &[]).unwrap();
        assert_eq!(
            cleared,
            format!(
                "## Description\r\n\r\nDetails\r\n\r\n## {title}\r\n<!-- {marker}:BEGIN -->\r\nKeep prose.\r\n<!-- {marker}:END -->\r\n\r\n## Implementation Plan\r\n\r\nPlan"
            )
        );
        assert_eq!(update_criteria(&cleared, kind, &[]).unwrap(), cleared);
        let prose = format!("## {title}\r\nProse only\r\n");
        assert_eq!(update_criteria(&prose, kind, &[]).unwrap(), prose);
    }
}

#[test]
fn checklist_legacy_boundaries_and_canonical_family_placement() {
    let source = "## Description\n\nDetails\n\n## Acceptance Criteria\n- [x] Old\n\n## Custom\nKeep\n\n## Implementation Plan\n\nPlan";
    assert_eq!(
        parse_criteria(source, ChecklistKind::AcceptanceCriteria),
        vec![criterion(1, "Old", true)]
    );
    let updated = update_criteria(
        source,
        ChecklistKind::AcceptanceCriteria,
        &[criterion(1, "New", true)],
    )
    .unwrap();
    assert_eq!(
        updated,
        "## Description\n\nDetails\n\n## Custom\nKeep\n\n## Acceptance Criteria\n<!-- AC:BEGIN -->\n- [x] #1 New\n<!-- AC:END -->\n\n## Implementation Plan\n\nPlan"
    );
    let with_dod = update_criteria(
        &updated,
        ChecklistKind::DefinitionOfDone,
        &[criterion(1, "Test", false)],
    )
    .unwrap();
    assert!(with_dod.contains("<!-- AC:END -->\n\n## Definition of Done\n<!-- DOD:BEGIN -->\n- [ ] #1 Test\n<!-- DOD:END -->\n\n## Implementation Plan"));
}

#[test]
fn checklist_mark_validation_ignores_fences_html_and_foreign_blocks() {
    for (kind, title, marker) in checklist_cases() {
        let prefix = format!("## {title}\n<!-- {marker}:BEGIN -->\n");
        for example in [
            "~~~\n- [X] Example\n~~~",
            "<!--\n- [X] Example\n-->",
            "<div>\n- [X] Example\n\n",
            "<!-- COMMENT:BEGIN -->\n- [X] Example\n<!-- COMMENT:END -->",
        ] {
            let source = format!("{prefix}{example}\n- [ ] #1 Real\n<!-- {marker}:END -->");
            assert_valid_checklist_marks(&source, kind).unwrap();
        }
        let source = format!("{prefix}- [X] #1 Invalid\n<!-- {marker}:END -->");
        assert!(
            assert_valid_checklist_marks(&source, kind)
                .unwrap_err()
                .to_string()
                .contains("Use [ ] or [x]")
        );
    }
}

#[test]
fn comments_roundtrip_multiline_bodies_and_written_dates_in_canonical_format() {
    let original = "## Implementation Notes\n\nProgress\n\n## Final Summary\n\nDone";
    let comments = [
        comment(
            7,
            "First\n\n## Intended heading\n\nTail",
            "2026-05-31 10:45",
            Some(" @reviewer \n team "),
        ),
        comment(8, "Second", "", None),
    ];
    let updated = update_comments(original, &comments).unwrap();
    assert_eq!(
        updated,
        "## Implementation Notes\n\nProgress\n\n## Comments\n\n<!-- COMMENTS:BEGIN -->\nauthor: @reviewer team\ncreated: 2026-05-31 10:45\n\n---\n\nFirst\n\n## Intended heading\n\nTail\n\n---\n\n---\n\nSecond\n\n---\n<!-- COMMENTS:END -->\n\n## Final Summary\n\nDone"
    );
    assert_eq!(
        parse_comments(&updated),
        vec![
            comment(
                1,
                "First\n\n## Intended heading\n\nTail",
                "2026-05-31 10:45",
                Some("@reviewer team")
            ),
            comment(2, "Second", "", None)
        ]
    );
}

#[test]
fn comments_legacy_formats_and_unmanaged_namesakes_are_preserved() {
    let original = "## Comments\nFreeform note\n\n## Custom\nKeep\n\n## Comments\n<!-- COMMENTS:BEGIN -->\n<!-- COMMENT:BEGIN -->\nindex: 9\nauthor: Alex\ncreated: 2026-06-07 21:21\n\nLegacy\n\n## Heading\nTail\n<!-- COMMENT:END -->\n<!-- COMMENTS:END -->";
    assert_eq!(
        parse_comments(original),
        vec![comment(
            9,
            "Legacy\n\n## Heading\nTail",
            "2026-06-07 21:21",
            Some("Alex")
        )]
    );
    let updated = update_comments(original, &parse_comments(original)).unwrap();
    assert!(updated.contains("Freeform note\n\n## Custom\nKeep"));
    assert_eq!(
        parse_comments(&updated),
        vec![comment(
            1,
            "Legacy\n\n## Heading\nTail",
            "2026-06-07 21:21",
            Some("Alex")
        )]
    );
    assert!(!updated.contains("<!-- COMMENT:BEGIN -->"));
    assert!(
        update_comments(&updated, &[])
            .unwrap()
            .contains("Freeform note")
    );
}

#[test]
fn comments_refuse_reserved_markers_and_delimiters_in_each_field() {
    for bad in ["contains <!-- comment:END -->", "line\n --- \nline"] {
        for comments in [
            vec![comment(1, bad, "", None)],
            vec![comment(
                1,
                "Body",
                "",
                Some(if bad.starts_with("line") { "---" } else { bad }),
            )],
            vec![comment(1, "Body", bad, None)],
        ] {
            assert!(update_comments("", &comments).is_err(), "{comments:?}");
        }
    }
}

#[test]
fn safety_and_raw_decision_editor_preserve_reserved_line_rules() {
    assert!(assert_section_input_safe("<!-- SECTION:NOTES:END -->", ImplementationNotes).is_err());
    assert_section_input_safe(" <!-- SECTION:NOTES:END -->", ImplementationNotes).unwrap();
    assert_section_input_safe("Inline <!-- SECTION:NOTES:END -->", ImplementationNotes).unwrap();
    assert_section_input_safe("<!-- SECTION:DESCRIPTION:BEGIN -->", ImplementationNotes).unwrap();
    assert!(assert_section_input_safe("<!-- SECTION:DESCRIPTION:BEGIN -->", Context).is_err());
    let original =
        "## Context\n\n<!-- SECTION:CONTEXT:BEGIN -->\nBody\n<!-- SECTION:CONTEXT:END -->";
    validate_decision_editor_structure(original, &original.replace("Body", "New")).unwrap();
    assert!(
        validate_decision_editor_structure(
            original,
            &original.replace("CONTEXT:END", "DECISION:END")
        )
        .is_err()
    );
    assert!(
        validate_decision_editor_structure("## Context\nOne", "## Context\nOne\n\n## Context\nTwo")
            .is_err()
    );
}

// Run the real reference once for the corpus. No reference output is committed,
// and the native implementation never delegates parsing or mutation to Bun.
use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Stdio},
};

const REFERENCE: &str = r#"
import {
    extractStructuredSection, getStructuredSections, updateStructuredSections,
    getDecisionSections, updateDecisionSections, assertSectionInputHasNoMarkerLines,
    assertDecisionEditorStructure, AcceptanceCriteriaManager, DefinitionOfDoneManager,
    assertValidChecklistMarks, CommentsManager
} from "./src/markdown/structured-sections.ts";
const cases = JSON.parse(await Bun.stdin.text());
const decisionKeys = ["context", "decision", "consequences", "alternatives"];
const result = cases.map(c => {
    try {
        const manager = c.kind === "AC" ? AcceptanceCriteriaManager : DefinitionOfDoneManager;
        let value;
        switch(c.op) {
        case "extract":
            value = (decisionKeys.includes(c.key) ? getDecisionSections(c.input)[c.key] : extractStructuredSection(c.input, c.key)) ?? null;
            break;
        case "sections":
            if (!Object.keys(c.changes).length) { value = c.input; break; }
            const updates = Object.fromEntries(Object.entries(c.changes).map(([k, v]) => [k, v ?? ""]));
            value = Object.keys(updates).some(k => decisionKeys.includes(k))
                ? updateDecisionSections(c.input, updates, c.input.includes("\r\n") ? "\r\n" : "\n")
                : updateStructuredSections(c.input, {...getStructuredSections(c.input), ...updates});
            break;
        case "criteria": value = manager.parseAllCriteria(c.input); break;
        case "checklist": value = manager.updateContent(c.input, c.criteria); break;
        case "marks": assertValidChecklistMarks(c.input, c.kind); value = null; break;
        case "comments": value = CommentsManager.parseAllComments(c.input); break;
        case "writeComments": value = CommentsManager.updateContent(c.input, c.comments); break;
        case "safe": assertSectionInputHasNoMarkerLines(c.input, c.key); value = null; break;
        case "editor": assertDecisionEditorStructure(c.input, c.edited); value = null; break;
        default: throw new Error("unknown test operation");
        }
        return {ok: value};
    } catch(error) { return {error: error.message}; }
});
process.stdout.write(JSON.stringify(result));
"#;

fn reference_key(name: &str) -> backlog_md::sections::SectionKey {
    match name {
        "description" => Description,
        "implementationPlan" => ImplementationPlan,
        "implementationNotes" => ImplementationNotes,
        "finalSummary" => FinalSummary,
        "context" => Context,
        "decision" => Decision,
        "consequences" => Consequences,
        "alternatives" => Alternatives,
        _ => panic!("unknown fixture section {name}"),
    }
}
fn native_result(case: &Value) -> Value {
    let input = case["input"].as_str().unwrap();
    let key = || reference_key(case["key"].as_str().unwrap());
    let kind = if case["kind"] == "AC" {
        ChecklistKind::AcceptanceCriteria
    } else {
        ChecklistKind::DefinitionOfDone
    };
    let result = match case["op"].as_str().unwrap() {
        "extract" => Ok(json!(extract_section(input, key()))),
        "sections" => {
            let changes = case["changes"].as_object().unwrap().iter().map(|(key, value)| (reference_key(key), value.as_str().map(str::to_owned))).collect();
            update_sections(input, &changes).map(|s| json!(s))
        }
        "criteria" => Ok(Value::Array(parse_criteria(input, kind).iter().map(|item| json!({"index": item.index, "text": item.text, "checked": item.checked})).collect())),
        "checklist" => {
            let criteria: Vec<_> = case["criteria"].as_array().unwrap().iter().map(|item| criterion(item["index"].as_u64().unwrap() as usize, item["text"].as_str().unwrap(), item["checked"].as_bool().unwrap())).collect();
            update_criteria(input, kind, &criteria).map(|s| json!(s))
        }
        "marks" => assert_valid_checklist_marks(input, kind).map(|()| Value::Null),
        "comments" => Ok(Value::Array(parse_comments(input).iter().map(|item| {
            let mut value = json!({"index": item.index, "body": item.body, "createdDate": item.created_date});
            if let Some(author) = &item.author { value["author"] = json!(author); }
            value
        }).collect())),
        "writeComments" => {
            let comments: Vec<_> = case["comments"].as_array().unwrap().iter().map(|item| comment(item["index"].as_u64().unwrap() as usize, item["body"].as_str().unwrap(), item["createdDate"].as_str().unwrap(), item["author"].as_str())).collect();
            update_comments(input, &comments).map(|s| json!(s))
        }
        "safe" => assert_section_input_safe(input, key()).map(|()| Value::Null),
        "editor" => validate_decision_editor_structure(input, case["edited"].as_str().unwrap()).map(|()| Value::Null),
        _ => panic!("unknown fixture operation"),
    };
    match result {
        Ok(value) => json!({"ok": value}),
        Err(error) => json!({"error": error.to_string()}),
    }
}
fn compare_reference(cases: &[Value]) {
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
        .expect("actual TypeScript comparisons require Bun on PATH or BACKLOG_REFERENCE_BUN");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(serde_json::to_string(cases).unwrap().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "TypeScript comparison failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    eprintln!(
        "Compared {} native operations with actual TypeScript",
        cases.len()
    );
    assert_eq!(expected.len(), cases.len());
    let mismatches: Vec<_> = cases
        .iter()
        .zip(expected)
        .enumerate()
        .filter_map(|(index, (case, expected))| {
            let actual = native_result(case);
            (actual != expected)
                .then(|| format!("Case {index}: {case}\nExpected: {expected}\nNative: {actual}"))
        })
        .collect();
    assert!(
        mismatches.is_empty(),
        "{} reference mismatches:\n{}",
        mismatches.len(),
        mismatches.join("\n\n")
    );
}
fn case(op: &str, input: &str, fields: Value) -> Value {
    let mut value = json!({"op": op, "input": input});
    value
        .as_object_mut()
        .unwrap()
        .extend(fields.as_object().unwrap().clone());
    value
}

#[test]
fn task_and_decision_operations_match_actual_typescript() {
    let mut cases = Vec::new();
    let keys = [
        ("description", "Description", "DESCRIPTION"),
        ("implementationPlan", "Implementation Plan", "PLAN"),
        ("implementationNotes", "Implementation Notes", "NOTES"),
        ("finalSummary", "Final Summary", "FINAL_SUMMARY"),
        ("context", "Context", "CONTEXT"),
        ("decision", "Decision", "DECISION"),
        ("consequences", "Consequences", "CONSEQUENCES"),
        ("alternatives", "Alternatives", "ALTERNATIVES"),
    ];
    let bodies = [
        "Old",
        "First\n\n\n\nTail",
        "Unicode café 🦀\n\nTail",
        "Old\n\n## Custom\n\nPROOF",
        "Old\n   ## Custom\nPROOF",
        "Old\n##\nPROOF",
        "Old\n\nCustom\n---\nPROOF",
        "~~~md\n## Example\n\n\nTail\n~~~",
        "~~~md\n## Example",
        "    ## Code\n\nTail",
        "> ## Quote\n\nTail",
        "- item\n  ## Nested",
        "<!--\n## Example\n-->",
        "<!--\n--!>\n## Example\n-->",
        "<pre>\n## Example\n</script>\n## Custom",
        "<pre/>\n## Custom",
        "<pre />\n## Example",
        "<div>\n## Example\n</div>",
        "Old\n<widget>\n## Custom",
        "<widget label=\">\">\n## Example\n</widget>",
        "-\t<widget>\n   ## Custom",
        "-\t<widget>\n\t## Example",
        "- ~~~md\n  ## Example\n  ~~~",
        "- ~~~md\n  code\n## Custom",
        "Inline ## marker\n### Child\nTail",
    ];
    for (name, title, marker) in keys {
        for body in bodies {
            for marked in [false, true] {
                let section = if marked {
                    format!(
                        "## {title}\n\n<!-- SECTION:{marker}:BEGIN -->\n{body}\n<!-- SECTION:{marker}:END -->"
                    )
                } else {
                    format!("## {title}\n\n{body}")
                };
                for newline in ["\n", "\r\n"] {
                    let source = format!("Preamble\n\n{section}\n\n## Final custom\nKeep  \n")
                        .replace('\n', newline);
                    cases.push(case("extract", &source, json!({"key": name})));
                    for value in [
                        Value::Null,
                        json!("New"),
                        json!(format!("{body}\n\nAppended")),
                    ] {
                        cases.push(case("sections", &source, json!({"changes": {name: value}})));
                    }
                }
            }
        }
        for suffix in ["BEGIN", "END"] {
            let line = format!("<!-- SECTION:{marker}:{suffix} -->");
            for input in [
                line.clone(),
                format!(" {line}"),
                format!("Inline {line}"),
                format!("~~~\n{line}\n~~~"),
                format!("{line}\t "),
            ] {
                cases.push(case("safe", &input, json!({"key": name})));
            }
        }
        let nested = format!(
            "## {title}\n\n<!-- SECTION:{marker}:BEGIN -->\n<!-- SECTION:{marker}:BEGIN -->\nFIRST\n<!-- SECTION:{marker}:END -->\nSECOND\n<!-- SECTION:{marker}:END -->"
        );
        cases.push(case("extract", &nested, json!({"key":name})));
        cases.push(case("sections", &nested, json!({"changes":{name:"New"}})));
        let duplicate = format!("## {title}\nOne\n\n## {title}\nTwo");
        cases.push(case("extract", &duplicate, json!({"key":name})));
        cases.push(case(
            "sections",
            &duplicate,
            json!({"changes":{name:"New"}}),
        ));
    }
    for input in [
        "## Description\n\n<!-- SECTION:DESCRIPTION:BEGIN -->\n## Implementation Notes\n\nExample\n<!-- SECTION:DESCRIPTION:END -->",
        "## Implementation Notes\n\n<!-- SECTION:NOTES:BEGIN -->\nNotes\n<!-- SECTION:NOTES:END -->\n\n## Implementation Notes\n\nOld\n\n## Custom\nPROOF",
        "## Context\n\n## Decision\n\n## Consequences\n\n",
        "~~~\n## Context\nFake\n~~~\n\n## Decision\nReal",
        "- item\n  ## Context\n  Nested\n\n## Decision\nReal",
        "> ## Context\n> Nested\n\n## Decision\nReal",
        "## Context\n\n<!-- SECTION:CONTEXT:BEGIN -->\n~~~\nExample\n<!-- SECTION:CONTEXT:END -->\n\n## Decision\n\n<!-- SECTION:DECISION:BEGIN -->\nChoice\n<!-- SECTION:DECISION:END -->",
        "## Context\n<!-- SECTION:CONTEXT:BEGIN -->\nUnclosed\n\n## Decision\nOld",
    ] {
        for (name, _, _) in keys {
            cases.push(case("extract", input, json!({"key": name})));
        }
        for name in ["description", "implementationNotes", "context", "decision"] {
            cases.push(case("sections", input, json!({"changes":{name:"New"}})));
        }
    }
    for original in [
        "## Context\nOne",
        "## Context\n\n<!-- SECTION:CONTEXT:BEGIN -->\nBody\n<!-- SECTION:CONTEXT:END -->",
    ] {
        for edited in [
            original.to_owned(),
            original.replace("Body", "New"),
            format!("{original}\n\n## Context\nTwo"),
            original.replace("CONTEXT:END", "DECISION:END"),
            format!("{original}\n <!-- SECTION:DECISION:END -->"),
        ] {
            cases.push(case("editor", original, json!({"edited":edited})));
        }
    }
    compare_reference(&cases);
}

#[test]
fn checklist_and_comment_operations_match_actual_typescript() {
    let mut cases = Vec::new();
    let bodies = [
        "- [ ] #7 First\n- [x] #2 Second",
        "- [ ] Legacy\n- [x] Second",
        "### Group\n- [ ] #1 First\n  continuation\nTail",
        "- [ ] #1 First\n<!-- SECTION:CUSTOM:BEGIN -->\n- [ ] Opaque\n\n\nTail\n<!-- SECTION:CUSTOM:END -->",
        "- [ ] #1 First\n  - [ ] Nested\n\t- [ ] Tabbed\n- [x] #2 Second",
        "Prose only",
        "",
        "~~~\n- [X] Example\n~~~\n- [ ] #1 Real",
        "<div>\n- [X] Example\n\n- [ ] #1 Real",
        "- [X] #1 Invalid",
        "- [✓] Invalid",
        "- [ ] #42 ",
        "- [ ] #42 #9 Nested",
        "- [ ] #1 Café 🦀",
    ];
    for (_, title, marker) in checklist_cases() {
        for body in bodies {
            for marked in [false, true] {
                let section = if marked {
                    format!("## {title}\n<!-- {marker}:BEGIN -->\n{body}\n<!-- {marker}:END -->")
                } else {
                    format!("## {title}\n\n{body}")
                };
                for newline in ["\n", "\r\n"] {
                    let input = format!("## Description\n\nDetails\n\n{section}\n\n## Custom\nKeep\n\n## Implementation Plan\n\nPlan").replace('\n', newline);
                    cases.push(case("criteria", &input, json!({"kind":marker})));
                    cases.push(case("marks", &input, json!({"kind":marker})));
                    for items in [
                        json!([]),
                        json!([{"index":2,"text":"Second","checked":true},{"index":1,"text":"First","checked":false}]),
                    ] {
                        cases.push(case(
                            "checklist",
                            &input,
                            json!({"kind":marker,"criteria":items}),
                        ));
                    }
                }
            }
        }
        for body in [
            format!("<!-- {marker}:END -->"),
            format!("<!-- {marker}:BEGIN -->\nLost"),
            format!(
                "<!-- {marker}:BEGIN -->\n<!-- {marker}:BEGIN -->\n<!-- {marker}:END -->\n<!-- {marker}:END -->"
            ),
            format!(
                "<!-- COMMENTS:BEGIN -->\n<!-- {marker}:END -->\n<!-- COMMENTS:END -->\n## {title}\n- [ ] Real"
            ),
            format!(
                "<!-- COMMENTS:BEGIN -->\nUnclosed\n<!-- COMMENTS:BEGIN -->\n<!-- {marker}:END -->\n<!-- COMMENTS:END -->\n## {title}\n- [ ] Real"
            ),
            format!(
                "<!-- {marker}:BEGIN -->\n- [ ] #1 First\n<!-- {marker}:END -->\n\n## {title}\n- [ ] Legacy"
            ),
            format!(
                "<!-- {marker}:BEGIN -->\n- [ ] #1 First\n<!-- {marker}:END -->\n\n## {title}\n<!-- {marker}:BEGIN -->\n- [x] #8 Second\n<!-- {marker}:END -->"
            ),
            format!("Prose <!-- {marker}:BEGIN --> inline\n\n## {title}\n- [x] Legacy"),
            format!(
                "<!-- COMMENTS:BEGIN -->\n## {title}\n<!-- {marker}:BEGIN -->\nFake\n<!-- {marker}:END -->\n<!-- COMMENTS:END -->"
            ),
        ] {
            let input = format!("## {title}\n{body}");
            cases.push(case("criteria", &input, json!({"kind":marker})));
            cases.push(case("marks", &input, json!({"kind":marker})));
            cases.push(case(
                "checklist",
                &input,
                json!({"kind":marker,"criteria":[]}),
            ));
        }
    }
    let comments = json!([{"index":1,"body":"First\n\n## Child\n\nTail","createdDate":"2026-05-31 10:45","author":" @Alex \n Team "},{"index":2,"body":"Second","createdDate":""}]);
    for input in [
        "",
        "## Description\n\nDetails",
        "## Comments\nFreeform\n\n## Custom\nKeep",
        "## Comments\n<!-- COMMENTS:BEGIN -->\nauthor: Alex\ncreated: 2026-05-31 10:45\n---\nFirst\n---\n<!-- COMMENTS:END -->",
        "## Comments\n\n<!-- COMMENTS:BEGIN -->\n<!-- COMMENT:BEGIN -->\nindex: 7tail\nauthor: Alex\ncreated: 2026-05-31 10:45\n\nFirst\n\n## Child\nTail\n<!-- COMMENT:END -->\n<!-- COMMENTS:END -->",
        "## Description\n\n<!-- SECTION:DESCRIPTION:BEGIN -->\n~~~md\n## Comments\n<!-- COMMENTS:BEGIN -->\n---\nFake\n---\n<!-- COMMENTS:END -->\n~~~\n<!-- SECTION:DESCRIPTION:END -->",
        "## Implementation Notes\n\nProgress\n\n## Final Summary\n\nDone",
        "## Comments\n\n<!-- COMMENTS:BEGIN -->\n---\nFirst\n---\n\n---\nSecond\n---\n<!-- COMMENTS:END -->",
        "## Comments\n<!-- COMMENTS:BEGIN -->\n---\nIncomplete",
    ] {
        for newline in ["\n", "\r\n"] {
            let input = input.replace('\n', newline);
            cases.push(case("comments", &input, json!({})));
            cases.push(case("writeComments", &input, json!({"comments": comments})));
            cases.push(case("writeComments", &input, json!({"comments": []})));
        }
    }
    for field in ["body", "author", "createdDate"] {
        for bad in [
            "<!-- COMMENT:BEGIN -->",
            "line\n---\ntail",
            "---",
            "<!-- comments:END -->",
            " ",
            "plain\r\nnext",
        ] {
            let mut item = json!({"index":1, "body":"Valid", "author":"Alex", "createdDate":"2026-01-01 10:00"});
            item[field] = json!(bad);
            cases.push(case("writeComments", "", json!({"comments":[item]})));
        }
    }
    compare_reference(&cases);
}

#[test]
fn boundary_and_blank_line_rules_match_actual_typescript() {
    let mut cases = Vec::new();
    let keys = [
        ("description", "Description", "DESCRIPTION"),
        ("implementationPlan", "Implementation Plan", "PLAN"),
        ("implementationNotes", "Implementation Notes", "NOTES"),
        ("finalSummary", "Final Summary", "FINAL_SUMMARY"),
    ];
    for (name, title, marker) in keys {
        for body in [
            "",
            " ",
            "\t ",
            "\n",
            "\n\n",
            "Old",
            "Old\n\n\nTail",
            "Old\n## Notes\nPreserved",
            "Old\n## Notes & Comments (Optional)\nTail",
        ] {
            for gap in ["\n", "\n\n", " \t\n\n\n"] {
                for next in [
                    "",
                    "## Acceptance Criteria\n- [ ] Legacy",
                    "## Implementation Notes\n\nOther",
                    "## Final Summary\n\nSummary",
                    "## Acceptance Criteria (Optional)\nExtra",
                ] {
                    let input = format!("## {title}{gap}{body}\n\n{next}");
                    cases.push(case("extract", &input, json!({"key":name})));
                    cases.push(case("sections", &input, json!({"changes":{name:"New"}})));
                }
            }
        }
        for marker_body in [
            format!("<!-- SECTION:{marker}:BEGIN -->\nBody\n<!-- SECTION:{marker}:END -->"),
            format!("<!-- SECTION:{marker}:BEGIN -->\nBody\n<!-- SECTION:{marker}:END -->\t "),
            format!("<!-- SECTION:{marker}:BEGIN -->\nBody"),
            format!("Prefix <!-- SECTION:{marker}:BEGIN --> inline\nTail"),
        ] {
            let input = format!("## {title}\n\n{marker_body}");
            cases.push(case("extract", &input, json!({"key":name})));
            cases.push(case("sections", &input, json!({"changes":{name:"New"}})));
        }
    }
    let fence = "\x60\x60\x60";
    let bodies = vec![
        format!("before\n\n\n{fence}md\n\n\ncode\n\n\n{fence}\n\n\nafter"),
        format!("{fence}\n\n\ncode"),
        format!("    {fence}\n\n\nx"),
        format!("   {fence}\n\n\nx"),
        format!("{fence}js \x60example\x60\n\n\nx"),
        format!("~~~js {fence}\n\n\nx"),
        format!("- item\n\n    {fence}\n    code\n\n\n    more\n    {fence}\nafter"),
        format!(
            "1. outer\n   - inner\n\n       {fence}\n       code\n\n\n       more\n       {fence}\nafter"
        ),
        format!("<!--\n{fence}\n--!>\nafter\n\n\nmore"),
        format!("<div>\n{fence}\n</div>\n\n\nmore"),
        format!("- item\n    <div>\n    {fence}\n\n\n    code\n    </div>\nafter\n\n\nmore"),
        "<!--\n<!-- SECTION:DESCRIPTION:END -->\n\n\nmore".to_owned(),
        "<!--\n<!-- SECTION:DESCRIPTION:END -->  \n\n\nmore".to_owned(),
        format!("<pre>\n<!-- SECTION:DESCRIPTION:END -->  \n{fence}\n\n\ncode"),
    ];
    for body in bodies {
        cases.push(case(
            "sections",
            "",
            json!({"changes":{"implementationNotes":body,"finalSummary":"alpha\n\n\nbeta"}}),
        ));
    }
    for family in ["AC", "DOD"] {
        let title = if family == "AC" {
            "Acceptance Criteria"
        } else {
            "Definition of Done"
        };
        for body in [
            "- [ ] #1 A\n- [ ] #2 B",
            "<!-- SECTION:CUSTOM:BEGIN -->\n- [ ] Opaque\n<!-- SECTION:CUSTOM:END -->\n- [ ] #1 A",
            "~~~\n- [X] Example\n~~~\n- [ ] #1 A",
            "<!--\n\x60\x60\x60\n--!>\n- [X] Invalid",
        ] {
            for prefix in ["", "~~~\n", "> quoted\n"] {
                let input = format!(
                    "{prefix}## {title}\n<!-- {family}:BEGIN -->\n{body}\n<!-- {family}:END -->"
                );
                cases.push(case("criteria", &input, json!({"kind":family})));
                cases.push(case("marks", &input, json!({"kind":family})));
            }
        }
    }
    compare_reference(&cases);
}
