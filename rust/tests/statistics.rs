use backlog_md::{
    records::{TaskRecord, parse_task},
    statistics::{TaskStatisticsRecord, get_task_statistics, parse_stored_utc_millis},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    process::{Command, Stdio},
};

const NOW_MS: i64 = 1_794_398_400_000; // 2026-11-11T12:00:00Z

fn task(id: &str, title: &str, status: &str, created: &str) -> TaskRecord {
    parse_task(&format!(
        "---\nid: {id}\ntitle: {title}\nstatus: {status}\ncreated_date: {created}\n---\n"
    ))
    .unwrap()
}

fn corpus<'a>(tasks: &'a [TaskRecord], archived_ids: &[&str]) -> Vec<TaskStatisticsRecord<'a>> {
    tasks
        .iter()
        .map(|task| TaskStatisticsRecord {
            task,
            archived: archived_ids.contains(&task.id.as_str()),
        })
        .collect()
}

fn reference() -> Value {
    let script = r#"
const RealDate = Date;
const fixedNow = 1794398400000;
globalThis.Date = class extends RealDate {
 constructor(...args) { super(...(args.length ? args : [fixedNow])); }
 static now() { return fixedNow; }
};
const { getTaskStatistics } = await import("./src/core/statistics.ts");
const makeTask = (id, status, createdDate, extra = {}) => ({id,title:id,status,createdDate,assignee:[],labels:[],dependencies:[],rawContent:"",...extra});
const tasks = [
 makeTask("task-1","To Do","2026-11-04T12:00:00Z",{dependencies:["task-8"]}),
 makeTask("task-2","In Progress","2026-11-01",{updatedDate:"2026-11-05T11:00:00-01:00",priority:" Very High "}),
 makeTask("task-3","Done","2026-10-01T00:00:00Z",{updatedDate:"2026-10-04T23:59:59Z",priority:"none"}),
 makeTask("task-4","Done","2026-11-10",{source:"archived"}),
 makeTask("task-5","To Do","2026-10-01",{source:"archived"}),
 makeTask("task-6","","2026-11-01"),
 makeTask("task-7","Review","2026-11-01"),
 makeTask("task-8","To Do","2026-11-10",{source:"archived"}),
 makeTask("task-9","To Do","2026-10-01",{updatedDate:"2026-10-02"}),
 makeTask("task-10","To Do","not-a-date"),
 ...Array.from({length:6},(_,i)=>makeTask(`task-${11+i}`,"To Do","2026-11-11T09:00:00Z",{updatedDate:"2026-11-11T09:00:00Z",dependencies:["task-8"]})),
 ...Array.from({length:6},(_,i)=>makeTask(`task-${20+i}`,"To Do","2026-09-01",{updatedDate:"2026-09-02"})),
];
const stats = getTaskStatistics(tasks,[makeTask("draft-1","","","")], ["To Do","In Progress","Done"], ["Very High","None"]);
const empty = getTaskStatistics([],[], ["To Do","In Progress","Done"], ["Very High","None"]);
const ids = values => values.map(task => task.id);
process.stdout.write(JSON.stringify({
 statusCounts:Object.fromEntries(stats.statusCounts), priorityCounts:Object.fromEntries(stats.priorityCounts),
 statusEntries:Array.from(stats.statusCounts), priorityEntries:Array.from(stats.priorityCounts),
 noPriorityCount:stats.noPriorityCount,totalTasks:stats.totalTasks,completedTasks:stats.completedTasks,
 completionPercentage:stats.completionPercentage,draftCount:stats.draftCount,
 averageTaskAge:stats.projectHealth.averageTaskAge,
 recentCreated:ids(stats.recentActivity.created),recentUpdated:ids(stats.recentActivity.updated),
 stale:ids(stats.projectHealth.staleTasks),blocked:ids(stats.projectHealth.blockedTasks),
 empty:{statusEntries:Array.from(empty.statusCounts),priorityEntries:Array.from(empty.priorityCounts),totalTasks:empty.totalTasks,completedTasks:empty.completedTasks,completionPercentage:empty.completionPercentage,draftCount:empty.draftCount,averageTaskAge:empty.projectHealth.averageTaskAge},
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

fn reference_activity_order() -> Value {
    let script = r#"
const RealDate = Date;
const fixedNow = 1794398400000;
globalThis.Date = class extends RealDate { constructor(...args) { super(...(args.length ? args : [fixedNow])); } static now() { return fixedNow; } };
const { getTaskStatistics } = await import("./src/core/statistics.ts");
const tasks = [
 {id:"task-1",title:"task-1",status:"Done",source:"archived",createdDate:"2026-11-10",assignee:[],labels:[],dependencies:[],rawContent:""},
 {id:"task-2",title:"task-2",status:"To Do",createdDate:"2026-11-10",assignee:[],labels:[],dependencies:[],rawContent:""},
];
const stats = getTaskStatistics(tasks,[],["To Do","Done"]);
process.stdout.write(JSON.stringify(stats.recentActivity.created.map(task=>task.id)));
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

fn reference_duplicate_sources() -> Value {
    let script = r#"
const { getTaskStatistics } = await import("./src/core/statistics.ts");
const tasks = [
 {id:"task-1",title:"active",status:"To Do",createdDate:"2026-11-10",source:"active",dependencies:[]},
 {id:"task-1",title:"archived",status:"To Do",createdDate:"2026-11-10",source:"archived",dependencies:[]},
];
const stats = getTaskStatistics(tasks,[],["To Do","Done"]);
process.stdout.write(JSON.stringify({totalTasks:stats.totalTasks,statusCounts:Array.from(stats.statusCounts)}));
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
fn rust_task_statistics_match_typescript_with_archives_and_fixed_utc_time() {
    let mut active = vec![
        task("task-1", "task-1", "To Do", "2026-11-04T12:00:00Z"),
        task("task-2", "task-2", "In Progress", "2026-11-01"),
        task("task-3", "task-3", "Done", "2026-10-01T00:00:00Z"),
        task("task-6", "task-6", "", "2026-11-01"),
        task("task-7", "task-7", "Review", "2026-11-01"),
        task("task-9", "task-9", "To Do", "2026-10-01"),
        task("task-10", "task-10", "To Do", "not-a-date"),
    ];
    active[0].dependencies = vec!["task-8".into()];
    active[1].updated_date = Some("2026-11-05T11:00:00-01:00".into());
    active[1].priority = Some(" Very High ".into());
    active[2].updated_date = Some("2026-10-04T23:59:59Z".into());
    active[2].priority = Some("none".into());
    active[5].updated_date = Some("2026-10-02".into());
    active.extend((11..=16).map(|id| {
        let mut task = task(
            &format!("task-{id}"),
            &format!("task-{id}"),
            "To Do",
            "2026-11-11T09:00:00Z",
        );
        task.updated_date = Some("2026-11-11T09:00:00Z".into());
        task.dependencies = vec!["task-8".into()];
        task
    }));
    active.extend((20..=25).map(|id| {
        let mut task = task(
            &format!("task-{id}"),
            &format!("task-{id}"),
            "To Do",
            "2026-09-01",
        );
        task.updated_date = Some("2026-09-02".into());
        task
    }));
    let archived = vec![
        task("task-4", "task-4", "Done", "2026-11-10"),
        task("task-5", "task-5", "To Do", "2026-10-01"),
        task("task-8", "task-8", "To Do", "2026-11-10"),
    ];
    let mut archived = archived;
    archived[1].updated_date = Some("2026-10-02".into());
    let mut tasks = active;
    tasks.extend(archived);
    tasks.sort_by_key(|task| {
        task.id
            .strip_prefix("task-")
            .unwrap()
            .parse::<u32>()
            .unwrap()
    });
    let records = corpus(&tasks, &["task-4", "task-5", "task-8"]);
    let rust = get_task_statistics(
        &records,
        1,
        &["To Do".into(), "In Progress".into(), "Done".into()],
        &["Very High".into(), "None".into()],
        NOW_MS,
    );
    let empty = get_task_statistics(
        &[],
        0,
        &["To Do".into(), "In Progress".into(), "Done".into()],
        &["Very High".into(), "None".into()],
        NOW_MS,
    );
    assert_eq!(
        reference(),
        json!({
            "statusCounts": rust.status_counts.iter().cloned().collect::<BTreeMap<_, _>>(),
            "priorityCounts": rust.priority_counts.iter().cloned().collect::<BTreeMap<_, _>>(),
            "statusEntries": rust.status_counts,
            "priorityEntries": rust.priority_counts,
            "noPriorityCount": rust.no_priority_count,
            "totalTasks": rust.total_tasks,
            "completedTasks": rust.completed_tasks,
            "completionPercentage": rust.completion_percentage,
            "draftCount": rust.draft_count,
            "averageTaskAge": if rust.project_health.average_task_age.is_finite() { json!(rust.project_health.average_task_age as i64) } else { Value::Null },
            "recentCreated": rust.recent_activity.created.iter().map(|task| task.id.as_str()).collect::<Vec<_>>(),
            "recentUpdated": rust.recent_activity.updated.iter().map(|task| task.id.as_str()).collect::<Vec<_>>(),
            "stale": rust.project_health.stale_tasks.iter().map(|task| task.id.as_str()).collect::<Vec<_>>(),
            "blocked": rust.project_health.blocked_tasks.iter().map(|task| task.id.as_str()).collect::<Vec<_>>(),
            "empty": {
                "statusEntries": empty.status_counts,
                "priorityEntries": empty.priority_counts,
                "totalTasks": empty.total_tasks,
                "completedTasks": empty.completed_tasks,
                "completionPercentage": empty.completion_percentage,
                "draftCount": empty.draft_count,
                "averageTaskAge": empty.project_health.average_task_age as i64,
            },
        })
    );
    assert_eq!(rust.recent_activity.created.len(), 5);
    assert_eq!(rust.recent_activity.updated.len(), 5);
    assert_eq!(rust.project_health.stale_tasks.len(), 5);
    assert_eq!(rust.project_health.blocked_tasks.len(), 5);
}

#[test]
fn stored_utc_dates_validate_calendar_values_and_apply_offsets() {
    assert_eq!(
        parse_stored_utc_millis("2026-11-01"),
        Some(1_793_491_200_000)
    );
    assert_eq!(
        parse_stored_utc_millis("2024-02-29"),
        Some(1_709_164_800_000)
    );
    assert_eq!(
        parse_stored_utc_millis("2026-10-04T23:59:59-01:00"),
        parse_stored_utc_millis("2026-10-05T00:59:59Z"),
    );
    assert_eq!(parse_stored_utc_millis("2026-02-29"), None);
    assert_eq!(parse_stored_utc_millis("2026-11-11T25:00Z"), None);
}

#[test]
fn recent_activity_keeps_the_combined_corpus_order_when_timestamps_tie() {
    let tasks = vec![
        task("task-1", "task-1", "Done", "2026-11-10"),
        task("task-2", "task-2", "To Do", "2026-11-10"),
    ];
    let records = corpus(&tasks, &["task-1"]);
    let stats = get_task_statistics(&records, 0, &["To Do".into(), "Done".into()], &[], NOW_MS);
    let rust = stats
        .recent_activity
        .created
        .iter()
        .map(|task| task.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(reference_activity_order(), json!(rust));
}

#[test]
fn archived_source_is_applied_per_record_when_ids_collide() {
    let tasks = [
        task("task-1", "active", "To Do", "2026-11-10"),
        task("task-1", "archived", "To Do", "2026-11-10"),
    ];
    let records = [
        TaskStatisticsRecord {
            task: &tasks[0],
            archived: false,
        },
        TaskStatisticsRecord {
            task: &tasks[1],
            archived: true,
        },
    ];
    let stats = get_task_statistics(&records, 0, &["To Do".into(), "Done".into()], &[], NOW_MS);
    assert_eq!(
        reference_duplicate_sources(),
        json!({"totalTasks":stats.total_tasks,"statusCounts":stats.status_counts}),
    );
}
