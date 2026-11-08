# Native Configuration and Discovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Root reviews this brief before dispatch; this document does not authorize this research agent to implement it.

**Goal:** Add native parsing/serialization, explicit configuration defaults, and project-directory discovery that preserve the supported existing product rules without adding a CLI or performing record migrations.

**Architecture:** Keep configuration semantics in `rust/src/config.rs` and project/cwd selection in `rust/src/discovery.rs`. Reuse one crate-private raw YAML mapping reader in the existing frontmatter owner, keeping record preprocessing exclusively in its record wrapper. Configuration stays sparse; initialization and schema migration remain explicit pure operations, while storage, CLI formatting/validation, watchers and physical migrations remain later callers.

**Tech Stack:** Existing Cargo package, Rust 1.98, serde_json, serde_yaml_ng, yaml-edit, and std. No dependency additions. The existing Git executable is an optional discovery fallback.

**Spec:** `docs/superpowers/specs/2026-11-07-rust-migration-design.md:104-116`, `MANIFESTO.md`, and `/tmp/backlog-rust-migration/config-research.md`. The broad contract inventory is a research checklist, not authorization to implement internal/dead fields.

## Global Constraints

- Existing projects must work without data conversion; the native executable must not delegate to TypeScript (approved design:8-18,60-62).
- One internal Cargo package; keep TypeScript product intact during parity work (design:64-76).
- Git is optional; never make it a prerequisite for discovery of an ordinary Markdown project.
- No cache, watcher, live publication, data relocation, new settings, new file format, JSON-config reader, physical record migration, CLI adapter, or universal --cwd flag in this component.
- Config YAML must not inherit unquoted record assignee/reporter or due_date preprocessing.
- Sparse absent values, explicit empty lists and explicit false remain distinguishable. No eager application of migration defaults while parsing.
- No worktree or approval prompts. Root's current authorization permits verified commits/pushes directly to main and waives issue/PR creation. Root owns task bookkeeping, final review and commit subject.

## Review Focus

1. A raw scalar `default_assignee: 007` is a **number** and must be refused; `[1, 2]` is an accepted list and becomes `["1", "2"]` (`config-commands.test.ts:395-439`). Covered in Task 1/2.
2. Malformed unread YAML must not make a valid list silently change value; a real top-level key outranks a nested look-alike (`operations.ts:118-147,221-255`). Covered in Task 2.
3. A recognized root config pointing at a nonexistent custom directory stays canonical, while an unrelated placeholder root file does not hide a valid folder config (`backlog-directory.test.ts:19-81`). Covered in Task 4.
4. Clearing an option must not be undone by parser/migration/init defaults, and reading a saved file in the same object must not masquerade as disk persistence (`core/init.ts:135-207`). Covered in Task 2/3.
5. A custom lexical descendant symlink outside the project is accepted by documented contract; no realpath confinement (`ADVANCED-CONFIG.md:74`). Covered in Task 4.

## File map and exact interfaces

Modify:

- `rust/src/frontmatter.rs`: extract one raw YAML mapping reader; preserve existing record parser behavior.
- `rust/src/lib.rs`: expose `pub mod config; pub mod discovery;` beside existing modules.
- `rust/README.md`: briefly identify the two new component responsibilities and their remaining integration limits.

Create:

- `rust/src/config.rs`: types, strict selected-key YAML parsing, legacy scalar parsing, deterministic serialization, pure fresh-init/schema defaults.
- `rust/src/discovery.rs`: lexical path normalization, root/folder config recognition, runtime cwd and nearest project lookup.
- `rust/tests/config.rs`, `rust/tests/discovery.rs`: focused public component behavior. Use unit tests inside frontmatter for the crate-private helper. Temp fixtures use std::env::temp_dir, unique process/counter suffix, and Drop cleanup; no new tempfile dependency.

### Shared YAML boundary

In `frontmatter.rs`:

```rust
pub(crate) fn parse_yaml_mapping(yaml: &str)
    -> Result<serde_json::Map<String, serde_json::Value>, FrontmatterError>;
```

This function contains current `canonical_integers` -> serde_yaml_ng parse -> apply_merge -> serde_json conversion -> mapping/null-root validation (`frontmatter.rs:139-152`). It does **not** call `preprocess`. Keep existing private record wrapper:

```rust
fn parse_yaml(yaml: &str) -> Result<Map<String, Value>, FrontmatterError> {
    parse_yaml_mapping(&preprocess(yaml))
}
```

