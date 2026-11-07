use backlog_md::frontmatter::{parse_frontmatter, update_frontmatter};
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn reads_a_record_without_rewriting_its_body() {
    let source =
        "---\nid: TASK-1\ntitle: Original\nlabels: [one, 'two']\n---\n\n## Description\n\n`code`\n";
    let parsed = parse_frontmatter(source).unwrap();
    assert_eq!(parsed.data["id"], "TASK-1");
    assert_eq!(parsed.data["labels"], json!(["one", "two"]));
    assert_eq!(parsed.content, "\n## Description\n\n`code`\n");
}

#[test]
fn missing_or_empty_frontmatter_keeps_the_complete_body() {
    let source = "## Notes\n\n---\nbody_key: stays body text\n---\n";
    let parsed = parse_frontmatter(source).unwrap();
    assert!(parsed.data.is_empty());
    assert_eq!(parsed.content, source);
    let empty = parse_frontmatter("---\n---\n\nbody\n").unwrap();
    assert!(empty.data.is_empty());
    assert_eq!(empty.content, "\nbody\n");
}

#[test]
fn accepts_existing_assignee_and_reporter_conventions() {
    for (header, expected) in [
        ("assignee: @alex\n", json!("@alex")),
        (
            "assignee: [@alex, 'someone']\n",
            json!(["@alex", "someone"]),
        ),
        ("assignee: ['already quoted']\n", json!(["already quoted"])),
    ] {
        assert_eq!(
            parse_frontmatter(&format!("---\n{header}---\n"))
                .unwrap()
                .data["assignee"],
            expected
        );
    }
    assert_eq!(
        parse_frontmatter("---\nreporter: @sam\n---\n")
            .unwrap()
            .data["reporter"],
        "@sam"
    );
}

#[test]
fn preserves_written_due_dates_and_timestamp_offsets() {
    for value in [
        "2026-02-01",
        "2026-02-01T23:30:00-08:00",
        "2026-02-01 00:00:00Z",
    ] {
        assert_eq!(
            parse_frontmatter(&format!("---\ndue_date: {value}\n---\n"))
                .unwrap()
                .data["due_date"],
            value
        );
    }
    assert_eq!(
        parse_frontmatter("---\n'due_date': 2026-02-01 # written day\n---\n")
            .unwrap()
            .data["due_date"],
        "2026-02-01"
    );
    assert_eq!(
        parse_frontmatter("---\ndue_date: null\n---\n")
            .unwrap()
            .data["due_date"],
        json!(null)
    );
    assert_eq!(
        parse_frontmatter("---\ncreated_date: 2026-02-01T00:00:00+02:00\n---\n")
            .unwrap()
            .data["created_date"],
        "2026-02-01T00:00:00+02:00"
    );
}

#[test]
fn keeps_crlf_and_bom_outside_the_semantic_header() {
    let parsed = parse_frontmatter("\u{feff}---\r\nid: TASK-1\r\n---\r\n\r\nbody\r\n").unwrap();
    assert_eq!(parsed.data["id"], "TASK-1");
    assert_eq!(parsed.content, "\r\nbody\r\n");
}

#[test]
fn repeated_invalid_yaml_and_missing_closing_delimiters_fail() {
    for _ in 0..2 {
        assert!(parse_frontmatter("---\ntitle: [unclosed\n---\nbody\n").is_err());
        assert!(parse_frontmatter("---\ntitle: no closing delimiter\n").is_err());
    }
}

#[test]
fn parsed_values_do_not_share_mutable_state() {
    let source = "---\nlabels: [one]\n---\n";
    let mut first = parse_frontmatter(source).unwrap();
    first.data.insert("labels".into(), json!(["changed"]));
    assert_eq!(
        parse_frontmatter(source).unwrap().data["labels"],
        json!(["one"])
    );
}

