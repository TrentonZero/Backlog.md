use backlog_md::{
    config::Config,
    draft_store::DraftStore,
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

#[test]
fn status_promotion_rejects_allocated_self_and_cycles_before_moving_the_draft() {
    for mode in ["self", "cycle"] {
        let fixture = Fixture::new();
        let reference_fixture = Fixture::new();
        let draft_dependencies = "TASK-1";
        fixture.write("backlog/drafts/draft-1 - Draft.md", &format!("---\nid: DRAFT-1\ntitle: Draft\nstatus: Draft\ncreated_date: 2026-01-01\ndependencies: [{draft_dependencies}]\n---\nDraft\n"));
        reference_fixture.write("backlog/drafts/draft-1 - Draft.md", &format!("---\nid: DRAFT-1\ntitle: Draft\nstatus: Draft\ncreated_date: 2026-01-01\ndependencies: [{draft_dependencies}]\n---\nDraft\n"));
        if mode == "cycle" {
            let existing = "---\nid: TASK-1\ntitle: Existing\nstatus: To Do\ncreated_date: 2026-01-01\ndependencies: [TASK-2]\n---\nExisting\n";
            fixture.write("backlog/tasks/task-1 - Existing.md", existing);
            reference_fixture.write("backlog/tasks/task-1 - Existing.md", existing);
        }
        let live = reference_script(
            &reference_fixture.0,
            r#"
import { Core } from "./src/core/backlog.ts";
const core = new Core(process.env.BACKLOG_DEPENDENCY_ROOT);
let message; try { await core.editTaskOrDraft("DRAFT-1",{status:"To Do"},false); } catch (error) { message = error.message; }
const draftPath = process.env.BACKLOG_DEPENDENCY_ROOT+"/backlog/drafts/draft-1 - Draft.md";
process.stdout.write(JSON.stringify({message,draftExists:!!await core.filesystem.loadDraft("DRAFT-1"),source:await Bun.file(draftPath).text(),task:await core.filesystem.loadTask("TASK-2")}));
"#,
        );
        let store = TaskStore::new(&fixture.0, "backlog", "task");
        let error = store
            .promote_draft_for_status_update("DRAFT-1", "To Do", &Config::default())
            .unwrap_err()
            .to_string();
        assert_eq!(live["message"], error, "{mode}");
        assert_eq!(
            fixture.0.join("backlog/drafts/draft-1 - Draft.md").exists(),
            live["draftExists"]
        );
        assert_eq!(
            std::fs::read_to_string(fixture.0.join("backlog/drafts/draft-1 - Draft.md")).unwrap(),
            live["source"]
        );
        assert!(!fixture.0.join("backlog/tasks/task-2 - Draft.md").exists());
    }
}

#[test]
fn status_demotion_rejects_allocated_draft_self_and_cycles_before_cleanup() {
    for mode in ["self", "cycle"] {
        let fixture = Fixture::new();
        let reference_fixture = Fixture::new();
        let task_dependencies = if mode == "self" { "DRAFT-1" } else { "TASK-2" };
        let target = format!(
            "---\nid: TASK-1\ntitle: Target\nstatus: To Do\ncreated_date: 2026-01-01\ndependencies: [{task_dependencies}]\n---\nTarget\n"
        );
        fixture.write("backlog/tasks/task-1 - Target.md", &target);
        reference_fixture.write("backlog/tasks/task-1 - Target.md", &target);
        if mode == "cycle" {
            let dependent = "---\nid: TASK-2\ntitle: Existing\nstatus: To Do\ncreated_date: 2026-01-01\ndependencies: [DRAFT-1]\n---\nExisting\n";
            fixture.write("backlog/tasks/task-2 - Existing.md", dependent);
            reference_fixture.write("backlog/tasks/task-2 - Existing.md", dependent);
        }
        let live = reference_script(
            &reference_fixture.0,
            r#"
import { Core } from "./src/core/backlog.ts";
const core = new Core(process.env.BACKLOG_DEPENDENCY_ROOT);
let message; try { await core.updateTaskFromInput("TASK-1",{status:"Draft"},false); } catch (error) { message = error.message; }
const root = process.env.BACKLOG_DEPENDENCY_ROOT;
const linkedPath = root+"/backlog/tasks/task-2 - Existing.md";
let linkedSource; try { linkedSource = await Bun.file(linkedPath).text(); } catch {}
process.stdout.write(JSON.stringify({message,source:await Bun.file(root+"/backlog/tasks/task-1 - Target.md").text(),draft:await core.filesystem.loadDraft("DRAFT-1"),linkedSource}));
"#,
        );
        let store = TaskStore::new(&fixture.0, "backlog", "task");
        let source = fixture.0.join("backlog/tasks/task-1 - Target.md");
        let linked_source_path = fixture.0.join("backlog/tasks/task-2 - Existing.md");
        let linked_source_before = linked_source_path
            .exists()
            .then(|| std::fs::read_to_string(&linked_source_path).unwrap());
        let error = store
            .demote_task_for_status_update("TASK-1", &Config::default())
            .unwrap_err()
            .to_string();
        assert_eq!(live["message"], error, "{mode}");
        assert_eq!(std::fs::read_to_string(&source).unwrap(), live["source"]);
        if let Some(linked_source_before) = linked_source_before {
            assert_eq!(
                Some(linked_source_before),
                live["linkedSource"].as_str().map(str::to_owned)
            );
            assert_eq!(
                std::fs::read_to_string(linked_source_path).unwrap(),
                live["linkedSource"]
            );
        }
        assert!(
            !fixture
                .0
                .join("backlog/drafts/draft-1 - Target.md")
                .exists()
        );
        assert!(live["draft"].is_null());
    }
}

#[test]
fn status_transitions_keep_unresolved_legacy_dependencies_like_typescript() {
    let promotion = Fixture::new();
    let promotion_reference = Fixture::new();
    let draft = "---\nid: DRAFT-1\ntitle: Draft\nstatus: Draft\ncreated_date: 2026-01-01\ndependencies: [TASK-999]\n---\nDraft\n";
    promotion.write("backlog/drafts/draft-1 - Draft.md", draft);
    promotion_reference.write("backlog/drafts/draft-1 - Draft.md", draft);
    let promoted_live = reference_script(
        &promotion_reference.0,
        r#"
import { Core } from "./src/core/backlog.ts";
const core = new Core(process.env.BACKLOG_DEPENDENCY_ROOT);
const promoted = await core.editTaskOrDraft("DRAFT-1",{status:"In Progress"},false);
process.stdout.write(JSON.stringify({id:promoted.task.id,status:promoted.task.status,dependencies:promoted.task.dependencies}));
"#,
    );
    let tasks = TaskStore::new(&promotion.0, "backlog", "task");
    assert!(
        tasks
            .promote_draft_for_status_update("DRAFT-1", "In Progress", &Config::default())
            .unwrap()
    );
    let promoted = tasks.load_task("TASK-1").unwrap().unwrap().task;
    assert_eq!(promoted_live["id"], promoted.id);
    assert_eq!(promoted_live["status"], promoted.status);
    assert_eq!(
        promoted_live["dependencies"],
        serde_json::json!(promoted.dependencies)
    );
    assert_eq!(promoted.dependencies, ["TASK-999"]);

    let demotion = Fixture::new();
    let demotion_reference = Fixture::new();
    let task = "---\nid: TASK-1\ntitle: Task\nstatus: To Do\ncreated_date: 2026-01-01\ndependencies: [TASK-999]\n---\nTask\n";
    demotion.write("backlog/tasks/task-1 - Task.md", task);
    demotion_reference.write("backlog/tasks/task-1 - Task.md", task);
    let demoted_live = reference_script(
        &demotion_reference.0,
        r#"
import { Core } from "./src/core/backlog.ts";
const core = new Core(process.env.BACKLOG_DEPENDENCY_ROOT);
const demoted = await core.updateTaskFromInput("TASK-1",{status:"Draft"},false);
process.stdout.write(JSON.stringify({id:demoted.id,status:demoted.status,dependencies:demoted.dependencies}));
"#,
    );
    let tasks = TaskStore::new(&demotion.0, "backlog", "task");
    let result = tasks
        .demote_task_for_status_update("TASK-1", &Config::default())
        .unwrap();
    assert!(result.success);
    let demoted = DraftStore::new(&demotion.0, "backlog")
        .load_draft("DRAFT-1")
        .unwrap()
        .unwrap()
        .task;
    assert_eq!(demoted_live["id"], demoted.id);
    assert_eq!(demoted_live["status"], demoted.status);
    assert_eq!(
        demoted_live["dependencies"],
        serde_json::json!(demoted.dependencies)
    );
    assert_eq!(demoted.dependencies, ["TASK-999"]);
}
