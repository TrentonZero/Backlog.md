---
id: BACK-802.7
title: Port durable record storage and compatible locks to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-08T20:29:00Z'
updated_date: '2026-11-08T20:47:00Z'
labels: []
dependencies:
  - BACK-802.6
references:
  - file-storage
  - src/file-system/operations.ts
  - src/test/atomic-task-create.test.ts
  - src/test/atomic-task-edit.test.ts
  - src/test/worktree-task-id-allocation.test.ts
  - rust/src/serialization.rs
  - docs/superpowers/specs/2026-11-07-rust-migration-design.md
modified_files:
  - rust/tests/storage.rs
  - rust/Cargo.toml
  - rust/Cargo.lock
  - rust/src/storage.rs
  - rust/src/lib.rs
parent_task_id: BACK-802
type: task
ordinal: 415000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add the Rust file-storage boundary needed by the staged executable migration: source-preserving record reads and writes, safe file creation and moves, and locks that coordinate with the currently shipped executable. Keep application lifecycle and CLI behavior for later stages.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust storage reads and writes Markdown records through caller-supplied paths and existing Rust serializers, preserving source data.
- [x] #2 Atomic creation and no-overwrite moves match the current file-storage contract, and failed operations leave the expected file state.
- [x] #3 Rust creation and entity locks coordinate with the shipped proper-lockfile paths and distinguish waiting creation locks from fail-fast record-edit locks.
- [x] #4 Temporary-directory and cross-runtime tests cover success, contention, stale/released locks, collisions, and partial-failure outcomes; Cargo checks pass.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Inspect the existing file-storage lock paths, retry/stale behavior, and atomic create/move contracts. 2. Add isolated failing tests for Rust storage operations and TypeScript lock interoperability. 3. Implement a small Rust storage module that composes the record serializers with source-preserving reads, atomic creation, no-overwrite moves, and compatible create/entity locks. 4. Run locked Cargo formatting, Clippy, tests, and the focused TypeScript storage suites; obtain required reviews.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented the explicit-path storage primitives, atomic create/move handling, and create/entity lock directories with heartbeat/stale recovery. Focused storage tests pass (8), including TypeScript proper-lockfile interoperability and the disappeared-source behavior. Full stage checks and reviews remain.

Final verification: cargo fmt --check, cargo clippy --locked --all-targets -- -D warnings, and cargo test --locked pass. All 9 storage integration tests pass, including cross-runtime create/entity lock contention through shared Git common-dir paths, stale-lock races, heartbeat ownership, collisions, and missing-source moves. Existing Bun atomic-create/edit/worktree suites pass: 23 tests, 74 assertions. Cold and full-context reviews found no blocking findings.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added the Rust storage module for source-preserving record reads/writes, atomic no-overwrite creation and moves, and compatible create/entity locks. Locks coordinate with proper-lockfile paths, refresh and detect ownership, reclaim stale locks safely among Rust processes, and release promptly. Verified with the full locked Cargo suite, Clippy, formatting, 9 storage tests, and 23 existing Bun concurrency tests. Cold simplicity and full-context complexity reviews approved.
<!-- SECTION:FINAL_SUMMARY:END -->
