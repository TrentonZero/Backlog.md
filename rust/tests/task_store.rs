use backlog_md::task_store::{TaskStore, TaskStoreError};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "backlog-task-store-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn file(&self, path: &str, content: &str) -> PathBuf {
        let path = self.0.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn record_shape(task: &backlog_md::records::TaskRecord, path: &Path) -> Value {
    json!({
        "id": task.id,
        "title": task.title,
        "parentTaskId": task.parent_task_id,
        "filePath": path,
    })
}

fn reference(root: &Path) -> Value {
    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
const core = new Core(process.env.BACKLOG_TASK_STORE_ROOT);
const shape = task => ({id:task.id, title:task.title, parentTaskId:task.parentTaskId ?? null, filePath:task.filePath ?? null});
const active = await core.filesystem.listTasks();
const archived = await core.filesystem.listArchivedTasks();
const lookups = [];
for (const id of ["1", "2", "3", "5", "8", "99"]) {
  try { lookups.push({id, record: (task => task ? shape(task) : null)(await core.filesystem.loadTask(id))}); }
  catch (error) { lookups.push({id, error: {name:error.name, message:error.message, candidates:error.candidates}}); }
}
process.stdout.write(JSON.stringify({active:active.map(shape), archived:archived.map(shape), lookups}));
"#;
    let bun = std::env::var_os("BACKLOG_REFERENCE_BUN").unwrap_or_else(|| "bun".into());
    let output = Command::new(bun)
        .args(["--eval", SCRIPT])
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
        .env("BACKLOG_TASK_STORE_ROOT", root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run live TypeScript task store");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn task_source(id: &str, title: &str, parent: Option<&str>) -> String {
    let parent = parent.map_or(String::new(), |parent| {
        format!("parent_task_id: {parent}\n")
    });
    format!(
        "---\nid: {id}\ntitle: {title}\nstatus: To Do\ncreated_date: '2025-01-01'\n{parent}---\nBody\n"
    )
}

#[test]
fn local_task_lists_and_lookups_match_live_typescript() {
    let fixture = Fixture::new();
    let active_path = fixture.file(
        "backlog/tasks/task-01 - Active.md",
        &task_source("task-01", "Active", Some("task-2")),
    );
    fixture.file("backlog/tasks/task-2 - Malformed.md", "---\nid: [\n---\n");
    fixture.file(
        "backlog/tasks/task-5 - Duplicate active.md",
        &task_source("TASK-5", "Duplicate active", None),
    );
    fixture.file(
        "backlog/tasks/task-8 - Invalid dependencies.md",
        &task_source("TASK-8", "Invalid dependencies", None).replace(
            "status: To Do\n",
            "status: To Do\ndependencies:\n  invalid: mapping\n",
        ),
    );
    let archived_path = fixture.file(
        "backlog/archive/tasks/task-03 - Archived.md",
        &task_source("task-03", "Archived", None),
    );
    fixture.file(
        "backlog/completed/task-4 - Legacy archive.md",
        &task_source("TASK-4", "Legacy archive", None),
    );
    let duplicate_archive_path = fixture.file(
        "backlog/archive/tasks/task-005 - Duplicate archived.md",
        &task_source("TASK-005", "Duplicate archived", None),
    );

    let expected = reference(&fixture.0);
    let store = TaskStore::new(&fixture.0, "backlog", "task");
    let active = store.list_tasks();
    let archived = store.list_archived_tasks();
    let lookups = ["1", "2", "3", "5", "8", "99"].map(|id| {
        match store.load_task(id) {
            Ok(Some(entry)) => json!({"id": id, "record": record_shape(&entry.task, &entry.path)}),
            Ok(None) => json!({"id": id, "record": null}),
            Err(TaskStoreError::Ambiguous(error)) => json!({"id": id, "error": {"name": error.name, "message": error.to_string(), "candidates": error.candidates}}),
            Err(TaskStoreError::Dependencies(message)) => json!({"id": id, "error": {"name": "TaskDependenciesParseError", "message": message}}),
            Err(error) => json!({"id": id, "error": {"name": "TaskStoreError", "message": error.to_string()}}),
        }
    });
    let actual = json!({
        "active": active.iter().map(|entry| record_shape(&entry.task, &entry.path)).collect::<Vec<_>>(),
        "archived": archived.iter().map(|entry| record_shape(&entry.task, &entry.path)).collect::<Vec<_>>(),
        "lookups": lookups,
    });
    assert_eq!(actual, expected);
    assert_eq!(
        actual["active"][0]["filePath"],
        active_path.to_string_lossy().as_ref()
    );
    assert_eq!(
        actual["archived"][0]["filePath"],
        archived_path.to_string_lossy().as_ref()
    );
    assert_eq!(
        actual["lookups"][3]["error"]["candidates"][0],
        duplicate_archive_path.to_string_lossy().as_ref()
    );
}