Both existing record parse and update paths continue calling the wrapper. `canonical_integers` stays private in this owner. Config calls the raw helper; it does not duplicate merge or bare-integer semantics or export the raw helper outside the crate.

### Configuration representation

Use `#[derive(Clone, Debug, PartialEq)]` (do not derive Eq for f64 fields):

```rust
pub struct Config {
    pub project_name: String,
    pub statuses: Vec<String>,
    pub labels: Vec<String>,
    pub date_format: String,
    pub default_assignee: Option<Vec<String>>,
    pub default_reporter: Option<String>,
    pub types: Option<Vec<String>>,
    pub priorities: Option<Vec<String>>,
    pub projects: Option<Vec<String>>,
    pub definition_of_done: Option<Vec<String>>,
    pub default_status: Option<String>,
    pub default_editor: Option<String>,
    pub max_column_width: Option<f64>,
    pub default_port: Option<f64>,
    pub zero_padded_ids: Option<f64>,
    pub active_branch_days: Option<f64>,
    pub auto_open_browser: Option<bool>,
    pub hide_empty_columns: Option<bool>,
    pub remote_operations: Option<bool>,
    pub auto_commit: Option<bool>,
    pub filesystem_only: Option<bool>,
    pub bypass_git_hooks: Option<bool>,
    pub check_active_branches: Option<bool>,
    pub on_status_change: Option<String>,
    pub task_prefix: Option<String>,
    pub backlog_directory: Option<String>,
}

pub enum ConfigLocation { Folder, Root }

pub struct ConfigValueError {
    pub path: std::path::PathBuf,
    pub key: &'static str,
    pub problem: String,
    pub remedy: String,
}

pub fn parse_config(source: &str, path: &std::path::Path)
    -> Result<Config, ConfigValueError>;
pub fn serialize_config(config: &Config, location: ConfigLocation,
                        selected_backlog_directory: &str) -> String;
pub fn initial_config(project_name: &str, task_prefix: Option<&str>,
                      filesystem_only: bool) -> Result<Config, String>;
pub fn needs_schema_migration(config: &Config) -> bool;
pub fn migrate_config(config: Option<Config>) -> Config;
```

Implement Display + std::error::Error for ConfigValueError with the existing human-facing prefix, key/path, problem and remedy (`operations.ts:153-195`). Values stored by the old scalar parser are JavaScript numbers, including parseInt prefixes and NaN for unreadable integer text; f64 is a deliberate internal representation to avoid silently introducing stricter startup validation or an i64 range restriction. Actual valid CLI integer constraints belong to the later CLI adapter. No generic numeric abstraction, serde schema, dynamic key registry, getters for every field, or numeric validation framework is needed. Private decimal parse-prefix/number-format helpers are sufficient; tests name concrete existing values below.

`Config::default()` may be implemented solely as the **sparse parse result**: empty project name, default statuses, labels [], yyyy-mm-dd, every optional field None. Do not make it initialization or migration default.

No `task_resolution_strategy`, `include_date_time_in_dates`, `mcp`, or persisted legacy milestones fields are added. Their TS interface/inventory appearances do not prove durable file support; evidence is documented in research. Do not invent aliases for those fields.

### Discovery representation

```rust
pub enum BacklogDirectorySource { Backlog, Hidden, Custom }

pub struct BacklogDirectoryResolution {
    pub project_root: PathBuf,
    pub backlog_directory: Option<String>,
    pub backlog_path: Option<PathBuf>,
    pub source: Option<BacklogDirectorySource>,
    pub config_path: Option<PathBuf>,
    pub config_location: Option<ConfigLocation>,
    pub root_config_path: PathBuf,
    pub root_config_exists: bool,
}

pub enum RuntimeCwdSource { Option, Environment, Process }
pub struct RuntimeCwd {
    pub path: PathBuf,
    pub source: RuntimeCwdSource,
}
pub struct RuntimeCwdError {
    pub path: PathBuf,
    pub source: RuntimeCwdSource,
}

pub fn normalize_backlog_directory(value: &str) -> Option<String>;
pub fn resolve_backlog_directory(project_root: &Path) -> BacklogDirectoryResolution;
pub fn resolve_runtime_cwd(option: Option<&str>, environment: Option<&str>,
                           process_cwd: &Path) -> Result<RuntimeCwd, RuntimeCwdError>;
pub fn find_project_root(start: &Path) -> Option<PathBuf>;
```

