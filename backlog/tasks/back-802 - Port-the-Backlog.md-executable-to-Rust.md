---
id: BACK-802
title: Port the Backlog.md executable to Rust
status: In Progress
assignee:
  - '@codex'
created_date: '2026-11-07T21:53:00Z'
updated_date: '2026-11-12T15:35:00Z'
labels: []
dependencies:
  - BACK-801
references:
  - docs/superpowers/specs/2026-11-07-rust-migration-design.md
  - docs/superpowers/specs/rust-contract-inventory.md
  - rust/README.md
  - BACK-802.3
  - BACK-802.4
  - BACK-802.5
  - BACK-802.6
  - BACK-802.7
  - BACK-802.8
  - BACK-802.9
modified_files:
  - >-
    backlog/tasks/back-802.13 -
    Port-identity-safe-task-creation-and-updates-to-Rust.md
  - >-
    backlog/tasks/back-802.14 -
    Port-branch-snapshot-task-ID-reservation-to-Rust.md
  - >-
    backlog/tasks/back-802.15 -
    Port-remote-branch-task-ID-reservation-to-Rust.md
  - >-
    backlog/tasks/back-802.16 -
    Port-task-archive-and-restore-operations-to-Rust.md
  - >-
    backlog/tasks/back-802.17 -
    Port-archived-task-deletion-and-link-cleanup-to-Rust.md
  - rust/src/git.rs
  - rust/src/storage.rs
  - rust/src/task_store.rs
  - rust/tests/task_store.rs
  - rust/tests/task_store_mutations.rs
  - rust/tests/storage.rs
  - rust/tests/git.rs
type: feature
ordinal: 408000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The maintainer requested and approved a complete executable migration to Rust while retaining the React browser and existing npm launcher. BACK-799 established the approved design; BACK-800 established the compatibility inventory and comparison harness; BACK-801 implemented the first native frontmatter component. Those foundations do not complete the port. This task preserves the outstanding initiative and completion gates for continued implementation.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Rust shared records, configuration, storage, locks, allocation, identity and Git operations preserve supported Markdown behavior, including old/new writer coordination during migration.
- [ ] #2 The complete documented CLI, output schemas, workflow instructions, editor and watch behavior run through the Rust shared model without Bun delegation.
- [ ] #3 The retained React browser works with the Rust HTTP/WebSocket server with existing behavior and reviewed desktop/narrow-screen interaction.
- [ ] #4 The shipped MCP stdio tools, schemas, resources, root discovery and lifecycle work through the Rust shared model.
- [ ] #5 The Rust TUI preserves supported human workflows and passes required interactive and visual checks.
- [ ] #6 All six native distribution targets, npm launcher, Nix packaging, version derivation and release flow are verified before cutover.
- [ ] #7 Required compatibility, platform and review gates pass before the old backend is retired; no compiling scaffold or partial CLI is reported as a complete port.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [ ] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Continue the eight stages in docs/superpowers/specs/2026-11-07-rust-migration-design.md. Completed foundations: design, contract inventory/harness, initial source-preserving Rust frontmatter. Next: remaining record parsing and structured sections, configuration and durable storage with compatible locks, then domain/Git operations and the complete CLI before adapters and distribution. Research and record each bounded implementation plan when picked up. Do not cut over or mark this initiative Done until every accepted surface and platform gate is verified.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Completed stages: BACK-802.1 managed sections, BACK-802.2 Rust CI, BACK-802.3 config parsing/defaults, BACK-802.4 native discovery, BACK-802.5 typed record parsing, BACK-802.6 record serialization, BACK-802.7 storage and compatible locks, BACK-802.8 task ID normalization/allocation, BACK-802.9 shared entity identity and ambiguity resolution, BACK-802.10 Git repository/worktree discovery, BACK-802.11 filename ID reservation across worktrees, BACK-802.12 local task listing/lookup, BACK-802.13 local task create/update, BACK-802.14 local branch snapshot reservation, BACK-802.15 remote branch snapshot reservation, BACK-802.16 task archive/restore. Windows execution remains unchecked; GitHub reports zero Actions runs.\n\nOverall executable migration remains In Progress. Other Rust domain operations, CLI behavior, browser server, MCP, TUI, and distribution/cutover gates remain outstanding.\n\nBACK-802.10 added Git repository roots, branch names, and linked worktree discovery. BACK-802.11 added active/archive task filename ID reservations across the current project and linked worktrees. BACK-802.12 added local task listing and identity-safe lookup with TypeScript parity. BACK-802.13 added locked local task creation and updates with source-preserving writes. BACK-802.14 added allocation from configured recent local branch snapshots. BACK-802.15 added bounded origin refresh and task/archive filename reservation across recent remote branches. BACK-802.16 added locked, source-preserving TaskStore archive and restore operations with no-overwrite moves, state errors, and relative symlink handling. Rust and focused Bun compatibility checks passed; cold simplicity and full-context complexity reviews found no remaining blockers.

