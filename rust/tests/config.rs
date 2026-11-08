//! Stored-config contracts: src/file-system/operations.ts:118-255, 2315-2581.
use backlog_md::config::{Config, ConfigLocation, parse_config, serialize_config};
use std::path::Path;

fn parse(source: &str) -> Config {
    parse_config(source, Path::new("project/backlog/config.yml")).unwrap()
}

#[test]
fn sparse_defaults_do_not_initialize_optional_settings() {
    let config = parse("");
    assert_eq!(config.project_name, "");
    assert_eq!(config.statuses, ["To Do", "In Progress", "Done"]);
    assert_eq!(config.labels, Vec::<String>::new());
    assert_eq!(config.date_format, "yyyy-mm-dd");
    assert_eq!(config, Config::default());
    assert_eq!(config.default_assignee, None);
    assert_eq!(config.default_port, None);
    assert_eq!(config.auto_commit, None);
    assert_eq!(config.types, None);
    assert_eq!(parse("statuses: []\n").statuses, Vec::<String>::new());
    assert_eq!(parse("statuses:\n").statuses, config.statuses);
    assert_eq!(parse("default_assignee:\n").default_assignee, Some(vec![]));
    assert_eq!(parse("projectName: ignored\n").project_name, "");
}

#[test]
fn only_actual_scalar_aliases_are_accepted_and_last_line_wins() {
    for (key, canonical) in [
        ("filesystemOnly", "filesystem_only"),
        ("onStatusChange", "on_status_change"),
        ("backlogDirectory", "backlog_directory"),
    ] {
        let value = if key == "filesystemOnly" {
            "true"
        } else {
            "'custom'"
        };
        assert_eq!(
            parse(&format!("{key}: {value}\n")),
            parse(&format!("{canonical}: {value}\n"))
        );
    }
    assert_eq!(
        parse("filesystem_only: true\nfilesystemOnly: false\n").filesystem_only,
        Some(false)
    );
}

#[test]
fn lists_preserve_yaml_quoting_and_legacy_single_assignee() {
    let flow = parse(
        r#"statuses: ["a, b", "a\"b\\c"]
labels: [x]
types: [task]
priorities: [high]
projects: [P]
default_assignee: " @alex " # owner
"#,
    );
    assert_eq!(flow.statuses, ["a, b", "a\"b\\c"]);
    assert_eq!(
        flow.statuses,
        parse("statuses:\n  - 'a, b'\n  - 'a\"b\\c'\n").statuses
    );
    assert_eq!(flow.default_assignee, Some(vec!["@alex".into()]));
    assert_eq!(flow.types, Some(vec!["task".into()]));
    assert_eq!(flow.priorities, Some(vec!["high".into()]));
    assert_eq!(flow.projects, Some(vec!["P".into()]));
}

#[test]
fn list_members_use_javascript_string_conversion_then_trim() {
    let config =
        parse("statuses: [1, 007, 2.0, true, null, [a, null, false], {name: x}, ' ', ' x ']\n");
    assert_eq!(
        config.statuses,
        [
            "1",
            "7",
            "2",
            "true",
            "null",
            "a,,false",
            "[object Object]",
            "x"
        ]
    );
    assert_eq!(
        parse("statuses: [1e21, 1e-7, 0.000001, -0, 1000000000000000100]\n").statuses,
        ["1e+21", "1e-7", "0.000001", "0", "1000000000000000100"]
    );
}

#[test]
fn wrong_list_shapes_name_the_owning_key_path_and_type() {
    for key in [
        "statuses",
        "labels",
        "types",
        "priorities",
        "projects",
        "default_assignee",
    ] {
        for (value, actual) in [
            ("42", "a number"),
            ("007", "a number"),
            ("true", "a boolean"),
            ("{name: x}", "a mapping"),
            ("ordinary", "a scalar"),
        ] {
            if key == "default_assignee" && value == "ordinary" {
                continue;
            }
            let error = parse_config(
                &format!("{key}: {value}\n"),
                Path::new("project/config.yml"),
            )
            .unwrap_err();
            assert_eq!(error.key, key);
            assert_eq!(error.path, Path::new("project/config.yml"));
            let expected = if key == "default_assignee" {
                "a list or a single name"
            } else {
                "a list"
            };
            assert!(
                error
                    .problem
                    .contains(&format!("expected {expected}, got {actual}")),
                "{error}"
            );
            assert!(error.to_string().starts_with(
                "Backlog could not start because project/config.yml has an invalid value for"
            ));
            assert!(error.remedy.contains("Edit that key"));
        }
    }
}

