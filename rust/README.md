# Rust migration components

This Cargo package contains the staged [Rust executable migration](../docs/superpowers/specs/2026-11-07-rust-migration-design.md).
The `backlog` binary currently provides root help/version plus noninteractive
`task create`, `task edit`, `task list`, `task view`, `task archive`,
`task restore`, and `task delete` commands. List and view support plain or JSON
output; task creation supports confirmation output or full plain detail; task
edit supports core fields, dependency links, references, documentation, explicit
clears, incremental list changes, and optional full plain detail.
The shipped launcher,
interactive TUI, browser server and MCP adapter still use TypeScript; installation
cutover has not happened.

Run the Rust task list from this repository with:

```sh
cargo run --manifest-path rust/Cargo.toml -- task list --plain
cargo run --manifest-path rust/Cargo.toml -- task view TASK-1 --json
cargo run --manifest-path rust/Cargo.toml -- task create "Add tests" --parent TASK-1
```

Use Rust 1.98 or later:

```sh
cargo test --manifest-path rust/Cargo.toml
cargo fmt --manifest-path rust/Cargo.toml --check
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
```

The [frontmatter module](src/frontmatter.rs) owns parsing Markdown headers and
returning source with explicit metadata changes. It accepts the existing
unquoted assignee/reporter conventions and keeps due-date text. Its semantic
parser input canonicalizes unquoted integer spellings such as `007` to match
the current product; quoted/tagged strings and the original source stay intact.
Opaque timestamp scalars are represented as strings rather than JavaScript
Date objects; this internal representation is not a public wire format.

Updates edit a fresh YAML concrete syntax tree, check untouched root-entry
source, and reparse the result against exactly the intended metadata. These
checks refuse changes that would rewrite unknown aliases or let a YAML merge
undo a requested clear. Delimiters, BOM, body text and existing line endings
remain outside semantic serialization. No function writes files: the future
storage/application operations will own locks, identity checks and persistence.

The [tests](tests/frontmatter.rs) exercise parsing, source preservation,
references and malformed input. The [implementation plan](../docs/superpowers/plans/2026-11-07-rust-frontmatter.md)
records dependency choices and verification. This component alone does not
establish CLI, server, protocol, TUI, concurrency or platform parity.

The [sections module](src/sections.rs) owns task and decision section bodies,
Acceptance Criteria, Definition of Done and comments. It preserves the existing
family-specific formats: task edits use canonical ordering and spacing; decision
edits preserve outside source and wrap existing legacy bodies in markers.
Omitted section keys retain their values; `None` or an empty string clears a
requested body. An empty change map returns the original source.

Checklist reads use the sequential indices consumed by indexed application
operations. Replacements sort supplied items by index, write sequential numbers,
and retain surrounding prose and known foreign marker blocks. Call
`assert_valid_checklist_marks` before an indexed checklist change, and
`assert_section_input_safe` for new task section input. Raw decision editors call
`validate_decision_editor_structure` before field extraction. These are the
existing validation boundaries; the future application layer owns their ordering.

The pinned `markdown` parser supplies CommonMark heading boundaries for legacy
task ambiguity checks and decision sections. Markers, checklists, comments and
blank-line normalization retain their reference-specific source rules. The
component performs no filesystem operations.

The [section tests](tests/sections.rs) include direct native regressions and
actual TypeScript comparisons of parsed data, exact returned source and refusals.
Those comparisons require the repository's Bun dependencies and Bun on `PATH`;
set `BACKLOG_REFERENCE_BUN` to an explicit Bun executable when needed:

```sh
BACKLOG_REFERENCE_BUN=/path/to/bun cargo test --manifest-path rust/Cargo.toml
```

Bun is only a test reference; the native library does not delegate operations.

The [config module](src/config.rs) owns sparse stored configuration parsing,
known-field serialization, and separate fresh-init and schema-migration defaults.
It shares raw YAML semantics with frontmatter while keeping record compatibility
preprocessing out of configuration. Config parsing and serialization operate on
strings; persistence and physical record migrations remain later work.

The [discovery module](src/discovery.rs) selects runtime cwd, root or folder
configuration, and the nearest marked project. The Rust CLI uses it for local
task listing. Selection is read-only. Root
metadata recognition is independent of full config parsing; custom directories
use the existing lexical path rule, including descendant symlinks. Ancestor
markers work without Git; the installed Git executable is only an optional
fallback. Discovery adds no caches or watchers.

The [discovery tests](tests/discovery.rs) cover precedence, invalid overrides,
config markers, root-only paths and symlinks. Host separator rules are tested
separately: Unix replaces backslashes after normalization, while Windows treats
them as separators during normalization. Windows-specific fixtures run on native
Windows CI; a Unix run does not establish Windows parity. Windows symlink tests
skip only when fixture creation lacks permission.

Run focused configuration and discovery checks with:

```sh
cargo test --manifest-path rust/Cargo.toml --test config
cargo test --manifest-path rust/Cargo.toml --test discovery
cargo test --manifest-path rust/Cargo.toml --lib
```