#[test]
fn reads_bare_leading_zero_integers_as_decimal_without_changing_quoted_strings() {
    let source = "---\ntitle: 世界\ncode: 007\nlabels: [007, '007']\nexplicit: !!str 007\nnumber: &number 007\ncopy: *number\n---\nBody\n";
    let parsed = parse_frontmatter(source).unwrap();
    assert_eq!(parsed.data["code"], json!(7));
    assert_eq!(parsed.data["labels"], json!([7, "007"]));
    assert_eq!(parsed.data["explicit"], "007");
    assert_eq!(parsed.data["copy"], json!(7));
    let saved = update_frontmatter(
        source,
        &BTreeMap::from([("title".into(), Some(json!("Changed")))]),
    )
    .unwrap();
    assert!(saved.contains("code: 007\n"));
    assert!(saved.contains("labels: [007, '007']\n"));
}

#[test]
fn updates_keep_unknown_yaml_and_the_entire_body() {
    let metadata = "# external approval\napproval:\n  origin: {app: example}\n  version: 1.10\n  note: |\n    title: inside a value\n    ---\nreviewed: 2026-02-01\ncode: 007\nvalues: [one, 'two'] # keep style\nexternal: &external {ratio: 1.10}\nexternal_copy: *external\n";
    let body = "\n---\nbody_key: stays in the body\n---\n\nFollowing text\n";
    let source =
        format!("---\nid: TASK-1\ntitle: Original\nassignee: [@alex]\n{metadata}---\n{body}");
    let changes = BTreeMap::from([("title".into(), Some(json!("Changed")))]);
    let saved = update_frontmatter(&source, &changes).unwrap();
    assert!(saved.contains(metadata));
    let parsed = parse_frontmatter(&saved).unwrap();
    assert_eq!(parsed.data["title"], "Changed");
    assert_eq!(parsed.data["external"], parsed.data["external_copy"]);
    assert_eq!(parsed.content, body);
    assert_eq!(update_frontmatter(&saved, &changes).unwrap(), saved);
}

#[test]
fn unchanged_updates_return_the_original_source() {
    let source =
        "\u{feff}---\r\ntitle: &title 'Original' # annotation\r\nowner: *title\r\n---\r\nBody\r\n";
    assert_eq!(
        update_frontmatter(source, &BTreeMap::new()).unwrap(),
        source
    );
    assert_eq!(
        update_frontmatter(
            source,
            &BTreeMap::from([("title".into(), Some(json!("Original")))])
        )
        .unwrap(),
        source
    );
}

#[test]
fn clears_optional_fields_but_preserves_explicit_unknown_null_entries() {
    let source = "---\nunknown_before:\ntitle: Original\ntags: [old]\nupdated_date: 2026-01-01\nunknown_after:\n---\nBody\n";
    let changes = BTreeMap::from([("tags".into(), None), ("updated_date".into(), None)]);
    let saved = update_frontmatter(source, &changes).unwrap();
    let parsed = parse_frontmatter(&saved).unwrap();
    assert!(!parsed.data.contains_key("tags"));
    assert!(!parsed.data.contains_key("updated_date"));
    assert_eq!(parsed.data["unknown_before"], json!(null));
    assert_eq!(parsed.data["unknown_after"], json!(null));
    assert!(saved.contains("unknown_before:\n"));
    assert!(saved.contains("unknown_after:\n"));
    let null = update_frontmatter(
        &saved,
        &BTreeMap::from([("tags".into(), Some(json!(null)))]),
    )
    .unwrap();
    assert!(parse_frontmatter(&null).unwrap().data.contains_key("tags"));
}

