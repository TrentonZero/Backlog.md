# Rust migration design

Status: approved by the maintainer on 2026-11-07: “yes do it”.
Planning task: BACK-799.

## Intent and accepted direction

Replace the Backlog.md executable with Rust while preserving the product described
in [the manifesto](../../../MANIFESTO.md). Developers and agents continue to use
the same Markdown records through the canonical CLI, terminal UI, browser, and
optional MCP adapter. Existing projects must work without data conversion.

The maintainer requested planning and implementation of the entire port, then
accepted this one's recommendation. The proposed interpretation is that React
remains the browser implementation; the executable, shared model, persistence,
CLI, TUI, HTTP server, and MCP adapter become Rust. Browser TypeScript, its
asset build, and the existing npm JavaScript launcher are deliberate exceptions
to the language migration.

The maintainer approved this document as the architectural review point and
authorized implementation of the staged migration. Implementation plans may
settle routine choices within these boundaries without another approval round.

## Current system and migration boundaries

The inspected package declares Bun 1.4.4 tooling and version 1.55.3. The current
source includes 337 files under `src/test`; their existence is evidence of
coverage areas, not evidence that the tests currently pass.

| Existing owner | Responsibility to preserve | Proposed destination |
| --- | --- | --- |
| `src/types`, `src/markdown` | Record shapes, YAML, marked sections and checklists | Rust model and Markdown modules |
| `src/file-system`, `src/core` | Discovery, allocation, identity, lifecycle, validation, query, repair | Shared Rust application and storage modules |
| `src/git` | Git commands, branch and worktree records, optional commits | Rust Git module using the Git executable |
| `src/cli.ts`, CLI help and workflow instructions | Commands, flags, machine output, human guidance | Rust CLI adapter and embedded instruction resources |
| `src/ui` | Board, list, editing, terminal input and editor handoff | Rust TUI adapter |
| `src/server/index.ts` | HTTP endpoints, assets and WebSocket updates | Rust HTTP adapter |
| `src/mcp` | Stdio protocol, tools, schemas, resources, root discovery | Rust MCP adapter |
| `src/web` | React browser interface | Retained React application |
| `scripts`, workflows, npm launcher, Nix files | Build and distribution | Cargo executable build plus browser asset build and platform packaging |

These are responsibility boundaries, not a requirement to reproduce every
TypeScript class or make a separate Rust crate for each directory.

## Approaches

1. **Replace every surface at once, including a Rust/WebAssembly browser.**
   Removes browser TypeScript but couples a new UI implementation to storage,
   command, protocol and packaging changes. It creates the largest review and
   regression burden.
2. **Build the Rust executable alongside the existing executable, retaining
   React, then cut over after parity. Recommended.** The current implementation
   remains an executable compatibility reference during development. Each Rust
   capability is independently testable before replacing the shipped binary.
   The temporary cost is maintaining two implementations during the port.
3. **Ship Rust commands that delegate missing behavior to Bun.** Offers earlier
   partial releases but retains a second runtime and mutation implementation.
   It does not satisfy the intended Rust executable replacement.

The proposed selection is approach 2. The Rust executable never dispatches
unimplemented operations to the TypeScript executable. It remains unreleased
until the full supported command and interface inventory is covered.

## Proposed implementation structure

Start with one Cargo package, one library and one `backlog` binary, under
`rust/`. Keep the existing source tree intact during parity work. Group Rust
modules by the responsibilities in the table. Adapters call the same application
operations; they do not edit record files independently.

Use ordinary Rust structs and explicit request/result types. Keep source text
alongside parsed records when mutation must preserve unknown YAML or unmanaged
Markdown. The application layer owns identity resolution, validation, mutation
ordering and publication of results. Storage owns filesystem operations and
locks. The Git module invokes the installed Git executable to preserve user
configuration and the existing optional-Git contract.

Proposed library families, subject to verification against current official
documentation before adoption: clap for argument parsing; serde and serde_json
for wire values; Tokio and axum for HTTP/WebSockets; ratatui and crossterm for
the TUI; notify for filesystem events. Select a YAML parser only after proving
source-preserving edits, anchors and aliases against the existing fixtures.
Select the MCP implementation against the currently shipped protocol and tool
inventory. Do not upgrade the MCP protocol as part of this migration.

Exact dependency versions and the minimum Rust version belong in the first
implementation plan after that investigation. No claim of library suitability
or supported versions has been verified yet.

Keep browser wire types in a browser contract module when the old backend types
are retired. Verify those shapes against Rust responses. Do not retain backend
TypeScript modules solely to satisfy browser type imports.

Build static React assets with the existing frontend toolchain, separate from
the backend build, and embed the resulting assets in the Rust executable.
The native executable needs neither Bun nor Node. The existing npm JavaScript
launcher remains and still needs Node to locate and launch the native executable;
direct native installation does not. This packaging choice preserves the current
npm entry point and signal/watch handling rather than adding a launcher rewrite.
Contributors still need the frontend toolchain when changing or rebuilding
browser assets. Preserve deep-link SPA
fallback, Markdown preview, Mermaid, styling and live updates.

## Compatibility requirements

- Preserve documented CLI names, aliases, options, defaults, input conventions,
  exit behavior, plain output semantics, JSON schemas and watch framing.
- Preserve configuration discovery, validation and migrations, including custom
  backlog paths, custom prefixes, configured statuses and optional Git.