BACK-802.17 added archived-only Rust task deletion. It refuses active/ambiguous tasks and tasks with direct children across active tasks, archived tasks, or drafts; cleans dependency and exact task-reference links under entity and shared create locks before removing the archive file. Live TypeScript comparisons cover cleanup, preservation, refusal cases, and ID reuse. Rust full checks and scoped Bun deletion/reference tests passed; reviews found no remaining blockers. Groma was unavailable in the workspace.

BACK-802.18 added local draft listing, lookup, create and update in Rust with live TypeScript comparisons. BACK-802.19 added draft archive, promotion and demotion with identity-safe locks and lifecycle comparisons. BACK-802.20 added local document listing, lookup, padded creation and source-preserving updates, including identity ambiguity, archive read-only, and TypeScript traversal/order parity. All 20 currently tracked subtasks are complete; the overall executable migration remains In Progress. Remaining initiative gates include the complete Rust CLI, browser HTTP/WebSocket server, MCP adapter, TUI, and native/npm/Nix distribution and cutover validation.

BACK-802.21 completed: Rust document archive, restore, and delete now preserve source bytes and relative paths, enforce locks and identity checks, refuse destination collisions, unlink symlinks safely, report documentation backlinks, and preserve ID reservation rules. Full Rust fmt/Clippy/tests and 96 focused Bun tests passed; review found no blockers. Groma was unavailable. The overall migration remains In Progress; CLI, browser HTTP/WebSocket server, MCP, TUI, distribution and final cutover gates remain.

BACK-802.22 completed: added Rust DecisionStore listing, active/archive identity lookup, DEC allocation/creation, and locked partial updates with source-preserving managed-section serialization, validation, supersession, and TypeScript parity. Full Rust checks/tests and 46 focused Bun decision tests passed; independent reviews found no blockers after fixes. Overall migration remains In Progress; remaining application domains, canonical CLI, browser HTTP/WebSocket server, MCP, TUI, and distribution/cutover gates are outstanding.

BACK-802.23 completed: Rust DecisionStore now archives, restores, and deletes decisions with shared locking, identity rechecks, no-overwrite moves, byte preservation, symlink-safe unlinking, task/draft backlinks, and branch-aware ID reservation. Full Rust fmt/Clippy and 150 tests passed; focused Bun lifecycle comparisons passed 122 tests. The full migration remains In Progress; next are remaining domain stores and then CLI/server/adapters/distribution gates. Groma was unavailable.

BACK-802.24 completed: added Rust milestone active/archive listing, TypeScript-priority identifier resolution, and locked creation with ID allocation across archived records, source-compatible titles/descriptions, due dates, and file serialization. Full Rust fmt/Clippy and 152 tests passed; 13 focused Bun tests passed; reviews found no remaining blockers. Remaining overall migration surfaces are milestone lifecycle/update, remaining CLI/domain behaviors, HTTP/WebSocket browser server, MCP, TUI, packaging, and cutover.

BACK-802.25 completed: Rust milestone rename/archive now preserve default description and due-date semantics, reject rename and archive destination collisions without replacing either source, and return refusal results on storage failures. Full Rust fmt/Clippy and 154 tests passed; focused Bun suites passed 13 tests; independent reviews found no remaining blockers. Remaining Rust milestone deletion, remaining CLI/domain behavior, HTTP/WebSocket server, MCP, TUI, packaging, and cutover.

