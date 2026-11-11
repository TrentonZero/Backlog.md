use backlog_md::{
    config::Config,
    decision_store::{DecisionCreateInput, DecisionStore, DecisionUpdateInput},
    storage::Storage,
};
use serde_json::Value;
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    sync::Arc,
    sync::atomic::{AtomicUsize, Ordering},
    thread,
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "backlog-decision-store-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("backlog/decisions")).unwrap();
        Self(root)
    }

    fn decision(&self, folder: &str, name: &str, source: &str) {
        let path = self.0.join("backlog").join(folder).join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, source).unwrap();
    }
}

#[test]
fn refuses_ambiguous_and_archived_updates_and_uses_the_shared_entity_lock() {
    let fixture = Fixture::new();
    let reference_fixture = Fixture::new();
    let source =
        "---\nid: decision-1\ntitle: Existing\ndate: 2025-01-01\n---\n## Decision\nChoice\n";
    fixture.decision("decisions", "decision-1 - Existing.md", source);
    fixture.decision("archive/decisions", "decision-1 - Existing.md", source);
    reference_fixture.decision("decisions", "decision-1 - Existing.md", source);
    reference_fixture.decision("archive/decisions", "decision-1 - Existing.md", source);
    let store = DecisionStore::new(&fixture.0, "backlog", "task");
    let ambiguous = store.get_decision("1").unwrap_err().to_string();
    assert!(ambiguous.contains("Decision ID decision-1 is ambiguous; 2 files match:"));

    std::fs::remove_file(
        fixture
            .0
            .join("backlog/archive/decisions/decision-1 - Existing.md"),
    )
    .unwrap();
    let target = fixture.0.join("backlog/decisions/decision-1 - Existing.md");
    let blocker = Storage::new(fixture.0.join("backlog"), None)
        .acquire_entity_lock("decision", "decision-1", &target)
        .unwrap();
    let locked = store
        .update_decision(
            "1",
            DecisionUpdateInput {
                title: Some("Changed".into()),
                ..Default::default()
            },
        )
        .unwrap_err()
        .to_string();
    drop(blocker);
    assert!(locked.contains("lock"));
    std::fs::rename(
        &target,
        fixture
            .0
            .join("backlog/archive/decisions/decision-1 - Existing.md"),
    )
    .unwrap();
    let archived = store
        .update_decision(
            "1",
            DecisionUpdateInput {
                title: Some("Changed".into()),
                ..Default::default()
            },
        )
        .unwrap_err()
        .to_string();
    assert!(archived.contains("archived"));

    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
const core = new Core(process.env.BACKLOG_DECISION_ROOT);
let ambiguous = "";
try { await core.getDecision("1"); } catch (error) { ambiguous = error.message; }
await (await import("node:fs/promises")).rm(process.env.BACKLOG_DECISION_ROOT+"/backlog/decisions/decision-1 - Existing.md");
let archived = "";
try { await core.updateDecisionFromInput("1", {title:"Changed"}); } catch (error) { archived = error.message; }
process.stdout.write(JSON.stringify({ambiguous, archived}));
"#;
    let expected = reference(SCRIPT, &reference_fixture);
    assert_eq!(expected["ambiguous"], ambiguous);
    assert_eq!(expected["archived"], archived);
}

#[test]
fn decision_input_validation_matches_typescript_and_does_not_create_partial_files() {
    let fixture = Fixture::new();
    let reference_fixture = Fixture::new();
    let store = DecisionStore::new(&fixture.0, "backlog", "task");
    let config = Config::default();
    let empty_title = store
        .create_decision(
            DecisionCreateInput {
                title: "  ".into(),
                status: None,
                context: String::new(),
                decision: String::new(),
                consequences: String::new(),
                alternatives: None,
            },
            &config,
        )
        .unwrap_err()
        .to_string();
    let unsafe_section = store
        .create_decision(
            DecisionCreateInput {
                title: "Unsafe".into(),
                status: None,
                context: "<!-- SECTION:CONTEXT:BEGIN -->".into(),
                decision: String::new(),
                consequences: String::new(),
                alternatives: None,
            },
            &config,
        )
        .unwrap_err()
        .to_string();
    assert!(store.list_decisions().is_empty());

    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
const core = new Core(process.env.BACKLOG_DECISION_ROOT);
let emptyTitle = "";
let unsafeSection = "";
try { await core.createDecisionFromInput({title:"  "}); } catch (error) { emptyTitle = error.message; }
try { await core.createDecisionFromInput({title:"Unsafe", context:"<!-- SECTION:CONTEXT:BEGIN -->"}); } catch (error) { unsafeSection = error.message; }
process.stdout.write(JSON.stringify({emptyTitle, unsafeSection}));
"#;
    let expected = reference(SCRIPT, &reference_fixture);
    assert_eq!(expected["emptyTitle"], empty_title);
    assert_eq!(expected["unsafeSection"], unsafe_section);
    assert_eq!(expected["emptyTitle"], "Decision title cannot be empty.");
}

