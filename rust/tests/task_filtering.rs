use backlog_md::{
    records::{MilestoneRecord, TaskRecord, parse_task},
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

fn milestone(id: &str, title: &str) -> MilestoneRecord {
    MilestoneRecord {
        id: id.into(),
        title: title.into(),
        due_date: None,
        description: String::new(),
        raw_content: String::new(),
    }
}

fn ids<'a>(tasks: &[&'a TaskRecord]) -> Vec<&'a str> {
    tasks.iter().map(|task| task.id.as_str()).collect()
}

fn reference() -> Value {
    let script = r#"
import { createTaskFilterMatcher } from "./src/utils/task-search.ts";
const tasks = [
 {id:"TASK-1",status:"To Do",type:" Feature ",project:" Alpha ",priority:" High ",assignee:[" Alice ",""],labels:["UI","bug"],parentTaskId:"TASK-01",modifiedFiles:[" src/web/components/SearchBox.tsx ",""]},
 {id:"TASK-2",status:"Done",type:"docs",project:"beta",assignee:[" "],labels:["infra"]},
 {id:"TASK-3",status:"To Do",type:"task",project:"Alpha",assignee:[],labels:[],modifiedFiles:["docs/readme.md"]},
];
const selected = options => tasks.filter(createTaskFilterMatcher(options,tasks)).map(task=>task.id);
process.stdout.write(JSON.stringify({
 combined:selected({status:[" to do "],excludeStatus:["closed"],type:["FEATURE"],project:["ALPHA"],priority:["high"],assignee:["alice"],labels:["ui","missing"]}),
 labelsAllParent:selected({labels:["ui"," BUG "],labelMatch:"all",parentTaskId:"task-1"}),
 unassigned:selected({unassigned:true}),
 excluded:selected({excludeStatus:[" to do "]}),
 empty:selected({status:[" ",""],type:[" ",""],project:[],priority:[" "],assignee:[" "],labels:[" "]}),
 modifiedMany:selected({modifiedFiles:[" COMPONENTS/searchbox "," READme "]}),
 modifiedEmpty:selected({modifiedFiles:[" ",""]}),
 modifiedUnmatched:selected({modifiedFiles:["missing.ts"]}),
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

fn milestone_reference() -> Value {
    let script = r#"
import { createMilestoneFilterValueResolver, NO_MILESTONE_FILTER_VALUE } from "./src/utils/milestone-filter.ts";
import { createTaskFilterMatcher } from "./src/utils/task-search.ts";
const milestones = [
 {id:"M-8",title:"Quarterly Roadmap"}, {id:"M-9",title:"M-8"},
 {id:"M-10",title:"Shared"}, {id:"M-11",title:"Shared"}, {id:"M-12",title:"Retired"}, {id:"M-000",title:"Zero"},
];
const tasks = [
 {id:"TASK-1",milestone:"M-8"}, {id:"TASK-2",milestone:"08"},
 {id:"TASK-3",milestone:"Quarterly Roadmap"}, {id:"TASK-4",milestone:"M-9"},
 {id:"TASK-5",milestone:"Shared"}, {id:"TASK-6",milestone:"M-10"},
 {id:"TASK-7",milestone:"M-11"}, {id:"TASK-8",milestone:"M-12"},
 {id:"TASK-9",milestone:"Retired"}, {id:"TASK-10",milestone:" "}, {id:"TASK-11"}, {id:"TASK-12",milestone:"M-000"},
];
const resolver = createMilestoneFilterValueResolver(milestones);
const selected = query => tasks.filter(createTaskFilterMatcher({milestone:query,resolveMilestoneLabel:resolver},tasks)).map(task=>task.id);
process.stdout.write(JSON.stringify({alias:selected("08"), zeroAlias:selected("0"), exactIdCollision:selected("M-8"), duplicateTitle:selected("Shared"), archivedAlias:selected("12"), noMilestone:selected(NO_MILESTONE_FILTER_VALUE), blank:selected(" ")}));
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
    first.modified_files = vec![" src/web/components/SearchBox.tsx ".into(), "".into()];
    let mut second = task("TASK-2", "Done");
    second.task_type = Some("docs".into());
    second.project = Some("beta".into());
    second.assignee = vec![" ".into()];
    second.labels = vec!["infra".into()];
    let mut third = task("TASK-3", "To Do");
    third.task_type = Some("task".into());
    third.project = Some("Alpha".into());
    third.modified_files = vec!["docs/readme.md".into()];
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
    let modified_many = TaskFilter {
        modified_files: vec![" COMPONENTS/searchbox ".into(), " READme ".into()],
        ..TaskFilter::default()
    };
    let modified_empty = TaskFilter {
        modified_files: vec![" ".into(), "".into()],
        ..TaskFilter::default()
    };
    let modified_unmatched = TaskFilter {
        modified_files: vec!["missing.ts".into()],
        ..TaskFilter::default()
    };
    let active = tasks.as_slice();
    let rust = json!({
        "combined": ids(&filter_tasks(active, &combined, "task", &[])),
        "labelsAllParent": ids(&filter_tasks(active, &labels_all_parent, "task", &[])),
        "unassigned": ids(&filter_tasks(active, &unassigned, "task", &[])),
        "excluded": ids(&filter_tasks(active, &excluded, "task", &[])),
        "empty": ids(&filter_tasks(active, &empty, "task", &[])),
        "modifiedMany": ids(&filter_tasks(active, &modified_many, "task", &[])),
        "modifiedEmpty": ids(&filter_tasks(active, &modified_empty, "task", &[])),
        "modifiedUnmatched": ids(&filter_tasks(active, &modified_unmatched, "task", &[])),
    });
    assert_eq!(reference(), rust);
}

#[test]
fn milestone_filters_match_active_archive_alias_title_collision_and_empty_values() {
    let tasks = vec![
        ("TASK-1", Some("M-8")),
        ("TASK-2", Some("08")),
        ("TASK-3", Some("Quarterly Roadmap")),
        ("TASK-4", Some("M-9")),
        ("TASK-5", Some("Shared")),
        ("TASK-6", Some("M-10")),
        ("TASK-7", Some("M-11")),
        ("TASK-8", Some("M-12")),
        ("TASK-9", Some("Retired")),
        ("TASK-10", Some(" ")),
        ("TASK-11", None),
        ("TASK-12", Some("M-000")),
    ]
    .into_iter()
    .map(|(id, value)| {
        let mut record = task(id, "To Do");
        record.milestone = value.map(str::to_owned);
        record
    })
    .collect::<Vec<_>>();
    let milestones = vec![
        milestone("M-8", "Quarterly Roadmap"),
        milestone("M-9", "M-8"),
        milestone("M-10", "Shared"),
        milestone("M-11", "Shared"),
        milestone("M-12", "Retired"),
        milestone("M-000", "Zero"),
    ];
    let active_and_archived = milestones.as_slice();
    let select = |query: &str| {
        let filter = TaskFilter {
            milestone: Some(query.into()),
            ..TaskFilter::default()
        };
        ids(&filter_tasks(&tasks, &filter, "task", active_and_archived))
    };
    let rust = json!({
        "alias": select("08"), "zeroAlias": select("0"), "exactIdCollision": select("M-8"),
        "duplicateTitle": select("Shared"), "archivedAlias": select("12"),
        "noMilestone": select("\0no-milestone"), "blank": select(" "),
    });
    assert_eq!(milestone_reference(), rust);
}