Use the `ConfigLocation` enum from config rather than another root/folder bool/enum. RuntimeCwdError Display names --cwd/BACKLOG_CWD exactly. No process-global env/cwd mutation is needed for unit tests. `find_project_root` owns optional installed-Git fallback; an internal helper accepting a Git-root callback permits a deterministic fallback unit test without changing the public signature or requiring a Git abstraction layer.

## Task 1: Split raw YAML mapping parsing from record compatibility preprocessing

**Files:** modify `rust/src/frontmatter.rs`; tests in its `#[cfg(test)]` module; retain existing `rust/tests/frontmatter.rs`.

**Consumes:** existing FrontmatterError, canonical_integers, preprocessing and merge behavior.
**Produces:** `pub(crate) parse_yaml_mapping` above; existing public record API unchanged.

- [x] Add a failing boundary test: raw `default_assignee: 007` yields a JSON Number 7; quoted/tagged string spellings remain strings. Raw `default_assignee: @alex` returns InvalidYaml, while current `parse_frontmatter` continues to accept its record assignee convention. Evidence: frontmatter.rs:73-152 and config strict-syntax tests:373.
- [x] Add a small raw mapping test with an external anchor/merge and a null root => empty map; scalar root => ExpectedMapping. This protects existing shared YAML behavior rather than adding another parser.
- [x] Run `cargo test --manifest-path rust/Cargo.toml --lib` and observe expected missing-helper failures before implementation.
- [x] Extract the helper exactly as above. No record-specific preprocessing on the raw path; no changes to canonical integer traversal or existing frontmatter update verification.
- [x] Run `cargo test --manifest-path rust/Cargo.toml --lib` and `cargo test --manifest-path rust/Cargo.toml --test frontmatter`. Both must pass; compare any new behavior with existing assertions before altering fixtures.

This small extraction can be reviewed and committed with Task 2 since its consumer explains why it exists.

## Task 2: Parse and serialize supported stored configuration

**Files:** create `rust/src/config.rs`, `rust/tests/config.rs`; modify `rust/src/lib.rs`.

**Consumes:** Task 1 raw YAML mapping helper.
**Produces:** Config/ConfigLocation/ConfigValueError, parse_config and serialize_config. Task 3 consumes the same Config; Task 4 consumes ConfigLocation only.

- [x] Add table-driven failing tests for sparse parse defaults and actual aliases. Assert default statuses/labels/date format; explicit statuses [] remains []; absent assignee is None, empty assignee key becomes Some([]); filesystem_only/filesystemOnly, callback spellings and backlog-directory spellings work. `projectName` does not become a full-parser alias. Sources: operations.ts:2315-2435.
- [x] Add selected-list tests: block and flow sequences agree; quoted commas/escapes survive; default_assignee permits a string scalar; numbers **inside** a sequence stringify (`[1,2]` => `["1","2"]`). Numeric scalar `42` and `007`, true, mapping, and ordinary scalar for non-assignee lists produce ConfigValueError naming key/path and expected/actual type. `007` remains numeric through raw helper. Sources: config-commands.test.ts:231-251,373-439.
- [x] Add independence/selection tests: malformed statuses refuses; malformed unread `custom-setting` does not invalidate valid statuses; column-zero statuses outranks nested look-alike; last top-level duplicate wins; legacy tab/space indentation accepted; aliases resolve against another key's anchor. Sources: config-commands.test.ts:442-548. Do not parse the entire document first.
- [x] Add DoD tests: null => Some([]); trim/drop blank and non-string entries; quoted commas; Windows paths with old unescaped double-quote backslashes; malformed/wrong-shape DoD returns None under the existing DoD compatibility path. Sources: definition-of-done.test.ts:34-182, operations.ts:2482-2581. Reuse raw helper after only the existing DoD backslash compatibility transformation; do not call record preprocessing.
- [x] Add legacy scalar tests: quote-removal behavior for project/editor, surrounding-only callback quote removal, booleans raw lowercased true only, integer parse-prefix (`max_column_width: 20px` => Some(20.0)), unreadable integer => Some(NaN), absent scalar => None. These preserve the inspected line-parser contract without adding new startup refusals (operations.ts:2338-2404). Use is_nan rather than whole-struct equality for NaN.
- [x] Add deterministic serialize/parse tests: fixed key order and trailing newline; root save writes the selected backlog_directory, folder save omits it even when present in Config; false booleans survive; assignee/DoD escaping and order survive; optional empty types/priorities/projects omitted, statuses/labels always emitted; explicit empty DoD emitted. Sources: operations.ts:2277-2291,2438-2479. Round-trip only fields the current writer promises to persist; do not add source-preserving config edits or “fix” writer quoting as a side effect.
- [x] Run `cargo test --manifest-path rust/Cargo.toml --test config` and observe expected missing-API failures.
- [x] Implement only the selected-key extractor, strict list reader, legacy scalar/DoD handling and serializer needed by these tests. Config's raw YAML helper errors are translated to ConfigValueError at the owning key; do not expose a frontmatter-specific error to a person repairing config.
- [x] Run `cargo test --manifest-path rust/Cargo.toml --test config` and `cargo test --manifest-path rust/Cargo.toml --test frontmatter`; require pass.

