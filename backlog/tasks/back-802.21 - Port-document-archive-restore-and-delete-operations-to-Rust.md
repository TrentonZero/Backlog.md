---
id: BACK-802.21
title: Port document archive, restore and delete operations to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-09T13:25:00Z'
updated_date: '2026-11-11T21:53:00Z'
labels: []
dependencies:
  - BACK-802.20
references:
  - src/core/backlog.ts
  - src/file-system/operations.ts
  - rust/src/document_store.rs
  - rust/src/storage.rs
modified_files:
  - rust/src/document_store.rs
  - rust/tests/document_store.rs
  - rust/tests/document_lifecycle.rs
parent_task_id: BACK-802
type: task
ordinal: 429000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Local document reads, creation, and updates now have a Rust store, but document lifecycle changes still depend on TypeScript. Port the remaining file lifecycle operations while preserving the shared DOC identity rules, subfolder paths, source bytes, symbolic links, locks, and the backlinks humans already rely on.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust archives and restores uniquely resolved documents under the shared document entity lock, rechecks identity after locking, preserves the original bytes and relative subfolder, and refuses occupied destinations without replacing either file.
- [x] #2 Missing, ambiguous, wrong-state, and archived-read-only cases match the existing document lifecycle errors and outcomes across active and archived folders.
- [x] #3 Rust deletes only uniquely resolved archived documents after locking and rechecking the same file; it removes a symbolic link itself and reports task/draft documentation backlinks without changing those records.
- [x] #4 Archived document IDs remain reserved for creation, while deletion releases an ID only when no remaining local document file claims it.
- [x] #5 Isolated live TypeScript comparisons cover archive, restore, deletion, path preservation, destination collisions, symlinks, ambiguity/refusals, backlinks, and ID occupancy.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Reuse DocumentStore identity resolution and the TypeScript Core lifecycle contract; return an operation result with the document ID and records still listing vacated paths.
2. Add isolated Rust tests and live TypeScript comparisons for archive/restore, error states, path/symlink preservation, collisions, backlinks, and ID reservation before adding methods.
3. Implement archive and restore as entity-locked, identity-rechecked Storage moves; implement archived-only deletion with source identity verification, link-safe unlink, and read-only task/draft backlink reporting.
4. Run focused compatibility tests, Rust format/Clippy/full tests, and scoped existing Bun document lifecycle tests; review the resulting diff.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented Rust document archive, restore, and archived-only deletion with entity locking, identity rechecks, source-preserving moves, no-overwrite behavior, symlink-safe unlink, and backlink reporting. Verification: cargo fmt --check; cargo clippy --locked --all-targets -- -D warnings; full locked Rust test suite passed; focused Bun document suites passed 96/96. Cold simplicity and full-context complexity reviews found no blockers. Groma instructions unavailable because the command is not installed.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Ported document archive, restore, and delete to DocumentStore with TypeScript-compatible identity, error, collision, path, symlink, backlink, and ID-reservation behavior. Verified with Rust fmt, Clippy, full Cargo tests, and 96 focused Bun tests; reviews found no blockers. Groma was unavailable in the workspace.
<!-- SECTION:FINAL_SUMMARY:END -->
