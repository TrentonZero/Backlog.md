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
            "backlog-milestone-store-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("backlog/milestones")).unwrap();
        Self(root)
    }

    fn write(&self, path: &str, source: &str) {
        let path = self.0.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, source).unwrap();
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
fn lists_resolves_and_creates_milestones_like_typescript() {
    let fixture = Fixture::new();
    fixture.write(
        "backlog/milestones/m-10 - Later.md",
        "---\nid: m-10\ntitle: Later\n---\n## Description\n\nLater release\n",
    );
    fixture.write(
        "backlog/milestones/m-2 - Earlier.md",
        "---\nid: m-2\ntitle: Earlier\n---\n## Description\n\nEarlier release\n",
    );
    fixture.write(
        "backlog/archive/milestones/m-8 - Archived.md",
        "---\nid: m-8\ntitle: Archived\n---\n## Description\n\nOld\n",
    );
    let reference_fixture = Fixture::new();
    reference_fixture.write(
        "backlog/milestones/m-10 - Later.md",
        "---\nid: m-10\ntitle: Later\n---\n## Description\n\nLater release\n",
    );
    reference_fixture.write(
        "backlog/milestones/m-2 - Earlier.md",
        "---\nid: m-2\ntitle: Earlier\n---\n## Description\n\nEarlier release\n",
    );
    reference_fixture.write(
        "backlog/archive/milestones/m-8 - Archived.md",
        "---\nid: m-8\ntitle: Archived\n---\n## Description\n\nOld\n",
    );
    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
const core = new Core(process.env.BACKLOG_MILESTONE_ROOT);
const active = await core.filesystem.listMilestones();
const archived = await core.filesystem.listArchivedMilestones();
const loaded = await core.filesystem.loadMilestone("2");
const created = await core.filesystem.createMilestone("Newest", "Ships it", "2026-12-31");
const timed = await core.filesystem.createMilestone("Timed", undefined, "2026-12-31T12:30:00-05:00");
const spaced = await core.filesystem.createMilestone("  Space  Name ");
let invalid; try { await core.filesystem.createMilestone("Invalid", undefined, "2026-02-31"); } catch (error) { invalid = error.message; }
process.stdout.write(JSON.stringify({active:active.map(m=>m.id), archived:archived.map(m=>m.id), loaded:loaded?.title, created, timed, spaced, invalid}));
"#;
    let live = reference(SCRIPT, &reference_fixture.0);
    let store = MilestoneStore::new(&fixture.0, "backlog");
    assert_eq!(
        store
            .list_active()
            .iter()
            .map(|m| m.id.as_str())
            .collect::<Vec<_>>(),
        ["m-2", "m-10"]
    );
    assert_eq!(store.load_active("2").unwrap().title, "Earlier");
    let created = store
        .create("Newest", Some("Ships it"), Some("2026-12-31"))
        .unwrap();
    assert_eq!(created.id, "m-11");
    assert_eq!(created.description, "Ships it");
    assert_eq!(created.due_date.as_deref(), Some("2026-12-31"));
    let timed = store
        .create("Timed", None, Some("2026-12-31T12:30:00-05:00"))
        .unwrap();
    assert_eq!(timed.id, "m-12");
    assert_eq!(timed.due_date.as_deref(), Some("2026-12-31"));
    let spaced = store.create("  Space  Name ", None, None).unwrap();
    assert_eq!(spaced.title, "  Space  Name ");
    assert_eq!(
        store
            .create("Invalid", None, Some("2026-02-31"))
            .unwrap_err(),
        "Due date must be a date in YYYY-MM-DD format (for example, 2026-08-10)."
    );
    assert_eq!(live["active"], serde_json::json!(["m-2", "m-10"]));
    assert_eq!(live["archived"], serde_json::json!(["m-8"]));
    assert_eq!(live["loaded"], "Earlier");
    assert_eq!(live["created"]["id"], "m-11");
    assert_eq!(live["created"]["description"], "Ships it");
    assert_eq!(live["created"]["dueDate"], "2026-12-31");
    assert_eq!(live["timed"]["id"], timed.id);
    assert_eq!(live["timed"]["dueDate"], serde_json::json!(timed.due_date));
    assert_eq!(live["spaced"]["id"], spaced.id);
    assert_eq!(live["spaced"]["title"], spaced.title);
    assert_eq!(
        live["invalid"],
        store
            .create("Invalid", None, Some("2026-02-31"))
            .unwrap_err()
    );
    assert_eq!(
        std::fs::read_to_string(fixture.0.join("backlog/milestones/m-11 - newest.md")).unwrap(),
        std::fs::read_to_string(
            reference_fixture
                .0
                .join("backlog/milestones/m-11 - newest.md")
        )
        .unwrap()
    );
}

#[test]
fn milestone_alias_collisions_follow_typescript_priority() {
    let fixture = Fixture::new();
    fixture.write(
        "backlog/milestones/m-02 - Leading-Zero.md",
        "---\nid: m-02\ntitle: Leading Zero\n---\n",
    );
    fixture.write(
        "backlog/milestones/m-2 - Canonical.md",
        "---\nid: m-2\ntitle: Canonical\n---\n",
    );
    fixture.write(
        "backlog/milestones/m-7 - Numeric-Title.md",
        "---\nid: m-7\ntitle: 2\n---\n",
    );
    let reference_fixture = Fixture::new();
    reference_fixture.write(
        "backlog/milestones/m-02 - Leading-Zero.md",
        "---\nid: m-02\ntitle: Leading Zero\n---\n",
    );
    reference_fixture.write(
        "backlog/milestones/m-2 - Canonical.md",
        "---\nid: m-2\ntitle: Canonical\n---\n",
    );
    reference_fixture.write(
        "backlog/milestones/m-7 - Numeric-Title.md",
        "---\nid: m-7\ntitle: 2\n---\n",
    );
    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
const fs = new Core(process.env.BACKLOG_MILESTONE_ROOT).filesystem;
const alias = await fs.loadMilestone("2");
const title = await fs.loadMilestone("Canonical");
process.stdout.write(JSON.stringify({alias:alias?.id, title:title?.id}));
"#;
    let live = reference(SCRIPT, &reference_fixture.0);
    let store = MilestoneStore::new(&fixture.0, "backlog");
    let alias = store.load_active("2").map(|record| record.id);
    let title = store.load_active("Canonical").map(|record| record.id);
    assert_eq!(live["alias"], serde_json::json!(alias));
    assert_eq!(live["title"], serde_json::json!(title));
    assert_eq!(alias.as_deref(), Some("m-2"));
    assert_eq!(title.as_deref(), Some("m-2"));
}