BACK-802.26 completed: added Rust milestone removal coordination for keep/clear/reassign, target validation, task assignment updates, milestone archival, and task rollback on archive refusal. Full Rust fmt/Clippy and 158 tests passed; 13 focused Bun tests passed; independent reviews cleared their original findings. Remaining Rust milestone create/rename/archive/removal orchestration into CLI, remaining domain/CLI parity, HTTP/WebSocket browser server, MCP, TUI, packaging and final cutover.

BACK-802.27 completed: TaskStore creation and dependency edits now validate active, archived, and draft dependencies in Rust, canonicalize and deduplicate aliases, reject invalid/self/cyclic references before persistence, and preserve source on refused edits. Live TypeScript parity tests cover successful create/edit Markdown dependencies and refusal messages. Full Rust fmt/Clippy and 163 tests passed; 24 focused Bun dependency tests passed; required simplicity and complexity reviews found no blockers. Remaining migration work: task dependency operations outside these TaskStore paths, canonical Rust CLI/application commands, HTTP/WebSocket browser server, MCP, TUI, native/npm/Nix distribution, and final compatibility/cutover gates. Groma is unavailable in the workspace.

BACK-802.28 completed: added Rust status-driven promotion and demotion operations that validate dependencies against newly allocated identities before writes or link cleanup, preserve unresolved legacy references, and retain existing direct lifecycle semantics. Full Rust fmt/Clippy and 166 tests passed; 24 focused Bun dependency tests passed; simplicity and complexity reviews found no blockers. Remaining initiative work includes the rest of Rust application/query behavior, canonical CLI, HTTP/WebSocket browser server, MCP, TUI, native/npm/Nix distribution, and final compatibility/cutover gates. Groma remains unavailable.

BACK-802.29 completed: added the Rust dependency graph read model with live TypeScript parity for canonical aliases, active/draft/archive records, missing and ambiguous IDs, cycles, diamonds, archived completion, separate dependency/dependent depths, and deterministic ordering. The model accepts externally carried ambiguous IDs and is read-only. Full Rust fmt/Clippy and 168 tests passed; 37 focused Bun graph/CLI tests passed; required reviews found no blockers. The overall migration remains active with application queries, canonical CLI, browser HTTP/WebSocket server, MCP, TUI, distribution, and cutover gates still outstanding. Groma is unavailable.

BACK-802.30 completed: added Rust readiness queries over the active+archived task corpus with draft exclusion, default/custom terminal statuses, blockers, missing/ambiguous IDs, and not-actionable archived/terminal targets. Dependency graph and readiness now share TaskRecordIndex for canonical identity, ambiguity, and completion rules. Full Rust fmt/Clippy and 170 tests passed; 24 focused Bun readiness/surface tests passed; required reviews found no blockers. Remaining application/query, canonical CLI, browser HTTP/WebSocket server, MCP, TUI, distribution, and compatibility/cutover work remains. Groma is unavailable.

BACK-802.31 Port task detail assembly to Rust is complete. Added TaskDetail and build_task_detail using the same active/archived corpus, statuses, ambiguous IDs, and prefix for both derived queries. Verified TypeScript parity across resolved, blocked, missing, ambiguous, archived, and terminal cases; serialization excludes derived fields. Rust suite: 172 passed; fmt and Clippy clean; focused TypeScript suite: 24 passed. Cold simplicity and complexity reviews had no blocking findings. Groma is unavailable (command not found).

BACK-802.32 Port task list readiness projection to Rust is complete. Added TaskListItem and with_readiness with one ReadinessGraph over the full active+archived corpus, independent of display filters. TypeScript parity verifies blocked hidden dependencies, ready, missing, ambiguous, archived, and terminal rows; serialization keeps readiness out of Markdown. Rust suite: 173 passed; fmt and Clippy clean; focused TypeScript suite: 24 passed. Reviews found no blockers. Groma remains unavailable.

BACK-802.33 Port task list ordering to Rust is complete. Added canonical default/id/ordinal/priority sorts and status grouping, including configured priority normalization/fallback and stable numeric-ID tie behavior. Legacy NaN ordinal comparisons use a stable insertion path. Live TypeScript parity covers sort modes, grouping, custom priorities, leading-zero equivalent IDs, and mixed NaN/finite ordinals. Rust suite: 174 passed; fmt and Clippy clean; relevant TypeScript suites: 38 passed. Reviews found no blockers. Groma remains unavailable.