#[test]
fn retains_flow_and_indented_root_mapping_source() {
    for (source, retained) in [
        (
            "---\n{id: doc-1, title: Original, external, code: 007, nested: {ratio: 1.10}}\n---\nBody",
            "nested: {ratio: 1.10}",
        ),
        (
            "---\n  id: doc-1\n  title: Original\n  ? 'owner: team'\n  :\n    code: 007\n    note: |2\n      Keep these spaces\n  ratio: 1.10\n---\nBody\n",
            "  ? 'owner: team'\n  :\n    code: 007\n    note: |2\n      Keep these spaces\n  ratio: 1.10\n",
        ),
    ] {
        let saved = update_frontmatter(
            source,
            &BTreeMap::from([("title".into(), Some(json!("Changed")))]),
        )
        .unwrap();
        assert!(saved.contains(retained), "{saved}");
        assert_eq!(parse_frontmatter(&saved).unwrap().data["title"], "Changed");
    }
}

#[test]
fn preserves_metadata_looking_lines_inside_unknown_quoted_values() {
    let unknown = "note: \"first\n# still the quoted value\ncreated_date: text\"\n";
    let source =
        format!("---\nid: doc-1\n{unknown}# title annotation\ntitle: Original\n---\nBody\n");
    let saved = update_frontmatter(
        &source,
        &BTreeMap::from([("title".into(), Some(json!("Changed")))]),
    )
    .unwrap();
    assert!(saved.contains(unknown));
    assert!(saved.contains("# title annotation"));
}

#[test]
fn changed_metadata_preserves_crlf_bom_and_body() {
    let source = "\u{feff}---\r\nid: TASK-1\r\ntitle: Original\r\nratio: 1.10 # ratio\r\n---\r\n\r\nbody\r\n";
    let saved = update_frontmatter(
        source,
        &BTreeMap::from([
            ("title".into(), Some(json!("Changed"))),
            ("labels".into(), Some(json!(["one", "two"]))),
            ("updated_date".into(), Some(json!("2026-02-01T00:00:00Z"))),
        ]),
    )
    .unwrap();
    assert!(saved.starts_with("\u{feff}---\r\n"));
    assert!(saved.contains("ratio: 1.10 # ratio\r\n"));
    assert!(saved.ends_with("---\r\n\r\nbody\r\n"));
    assert!(!saved.replace("\r\n", "").contains('\n'));
}

#[test]
fn reference_changing_updates_fail_instead_of_returning_modified_source() {
    let source = "---\nid: TASK-1\ntitle: &title Original\nowner: *title\n---\nBody\n";
    assert_eq!(parse_frontmatter(source).unwrap().data["owner"], "Original");
    for changes in [
        BTreeMap::from([("title".into(), Some(json!("Changed")))]),
        BTreeMap::from([("title".into(), None)]),
    ] {
        assert!(update_frontmatter(source, &changes).is_err());
    }
}

#[test]
fn merged_fields_cannot_silently_undo_a_requested_clear() {
    let source = "---\nid: doc-1\ntags: [old]\ndefaults: &defaults {tags: [old]}\n<<: *defaults\n---\nBody\n";
    assert_eq!(
        parse_frontmatter(source).unwrap().data["tags"],
        json!(["old"])
    );
    assert!(update_frontmatter(source, &BTreeMap::from([("tags".into(), None)])).is_err());
}

#[test]
fn adds_a_header_without_reinterpreting_existing_body_text() {
    let source = "## Notes\n\n---\nbody_key: stays body text\n---\n";
    let saved = update_frontmatter(
        source,
        &BTreeMap::from([("title".into(), Some(json!("New")))]),
    )
    .unwrap();
    let parsed = parse_frontmatter(&saved).unwrap();
    assert_eq!(parsed.data["title"], "New");
    assert_eq!(parsed.content, source);
}

#[test]
fn adds_metadata_to_a_comment_only_header_without_losing_the_comment() {
    let source = "---\n# retained header comment\n---\nBody\n";
    let saved = update_frontmatter(
        source,
        &BTreeMap::from([("title".into(), Some(json!("New")))]),
    )
    .unwrap();
    assert!(saved.contains("# retained header comment\n"));
    assert_eq!(parse_frontmatter(&saved).unwrap().data["title"], "New");
    assert_eq!(parse_frontmatter(&saved).unwrap().content, "Body\n");
}
