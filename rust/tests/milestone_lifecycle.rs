use backlog_md::milestone_store::MilestoneStore;
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
            "backlog-milestone-lifecycle-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("backlog/milestones")).unwrap();
        Self(root)
    }
    fn write(&self, relative: &str, source: &str) -> PathBuf {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, source).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn reference(script: &str, root: &PathBuf) -> Value {
    let output =
        Command::new(std::env::var_os("BACKLOG_REFERENCE_BUN").unwrap_or_else(|| "bun".into()))
            .args(["--eval", script])
            .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
            .env("BACKLOG_MILESTONE_ROOT", root)
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
fn milestone_rename_and_archive_match_typescript() {
    let source = "---\nid: m-0\ntitle: Release A\ndue_date: 2026-08-10\ncustom: removed by existing serializer\n---\n\n## Description\n\nMilestone: Release A\n\n## Notes\n\nKeep this.\n";
    let fixture = Fixture::new();
    fixture.write("backlog/milestones/m-0 - release-a.md", source);
    fixture.write("backlog/milestones/m-0 - taken.md", "occupied\n");
    let reference_fixture = Fixture::new();
    reference_fixture.write("backlog/milestones/m-0 - release-a.md", source);
    reference_fixture.write("backlog/milestones/m-0 - taken.md", "occupied\n");
    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
const fs = new Core(process.env.BACKLOG_MILESTONE_ROOT).filesystem;
const renamed = await fs.renameMilestone("Release A", "Release B");
const collision = await fs.renameMilestone("Release B", "Taken");
const collisionSource = await Bun.file(renamed.targetPath).text();
const collisionTarget = await Bun.file(process.env.BACKLOG_MILESTONE_ROOT + "/backlog/milestones/m-0 - taken.md").text();
let invalid; try { await fs.renameMilestone("m-0", "Invalid", "2026-02-31"); } catch (error) { invalid = error.message; }
const renamedContent = await Bun.file(renamed.targetPath).text();
const archived = await fs.archiveMilestone("m-0");
const archivedAgain = await fs.archiveMilestone("m-0");
process.stdout.write(JSON.stringify({renamed:{success:renamed.success, previousTitle:renamed.previousTitle, previousDueDate:renamed.previousDueDate, milestone:renamed.milestone, source:renamed.sourcePath?.split("backlog/")[1], target:renamed.targetPath?.split("backlog/")[1], content:renamedContent}, collision:collision.success, collisionSource, collisionTarget, invalid, archived:{success:archived.success, milestone:archived.milestone, source:archived.sourcePath?.split("backlog/")[1], target:archived.targetPath?.split("backlog/")[1]}, archivedAgain:archivedAgain.success}));
"#;
    let live = reference(SCRIPT, &reference_fixture.0);
    let store = MilestoneStore::new(&fixture.0, "backlog");
    let renamed = store.rename("Release A", "Release B", None).unwrap();
    assert!(renamed.success);
    assert_eq!(renamed.previous_title.as_deref(), Some("Release A"));
    assert_eq!(renamed.previous_due_date.as_deref(), Some("2026-08-10"));
    assert_eq!(renamed.milestone.as_ref().unwrap().title, "Release B");
    assert_eq!(
        renamed.milestone.as_ref().unwrap().description,
        "Milestone: Release B"
    );
    assert!(
        renamed
            .milestone
            .as_ref()
            .unwrap()
            .raw_content
            .contains("Keep this.")
    );
    assert_eq!(live["renamed"]["success"], renamed.success);
    assert_eq!(
        live["renamed"]["previousTitle"],
        serde_json::json!(renamed.previous_title)
    );
    assert_eq!(
        live["renamed"]["previousDueDate"],
        serde_json::json!(renamed.previous_due_date)
    );
    assert_eq!(
        live["renamed"]["milestone"]["title"],
        renamed.milestone.as_ref().unwrap().title
    );
    assert_eq!(live["renamed"]["source"], "milestones/m-0 - release-a.md");
    assert_eq!(live["renamed"]["target"], "milestones/m-0 - release-b.md");
    let collision = store.rename("Release B", "Taken", None).unwrap();
    assert!(!collision.success);
    assert_eq!(live["collision"], false);
    assert_eq!(
        std::fs::read_to_string(fixture.0.join("backlog/milestones/m-0 - release-b.md")).unwrap(),
        live["collisionSource"].as_str().unwrap()
    );
    assert_eq!(
        std::fs::read_to_string(fixture.0.join("backlog/milestones/m-0 - taken.md")).unwrap(),
        live["collisionTarget"].as_str().unwrap()
    );
    assert_eq!(
        store
            .rename("m-0", "Invalid", Some(Some("2026-02-31")))
            .unwrap_err(),
        live["invalid"].as_str().unwrap()
    );
    assert_eq!(
        std::fs::read_to_string(fixture.0.join("backlog/milestones/m-0 - release-b.md")).unwrap(),
        live["renamed"]["content"].as_str().unwrap()
    );
    let archived = store.archive("m-0").unwrap();
    assert!(archived.success);
    assert_eq!(live["archived"]["success"], archived.success);
    assert_eq!(
        live["archived"]["milestone"]["title"],
        archived.milestone.as_ref().unwrap().title
    );
    assert_eq!(live["archived"]["source"], "milestones/m-0 - release-b.md");
    assert_eq!(
        live["archived"]["target"],
        "archive/milestones/m-0 - release-b.md"
    );
    assert!(!store.archive("m-0").unwrap().success);
    assert_eq!(live["archivedAgain"], false);
    assert_eq!(
        std::fs::read_to_string(
            fixture
                .0
                .join("backlog/archive/milestones/m-0 - release-b.md")
        )
        .unwrap(),
        std::fs::read_to_string(
            reference_fixture
                .0
                .join("backlog/archive/milestones/m-0 - release-b.md")
        )
        .unwrap()
    );
}

#[test]
fn archive_collision_returns_false_and_preserves_both_paths_like_typescript() {
    let fixture = Fixture::new();
    let source = "---\nid: m-1\ntitle: Collision\n---\nContent\n";
    let source_path = fixture.write("backlog/milestones/m-1 - collision.md", source);
    let reference_fixture = Fixture::new();
    reference_fixture.write("backlog/milestones/m-1 - collision.md", source);
    fixture.write(
        "backlog/archive/milestones/m-1 - collision.md/keep.txt",
        "occupied\n",
    );
    reference_fixture.write(
        "backlog/archive/milestones/m-1 - collision.md/keep.txt",
        "occupied\n",
    );
    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
const fs = new Core(process.env.BACKLOG_MILESTONE_ROOT).filesystem;
const result = await fs.archiveMilestone("m-1");
const source = await Bun.file(process.env.BACKLOG_MILESTONE_ROOT + "/backlog/milestones/m-1 - collision.md").text();
const occupied = await Bun.file(process.env.BACKLOG_MILESTONE_ROOT + "/backlog/archive/milestones/m-1 - collision.md/keep.txt").text();
process.stdout.write(JSON.stringify({success:result.success, source, occupied}));
"#;
    let live = reference(SCRIPT, &reference_fixture.0);
    let result = MilestoneStore::new(&fixture.0, "backlog")
        .archive("m-1")
        .unwrap();
    assert!(!result.success);
    assert_eq!(live["success"], false);
    assert_eq!(
        std::fs::read_to_string(source_path).unwrap(),
        live["source"]
    );
    assert_eq!(
        std::fs::read_to_string(
            fixture
                .0
                .join("backlog/archive/milestones/m-1 - collision.md/keep.txt")
        )
        .unwrap(),
        live["occupied"]
    );
}
