use backlog_md::{
    dependency_graph::DependencyGraph,
    readiness::TaskReadiness,
    records::{TaskRecord, parse_task},
    serialization::serialize_task,
    task_detail::build_task_detail,
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

fn reference() -> Value {
    let script = r#"
import { toTaskDetail } from "./src/core/task-detail.ts";
const root = {id:"TASK-1",title:"Root",status:"To Do",dependencies:["TASK-2","TASK-404","TASK-8","TASK-9"]};
const tasks = [root,{id:"TASK-2",title:"Blocked",status:"In Progress",dependencies:["TASK-3"]},{id:"TASK-3",title:"Done",status:"Done",dependencies:[]}];
const archivedTasks = [{id:"TASK-8",title:"Archived",status:"Done",dependencies:[]}];
const detail = toTaskDetail(root,{tasks,archivedTasks,statuses:["To Do","In Progress","Done"],ambiguousIds:new Set(["TASK-9"])});
const terminal = toTaskDetail({id:"TASK-5",title:"Finished",status:"Done",dependencies:["TASK-2"]},{tasks:[{id:"TASK-5",title:"Finished",status:"Done",dependencies:["TASK-2"]},{id:"TASK-2",title:"Dependency",status:"To Do",dependencies:[]}],statuses:["To Do","In Progress","Done"]});
process.stdout.write(JSON.stringify({dependencyGraph:detail.dependencyGraph,readiness:detail.readiness,terminalReadiness:terminal.readiness}));
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

fn graph_json(graph: &DependencyGraph) -> Value {
    json!({
        "rootId": graph.root_id,
        "nodes": graph.nodes.iter().map(|node| json!({
            "id": node.id, "title": node.title, "status": node.status,
            "state": node.state.as_str(), "completed": node.completed,
            "dependencyDepth": node.dependency_depth, "dependentDepth": node.dependent_depth,
        })).collect::<Vec<_>>(),
        "edges": graph.edges.iter().map(|edge| json!({"from": edge.from, "to": edge.to})).collect::<Vec<_>>(),
    })
}

fn readiness_json(readiness: TaskReadiness) -> Value {
    json!({
        "isReady": readiness.is_ready,
        "isBlocked": readiness.is_blocked,
        "blockingDependencies": readiness.blocking_dependencies,
        "missingDependencies": readiness.missing_dependencies,
    })
}

#[test]
fn task_detail_assembly_matches_typescript_for_resolved_blocked_missing_ambiguous_and_archived_dependencies()
 {
    let root = task(
        "TASK-1",
        "Root",
        "To Do",
        &["TASK-2", "TASK-404", "TASK-8", "TASK-9"],
    );
    let tasks = vec![
        root.clone(),
        task("TASK-2", "Blocked", "In Progress", &["TASK-3"]),
        task("TASK-3", "Done", "Done", &[]),
    ];
    let archived = vec![task("TASK-8", "Archived", "Done", &[])];
    let statuses = ["To Do".into(), "In Progress".into(), "Done".into()];
    let ambiguous_ids = ["TASK-9".into()];
    let detail = build_task_detail(&root, &tasks, &archived, &statuses, &ambiguous_ids, "task");
    assert_eq!(detail.task, root);
    assert_eq!(
        reference(),
        json!({
            "dependencyGraph": graph_json(&detail.dependency_graph),
            "readiness": readiness_json(detail.readiness),
            "terminalReadiness": readiness_json(build_task_detail(
                &task("TASK-5", "Finished", "Done", &["TASK-2"]),
                &[task("TASK-5", "Finished", "Done", &["TASK-2"]), task("TASK-2", "Dependency", "To Do", &[])],
                &[], &statuses, &[], "task",
            ).readiness),
        })
    );
}

#[test]
fn terminal_task_detail_is_not_ready_and_serialization_keeps_derived_fields_out_of_markdown() {
    let finished = task("TASK-5", "Finished", "Done", &["TASK-2"]);
    let dependency = task("TASK-2", "Dependency", "To Do", &[]);
    let tasks = vec![finished.clone(), dependency];
    let statuses = ["To Do".into(), "In Progress".into(), "Done".into()];
    let detail = build_task_detail(&finished, &tasks, &[], &statuses, &[], "task");
    assert!(!detail.readiness.is_ready);
    assert!(!detail.readiness.is_blocked);
    assert!(detail.readiness.blocking_dependencies.is_empty());
    assert!(detail.readiness.missing_dependencies.is_empty());

    let markdown = serialize_task(&detail.task, &detail.task.raw_content).unwrap();
    assert!(!markdown.contains("dependency_graph"));
    assert!(!markdown.contains("readiness"));
}