- Read and mutate existing task, draft, document, decision and milestone files.
  Preserve unknown frontmatter, supported YAML references, unmanaged Markdown,
  marked-section rules, checklist state, dates and deterministic ordering.
- Preserve atomic creation, locked read/modify/write, no-overwrite moves, ID
  allocation across supported worktrees and branches, and ambiguous-ID refusal.
  Locks must coordinate with the existing executable during migration; a different
  lock implementation must not allow simultaneous writers to bypass each other.
- Preserve lifecycle and reference cleanup, archive read-only behavior,
  dependencies, parent/subtasks, search, filtering, ordering, statistics, board
  export, doctor previews and fingerprint-checked repair.
- Preserve Git configuration and trust, remote-operation controls, branch
  freshness policies and useful reporting when a file mutation succeeds but
  its commit or subsequent cleanup fails.
- Preserve HTTP payloads, response codes and WebSocket publication semantics
  consumed by the retained browser; preserve MCP tools, schemas and resources.
- Preserve human TUI workflows, focus, keyboard and mouse behavior, terminal
  restoration, external editor handoff, non-TTY behavior and shutdown.
- Preserve published Linux, macOS and Windows x64/arm64 distribution targets,
  npm installation, supported Nix packaging, version reporting and release flow.

No new feature, protocol revision, file format, database, hosted service,
telemetry, performance target or UI redesign is implied. Known unrelated
defects remain separate work; passing the baseline does not redefine a
documented contract that the baseline violates.

## Delivery sequence and verification gates

Each stage gets a bounded Backlog task, a researched implementation plan and a
feature branch before implementation. Each plan identifies exact functions,
fixture cases and commands. The sequence below is the migration roadmap, not
an assertion that those detailed plans have already been written.

1. **Contract baseline and test harness.** Inventory every CLI command and mode,
   HTTP route/event, MCP tool/resource, configuration key, interactive workflow
   and distribution path from source and public documentation. Record coverage
   links and discrepancies. Build process-level comparisons using isolated
   temporary projects and two explicit executable paths. Compare stdout,
   stderr, exit status, JSON structure and resulting files. Normalize only
   scenario-owned temporary paths and uncontrolled timestamps. Establish the
   current test/build baseline before treating it as an oracle.
2. **Rust records and storage.** Add the Cargo package with tests for parsing,
   serialization, source preservation, marked sections, dates, configuration,
   discovery, locking and no-overwrite atomic operations. Port the existing
   preservation and concurrency scenarios, including YAML aliases. Gate:
   shared fixtures pass and mutation failures leave the expected file state.
3. **Identity, Git and application operations.** Implement allocation, identity
   indexing, branch/worktree discovery, all record lifecycles, validation,
   dependencies, ordering, query, search, statistics and repair. Gate: existing
   supported scenarios agree on results and side effects, including concurrent
   old/new writers and partial-success reporting.
4. **Canonical CLI.** Implement the inventoried command tree, help/instructions,
   plain/JSON output, initialization, editors and JSON watch. Gate: every CLI
   inventory entry has a passing compatibility case; piped output, closed stdin,
   watch shutdown and no-Git projects work with the Rust binary.
5. **HTTP server and retained browser.** Separate the browser asset build;
   implement every current endpoint and WebSocket event over the Rust core.
   Gate: process-level HTTP and event contracts pass; existing browser DOM tests
   remain separate from parallel backend tests; rendered and interactive review
   confirms desktop behavior and narrow-screen preservation.
6. **MCP adapter.** Implement stdio lifecycle, roots, resources and the shipped
   tools over the same core. Gate: protocol-client tests verify initialization,
   schemas, success/error payloads and clean EOF/shutdown. Protocol stdout must
   contain no diagnostic logging.
7. **TUI.** Implement board/list/detail/composer flows, filters, multi-selection,
   archive/draft actions and editor handoff over the Rust core. Gate: PTY tests
   cover the current interactive scenarios, with visual and keyboard review and
   terminal cleanup on errors and interruption.
8. **Distribution and cutover.** Update npm platform packages, browser asset
   embedding, version derivation, CI, release workflow, Nix packaging and
   contributor documentation. Build all six targets and run native smoke/PTY
   checks where runners support them. Verify installation from outside this
   repository. Remove the replaced backend and its exclusive dependencies only
   after all compatibility gates pass. Retain React and its required types/tests.

For each Rust change run formatting, Clippy and the affected Cargo tests;
run the full Rust suite before cutover. TypeScript changes also require the
repository's type, Biome and affected Bun checks. Platform and interactive
checks remain separate. Record actual commands and results on the owning task.

## Review and completion

Obtain a cold builder review of this design before presenting it for approval.
Resolve material design questions with the maintainer. For substantial
implementation changes, obtain the repository-required cold simplicity review
after focused checks and a separate full-context complexity review.

The migration is complete only when the inventory has no unimplemented
supported entries, all required verification and reviews pass, installed
artifacts contain the Rust executable and browser assets, and no backend path
requires TypeScript/Bun at runtime. A compiling scaffold or partial CLI does
not qualify. Publishing a release or merging to main still follows the
repository's explicit approval rules.

## Design review record

A cold builder review identified an ambiguity between preserving the npm
launcher and promising Node-free installed execution. The design now explicitly
retains the npm launcher and limits the Node-free requirement to direct native
execution. This is the author's proposed packaging choice, not a separate
maintainer decision at review time. The maintainer subsequently approved the
complete design. No other material planning question was raised.
