use backlog_md::{
    records::{Comment, Criterion, TaskRecord, parse_task},
    task_search::{TaskSearchFields, build_task_search_body_text, build_task_search_fields},
};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::{Command, Stdio},
};

fn task(id: &str, title: &str) -> TaskRecord {
    parse_task(&format!(
        "---\nid: {id}\ntitle: {title}\nstatus: To Do\ncreated_date: 2026-01-01\n---\n"
    ))
    .unwrap()
}

fn reference() -> Value {
    let script = r#"
import { buildTaskSearchBodyText, buildTaskSearchFields } from "./src/utils/task-search.ts";
import { createTaskIdSearchVariants } from "./src/utils/task-id-search.ts";
const populated = {
 id:"TaSk-0007.02", title:"Export data", status:"To Do", description:"Description.",
 acceptanceCriteriaItems:[{index:2,text:"Second",checked:false},{index:1,text:"First",checked:true}],
 implementationPlan:"Plan.", implementationNotes:"Notes.",
 comments:[{index:1,body:"Comment A"},{index:2,body:"Comment B"}],
 labels:["backend","data"], assignee:["@morgan","@riley"],
 modifiedFiles:["src/a.rs","src/b.rs"], dependencies:["TASK-0008","task-9.03"],
};
const sparse = {id:"custom-ID",title:"Sparse",status:"To Do",description:"",comments:[],labels:[],assignee:[],modifiedFiles:[],dependencies:[]};
process.stdout.write(JSON.stringify({
 populatedBody:buildTaskSearchBodyText(populated), populatedFields:buildTaskSearchFields(populated),
 sparseBody:buildTaskSearchBodyText(sparse), sparseFields:buildTaskSearchFields(sparse),
 largeIdVariants:createTaskIdSearchVariants("TASK-9007199254740993"),
 overflowIdVariants:createTaskIdSearchVariants(`TASK-${"9".repeat(400)}`),
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

fn search_fields_value(fields: TaskSearchFields) -> Value {
    json!({
        "title": fields.title,
        "bodyText": fields.body_text,
        "id": fields.id,
        "idVariants": fields.id_variants,
        "dependencyIds": fields.dependency_ids,
        "modifiedFiles": fields.modified_files,
    })
}

#[test]
fn rust_search_fields_match_typescript_body_order_and_id_variants() {
    let mut populated = task("TaSk-0007.02", "Export data");
    populated.description = "Description.".into();
    populated.acceptance_criteria_items = vec![
        Criterion {
            index: 2,
            text: "Second".into(),
            checked: false,
        },
        Criterion {
            index: 1,
            text: "First".into(),
            checked: true,
        },
    ];
    populated.implementation_plan = Some("Plan.".into());
    populated.implementation_notes = Some("Notes.".into());
    populated.comments = vec![
        Comment {
            index: 1,
            body: "Comment A".into(),
            created_date: "2026-01-01".into(),
            author: None,
        },
        Comment {
            index: 2,
            body: "Comment B".into(),
            created_date: "2026-01-01".into(),
            author: None,
        },
    ];
    populated.labels = vec!["backend".into(), "data".into()];
    populated.assignee = vec!["@morgan".into(), "@riley".into()];
    populated.modified_files = vec!["src/a.rs".into(), "src/b.rs".into()];
    populated.dependencies = vec!["TASK-0008".into(), "task-9.03".into()];
    let sparse = task("custom-ID", "Sparse");
    let large = task("TASK-9007199254740993", "Large ID");
    let overflow = task(&format!("TASK-{}", "9".repeat(400)), "Overflow ID");

    let rust = json!({
        "populatedBody": build_task_search_body_text(&populated),
        "populatedFields": search_fields_value(build_task_search_fields(&populated)),
        "sparseBody": build_task_search_body_text(&sparse),
        "sparseFields": search_fields_value(build_task_search_fields(&sparse)),
        "largeIdVariants": build_task_search_fields(&large).id_variants,
        "overflowIdVariants": build_task_search_fields(&overflow).id_variants,
    });
    assert_eq!(reference(), rust);
}