**Parser details implementers must not guess:** top-level key outranks indented match; choose last match at that precedence; stop block at next mapping key with same/lower indentation, excluding sequence items. Retry raw whole-document read only after selected block parse error so anchors resolve; report original block error if both fail (`operations.ts:118-147,221-255`). List member conversion uses JavaScript String semantics for the types existing lists can hold: null => "null", bool lowercase, numbers canonical decimal, nested sequence comma joining, mapping => "[object Object]"; trim/drop empty afterwards. Keep any helper private. DoD retains string members only.

## Task 3: Keep fresh-init and schema defaults explicit

**Files:** extend `rust/src/config.rs`, `rust/tests/config.rs`.

**Consumes:** Config and sparse Config::default from Task 2.
**Produces:** initial_config, needs_schema_migration, migrate_config. No filesystem writes or physical migrations.

- [x] Add failing assertions for initial_config("P", None, false): To Do/In Progress/Done, labels [], defaultStatus To Do, yyyy-mm-dd, width 20, port 6420, browser/remote/branch true, commit/hooks/filesystem false, active days 30, prefix task, editor/padding unset (`core/init.ts:142-165`, constants/index.ts:77-88).
- [x] Assert explicit nonempty prefix letters only, reserved draft/doc/decision refused ignoring case, default task if absent/empty; filesystemOnly true forces branch/remote/commit/hooks false (`utils/prefix-config.ts:26-53`, init.ts:119-134). No other task-type/domain validation added.
- [x] Assert migrate_config(None) defaults: Untitled Project, empty editor/status, YYYY-MM-DD, width 80 and schema Git/browser defaults. Assert Some(existing sparse parsed config) preserves its name/date/statuses/labels and explicit false/empty values while filling missing migration options. needs_schema_migration checks defaultPort/autoOpenBrowser/remoteOperations/autoCommit; name/statuses are already present in the typed parsed Config (`config-migration.ts:43-58`).
- [x] Run the focused config test before implementation, then implement these three small functions and rerun it. Do not use one merged default object for all operations.

Re-initialization/advanced-option merging, wizard defaults, CLI get/list placeholders and set validations are **later adapter/application contracts**, not extra functions in this component. Keep the research distinctions for that work: a cleared editor stays unset; CLI filesystemOnly false does not restore other flags; get/list can display absent differently. This stage supplies the sparse Config that makes those behaviors possible.

Physical milestone extraction/creation and draft-file prefix migration remain later storage/application tasks (`core/backlog.ts:1838-1961`, prefix-migration.ts:20-123). This parser omits legacy milestones from current Config and serializer as today; it does not remove a legacy key from a file by merely reading it. Do not expose unused migration hooks or require storage APIs now.

## Task 4: Resolve runtime directory, canonical config and nearest project

**Files:** create `rust/src/discovery.rs`, `rust/tests/discovery.rs`; modify `rust/src/lib.rs`; later update README.

**Consumes:** ConfigLocation; std filesystem/path/process. Discovery metadata does not call full configuration parsing.
**Produces:** exact discovery types/functions above.

