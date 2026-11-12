use backlog_md::{
    records::{Comment, Criterion, MilestoneRecord, TaskRecord, parse_task},
    task_filtering::TaskFilter,
    task_search::{
        TaskSearchFields, TaskSearchIndex, build_task_search_body_text, build_task_search_fields,
    },
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

fn search_reference() -> Value {
    let script = r#"
import { createTaskSearchIndex } from "./src/utils/task-search.ts";
import { createMilestoneFilterValueResolver } from "./src/utils/milestone-filter.ts";
const tasks = [
 {id:"TASK-7",title:"Export report",status:"To Do",description:"Infrastructure backend",labels:["backend","infrastructure"],assignee:["@morgan"],dependencies:["TASK-8"],modifiedFiles:["src/web/components/SearchBox.tsx"],milestone:"M-1"},
 {id:"TASK-8",title:"Exported report",status:"Done",description:"User interface component",labels:["ui"],assignee:["@riley"],dependencies:[],modifiedFiles:["src/web/components/Results.tsx"],milestone:"M-2"},
 {id:"TASK-9",title:"Adjust database",status:"To Do",description:"Storage maintenance",labels:[],assignee:[],dependencies:[],modifiedFiles:[]},
];
const resolver = createMilestoneFilterValueResolver([
 {id:"M-1",title:"Release Plan"}, {id:"M-2",title:"Roadmap"},
]);
const index = createTaskSearchIndex(tasks);
const ids = (options) => index.search(options).map(task=>task.id);
process.stdout.write(JSON.stringify({
 typo:ids({query:"export reprot"}), body:ids({query:"morgan"}), oneCharId:ids({query:"7"}),
 idAndDependency:ids({query:"task-8"}), modifiedFile:ids({query:"searchbox"}),
 unmatched:ids({query:"no-such-task"}), empty:ids({query:""}), whitespace:ids({query:"   "}),
 filtered:ids({query:"export",status:"Done"}),
 filteredLabels:ids({query:"export",labels:["backend"]}),
 filteredMilestone:ids({query:"export",milestone:"release",resolveMilestoneLabel:resolver}),
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

#[test]
fn rust_fuzzy_task_search_matches_typescript_ranking_and_filter_composition() {
    let mut first = task("TASK-7", "Export report");
    first.description = "Infrastructure backend".into();
    first.labels = vec!["backend".into(), "infrastructure".into()];
    first.assignee = vec!["@morgan".into()];
    first.dependencies = vec!["TASK-8".into()];
    first.modified_files = vec!["src/web/components/SearchBox.tsx".into()];
    first.milestone = Some("M-1".into());
    let mut second = task("TASK-8", "Exported report");
    second.status = "Done".into();
    second.description = "User interface component".into();
    second.labels = vec!["ui".into()];
    second.assignee = vec!["@riley".into()];
    second.modified_files = vec!["src/web/components/Results.tsx".into()];
    second.milestone = Some("M-2".into());
    let mut third = task("TASK-9", "Adjust database");
    third.description = "Storage maintenance".into();
    let tasks = vec![first, second, third];
    let milestones = vec![
        MilestoneRecord {
            id: "M-1".into(),
            title: "Release Plan".into(),
            due_date: None,
            description: String::new(),
            raw_content: String::new(),
        },
        MilestoneRecord {
            id: "M-2".into(),
            title: "Roadmap".into(),
            due_date: None,
            description: String::new(),
            raw_content: String::new(),
        },
    ];
    let index = TaskSearchIndex::new(&tasks, &milestones);
    let search = |query: &str, filter: TaskFilter| {
        index
            .search(query, &filter, "task")
            .iter()
            .map(|task| task.id.as_str())
            .collect::<Vec<_>>()
    };
    let rust = json!({
        "typo": search("export reprot", TaskFilter::default()),
        "body": search("morgan", TaskFilter::default()),
        "oneCharId": search("7", TaskFilter::default()),
        "idAndDependency": search("task-8", TaskFilter::default()),
        "modifiedFile": search("searchbox", TaskFilter::default()),
        "unmatched": search("no-such-task", TaskFilter::default()),
        "empty": search("", TaskFilter::default()),
        "whitespace": search("   ", TaskFilter::default()),
        "filtered": search("export", TaskFilter { statuses: vec!["Done".into()], ..TaskFilter::default() }),
        "filteredLabels": search("export", TaskFilter { labels: vec!["backend".into()], ..TaskFilter::default() }),
        "filteredMilestone": search("export", TaskFilter { milestone: Some("release".into()), ..TaskFilter::default() }),
    });
    assert_eq!(search_reference(), rust);
}
