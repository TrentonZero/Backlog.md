use backlog_md::{
    records::{TaskRecord, parse_task},
    task_filtering::{LabelMatch, TaskFilter, filter_tasks},
};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::{Command, Stdio},
};

fn task(id: &str, status: &str) -> TaskRecord {
    parse_task(&format!(
        "---\nid: {id}\ntitle: {id}\nstatus: {status}\ncreated_date: 2026-01-01\n---\n"
    ))
    .unwrap()
}

fn ids<'a>(tasks: &[&'a TaskRecord]) -> Vec<&'a str> {
    tasks.iter().map(|task| task.id.as_str()).collect()
}

fn reference() -> Value {
    let script = r#"
import { createTaskFilterMatcher } from "./src/utils/task-search.ts";
const tasks = [
 {id:"TASK-1",status:"To Do",type:" Feature ",project:" Alpha ",priority:" High ",assignee:[" Alice ",""],labels:["UI","bug"],parentTaskId:"TASK-01"},
 {id:"TASK-2",status:"Done",type:"docs",project:"beta",assignee:[" "],labels:["infra"]},
 {id:"TASK-3",status:"To Do",type:"task",project:"Alpha",assignee:[],labels:[]},
];
const selected = options => tasks.filter(createTaskFilterMatcher(options,tasks)).map(task=>task.id);
process.stdout.write(JSON.stringify({
 combined:selected({status:[" to do "],excludeStatus:["closed"],type:["FEATURE"],project:["ALPHA"],priority:["high"],assignee:["alice"],labels:["ui","missing"]}),
 labelsAllParent:selected({labels:["ui"," BUG "],labelMatch:"all",parentTaskId:"task-1"}),
 unassigned:selected({unassigned:true}),
 excluded:selected({excludeStatus:[" to do "]}),
 empty:selected({status:[" ",""],type:[" ",""],project:[],priority:[" "],assignee:[" "],labels:[" "]}),
}));
"#;
    let output =
        Command::new(std::env::var_os("BACKLOG_REFERENCE_BUN").unwrap_or_else(|| "bun".into()))
            .args(["--eval", script])
            .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
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

#[test]
fn task_field_filters_match_typescript_normalization_and_composition() {
    let mut first = task("TASK-1", "To Do");
    first.task_type = Some(" Feature ".into());
    first.project = Some(" Alpha ".into());
    first.priority = Some(" High ".into());
    first.assignee = vec![" Alice ".into(), "".into()];
    first.labels = vec!["UI".into(), "bug".into()];
    first.parent_task_id = Some("TASK-01".into());
    let mut second = task("TASK-2", "Done");
    second.task_type = Some("docs".into());
    second.project = Some("beta".into());
    second.assignee = vec![" ".into()];
    second.labels = vec!["infra".into()];
    let mut third = task("TASK-3", "To Do");
    third.task_type = Some("task".into());
    third.project = Some("Alpha".into());
    let tasks = vec![first, second, third];
    let combined = TaskFilter {
        statuses: vec![" to do ".into()],
        exclude_statuses: vec!["closed".into()],
        types: vec!["FEATURE".into()],
        projects: vec!["ALPHA".into()],
        priorities: vec!["high".into()],
        assignees: vec!["alice".into()],
        labels: vec!["ui".into(), "missing".into()],
        ..TaskFilter::default()
    };
    let labels_all_parent = TaskFilter {
        labels: vec!["ui".into(), " BUG ".into()],
        label_match: LabelMatch::All,
        parent_task_id: Some("task-1".into()),
        ..TaskFilter::default()
    };
    let unassigned = TaskFilter {
        unassigned: true,
        ..TaskFilter::default()
    };
    let excluded = TaskFilter {
        exclude_statuses: vec![" to do ".into()],
        ..TaskFilter::default()
    };
    let empty = TaskFilter {
        statuses: vec![" ".into(), "".into()],
        types: vec![" ".into(), "".into()],
        priorities: vec![" ".into()],
        assignees: vec![" ".into()],
        labels: vec![" ".into()],
        ..TaskFilter::default()
    };
    let active = tasks.as_slice();
    let rust = json!({
        "combined": ids(&filter_tasks(active, &combined, "task")),
        "labelsAllParent": ids(&filter_tasks(active, &labels_all_parent, "task")),
        "unassigned": ids(&filter_tasks(active, &unassigned, "task")),
        "excluded": ids(&filter_tasks(active, &excluded, "task")),
        "empty": ids(&filter_tasks(active, &empty, "task")),
    });
    assert_eq!(reference(), rust);
}