#[test]
fn malformed_selected_keys_fail_without_parsing_unread_keys() {
    for key in [
        "statuses",
        "labels",
        "types",
        "priorities",
        "projects",
        "default_assignee",
    ] {
        for value in ["[\"bad", "[\"bad]"] {
            let error =
                parse_config(&format!("{key}: {value}\n"), Path::new("P/config.yml")).unwrap_err();
            assert_eq!(error.key, key);
            assert!(error.remedy.contains("valid YAML"));
        }
    }
    for key in ["custom-setting", "my.setting", "\"quoted-key\"", "2fa"] {
        assert_eq!(
            parse(&format!("statuses:\n- 'a: b'\n- Done\n{key}: [\"broken]\n")).statuses,
            ["a: b", "Done"]
        );
    }
}

#[test]
fn top_level_lists_outrank_nested_matches_and_last_duplicate_wins() {
    for source in [
        "statuses: [old]\nstatuses: [top]\nmeta:\n  statuses: [nested]\n",
        "statuses:\n  - top\nmeta: |\n  statuses: [\"broken\n",
    ] {
        assert_eq!(parse(source).statuses, ["top"]);
    }
    for source in [
        "\tstatuses: [legacy]\n",
        "  statuses: [legacy]\n",
        "\tstatuses:\n\t  - legacy\n",
    ] {
        assert_eq!(parse(source).statuses, ["legacy"]);
    }
    assert_eq!(
        parse("  statuses: [old]\n  statuses: [last]\n").statuses,
        ["last"]
    );
}

#[test]
fn selected_blocks_retry_document_context_for_external_anchors() {
    let config = parse("statuses: &workflow [To Do, Done]\nlabels: *workflow\n");
    assert_eq!(config.labels, ["To Do", "Done"]);
    assert_eq!(
        parse("default_status: &s 'To Do'\nstatuses: [*s]\n").statuses,
        ["To Do"]
    );
    let error = parse_config(
        "statuses: *missing\ncustom-setting: [\"bad]\n",
        Path::new("P.yml"),
    )
    .unwrap_err();
    assert_eq!(error.key, "statuses");
    assert!(error.problem.contains("anchor"), "{error:?}");
    assert!(!error.to_string().contains("frontmatter"), "{error}");
}

#[test]
fn definition_of_done_keeps_only_trimmed_nonempty_strings() {
    assert_eq!(
        parse("definition_of_done:\n").definition_of_done,
        Some(vec![])
    );
    assert_eq!(
        parse("definition_of_done: [' first ', 1, null, true, {}, [], ' ', 'a, b']\n")
            .definition_of_done,
        Some(vec!["first".into(), "a, b".into()])
    );
    assert_eq!(
        parse("definition_of_done:\n\n  - First\n\n  - Second\n").definition_of_done,
        Some(vec!["First".into(), "Second".into()])
    );
    for value in ["[\"bad]", "ordinary", "{name: x}"] {
        assert_eq!(
            parse(&format!("definition_of_done: {value}\n")).definition_of_done,
            None
        );
    }
    assert_eq!(
        parse("shared: &done [First]\ndefinition_of_done: *done\n").definition_of_done,
        Some(vec!["First".into()])
    );
    assert_eq!(
        parse("base: &defaults {definition_of_done: [Check]}\n<<: *defaults\n").definition_of_done,
        None
    );
}

#[test]
fn definition_of_done_preserves_legacy_windows_paths_without_record_preprocessing() {
    let config = parse(
        r#"definition_of_done: ["Run .\scripts\check.ps1", "Run C:\temp\tasks", "say \"yes\""]
"#,
    );
    assert_eq!(
        config.definition_of_done,
        Some(vec![
            r"Run .\scripts\check.ps1".into(),
            r"Run C:\temp\tasks".into(),
            "say \"yes\"".into()
        ])
    );
    assert_eq!(
        parse("definition_of_done: [@alex]\n").definition_of_done,
        None
    );
}

#[test]
fn scalar_lines_keep_legacy_quote_boolean_and_parse_int_semantics() {
    let config = parse(
        "project_name: \"O'Reilly\"\ndefault_editor: 'a\"b'\ndefault_reporter: '@alex'\ndefault_status: 'To Do'\non_status_change: \"echo 'hello'\"\ntask_prefix: 'BACK'\nmax_column_width: 20px\ndefault_port: +6420tail\nzero_padded_ids: -03.9\nactive_branch_days: unreadable\nauto_open_browser: TRUE\nhide_empty_columns: 'true'\nremote_operations: true # comment\nauto_commit: false\nbypass_git_hooks: True\ncheck_active_branches: yes\n",
    );
    assert_eq!(config.project_name, "OReilly");
    assert_eq!(config.default_editor.as_deref(), Some("ab"));
    assert_eq!(config.default_reporter.as_deref(), Some("@alex"));
    assert_eq!(config.default_status.as_deref(), Some("To Do"));
    assert_eq!(config.on_status_change.as_deref(), Some("echo 'hello'"));
    assert_eq!(config.task_prefix.as_deref(), Some("BACK"));
    assert_eq!(config.max_column_width, Some(20.0));
    assert_eq!(config.default_port, Some(6420.0));
    assert_eq!(config.zero_padded_ids, Some(-3.0));
    assert!(config.active_branch_days.unwrap().is_nan());
    assert_eq!(config.auto_open_browser, Some(true));
    assert_eq!(config.hide_empty_columns, Some(false));
    assert_eq!(config.remote_operations, Some(false));
    assert_eq!(config.auto_commit, Some(false));
    assert_eq!(config.bypass_git_hooks, Some(true));
    assert_eq!(config.check_active_branches, Some(false));
    assert_eq!(parse("date_format: ''\n").date_format, "yyyy-mm-dd");
    for value in ["'20'", "", "NaN"] {
        assert!(
            parse(&format!("max_column_width: {value}\n"))
                .max_column_width
                .unwrap()
                .is_nan()
        );
    }
}

