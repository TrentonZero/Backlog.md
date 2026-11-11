use backlog_md::{
    records::{TaskRecord, parse_task},
    task_list::with_readiness,
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
import { withReadiness } from "./src/core/task-detail.ts";
const root = {id:"TASK-1",title:"Root",status:"To Do",dependencies:["TASK-2"]};
const ready = {id:"TASK-4",title:"Ready",status:"To Do",dependencies:[]};
const missing = {id:"TASK-5",title:"Missing",status:"To Do",dependencies:["TASK-404"]};
const ambiguous = {id:"TASK-6",title:"Ambiguous",status:"To Do",dependencies:["TASK-9"]};
const archived = {id:"TASK-8",title:"Archived",status:"Done",dependencies:[]};
const terminal = {id:"TASK-7",title:"Terminal",status:"Done",dependencies:["TASK-2"]};
const archivedOpen = {id:"TASK-10",title:"Archived open",status:"To Do",dependencies:["TASK-2"]};
const allTasks = [root,{id:"TASK-2",title:"Hidden dependency",status:"In Progress",dependencies:[]},ready,missing,ambiguous,terminal];
const projected = withReadiness([root,ready,missing,ambiguous,archived,terminal,archivedOpen],{tasks:allTasks,archivedTasks:[archived,archivedOpen],statuses:["To Do","In Progress","Done"],ambiguousIds:new Set(["TASK-9"])});
process.stdout.write(JSON.stringify(projected.map(({id,isReady})=>({id,isReady}))));
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
fn list_readiness_matches_typescript_and_uses_dependencies_outside_the_displayed_list() {
    let root = task("TASK-1", "Root", "To Do", &["TASK-2"]);
    let ready = task("TASK-4", "Ready", "To Do", &[]);
    let missing = task("TASK-5", "Missing", "To Do", &["TASK-404"]);
    let ambiguous = task("TASK-6", "Ambiguous", "To Do", &["TASK-9"]);
    let archived = task("TASK-8", "Archived", "Done", &[]);
    let terminal = task("TASK-7", "Terminal", "Done", &["TASK-2"]);
    let archived_open = task("TASK-10", "Archived open", "To Do", &["TASK-2"]);
    let hidden_dependency = task("TASK-2", "Hidden dependency", "In Progress", &[]);
    let all_tasks = vec![
        root.clone(),
        hidden_dependency,
        ready.clone(),
        missing.clone(),
        ambiguous.clone(),
        terminal.clone(),
    ];
    let displayed = vec![
        root,
        ready,
        missing,
        ambiguous,
        archived.clone(),
        terminal,
        archived_open.clone(),
    ];
    let statuses = ["To Do".into(), "In Progress".into(), "Done".into()];
    let ambiguous_ids = ["TASK-9".into()];
    let result = with_readiness(
        &displayed,
        &all_tasks,
        &[archived, archived_open],
        &statuses,
        &ambiguous_ids,
        "task",
    );
    assert_eq!(
        result.iter().map(|item| &item.task).collect::<Vec<_>>(),
        displayed.iter().collect::<Vec<_>>()
    );
    assert_eq!(
        reference(),
        json!(
            result
                .iter()
                .map(|item| json!({"id":item.task.id,"isReady":item.is_ready}))
                .collect::<Vec<_>>()
        )
    );
}
