---
id: BACK-802
title: Port the Backlog.md executable to Rust
status: In Progress
assignee:
  - '@codex'
created_date: '2026-11-07T21:53:00Z'
updated_date: '2026-11-11T22:49:00Z'
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
<!-- SECTION:NOTES:END -->