#[test]
fn serializer_keeps_fixed_order_root_selection_false_values_and_trailing_newline() {
    let config = parse(
        "project_name: P\ndefault_assignee: ['@alex']\ndefault_reporter: R\ndefault_status: To Do\nstatuses: [To Do, Done]\nlabels: [bug]\ntypes: [task]\npriorities: [high]\nprojects: [P]\ndefinition_of_done: [Check]\nmax_column_width: 20\ndefault_editor: vi\nauto_open_browser: false\nhide_empty_columns: false\ndefault_port: 6420\nremote_operations: false\nauto_commit: false\nfilesystem_only: false\nzero_padded_ids: 0\nbypass_git_hooks: false\ncheck_active_branches: false\nactive_branch_days: 0\nonStatusChange: echo\ntask_prefix: BACK\nbacklog_directory: ignored\n",
    );
    let expected = "project_name: \"P\"\ndefault_assignee: [\"@alex\"]\ndefault_reporter: \"R\"\ndefault_status: \"To Do\"\nstatuses: [\"To Do\", \"Done\"]\nlabels: [\"bug\"]\ntypes: [\"task\"]\npriorities: [\"high\"]\nprojects: [\"P\"]\ndefinition_of_done: [\"Check\"]\ndate_format: yyyy-mm-dd\nmax_column_width: 20\ndefault_editor: \"vi\"\nauto_open_browser: false\nhide_empty_columns: false\ndefault_port: 6420\nremote_operations: false\nauto_commit: false\nfilesystem_only: false\nzero_padded_ids: 0\nbypass_git_hooks: false\ncheck_active_branches: false\nactive_branch_days: 0\nonStatusChange: 'echo'\ntask_prefix: \"BACK\"\n";
    assert_eq!(
        serialize_config(&config, ConfigLocation::Folder, "selected"),
        expected
    );
    let root = serialize_config(&config, ConfigLocation::Root, "selected");
    assert_eq!(root, format!("{expected}backlog_directory: \"selected\"\n"));
    let mut persisted = config;
    persisted.backlog_directory = Some("selected".into());
    assert_eq!(parse(&root), persisted);
}

#[test]
fn serializer_omits_optional_empty_lists_and_truthy_numeric_options_only() {
    let mut config = parse(
        "statuses: []\nlabels: []\ntypes: []\npriorities: []\nprojects: []\ndefault_assignee: []\ndefinition_of_done: []\nmax_column_width: 0\ndefault_port: invalid\nzero_padded_ids: invalid\nactive_branch_days: invalid\n",
    );
    assert_eq!(
        serialize_config(&config, ConfigLocation::Folder, "backlog"),
        "project_name: \"\"\nstatuses: []\nlabels: []\ndefinition_of_done: []\ndate_format: yyyy-mm-dd\nzero_padded_ids: NaN\nactive_branch_days: NaN\n"
    );
    config.definition_of_done = None;
    assert!(
        !serialize_config(&config, ConfigLocation::Folder, "backlog")
            .contains("definition_of_done")
    );
}

#[test]
fn serializer_escapes_assignee_and_definition_of_done_without_changing_other_quoting() {
    let config = Config {
        default_assignee: Some(vec!["@a\"b\\c".into(), "@second".into()]),
        definition_of_done: Some(vec![
            "  say \"yes\"  ".into(),
            " ".into(),
            r"Run C:\temp\tasks".into(),
        ]),
        ..Config::default()
    };
    let saved = serialize_config(&config, ConfigLocation::Folder, "backlog");
    let loaded = parse(&saved);
    assert_eq!(loaded.default_assignee, config.default_assignee);
    assert_eq!(
        loaded.definition_of_done,
        Some(vec!["say \"yes\"".into(), r"Run C:\temp\tasks".into()])
    );
    let raw = Config {
        project_name: "a\"b".into(),
        statuses: vec!["a\"b".into()],
        ..Config::default()
    };
    assert!(
        serialize_config(&raw, ConfigLocation::Folder, "backlog")
            .starts_with("project_name: \"a\"b\"\nstatuses: [\"a\"b\"]\n")
    );
}