#[test]
fn update_lock_contention_matches_the_live_typescript_refusal() {
    let fixture = Fixture::new();
    let reference_fixture = Fixture::new();
    let source = "---\nid: decision-1\ntitle: Existing\ndate: 2025-01-01\nstatus: proposed\n---\n## Decision\nChoice\n";
    fixture.decision("decisions", "decision-1 - Existing.md", source);
    reference_fixture.decision("decisions", "decision-1 - Existing.md", source);
    let store = DecisionStore::new(&fixture.0, "backlog", "task");
    let target = fixture.0.join("backlog/decisions/decision-1 - Existing.md");
    let blocker = Storage::new(fixture.0.join("backlog"), None)
        .acquire_entity_lock("decision", "decision-1", &target)
        .unwrap();
    let rust_refused = store
        .update_decision(
            "1",
            DecisionUpdateInput {
                title: Some("Changed".into()),
                ..Default::default()
            },
        )
        .is_err();
    drop(blocker);
    assert_eq!(std::fs::read_to_string(target).unwrap(), source);

    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
const root = process.env.BACKLOG_DECISION_ROOT;
const core = new Core(root);
const decision = await core.getDecision("1");
let refused = false;
try { await core.filesystem.withContentRecordLock("decision", decision, () => core.updateDecisionFromInput("1", {title:"Changed"})); } catch { refused = true; }
process.stdout.write(JSON.stringify({refused, source:await Bun.file(root+"/backlog/decisions/decision-1 - Existing.md").text()}));
"#;
    let expected = reference(SCRIPT, &reference_fixture);
    assert!(rust_refused);
    assert_eq!(expected["refused"], true);
    assert_eq!(expected["source"], source);
}

