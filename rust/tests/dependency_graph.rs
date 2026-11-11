use backlog_md::{
    dependency_graph::build_dependency_graph,
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

fn reference() -> Value {
    let script = r#"
import { buildDependencyGraph } from "./src/utils/dependency-graph.ts";
const tasks = [
  {id:"TASK-1",title:"Root",status:"To Do",dependencies:["task-02","TASK-3","TASK-999","TASK-8"]},
  {id:"TASK-2",title:"Two",status:"To Do",dependencies:["TASK-4"]},
  {id:"TASK-3",title:"Three",status:"To Do",dependencies:["TASK-4"]},
  {id:"TASK-4",title:"Four",status:"To Do",dependencies:["TASK-1"]},
  {id:"TASK-6",title:"Six",status:"To Do",dependencies:["TASK-5"]},
  {id:"TASK-7",title:"Seven",status:"To Do",dependencies:["TASK-8"]},
  {id:"DRAFT-9",title:"Draft",status:"Draft",dependencies:["TASK-1"]},
];
const archived = [
  {id:"TASK-5",title:"Five",status:"Done",dependencies:["TASK-1"]},
  {id:"TASK-08",title:"Eight duplicate",status:"Done",dependencies:[]},
];
const graph = buildDependencyGraph(tasks[0],{tasks,archivedTasks:archived,statuses:["To Do","In Progress","Done"]});
process.stdout.write(JSON.stringify(graph));
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

fn graph_json(graph: &backlog_md::dependency_graph::DependencyGraph) -> Value {
    json!({
        "rootId": graph.root_id,
        "nodes": graph.nodes.iter().map(|node| json!({
            "id": node.id,
            "title": node.title,
            "status": node.status,
            "state": node.state.as_str(),
            "completed": node.completed,
            "dependencyDepth": node.dependency_depth,
            "dependentDepth": node.dependent_depth,
        })).collect::<Vec<_>>(),
        "edges": graph.edges.iter().map(|edge| json!({"from":edge.from,"to":edge.to})).collect::<Vec<_>>(),
    })
}

#[test]
fn graph_matches_live_typescript_for_cycles_diamonds_archived_and_unresolved_records() {
    let root = task(
        "TASK-1",
        "Root",
        "To Do",
        &["task-02", "TASK-3", "TASK-999", "TASK-8"],
    );
    let tasks = vec![
        root.clone(),
        task("TASK-2", "Two", "To Do", &["TASK-4"]),
        task("TASK-3", "Three", "To Do", &["TASK-4"]),
        task("TASK-4", "Four", "To Do", &["TASK-1"]),
        task("TASK-6", "Six", "To Do", &["TASK-5"]),
        task("TASK-7", "Seven", "To Do", &["TASK-8"]),
        task("DRAFT-9", "Draft", "Draft", &["TASK-1"]),
    ];
    let archived = vec![
        task("TASK-5", "Five", "Done", &["TASK-1"]),
        task("TASK-08", "Eight duplicate", "Done", &[]),
    ];
    let statuses = ["To Do".into(), "In Progress".into(), "Done".into()];
    let graph = build_dependency_graph(&root, &tasks, &archived, &statuses, &[], "task");
    let actual = graph_json(&graph);
    assert_eq!(reference(), actual);

    let reversed_tasks: Vec<_> = tasks.iter().cloned().rev().collect();
    let reversed_archived: Vec<_> = archived.iter().cloned().rev().collect();
    assert_eq!(
        graph,
        build_dependency_graph(
            &root,
            &reversed_tasks,
            &reversed_archived,
            &statuses,
            &[],
            "task"
        )
    );
}

#[test]
fn graph_honors_ambiguous_identities_reported_by_the_corpus_loader() {
    let root = task("TASK-1", "Root", "To Do", &["TASK-2"]);
    let tasks = vec![
        root.clone(),
        task("TASK-2", "Two", "To Do", &["TASK-3"]),
        task("TASK-3", "Three", "To Do", &[]),
    ];
    let ambiguous_ids = vec!["TASK-2".to_owned()];
    let graph = build_dependency_graph(
        &root,
        &tasks,
        &[],
        &["To Do".into(), "In Progress".into(), "Done".into()],
        &ambiguous_ids,
        "task",
    );
    let node = graph.nodes.iter().find(|node| node.id == "TASK-2").unwrap();
    assert_eq!(
        node.state,
        backlog_md::dependency_graph::DependencyNodeState::Ambiguous
    );
    assert_eq!(node.dependency_depth, Some(1));
    assert_eq!(graph.nodes.len(), 2);
    assert!(
        graph
            .edges
            .iter()
            .any(|edge| edge.from == "TASK-1" && edge.to == "TASK-2")
    );

    let script = r#"
import { buildDependencyGraph } from "./src/utils/dependency-graph.ts";
const root = {id:"TASK-1",title:"Root",status:"To Do",dependencies:["TASK-2"]};
const tasks = [root,{id:"TASK-2",title:"Two",status:"To Do",dependencies:["TASK-3"]},{id:"TASK-3",title:"Three",status:"To Do",dependencies:[]}];
process.stdout.write(JSON.stringify(buildDependencyGraph(root,{tasks,statuses:["To Do","In Progress","Done"],ambiguousIds:new Set(["TASK-2"])})));
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
    let expected: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(expected, graph_json(&graph));
}
