use backlog_md::{
    milestone_service::{TaskMilestoneHandling, remove_milestone},
    milestone_store::MilestoneStore,
    task_store::TaskStore,
};
use serde_json::Value;
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "backlog-milestone-removal-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("backlog/milestones")).unwrap();
        Self(root)
    }
    fn write(&self, relative: &str, source: &str) {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, source).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn reference(root: &PathBuf, handling: &str, reassign_to: &str) -> Value {
    let script = r#"
import { Core } from "./src/core/backlog.ts";
import { MilestoneHandlers } from "./src/mcp/tools/milestones/handlers.ts";
import { parseFrontmatter } from "./src/markdown/frontmatter.ts";
const core = new Core(process.env.BACKLOG_MILESTONE_ROOT);
const result = await new MilestoneHandlers(core).removeMilestone({name:"Release A", taskHandling:process.env.TASK_HANDLING, reassignTo:process.env.REASSIGN_TO || undefined});
const tasks = (await core.filesystem.listTasks()).sort((a,b)=>a.id.localeCompare(b.id));
const archived = await core.filesystem.listArchivedMilestones();
const sources = await Promise.all(["task-1 - first.md","task-2 - second.md","task-3 - other.md"].map(async name=>{const source=await Bun.file(process.env.BACKLOG_MILESTONE_ROOT+"/backlog/tasks/"+name).text(); const parsed=parseFrontmatter(source,["created_date","updated_date"]); const data={...parsed.data}; for(const key of ["assignee","dependencies","labels","references","documentation","modified_files"]) if(Array.isArray(data[key])&&data[key].length===0) delete data[key]; return {data,content:parsed.content.trimEnd()};}));
process.stdout.write(JSON.stringify({summary:result.content[0].text, tasks:tasks.map(t=>({id:t.id,milestone:t.milestone??null})), sources, archived:archived.map(m=>m.id)}));
"#;
    let output =
        Command::new(std::env::var_os("BACKLOG_REFERENCE_BUN").unwrap_or_else(|| "bun".into()))
            .args(["--eval", script])
            .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
            .env("BACKLOG_MILESTONE_ROOT", root)
            .env("TASK_HANDLING", handling)
            .env("REASSIGN_TO", reassign_to)
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

fn setup(fixture: &Fixture) {
    fixture.write(
        "backlog/milestones/m-0 - release-a.md",
        "---\nid: m-0\ntitle: Release A\n---\n",
    );
    fixture.write("backlog/tasks/task-1 - first.md", "---\nid: task-1\ntitle: First\nstatus: To Do\ncreated_date: 2026-01-01\nmilestone: m-0\n---\nFirst body\n");
    fixture.write("backlog/tasks/task-2 - second.md", "---\nid: task-2\ntitle: Second\nstatus: To Do\ncreated_date: 2026-01-01\nmilestone: release a\n---\nSecond body\n");
    fixture.write("backlog/tasks/task-3 - other.md", "---\nid: task-3\ntitle: Other\nstatus: To Do\ncreated_date: 2026-01-01\nmilestone: m-9\n---\nOther body\n");
}

#[test]
fn remove_clear_updates_matching_tasks_and_archives_like_typescript() {
    let fixture = Fixture::new();
    setup(&fixture);
    let reference_fixture = Fixture::new();
    setup(&reference_fixture);
    let live = reference(&reference_fixture.0, "clear", "");
    let milestones = MilestoneStore::new(&fixture.0, "backlog");
    let tasks = TaskStore::new(&fixture.0, "backlog", "task");
    let result = remove_milestone(
        &milestones,
        &tasks,
        "Release A",
        TaskMilestoneHandling::Clear,
        None,
    )
    .unwrap();
    assert_eq!(result.affected_task_ids, ["TASK-1", "TASK-2"]);
    assert_eq!(live["summary"], result.summary());
    let values: Vec<_> = tasks
        .list_tasks()
        .into_iter()
        .map(|entry| (entry.task.id, entry.task.milestone))
        .collect();
    assert_eq!(
        values,
        [
            ("TASK-1".into(), None),
            ("TASK-2".into(), None),
            ("TASK-3".into(), Some("m-9".into()))
        ]
    );
    assert_eq!(live["tasks"][0]["milestone"], serde_json::Value::Null);
    assert_eq!(live["tasks"][1]["milestone"], serde_json::Value::Null);
    assert_eq!(live["archived"], serde_json::json!(["m-0"]));
    for (index, path) in [
        "backlog/tasks/task-1 - first.md",
        "backlog/tasks/task-2 - second.md",
        "backlog/tasks/task-3 - other.md",
    ]
    .iter()
    .enumerate()
    {
        assert_source_matches(&fixture.0.join(path), &live["sources"][index]);
    }
}

#[test]
fn remove_keep_and_reassign_match_typescript() {
    for handling in [TaskMilestoneHandling::Keep, TaskMilestoneHandling::Reassign] {
        let fixture = Fixture::new();
        setup(&fixture);
        fixture.write(
            "backlog/milestones/m-2 - release-b.md",
            "---\nid: m-2\ntitle: Release B\n---\n",
        );
        let reference_fixture = Fixture::new();
        setup(&reference_fixture);
        reference_fixture.write(
            "backlog/milestones/m-2 - release-b.md",
            "---\nid: m-2\ntitle: Release B\n---\n",
        );
        let task_handling = if handling == TaskMilestoneHandling::Keep {
            "keep"
        } else {
            "reassign"
        };
        let live = reference(&reference_fixture.0, task_handling, "Release B");
        let milestones = MilestoneStore::new(&fixture.0, "backlog");
        let tasks = TaskStore::new(&fixture.0, "backlog", "task");
        let result = remove_milestone(
            &milestones,
            &tasks,
            "Release A",
            handling,
            (handling == TaskMilestoneHandling::Reassign).then_some("Release B"),
        )
        .unwrap();
        assert_eq!(live["summary"], result.summary());
        let values: Vec<_> = tasks
            .list_tasks()
            .into_iter()
            .map(|entry| entry.task.milestone)
            .collect();
        let expected = if handling == TaskMilestoneHandling::Keep {
            vec![
                Some("m-0".into()),
                Some("release a".into()),
                Some("m-9".into()),
            ]
        } else {
            vec![Some("m-2".into()), Some("m-2".into()), Some("m-9".into())]
        };
        assert_eq!(values, expected);
        assert_eq!(live["tasks"][0]["milestone"], serde_json::json!(values[0]));
        assert_eq!(live["tasks"][1]["milestone"], serde_json::json!(values[1]));
        assert_eq!(live["tasks"][2]["milestone"], serde_json::json!(values[2]));
        assert_eq!(live["archived"], serde_json::json!(["m-0"]));
        for (index, path) in [
            "backlog/tasks/task-1 - first.md",
            "backlog/tasks/task-2 - second.md",
            "backlog/tasks/task-3 - other.md",
        ]
        .iter()
        .enumerate()
        {
            assert_source_matches(&fixture.0.join(path), &live["sources"][index]);
        }
    }
}

fn assert_source_matches(path: &std::path::Path, live: &Value) {
    let source = std::fs::read_to_string(path).unwrap();
    let parsed = backlog_md::frontmatter::parse_frontmatter(&source).unwrap();
    let mut data = parsed.data;
    for key in [
        "assignee",
        "dependencies",
        "labels",
        "references",
        "documentation",
        "modified_files",
    ] {
        if data
            .get(key)
            .is_some_and(|value| value.as_array().is_some_and(Vec::is_empty))
        {
            data.remove(key);
        }
    }
    assert_eq!(live["data"], serde_json::Value::Object(data));
    assert_eq!(live["content"], parsed.content.trim_end());
}

#[test]
fn archive_failure_rolls_back_task_milestone_values_like_typescript() {
    let fixture = Fixture::new();
    setup(&fixture);
    fixture.write(
        "backlog/archive/milestones/m-0 - release-a.md/keep.txt",
        "occupied\n",
    );
    let reference_fixture = Fixture::new();
    setup(&reference_fixture);
    reference_fixture.write(
        "backlog/archive/milestones/m-0 - release-a.md/keep.txt",
        "occupied\n",
    );
    let script = r#"
import { Core } from "./src/core/backlog.ts";
import { MilestoneHandlers } from "./src/mcp/tools/milestones/handlers.ts";
const core = new Core(process.env.BACKLOG_MILESTONE_ROOT);
let message; try { await new MilestoneHandlers(core).removeMilestone({name:"Release A",taskHandling:"clear"}); } catch (error) { message = error.message; }
const tasks = await core.filesystem.listTasks();
const active = await core.filesystem.listMilestones();
process.stdout.write(JSON.stringify({message,tasks:tasks.map(t=>({id:t.id,milestone:t.milestone??null})),active:active.map(m=>m.id)}));
"#;
    let output =
        Command::new(std::env::var_os("BACKLOG_REFERENCE_BUN").unwrap_or_else(|| "bun".into()))
            .args(["--eval", script])
            .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
            .env("BACKLOG_MILESTONE_ROOT", &reference_fixture.0)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let live: Value = serde_json::from_slice(&output.stdout).unwrap();
    let milestones = MilestoneStore::new(&fixture.0, "backlog");
    let tasks = TaskStore::new(&fixture.0, "backlog", "task");
    let error = remove_milestone(
        &milestones,
        &tasks,
        "Release A",
        TaskMilestoneHandling::Clear,
        None,
    )
    .unwrap_err();
    assert_eq!(live["message"], error);
    let mut actual_tasks: Vec<_> = tasks
        .list_tasks()
        .into_iter()
        .map(|entry| serde_json::json!({"id":entry.task.id,"milestone":entry.task.milestone}))
        .collect();
    actual_tasks.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    assert_eq!(live["tasks"], serde_json::json!(actual_tasks));
    assert_eq!(
        live["active"],
        serde_json::json!(
            milestones
                .list_active()
                .iter()
                .map(|m| m.id.clone())
                .collect::<Vec<_>>()
        )
    );
}

#[test]
fn invalid_reassignment_refuses_without_mutating_either_store_like_typescript() {
    let fixture = Fixture::new();
    setup(&fixture);
    let reference_fixture = Fixture::new();
    setup(&reference_fixture);
    let script = r#"
import { Core } from "./src/core/backlog.ts";
import { MilestoneHandlers } from "./src/mcp/tools/milestones/handlers.ts";
const core = new Core(process.env.BACKLOG_MILESTONE_ROOT);
let message; try { await new MilestoneHandlers(core).removeMilestone({name:"Release A",taskHandling:"reassign",reassignTo:"Missing"}); } catch (error) { message = error.message; }
const tasks = (await core.filesystem.listTasks()).sort((a,b)=>a.id.localeCompare(b.id));
const milestone = await Bun.file(process.env.BACKLOG_MILESTONE_ROOT + "/backlog/milestones/m-0 - release-a.md").text();
process.stdout.write(JSON.stringify({message,tasks:tasks.map(t=>({id:t.id,milestone:t.milestone??null})),milestone}));
"#;
    let output =
        Command::new(std::env::var_os("BACKLOG_REFERENCE_BUN").unwrap_or_else(|| "bun".into()))
            .args(["--eval", script])
            .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
            .env("BACKLOG_MILESTONE_ROOT", &reference_fixture.0)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let live: Value = serde_json::from_slice(&output.stdout).unwrap();
    let milestones = MilestoneStore::new(&fixture.0, "backlog");
    let tasks = TaskStore::new(&fixture.0, "backlog", "task");
    let before = tasks
        .list_tasks()
        .into_iter()
        .map(|entry| (entry.task.id, entry.task.milestone))
        .collect::<Vec<_>>();
    assert_eq!(
        remove_milestone(
            &milestones,
            &tasks,
            "Release A",
            TaskMilestoneHandling::Reassign,
            Some("Missing")
        )
        .unwrap_err(),
        live["message"].as_str().unwrap()
    );
    let mut after: Vec<_> = tasks
        .list_tasks()
        .into_iter()
        .map(|entry| (entry.task.id, entry.task.milestone))
        .collect();
    after.sort_by(|a, b| a.0.cmp(&b.0));
    let mut before = before;
    before.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(after, before);
    assert_eq!(
        live["tasks"],
        serde_json::json!(
            after
                .iter()
                .map(|(id, milestone)| serde_json::json!({"id":id,"milestone":milestone}))
                .collect::<Vec<_>>()
        )
    );
    assert_eq!(
        std::fs::read_to_string(fixture.0.join("backlog/milestones/m-0 - release-a.md")).unwrap(),
        live["milestone"].as_str().unwrap()
    );
}
