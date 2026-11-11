---
id: BACK-802.23
title: Port decision archive, restore and delete operations to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-11T22:08:00Z'
updated_date: '2026-11-11T22:20:00Z'
labels: []
dependencies:
  - BACK-802.22
references:
  - src/core/backlog.ts
  - src/file-system/operations.ts
  - rust/src/decision_store.rs
  - rust/src/storage.rs
modified_files:
  - rust/src/decision_store.rs
  - rust/src/content_references.rs
  - rust/src/document_store.rs
  - rust/src/lib.rs
  - rust/tests/decision_store.rs
  - rust/tests/decision_lifecycle.rs
parent_task_id: BACK-802
type: task
ordinal: 431000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Port decision lifecycle operations into the Rust DecisionStore. Preserve decision identity, shared entity locking, byte-preserving moves, archived read-only behavior, safe deletion, ID reservation, and documentation backlink reporting.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust archives and restores uniquely resolved decisions under the shared decision lock, rechecks identity after locking, preserves source bytes and filenames, and refuses occupied destinations without overwriting either file.
- [x] #2 Missing, ambiguous, wrong-state, and archived-read-only outcomes match the existing decision lifecycle contract.
- [x] #3 Rust deletes only uniquely resolved archived decisions after locking and verifying identity, unlinks symlinks without deleting their targets, and reports task/draft documentation backlinks without modifying those records.
- [x] #4 Archived decision IDs remain reserved; deletion frees an ID only when no remaining local or committed branch filename claims it.
- [x] #5 Isolated Rust tests compare archive, restore, deletion, paths, collisions, symlinks, refusals, backlinks, and ID occupancy against live TypeScript.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Re-read the decision lifecycle Core and FileSystem contracts and existing TypeScript tests; inventory paths, errors, backlinks, locks, source preservation, Git effects, and ID reservation.
2. Add isolated Rust tests and live TypeScript comparisons for archive/restore/delete and refusal cases before implementation.
3. Implement identity-locked moves and archived-only deletion in DecisionStore using Storage move/unlink semantics and shared backlink reporting conventions.
4. Run Rust fmt, Clippy, full Cargo tests, and focused Bun decision lifecycle suites; review the diff and record review findings.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Verification: cargo fmt --check and cargo clippy --locked --all-targets -D warnings passed; full Rust suite passed 150 tests. Focused Bun decision/document lifecycle suites passed 122 tests with 863 assertions. Cold and complexity reviews found no blockers; the committed-branch reservation case was added and passed.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Implemented decision archive, restore and delete in Rust with shared locking, identity rechecks, byte-preserving moves, symlink-safe deletion, backlink reporting and local/committed-branch ID reservation. Rust and focused Bun comparisons passed.
<!-- SECTION:FINAL_SUMMARY:END -->
