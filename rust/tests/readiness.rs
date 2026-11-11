use backlog_md::{
    readiness::{ReadinessGraph, TaskReadiness},
    records::{TaskRecord, parse_task},
};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::{Command, Stdio},
};

fn task(id: &str, title: &str, status: &str, dependencies: &[&str]) -> TaskRecord {
    let mut record = parse_task(&format!(
        "---\nid: {id}\ntitle: {title}\nstatus: {status}\ncreated_date: 2026-01-01\n---\n"
    ))
    .unwrap();
    record.dependencies = dependencies.iter().map(|id| (*id).to_owned()).collect();
    record
}

fn readiness_json(readiness: TaskReadiness) -> Value {
    json!({
        "isReady": readiness.is_ready,
        "isBlocked": readiness.is_blocked,
        "blockingDependencies": readiness.blocking_dependencies,
        "missingDependencies": readiness.missing_dependencies,
    })
}

fn reference(script: &str) -> Value {
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
fn readiness_matches_live_typescript_for_aliases_blockers_missing_and_ambiguous_records() {
    let active = vec![
        task(
            "TASK-10",
            "Target",
            "To Do",
            &[
                "task-01", "TASK-2", "TASK-404", "TASK-8", "TASK-9", "DRAFT-4",
            ],
        ),
        task("TASK-2", "Unfinished", "In Progress", &[]),
        task("TASK-3", "Other", "Done", &[]),
        task("TASK-6", "No dependencies", "To Do", &[]),
        task("TASK-5", "Already finished", "Done", &["TASK-2"]),
        task("TASK-8", "Eight", "Done", &[]),
    ];
    let archived = vec![
        task("TASK-1", "Archived finished", "Done", &[]),
        task("TASK-08", "Ambiguous duplicate", "Done", &[]),
        task("TASK-7", "Archived target", "To Do", &["TASK-2"]),
    ];
    let ambiguous_ids = ["TASK-9".to_owned()];
    let statuses = ["To Do".into(), "In Progress".into(), "Done".into()];
    let graph = ReadinessGraph::new(&active, &archived, &statuses, &ambiguous_ids, "task");
    let rust = json!({
        "target": readiness_json(graph.get_task_readiness(&active[0])),
        "ready": readiness_json(graph.get_task_readiness(&active[3])),
        "finished": readiness_json(graph.get_task_readiness(&active[4])),
        "archived": readiness_json(graph.get_task_readiness(&archived[2])),
    });
    let expected = reference(
        r#"
import { createReadinessGraph, getTaskReadiness } from "./src/utils/readiness.ts";
const tasks = [
  {id:"TASK-10",title:"Target",status:"To Do",dependencies:["task-01","TASK-2","TASK-404","TASK-8","TASK-9","DRAFT-4"]},
  {id:"TASK-2",title:"Unfinished",status:"In Progress",dependencies:[]},
  {id:"TASK-3",title:"Other",status:"Done",dependencies:[]},
  {id:"TASK-6",title:"No dependencies",status:"To Do",dependencies:[]},
  {id:"TASK-5",title:"Already finished",status:"Done",dependencies:["TASK-2"]},
  {id:"TASK-8",title:"Eight",status:"Done",dependencies:[]},
];
const archived = [
  {id:"TASK-1",title:"Archived finished",status:"Done",dependencies:[]},
  {id:"TASK-08",title:"Ambiguous duplicate",status:"Done",dependencies:[]},
  {id:"TASK-7",title:"Archived target",status:"To Do",dependencies:["TASK-2"]},
];
// DRAFT-4 exists as a draft in the project, but task detail readiness deliberately excludes drafts.
const graph = createReadinessGraph({tasks,archivedTasks:archived,statuses:["To Do","In Progress","Done"],ambiguousIds:new Set(["TASK-9"])});
process.stdout.write(JSON.stringify({target:getTaskReadiness(tasks[0],graph),ready:getTaskReadiness(tasks[3],graph),finished:getTaskReadiness(tasks[4],graph),archived:getTaskReadiness(archived[2],graph)}));
"#,
    );
    assert_eq!(expected, rust);
}

#[test]
fn readiness_uses_default_and_custom_terminal_statuses_like_typescript() {
    let target = task("TASK-2", "Target", "To Do", &["TASK-1"]);
    let dependency = task("TASK-1", "Dependency", "Done", &[]);
    let tasks = [target.clone(), dependency.clone()];
    let default_graph = ReadinessGraph::new(&tasks, &[], &[], &[], "task");
    let custom_graph = ReadinessGraph::new(
        &tasks,
        &[],
        &["To Do".into(), "In Progress".into(), "Shipped".into()],
        &[],
        "task",
    );
    let rust = json!({
        "default": readiness_json(default_graph.get_task_readiness(&target)),
        "custom": readiness_json(custom_graph.get_task_readiness(&target)),
    });
    let expected = reference(
        r#"
import { createReadinessGraph, getTaskReadiness } from "./src/utils/readiness.ts";
const task = {id:"TASK-2",title:"Target",status:"To Do",dependencies:["TASK-1"]};
const dependency = {id:"TASK-1",title:"Dependency",status:"Done",dependencies:[]};
const defaultGraph = createReadinessGraph({tasks:[task,dependency],statuses:[]});
const customGraph = createReadinessGraph({tasks:[task,dependency],statuses:["To Do","In Progress","Shipped"]});
process.stdout.write(JSON.stringify({default:getTaskReadiness(task,defaultGraph),custom:getTaskReadiness(task,customGraph)}));
"#,
    );
    assert_eq!(expected, rust);
}