- [x] Add cwd test table: option > env > process; trim blank option to permit env; relative override resolves against supplied process cwd; selected missing/file override fails with source/path; no fallback. Sources: runtime-cwd.ts:14-61, runtime-cwd.test.ts:38-80.
- [x] Add lexical path table: trim and normalize nested dot/dotdot; reject absolute Unix/Windows/UNC, empty, '.', '..', escaping relative paths; allow ordinary descendant. Preserve current platform path-normalize behavior; on Unix a backslash is not automatically a parent separator before Node normalization. Add one host-appropriate separator fixture and document Windows-specific cases for native CI rather than pretending Unix proves Windows. Sources: backlog-directory.ts:189-205.
- [x] Add directory precedence fixture: valid root custom path beats both folder configs even when custom target missing; root without explicit path chooses config-bearing built-in first; placeholder root file falls back; backlog/config.yml beats config.yaml; .backlog/config beats bare backlog; both marked folders prefer backlog. All result fields must agree. Sources: backlog-directory.test.ts:19-81; backlog-directory.ts:81-186.
- [x] Add root-only setting/symlink fixture: folder-local backlog_directory cannot relocate the selected folder; invalid root path falls back to built-in; relative descendant symlink outside is accepted where symlink fixture creation is supported. Sources: ADVANCED-CONFIG.md:74. No realpath containment.
- [x] Add nearest-project fixture: search child to nearest config marker; bare folder is not a root; .backlog/config.yaml works; backlog.json regular file is recognized; nested custom data folder without its own marker does not hide parent root. Sources: `src/utils/find-backlog-root.ts` and `src/test/find-backlog-root.test.ts:22-169`. backlog.json remains marker-only.
- [x] Add private unit test for Git fallback: callback is invoked only after no ancestor marker, its returned path must itself have a marker, callback failure/no Git returns None. A regular project found by ancestor scan never requires Git. Do not add a Git module or runtime dependency.
- [x] Run `cargo test --manifest-path rust/Cargo.toml --test discovery` and `cargo test --manifest-path rust/Cargo.toml --lib`; observe expected missing-API failures.
- [x] Implement lexical normalization, lightweight metadata recognition, folder/root precedence, cwd and ancestor scan. Metadata recognizes nonempty project_name or projectName and root backlog_directory/backlogDirectory; exact root filename backlog.config.yml; folder filenames config.yml then config.yaml. Use private helpers, no public path setters/relocation API. Git fallback uses `Command::new("git").args(["rev-parse", "--show-toplevel"]).current_dir(start)` and treats unsuccessful execution as no root.
- [x] Run focused discovery and config tests; require pass. Update README with supported component scope and verification commands, explicitly retaining later migration/CLI/watcher work.

## Verification, review and handoff

- [x] Run `cargo fmt --manifest-path rust/Cargo.toml --check`.
- [x] Run `cargo test --manifest-path rust/Cargo.toml` (small existing Rust suite; validates the shared YAML extraction against records and sections).
- [x] Run `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings`.
- [x] Read the diff as a new developer: config owns stored values/defaults; discovery owns selection; raw YAML helper owns parser semantics; no record preprocessing leaked into config, no duplicated YAML parser behavior, no cache/watchers/new settings.
- [x] Verify the exact existing source-backed tests above before declaring parity. No Bun/TypeScript changes planned, so routine full Bun/type/Biome runs are not required by this component; root may run focused old tests as baseline evidence.
- [x] Root obtains the required cold simplicity review then separate full-context complexity review for this substantial component; apply only supported in-scope blockers and rerun affected checks. Root records actual commands/results, not anticipated passes.
- [x] Root commits/pushes verified component work directly to main under current authorization, using the owning task's exact ID/title where task workflow still applies. No implementation worker should invent a task ID or create an issue/PR.

## Deliberately unresolved outside this component

- Dead/internal settings (taskResolutionStrategy, includeDateTimeInDates, mcp.http), help schema omission of hideEmptyColumns, malformed-assignee stale documentation and watcher block-list drift remain inventory discrepancies. No native setting or protocol feature is inferred from a TS struct.
- Incidental parser quirks with malformed scalar content, exact filesystem-root ancestor loop behavior and platform separator differences are not reasons to add new validation/support machinery. Preserve proven supported flow; root decides a material product change before implementation.
- File I/O, missing/read errors versus parse refusals, config cache, atomicity/locking, source-preserving record edits and legacy milestone/draft physical conversion remain later owners. This component exposes strings/paths and pure functions, and does not mutate projects.

## Plan self-review

The five review-focus inputs have owning test steps. All proposed function names/types match their consumers; ConfigLocation has one owner. Every persisted Config field is read by the actual existing parser, including callback aliases and filesystem-only mode. No tests are justified solely by unused TS fields. This is a reviewed implementation recommendation, not evidence that any native work or validation command has run.