BACK-802.34 Port task list field filters to Rust is complete. Added exact status/exclude, type, project, priority, assignee/unassigned, any/all label, and canonical parent predicates. Filters compose with AND and preserve input order. Fuzzy text, modified files, and milestone resolution remain separate. Rust suite: 175 passed; fmt and Clippy clean; focused TypeScript filters: 32 passed. Reviews found no blockers. Groma remains unavailable.

BACK-802.35 Port task modified-file filters to Rust is complete. TaskFilter now matches trimmed/lowercased path substrings with OR semantics, ignores empty filters and stored paths, and rejects tasks without matching files for effective filters. Live TypeScript parity covers these cases. Rust suite: 175 passed; fmt and Clippy clean; focused TypeScript suite: 5 passed. Reviews found no blockers. Groma remains unavailable.

BACK-802.36 completed: exact Rust milestone filtering now resolves stored IDs, numeric aliases, and titles across the combined active/archive corpus; collision, duplicate-title, blank-query, and no-milestone semantics are covered by live TypeScript parity. Full Rust suite, clippy, TypeScript milestone filter suites (27 tests), formatting, and diff checks passed. Cold simplicity and complexity reviews cleared the implementation. Groma agent-instructions remains unavailable in this environment.

BACK-802.37 completed: Rust now applies Fuse-compatible closest-title milestone matching with stable full-corpus candidate resolution, preserving exact ID/title and no-milestone precedence. Live TypeScript parity covers fuzzy partial and typo queries, normalization, unmatched queries, and the hidden-closest-candidate case. Full Rust suite, clippy, rustfmt, and focused TypeScript milestone tests (27) passed. Cold and complexity reviews cleared the implementation. Groma agent-instructions is unavailable in this environment.

BACK-802.38 completed: added the Rust searchable body and indexed field builder with live TypeScript parity for field order, optional fields, criteria/comments, ID and dependency variants, and large numeric IDs. Full Rust suite, Clippy, rustfmt, and task-search parity (17 tests) passed; reviews found no blockers. Groma remains unavailable.

BACK-802.39 completed: implemented Fuse-compatible Rust task search with stable relevance ordering and shared full-corpus filter semantics. Full Rust suite, Clippy, rustfmt, focused TypeScript parity (49 tests), Biome check, and diff check passed; cold simplicity and complexity reviews found no blockers. Groma remains unavailable (command not found). Remaining migration work includes other app/query behavior, canonical CLI, HTTP/WebSocket browser server, MCP, TUI, distribution, and final compatibility/cutover gates.

BACK-802.40 completed: added the Rust task statistics projection and UTC date parser with live TypeScript parity for ordering, archives, duplicate IDs, malformed/offset dates, empty inputs, and result limits. Full Rust formatting, Clippy, and Cargo tests passed; the TypeScript statistics suite (12), Biome, and diff checks passed; reviews found no blockers. The statistics model is not yet wired into the CLI overview, browser endpoint, or TUI. Groma remains unavailable.

BACK-802.41 added the Rust CLI entrypoint and task list command. BACK-802.42 added task detail view with archived lookup and dependency/readiness output. BACK-802.43 added noninteractive task creation with live TypeScript parity and locked default ordinal assignment. Remaining migration surfaces include task edit/lifecycle commands, drafts/documents/decisions/milestones command adapters, browser server, MCP, TUI, distribution, and cutover gates.

BACK-802.44 completed: added Rust task edit for core fields, labels, aliases, clears, locked persistence, canonical identity errors, and default/plain output. Live TypeScript parity, source preservation, identity refusals, no-write failures, and explicit clears are covered. Full Rust tests, focused Bun edit/date/milestone tests (24), rustfmt, Clippy, build, Biome, and diff checks passed; cold simplicity and complexity reviews cleared. Groma remains unavailable (command not found).