#[test]
fn concurrent_decision_creates_claim_distinct_ids_without_overwriting() {
    let fixture = Fixture::new();
    let store = Arc::new(DecisionStore::new(&fixture.0, "backlog", "task"));
    let config = Config::default();
    let threads: Vec<_> = ["First", "Second"]
        .into_iter()
        .map(|title| {
            let store = Arc::clone(&store);
            let config = config.clone();
            thread::spawn(move || {
                store
                    .create_decision(
                        DecisionCreateInput {
                            title: title.into(),
                            status: None,
                            context: String::new(),
                            decision: String::new(),
                            consequences: String::new(),
                            alternatives: None,
                        },
                        &config,
                    )
                    .unwrap()
            })
        })
        .collect();
    let mut entries: Vec<_> = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    entries.sort_by(|left, right| left.decision.id.cmp(&right.decision.id));
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.decision.id.as_str())
            .collect::<Vec<_>>(),
        ["decision-1", "decision-2"]
    );
    assert_ne!(entries[0].path, entries[1].path);
    assert_eq!(store.list_decisions().len(), 2);
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn reference(script: &str, fixture: &Fixture) -> Value {
    let output =
        Command::new(std::env::var_os("BACKLOG_REFERENCE_BUN").unwrap_or_else(|| "bun".into()))
            .args(["--eval", script])
            .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
            .env("BACKLOG_DECISION_ROOT", &fixture.0)
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
fn lists_active_and_archived_decisions_and_resolves_identity_like_typescript() {
    let fixture = Fixture::new();
    fixture.decision(
        "decisions",
        "decision-10 - Ten.md",
        "---\nid: decision-10\ntitle: Ten\ndate: 2025-01-01\n---\n## Decision\nTen\n",
    );
    fixture.decision(
        "decisions",
        "decision-foo2 - Middle.md",
        "---\nid: decision-foo2\ntitle: Middle\ndate: 2025-01-01\n---\n## Decision\nMiddle\n",
    );
    fixture.decision(
        "decisions",
        "decision-2 - Two.md",
        "---\nid: DECISION-2\ntitle: Two\ndate: 2025-01-01\n---\n## Decision\nTwo\n",
    );
    fixture.decision(
        "archive/decisions",
        "decision-3 - Archived.md",
        "---\nid: decision-3\ntitle: Archived\ndate: 2025-01-01\n---\n## Decision\nArchived\n",
    );
    fixture.decision("decisions", "decision-4 - Broken.md", "---\nid: [\n---\n");

    let store = DecisionStore::new(&fixture.0, "backlog", "task");
    let active = store.list_decisions();
    let archived = store.list_archived_decisions();
    assert_eq!(
        active
            .iter()
            .map(|entry| entry.decision.id.as_str())
            .collect::<Vec<_>>(),
        ["DECISION-2", "decision-foo2", "decision-10"]
    );
    assert_eq!(archived.len(), 1);
    assert!(archived[0].archived);
    assert_eq!(
        store.get_decision("003").unwrap().unwrap().decision.title,
        "Archived"
    );

    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
const root = process.env.BACKLOG_DECISION_ROOT;
const core = new Core(root);
const active = await core.filesystem.listDecisions();
const archived = await core.filesystem.listArchivedDecisions();
const found = await core.getDecision("003");
process.stdout.write(JSON.stringify({active:active.map(d=>d.id), archived:archived.map(d=>d.id), found:found?.title ?? null}));
"#;
    assert_eq!(
        reference(SCRIPT, &fixture),
        serde_json::json!({"active":["DECISION-2", "decision-foo2", "decision-10"], "archived":["decision-3"], "found":"Archived"})
    );
}

#[test]
fn creates_and_partially_updates_decisions_without_losing_source_content() {
    let fixture = Fixture::new();
    let reference_fixture = Fixture::new();
    let existing_source = "---\nid: decision-1\ntitle: Existing\ndate: 2025-01-01\nstatus: proposed\ncustom: retained\n---\nUnmanaged note\n\n## Context\nOld context\n\n## Decision\nOld choice\n";
    fixture.decision("decisions", "decision-1 - Existing.md", existing_source);
    reference_fixture.decision("decisions", "decision-1 - Existing.md", existing_source);
    let store = DecisionStore::new(&fixture.0, "backlog", "task");
    let config = Config::default();
    let created = store
        .create_decision(
            DecisionCreateInput {
                title: "New choice".into(),
                status: None,
                context: "Context text".into(),
                decision: "Chosen path".into(),
                consequences: "Consequences".into(),
                alternatives: Some("Other path".into()),
            },
            &config,
        )
        .unwrap();
    assert_eq!(created.decision.id, "decision-2");
    let updated = store
        .update_decision(
            "1",
            DecisionUpdateInput {
                title: Some("Revised choice".into()),
                status: Some("superseded".into()),
                context: Some("New context".into()),
                decision: None,
                consequences: None,
                alternatives: Some(String::new()),
                superseded_by: Some("decision-2".into()),
            },
        )
        .unwrap();
    assert_eq!(updated.decision.title, "Revised choice");
    assert!(updated.decision.updated_date.is_some());
    assert_eq!(
        updated.decision.superseded_by.as_deref(),
        Some("decision-2")
    );
    let source = std::fs::read_to_string(&updated.path).unwrap();
    assert!(source.contains("custom: retained"));
    assert!(source.contains("Unmanaged note"));
    assert!(source.contains("New context"));
    let unchanged = store
        .update_decision("1", DecisionUpdateInput::default())
        .unwrap();
    assert_eq!(std::fs::read_to_string(&unchanged.path).unwrap(), source);

    const SCRIPT: &str = r#"
import { Core } from "./src/core/backlog.ts";
const root = process.env.BACKLOG_DECISION_ROOT;
const core = new Core(root);
const before = await core.filesystem.listDecisions();
const created = await core.createDecisionFromInput({title:"New choice", context:"Context text", decision:"Chosen path", consequences:"Consequences", alternatives:"Other path"});
const updated = await core.updateDecisionFromInput("1", {title:"Revised choice", status:"superseded", context:"New context", alternatives:"", supersededBy:created.id});
await core.updateDecisionFromInput("1", {});
process.stdout.write(JSON.stringify({before:before.map(d=>d.id), created:created.id, updated:{id:updated.id,title:updated.title,status:updated.status,context:updated.context,decision:updated.decision,supersededBy:updated.supersededBy,updatedDate:updated.updatedDate}, source:await Bun.file(root+"/backlog/decisions/"+updated.path).text()}));
"#;
    let expected = reference(SCRIPT, &reference_fixture);
    assert_eq!(expected["created"], "decision-2");
    assert_eq!(expected["updated"]["title"], "Revised choice");
    assert_eq!(expected["updated"]["context"], "New context");
    assert_eq!(expected["updated"]["supersededBy"], "decision-2");
    assert!(expected["updated"]["updatedDate"].is_string());
    assert!(
        expected["source"]
            .as_str()
            .unwrap()
            .contains("Unmanaged note")
    );
    let source = std::fs::read_to_string(&updated.path).unwrap();
    let mut rust_source = backlog_md::frontmatter::parse_frontmatter(&source).unwrap();
    let mut typescript_source =
        backlog_md::frontmatter::parse_frontmatter(expected["source"].as_str().unwrap()).unwrap();
    rust_source.data.remove("updated_date");
    typescript_source.data.remove("updated_date");
    assert_eq!(rust_source.data, typescript_source.data);
    assert_eq!(rust_source.content, typescript_source.content);
}
