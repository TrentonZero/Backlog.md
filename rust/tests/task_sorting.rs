use backlog_md::{
    records::{TaskRecord, parse_task},
    task_sorting::{section_tasks, sort_tasks},
};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::{Command, Stdio},
};

fn task(id: &str, status: &str, priority: Option<&str>, ordinal: Option<f64>) -> TaskRecord {
    let mut record = parse_task(&format!(
        "---\nid: {id}\ntitle: {id}\nstatus: {status}\ncreated_date: 2026-01-01\n---\n"
    ))
    .unwrap();
    record.priority = priority.map(str::to_owned);
    record.ordinal = ordinal;
    record
}

fn ids(tasks: &[TaskRecord]) -> Vec<String> {
    tasks.iter().map(|task| task.id.clone()).collect()
}

fn reference() -> Value {
    let script = r#"
import { sortTasks, sectionTasks } from "./src/utils/task-sorting.ts";
const tasks = [
 {id:"TASK-10",title:"TASK-10",status:"Done",priority:" HIGH "},
 {id:"TASK-2",title:"TASK-2",status:"Blocked",priority:"low",ordinal:20},
 {id:"TASK-3",title:"TASK-3",status:"to do",ordinal:10},
 {id:"TASK-4",title:"TASK-4",status:"Done",priority:"high",ordinal:20},
 {id:"TASK-5",title:"TASK-5",status:"To Do"},
 {id:"TASK-2.1",title:"TASK-2.1",status:"In Progress",priority:"medium",ordinal:20},
];
const statuses = ["To Do","In Progress","Done"];
const sectionIds = sortField => sectionTasks(tasks,sortField,statuses).map(section=>[section.status,section.tasks.map(task=>task.id)]);
const equivalentIds = [{id:"TASK-2"},{id:"TASK-02"}];
const invalidOrdinals = [{id:"TASK-2",ordinal:NaN,priority:"low"},{id:"TASK-1",ordinal:10,priority:"high"}];
const nanCases = [["NaN","10","1"],["10","NaN","1"],["10","1","NaN"],["1","NaN","10","2"],["10","NaN","1","2"]].map(values => {
 const items = values.map((value,index)=>({id:`TASK-${index+1}`,ordinal:value==="NaN"?NaN:Number(value)}));
 return {ordinal:sortTasks(items,"ordinal").map(task=>task.id),default:sortTasks(items).map(task=>task.id)};
});
process.stdout.write(JSON.stringify({
 default:sortTasks(tasks).map(task=>task.id), id:sortTasks(tasks,"id").map(task=>task.id),
 ordinal:sortTasks(tasks,"ordinal").map(task=>task.id), priority:sortTasks(tasks,"priority",["high","medium","low"]).map(task=>task.id),
 configured:sortTasks(tasks,"priority",[" LOW ","low"," HIGH ","Medium"]).map(task=>task.id),
 fallback:sortTasks(tasks,"priority",[" ",""]).map(task=>task.id),
 equivalentIds:sortTasks(equivalentIds,"id").map(task=>task.id),
 invalidOrdinal:sortTasks(invalidOrdinals,"ordinal").map(task=>task.id),
 invalidDefault:sortTasks(invalidOrdinals).map(task=>task.id),
 nanCases,
 sections:sectionIds("id"), flat:sectionIds("priority")
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
fn task_sort_orders_and_status_sections_match_typescript() {
    let tasks = vec![
        task("TASK-10", "Done", Some(" HIGH "), None),
        task("TASK-2", "Blocked", Some("low"), Some(20.0)),
        task("TASK-3", "to do", None, Some(10.0)),
        task("TASK-4", "Done", Some("high"), Some(20.0)),
        task("TASK-5", "To Do", None, None),
        task("TASK-2.1", "In Progress", Some("medium"), Some(20.0)),
    ];
    let equivalent_ids = [
        task("TASK-2", "To Do", None, None),
        task("TASK-02", "To Do", None, None),
    ];
    let invalid_ordinals = [
        task("TASK-2", "To Do", Some("low"), Some(f64::NAN)),
        task("TASK-1", "To Do", Some("high"), Some(10.0)),
    ];
    let nan_inputs = [
        [f64::NAN, 10.0, 1.0].as_slice(),
        [10.0, f64::NAN, 1.0].as_slice(),
        [10.0, 1.0, f64::NAN].as_slice(),
        [1.0, f64::NAN, 10.0, 2.0].as_slice(),
        [10.0, f64::NAN, 1.0, 2.0].as_slice(),
    ];
    let priorities = ["high".into(), "medium".into(), "low".into()];
    let configured_priorities = [
        " LOW ".into(),
        "low".into(),
        " HIGH ".into(),
        "Medium".into(),
    ];
    let empty_priorities = [" ".into(), "".into()];
    let statuses = ["To Do".into(), "In Progress".into(), "Done".into()];
    let sections = section_tasks(&tasks, Some("id"), &statuses);
    let actual = json!({
        "default": ids(&sort_tasks(&tasks, None, &priorities)),
        "id": ids(&sort_tasks(&tasks, Some("id"), &priorities)),
        "ordinal": ids(&sort_tasks(&tasks, Some("ordinal"), &priorities)),
        "priority": ids(&sort_tasks(&tasks, Some("priority"), &priorities)),
        "configured": ids(&sort_tasks(
            &tasks,
            Some("priority"),
            &configured_priorities,
        )),
        "fallback": ids(&sort_tasks(&tasks, Some("priority"), &empty_priorities)),
        "equivalentIds": ids(&sort_tasks(&equivalent_ids, Some("id"), &priorities)),
        "invalidOrdinal": ids(&sort_tasks(&invalid_ordinals, Some("ordinal"), &priorities)),
        "invalidDefault": ids(&sort_tasks(&invalid_ordinals, None, &priorities)),
        "nanCases": nan_inputs
            .iter()
            .map(|ordinals| {
                let items: Vec<_> = ordinals
                    .iter()
                    .enumerate()
                    .map(|(index, ordinal)| {
                        task(
                            &format!("TASK-{}", index + 1),
                            "To Do",
                            None,
                            Some(*ordinal),
                        )
                    })
                    .collect();
                json!({
                    "ordinal": ids(&sort_tasks(&items, Some("ordinal"), &priorities)),
                    "default": ids(&sort_tasks(&items, None, &priorities)),
                })
            })
            .collect::<Vec<_>>(),
        "sections": sections.iter().map(|section| json!([section.status, ids(&section.tasks)])).collect::<Vec<_>>(),
        "flat": section_tasks(&tasks, Some("priority"), &statuses).iter().map(|section| json!([section.status, ids(&section.tasks)])).collect::<Vec<_>>(),
    });
    assert_eq!(reference(), actual);
}
