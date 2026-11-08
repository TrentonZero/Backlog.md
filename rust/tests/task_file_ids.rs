use backlog_md::{identity::generate_next_id, task_file_ids::collect_task_file_ids};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "backlog-task-id fixture-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn file(root: &Path, path: &str, content: &str) {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    fn git(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn reference(project: &Path, prefix: &str) -> serde_json::Value {
    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
import { generateNextId } from "./src/utils/prefix-config.ts";
const core = new Core(process.env.BACKLOG_TASK_PROJECT);
const index = core as unknown as { loadTaskFileNameEntries(prefix: string): Promise<Array<{id:string}>> };
const entries = await index.loadTaskFileNameEntries(process.env.BACKLOG_TASK_PREFIX);
const ids = entries.map(entry => entry.id).sort();
process.stdout.write(JSON.stringify({ids, next: generateNextId(ids, process.env.BACKLOG_TASK_PREFIX)}));
"#;
    let bun = std::env::var_os("BACKLOG_REFERENCE_BUN").unwrap_or_else(|| "bun".into());
    let output = Command::new(bun)
        .args(["--eval", SCRIPT])
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
        .env("BACKLOG_TASK_PROJECT", project)
        .env("BACKLOG_TASK_PREFIX", prefix)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run live TypeScript task filename index");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn filename_ids_include_nested_active_and_archived_files_across_worktrees() {
    let fixture = Fixture::new();
    let main = fixture.0.join("repository");
    std::fs::create_dir_all(&main).unwrap();
    Fixture::git(&main, &["init", "-b", "main"]);
    Fixture::git(&main, &["config", "user.name", "Task ID Test"]);
    Fixture::git(&main, &["config", "user.email", "task-id@example.com"]);
    Fixture::file(&main, "apps/project/keep.txt", "tracked\n");
    Fixture::git(&main, &["add", "."]);
    Fixture::git(&main, &["commit", "-m", "Initialize"]);

    let worktree = fixture.0.join("linked worktree");
    let output = Command::new("git")
        .args([
            "worktree",
            "add",
            "-b",
            "feature",
            worktree.to_str().unwrap(),
        ])
        .current_dir(&main)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    for (root, files) in [
        (
            &main,
            vec![
                (
                    "apps/project/backlog/tasks/ops-01 - active.md",
                    "not frontmatter",
                ),
                ("apps/project/backlog/tasks/nested/ops-2 - nested.md", ""),
                ("apps/project/backlog/archive/tasks/ops-3 - archived.md", ""),
                ("apps/project/backlog/completed/ops-004.md", "malformed: ["),
            ],
        ),
        (
            &worktree,
            vec![
                ("apps/project/backlog/tasks/ops-5 - uncommitted.md", ""),
                ("apps/project/backlog/archive/tasks/ops-6 - archived.md", ""),
            ],
        ),
    ] {
        for (path, content) in files {
            Fixture::file(root, path, content);
        }
    }
    #[cfg(unix)]
    {
        let target = fixture.0.join("symlink-target.md");
        std::fs::write(&target, "unreadable frontmatter is irrelevant\n").unwrap();
        std::os::unix::fs::symlink(
            target,
            main.join("apps/project/backlog/tasks/ops-7 - linked.md"),
        )
        .unwrap();
    }

    let project = main.join("apps/project");
    let expected = reference(&project, "ops");
    let mut actual = collect_task_file_ids(&project, Path::new("backlog"), "ops");
    actual.sort();
    let ids: Vec<&str> = actual.iter().map(String::as_str).collect();
    let actual_result =
        serde_json::json!({"ids": actual, "next": generate_next_id(&ids, "ops", None)});
    assert_eq!(actual_result, expected);
    #[cfg(unix)]
    let expected_ids = ["OPS-004", "OPS-01", "OPS-3", "OPS-5", "OPS-6", "OPS-7"];
    #[cfg(not(unix))]
    let expected_ids = ["OPS-004", "OPS-01", "OPS-3", "OPS-5", "OPS-6"];
    assert_eq!(actual_result["ids"], serde_json::json!(expected_ids));
    #[cfg(unix)]
    assert_eq!(actual_result["next"], "OPS-8");
    #[cfg(not(unix))]
    assert_eq!(actual_result["next"], "OPS-7");
}

#[test]
fn non_git_projects_scan_only_their_own_task_files() {
    let fixture = Fixture::new();
    let project = fixture.0.join("project");
    Fixture::file(&project, "backlog/tasks/task-8 - local.md", "");
    Fixture::file(&fixture.0, "other/backlog/tasks/task-9 - unrelated.md", "");
    let expected = reference(&project, "task");
    let actual = collect_task_file_ids(&project, Path::new("backlog"), "task");
    let ids: Vec<&str> = actual.iter().map(String::as_str).collect();
    let actual_result =
        serde_json::json!({"ids": actual, "next": generate_next_id(&ids, "task", None)});
    assert_eq!(actual_result, expected);
    assert_eq!(actual_result["ids"], serde_json::json!(["TASK-8"]));
    assert_eq!(actual_result["next"], "TASK-9");
}