BACK-802.45 completed: added Rust task archive, restore, and delete commands with TypeScript-compatible help/output, locked TaskStore mutations, byte-preserving moves, subtask refusals, and task/draft link cleanup. Full Rust suite, focused TypeScript lifecycle tests (47), rustfmt, Clippy, build, Biome, and diff checks passed; required reviews cleared. Groma remains unavailable (command not found).

BACK-802.46 completed: extended Rust task edit with dependency aliases, reference and documentation replacement/add/remove/clear options, shared validation, locked writes, and source preservation. Full Rust tests, focused TypeScript suites (75 tests), rustfmt, Clippy, build, Biome, and diff checks passed; both reviews found no blockers. Groma remains unavailable (command not found).

Completed BACK-802.47: Rust task edit now supports Acceptance Criteria replacement, add, clear, indexed check/uncheck/remove, and Definition of Done add/check/uncheck/remove. Verified Rust-vs-TypeScript process parity, checked-state retention, malformed/no-op refusal, and source preservation. Full Rust suite, 49 focused Bun tests, Biome, rustfmt, Clippy, and build passed. Cold and full-context complexity reviews cleared the change. Groma agent instructions were unavailable in this environment.

Completed BACK-802.48: added Rust `draft list` with local read-only DraftStore access, plain and JSON outputs, sorting, count, and pagination. Verified zero-padded draft IDs and huge window values against TypeScript. Full Rust suite, 35 scoped Bun tests, Biome, rustfmt, Clippy, and build passed; cold simplicity and full-context complexity reviews cleared. Groma remains unavailable.

BACK-802.49 completed: Rust draft create now matches the supported TypeScript workflow with auto_commit unset/false. Cargo suite, 9 focused Bun tests, Biome, rustfmt, Clippy, and build passed; cold and complexity reviews cleared. BACK-802.50 tracks remaining selected-path Git commits for configured auto_commit.

BACK-802.52 completed: exposed Rust draft archive/promote and task demote CLI commands, including exact changed-path reporting for selected-path auto-commit. Full serial Rust suite and 60 scoped Bun tests passed; checks and reviews cleared. Remaining migration stages are unimplemented canonical CLI groups (documents, decisions, milestones, config/doctor/search/overview/completion/instructions), HTTP/WebSocket server and retained browser integration, MCP adapter, TUI, and six-target/npm/Nix distribution and cutover verification.

BACK-802.53 completed: added `task <id>` and the `tasks` alias, with TypeScript-compatible plain/JSON/default output, task help, and hidden `task complete` refusal behavior. Full serial Rust suite passed (65 CLI tests plus all unit/integration/doc tests), 54 scoped Bun tests passed, and formatting, Clippy, build, Biome and diff checks passed. BACK-802.54 is queued to add Rust `doc list --plain` with shared list-window behavior.

BACK-802.54 completed and pushed as 8b02ebe9: Rust `doc list --plain` now matches TypeScript ordering, empty output, paging, count and validation. Full Cargo suite, scoped Bun list-window tests (17), Biome, rustfmt and Clippy passed; reviews found no blockers. BACK-802.55 completed and pushed as eb055fe6: Rust `doc view` handles active/archived IDs, stored Markdown, archived notice, missing and ambiguity errors. Full Cargo suite, focused Bun document view/archive tests (41), Biome, rustfmt and Clippy passed; reviews found no blockers. BACK-802.56 document creation is active. BACK-802.57-.58 cover document update/lifecycle; BACK-802.59-.61 cover decision list/view, create/edit, and lifecycle, sequenced next.

BACK-802.56 completed: Rust `doc create` now matches title/type/path validation, default values, output and Markdown; explicit empty `--type` defaults to other. Full Cargo, rustfmt, Clippy, Biome, scoped Bun doc CLI tests (20), and isolated process parity passed; cold and full-context review findings were resolved. BACK-802.57 document update is active. The remaining canonical CLI tasks are tracked through BACK-802.71 (documents, decisions, milestones, search, config/agents, init, doctor/cleanup, workflow instructions/completion, batch edit/watch, overview/board export). BACK-802.72–.75 cover Rust HTTP/WebSocket with retained React, MCP, TUI, and platform distribution/cutover.
<!-- SECTION:NOTES:END -->
