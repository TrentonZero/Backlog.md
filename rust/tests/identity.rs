use backlog_md::identity::{
    canonical_task_id, extract_id_body, extract_id_from_filename, generate_next_id,
    generate_next_subtask_id, ids_equal, is_valid_task_id, normalize_id, normalize_task_id,
    task_ids_equal,
};
use serde_json::Value;
use std::process::{Command, Stdio};

#[test]
fn task_id_helpers_match_live_typescript() {
    const REFERENCE: &str = r#"
import { normalizeId, extractIdBody, extractIdFromFilename, generateNextId, generateNextSubtaskId, idsEqual } from "./src/utils/prefix-config.ts";
import { normalizeTaskId, canonicalTaskId, taskIdsEqual, isValidTaskId } from "./src/utils/task-id.ts";
process.stdout.write(JSON.stringify([
  normalizeId(" task-001.02 ", "task"),
  extractIdBody("JIRA-007.2", "jira"),
  extractIdFromFilename("TASK-003.02 - Child.md", "task"),
  extractIdFromFilename("task-12abc - Legacy.md", "task"),
  generateNextId(["task-1", "TASK-001", "task-8.9", "JIRA-90", "task-999999999999999999999999"], "task", 3),
  generateNextSubtaskId(["task-5", "task-005.01", "task-5.2", "task-5.2.1"], "5", "task", 4),
  idsEqual("123", "task-123", "task"),
  normalizeTaskId("jira-001.02"),
  canonicalTaskId("jira-001.02"),
  taskIdsEqual("task-000.01", "TASK-0.1"),
  taskIdsEqual("task-12x", "task-12"),
  isValidTaskId("legacy-12_part.2"),
  isValidTaskId("12x"),
  normalizeId("task-", "task"),
  extractIdBody("task-", "task")
]));
"#;
    let reference_bin = std::env::var_os("BACKLOG_REFERENCE_BUN").unwrap_or_else(|| "bun".into());
    let child = Command::new(reference_bin)
        .args(["--eval", REFERENCE])
        .current_dir(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap(),
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("live identity comparisons require Bun");
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    let actual = serde_json::json!([
        normalize_id(" task-001.02 ", "task"),
        extract_id_body("JIRA-007.2", "jira"),
        extract_id_from_filename("TASK-003.02 - Child.md", "task"),
        extract_id_from_filename("task-12abc - Legacy.md", "task"),
        generate_next_id(
            &[
                "task-1",
                "TASK-001",
                "task-8.9",
                "JIRA-90",
                "task-999999999999999999999999"
            ],
            "task",
            Some(3)
        ),
        generate_next_subtask_id(
            &["task-5", "task-005.01", "task-5.2", "task-5.2.1"],
            "5",
            "task",
            Some(4)
        ),
        ids_equal("123", "task-123", "task"),
        normalize_task_id("jira-001.02", "task"),
        canonical_task_id("jira-001.02", "task"),
        task_ids_equal("task-000.01", "TASK-0.1", "task"),
        task_ids_equal("task-12x", "task-12", "task"),
        is_valid_task_id("legacy-12_part.2"),
        is_valid_task_id("12x"),
        normalize_id("task-", "task"),
        extract_id_body("task-", "task")
    ]);
    assert_eq!(actual.as_array().unwrap(), &expected);
}
