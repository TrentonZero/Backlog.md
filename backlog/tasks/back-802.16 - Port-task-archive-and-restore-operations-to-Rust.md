---
id: BACK-802.16
title: Port task archive and restore operations to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-08T23:59:00Z'
updated_date: '2026-11-09T00:19:00Z'
labels: []
dependencies: []
modified_files:
  - rust/src/storage.rs
  - rust/src/task_store.rs
  - rust/tests/storage.rs
  - rust/tests/task_store_mutations.rs
parent_task_id: BACK-802
ordinal: 424000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Rust TaskStore can create, update, list, and resolve task records, but the CLI still has no native archive or restore operation. Humans and agents need to move completed or temporarily removed work off the active board and return it without losing task identity, content, or links. Implement these two file moves with the same locking, identity resolution, collision refusal, and source preservation rules as the existing product commands.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust archives an unambiguously resolved active task into the configured archive directory without changing its ID, status, content, or references.
- [x] #2 Rust restores an archived task to the active directory while preserving source and refusing collisions without overwriting records.
- [x] #3 Isolated live TypeScript comparisons verify archive and restore results, collisions, source preservation, and task ID occupancy.
- [x] #4 Missing records, archived/active state, malformed filename claims, and ambiguous local IDs produce the supported local TaskStore outcomes.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add a no-overwrite Storage move operation that preserves source bytes and metadata, preserves symlink referents with adjusted relative paths, and leaves the source intact on collision.
2. Add TaskStore archive and restore methods with task lock then shared create lock, re-resolution after locks, configured active/archive directories, and existing identity ambiguity handling.
3. Represent already archived, not archived, and destination collision refusals in TaskStoreError; return false for missing local records as TypeScript does.
4. Add isolated live TypeScript comparisons for archive/restore paths, source preservation, collisions, missing and wrong-state IDs, malformed and ambiguous records, and ID occupancy.
5. Run focused compatibility tests, rustfmt, locked Clippy and Rust suite, a scoped Bun suite, and required simplicity/complexity reviews.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
The required Groma guide could not be loaded because the groma executable is not installed in this workspace. Continuing against the repository task/spec and established migration workflow.

Verification: cargo fmt --manifest-path rust/Cargo.toml -- --check; cargo clippy --manifest-path rust/Cargo.toml --locked --all-targets -- -D warnings; BACKLOG_REFERENCE_BUN=... cargo test --manifest-path rust/Cargo.toml --locked; bun test ./src/test/task-frontmatter-preservation.test.ts ./src/test/cli-task-restore.test.ts (21 passed). Live Rust/TypeScript fixtures compare move outcomes, wrong-state and missing results, collisions, and ID occupancy. Storage tests cover byte-preserving moves, no-overwrite collisions, and relative symlinks across archive/tasks to tasks directory depths. Cold simplicity review found no blockers; full-context complexity review found and verified the symlink fix.

Reopened briefly for a final API clarity refinement: collision errors will use explicit archive/restore variants instead of a string discriminator.

Final API review replaced the collision error string discriminator with explicit archive and restore error variants. Rechecked: task_store_mutations tests (9 passed), storage tests (11 passed), Clippy, rustfmt, and diff check. The storage test uses the actual backlog/archive/tasks to backlog/tasks depth transition and checks the adjusted relative link text.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added source-preserving, no-overwrite task archive and restore operations to Rust TaskStore with per-task and shared allocator locks, identity re-resolution, and TypeScript-compatible state and collision errors. File moves preserve bytes and linked targets, including relative symlinks. Full Rust checks and focused Bun compatibility suites pass.
<!-- SECTION:FINAL_SUMMARY:END -->
