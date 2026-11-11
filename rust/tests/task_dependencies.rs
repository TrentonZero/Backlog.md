use backlog_md::{
    config::Config,
    records::{parse_markdown, parse_task},
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
            "backlog-task-dependencies-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("backlog/tasks")).unwrap();
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

fn reference(root: &PathBuf) -> Value {
    reference_script(
        root,
        r#"
import { Core } from "./src/core/backlog.ts";
const core = new Core(process.env.BACKLOG_DEPENDENCY_ROOT);
const created = await core.createTaskFromInput({title:"Dependent", dependencies:["TASK-1","task-01","TASK-2","draft-1"]}, false);
process.stdout.write(JSON.stringify({id:created.task.id,dependencies:created.task.dependencies}));
"#,
    )
}

fn reference_script(root: &PathBuf, script: &str) -> Value {
    let output =
        Command::new(std::env::var_os("BACKLOG_REFERENCE_BUN").unwrap_or_else(|| "bun".into()))
            .args(["--eval", script])
            .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
            .env("BACKLOG_DEPENDENCY_ROOT", root)
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

fn task(source: &str) -> backlog_md::records::TaskRecord {
    parse_task(source).unwrap()
}

fn task_record(id: &str, title: &str, dependencies: Vec<&str>) -> backlog_md::records::TaskRecord {
    let mut task = task(&format!(
        "---\nid: {id}\ntitle: {title}\nstatus: To Do\ncreated_date: 2026-01-01\n---\n"
    ));
    task.dependencies = dependencies.into_iter().map(str::to_owned).collect();
    task
}

#[test]
fn task_creation_resolves_and_deduplicates_task_archived_and_draft_dependencies_like_typescript() {
    let fixture = Fixture::new();
    fixture.write(
        "backlog/tasks/task-1 - Active.md",
        "---\nid: TASK-1\ntitle: Active\nstatus: To Do\ncreated_date: 2026-01-01\n---\nActive\n",
    );
    fixture.write(
        "backlog/archive/tasks/task-2 - Archived.md",
        "---\nid: TASK-2\ntitle: Archived\nstatus: Done\ncreated_date: 2026-01-01\n---\nArchived\n",
    );
    fixture.write(
        "backlog/drafts/draft-1 - Draft.md",
        "---\nid: DRAFT-1\ntitle: Draft\nstatus: Draft\ncreated_date: 2026-01-01\n---\nDraft\n",
    );
    let reference_fixture = Fixture::new();
    reference_fixture.write(
        "backlog/tasks/task-1 - Active.md",
        "---\nid: TASK-1\ntitle: Active\nstatus: To Do\ncreated_date: 2026-01-01\n---\nActive\n",
    );
    reference_fixture.write(
        "backlog/archive/tasks/task-2 - Archived.md",
        "---\nid: TASK-2\ntitle: Archived\nstatus: Done\ncreated_date: 2026-01-01\n---\nArchived\n",
    );
    reference_fixture.write(
        "backlog/drafts/draft-1 - Draft.md",
        "---\nid: DRAFT-1\ntitle: Draft\nstatus: Draft\ncreated_date: 2026-01-01\n---\nDraft\n",
    );
    let live = reference(&reference_fixture.0);
    let store = TaskStore::new(&fixture.0, "backlog", "task");
    let mut dependent =
        task("---\nid: task-0\ntitle: Dependent\nstatus: To Do\ncreated_date: 2026-01-01\n---\n");
    dependent.dependencies = vec![
        "TASK-1".into(),
        "task-01".into(),
        "TASK-2".into(),
        "draft-1".into(),
    ];
    let created = store.create_task(dependent, &Config::default()).unwrap();
    assert_eq!(live["id"], created.task.id);
    assert_eq!(
        live["dependencies"],
        serde_json::json!(created.task.dependencies)
    );
    assert_eq!(created.task.dependencies, ["TASK-1", "TASK-2", "DRAFT-1"]);
}

#[test]
fn dependency_edits_resolve_and_deduplicate_aliases_like_typescript() {
    let fixture = Fixture::new();
    fixture.write(
        "backlog/tasks/task-1 - Active.md",
        "---\nid: TASK-1\ntitle: Active\nstatus: To Do\ncreated_date: 2026-01-01\n---\nActive\n",
    );
    fixture.write(
        "backlog/tasks/task-2 - Dependent.md",
        "---\nid: TASK-2\ntitle: Dependent\nstatus: To Do\ncreated_date: 2026-01-01\n---\nDependent\n",
    );
    let reference_fixture = Fixture::new();
    reference_fixture.write(
        "backlog/tasks/task-1 - Active.md",
        "---\nid: TASK-1\ntitle: Active\nstatus: To Do\ncreated_date: 2026-01-01\n---\nActive\n",
    );
    reference_fixture.write(
        "backlog/tasks/task-2 - Dependent.md",
        "---\nid: TASK-2\ntitle: Dependent\nstatus: To Do\ncreated_date: 2026-01-01\n---\nDependent\n",
    );
    let live = reference_script(
        &reference_fixture.0,
        r#"
import { Core } from "./src/core/backlog.ts";
const core = new Core(process.env.BACKLOG_DEPENDENCY_ROOT);
const updated = await core.updateTaskFromInput("TASK-2",{dependencies:["task-01","TASK-1"]},false);
const source = await Bun.file(process.env.BACKLOG_DEPENDENCY_ROOT+"/backlog/tasks/task-2 - Dependent.md").text();
process.stdout.write(JSON.stringify({dependencies:updated.dependencies,source}));
"#,
    );
    let store = TaskStore::new(&fixture.0, "backlog", "task");
    let mut updated = store.load_task("TASK-2").unwrap().unwrap().task;
    updated.dependencies = vec!["task-01".into(), "TASK-1".into()];
    let result = store.update_task(updated).unwrap().unwrap();
    let source = std::fs::read_to_string(result.path).unwrap();
    assert_eq!(
        live["dependencies"],
        serde_json::json!(result.task.dependencies)
    );
    assert_eq!(result.task.dependencies, ["TASK-1"]);
    let actual_markdown = parse_markdown(&source).unwrap();
    let expected_markdown = parse_markdown(live["source"].as_str().unwrap()).unwrap();
    assert_eq!(
        actual_markdown.frontmatter["dependencies"],
        expected_markdown.frontmatter["dependencies"]
    );
    assert_eq!(actual_markdown.content, expected_markdown.content);
}

#[test]
fn missing_and_self_dependencies_are_refused_before_task_creation_like_typescript() {
    let fixture = Fixture::new();
    fixture.write("backlog/tasks/task-1 - Existing.md", "---\nid: TASK-1\ntitle: Existing\nstatus: To Do\ncreated_date: 2026-01-01\n---\nExisting\n");
    let reference_fixture = Fixture::new();
    reference_fixture.write("backlog/tasks/task-1 - Existing.md", "---\nid: TASK-1\ntitle: Existing\nstatus: To Do\ncreated_date: 2026-01-01\n---\nExisting\n");
    let live = reference_script(
        &reference_fixture.0,
        r#"
import { Core } from "./src/core/backlog.ts";
const core = new Core(process.env.BACKLOG_DEPENDENCY_ROOT);
async function capture(dependencies) { try { await core.createTaskFromInput({title:"New",dependencies},false); return "created"; } catch (error) { return error.message; } }
const missing = await capture(["TASK-999"]);
const self = await capture(["task-02"]);
process.stdout.write(JSON.stringify({missing,self,files:(await core.filesystem.listTasks()).map(t=>t.id)}));
"#,
    );
    let store = TaskStore::new(&fixture.0, "backlog", "task");
    let missing = store
        .create_task(
            task_record("task-0", "New", vec!["TASK-999"]),
            &Config::default(),
        )
        .unwrap_err()
        .to_string();
    let self_dependency = store
        .create_task(
            task_record("task-0", "New", vec!["task-02"]),
            &Config::default(),
        )
        .unwrap_err()
        .to_string();
    assert_eq!(live["missing"], missing);
    assert_eq!(live["self"], self_dependency);
    assert_eq!(live["files"], serde_json::json!(["TASK-1"]));
    assert_eq!(store.list_tasks().len(), 1);
}

#[test]
fn dependency_cycle_refusal_matches_live_typescript_and_preserves_task_bytes() {
    let fixture = Fixture::new();
    fixture.write(
        "backlog/tasks/task-1 - First.md",
        "---\nid: TASK-1\ntitle: First\nstatus: To Do\ncreated_date: 2026-01-01\n---\nFirst\n",
    );
    fixture.write("backlog/tasks/task-2 - Second.md", "---\nid: TASK-2\ntitle: Second\nstatus: To Do\ncreated_date: 2026-01-01\ndependencies:\n  - TASK-1\n---\nSecond\n");
    let reference_fixture = Fixture::new();
    reference_fixture.write(
        "backlog/tasks/task-1 - First.md",
        "---\nid: TASK-1\ntitle: First\nstatus: To Do\ncreated_date: 2026-01-01\n---\nFirst\n",
    );
    reference_fixture.write("backlog/tasks/task-2 - Second.md", "---\nid: TASK-2\ntitle: Second\nstatus: To Do\ncreated_date: 2026-01-01\ndependencies:\n  - TASK-1\n---\nSecond\n");
    let live = reference_script(
        &reference_fixture.0,
        r#"
import { Core } from "./src/core/backlog.ts";
const core = new Core(process.env.BACKLOG_DEPENDENCY_ROOT);
let message; try { await core.updateTaskFromInput("TASK-1",{dependencies:["TASK-2"]},false); } catch (error) { message = error.message; }
const source = await Bun.file(process.env.BACKLOG_DEPENDENCY_ROOT+"/backlog/tasks/task-1 - First.md").text();
process.stdout.write(JSON.stringify({message,source}));
"#,
    );
    let store = TaskStore::new(&fixture.0, "backlog", "task");
    let entry = store.load_task("TASK-1").unwrap().unwrap();
    let source_before = std::fs::read_to_string(&entry.path).unwrap();
    let mut updated = entry.task;
    updated.dependencies = vec!["TASK-2".into()];
    let error = store.update_task(updated).unwrap_err().to_string();
    let live_message = live["message"].as_str().unwrap().replace(
        &reference_fixture.0.display().to_string(),
        &fixture.0.display().to_string(),
    );
    assert_eq!(live_message, error);
    assert_eq!(std::fs::read_to_string(entry.path).unwrap(), source_before);
    assert_eq!(live["source"], source_before);
}

#[test]
fn ambiguous_task_dependency_refusal_matches_live_typescript_and_preserves_task_bytes() {
    let fixture = Fixture::new();
    fixture.write(
        "backlog/tasks/task-1 - First.md",
        "---\nid: TASK-1\ntitle: First\nstatus: To Do\ncreated_date: 2026-01-01\n---\nFirst\n",
    );
    fixture.write(
        "backlog/tasks/task-2 - Second.md",
        "---\nid: TASK-2\ntitle: Second\nstatus: To Do\ncreated_date: 2026-01-01\n---\nSecond\n",
    );
    fixture.write("backlog/archive/tasks/task-01 - Duplicate.md", "---\nid: TASK-01\ntitle: Duplicate\nstatus: Done\ncreated_date: 2026-01-01\n---\nDuplicate\n");
    let reference_fixture = Fixture::new();
    reference_fixture.write(
        "backlog/tasks/task-1 - First.md",
        "---\nid: TASK-1\ntitle: First\nstatus: To Do\ncreated_date: 2026-01-01\n---\nFirst\n",
    );
    reference_fixture.write(
        "backlog/tasks/task-2 - Second.md",
        "---\nid: TASK-2\ntitle: Second\nstatus: To Do\ncreated_date: 2026-01-01\n---\nSecond\n",
    );
    reference_fixture.write("backlog/archive/tasks/task-01 - Duplicate.md", "---\nid: TASK-01\ntitle: Duplicate\nstatus: Done\ncreated_date: 2026-01-01\n---\nDuplicate\n");
    let live = reference_script(
        &reference_fixture.0,
        r#"
import { Core } from "./src/core/backlog.ts";
const core = new Core(process.env.BACKLOG_DEPENDENCY_ROOT);
let message; try { await core.updateTaskFromInput("TASK-2",{dependencies:["TASK-1"]},false); } catch (error) { message = error.message; }
process.stdout.write(JSON.stringify({message,source:await Bun.file(process.env.BACKLOG_DEPENDENCY_ROOT+"/backlog/tasks/task-2 - Second.md").text()}));
"#,
    );
    let store = TaskStore::new(&fixture.0, "backlog", "task");
    let entry = store.load_task("TASK-2").unwrap().unwrap();
    let source = std::fs::read_to_string(&entry.path).unwrap();
    let mut updated = entry.task;
    updated.dependencies = vec!["TASK-1".into()];
    let error = store.update_task(updated).unwrap_err().to_string();
    let live_message = live["message"].as_str().unwrap().replace(
        &reference_fixture.0.display().to_string(),
        &fixture.0.display().to_string(),
    );
    assert_eq!(live_message, error);
    assert_eq!(std::fs::read_to_string(entry.path).unwrap(), source);
    assert_eq!(live["source"], source);
}
