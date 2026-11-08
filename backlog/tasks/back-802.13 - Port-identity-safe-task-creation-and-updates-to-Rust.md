---
id: BACK-802.13
title: Port identity-safe task creation and updates to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-08T21:31:00Z'
updated_date: '2026-11-08T21:51:00Z'
labels: []
dependencies:
  - BACK-802.12
modified_files:
  - rust/src/git.rs
  - rust/src/task_store.rs
  - rust/tests/task_store.rs
  - rust/tests/task_store_mutations.rs
  - rust/tests/git.rs
parent_task_id: BACK-802
type: task
ordinal: 421000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Rust can now read active and archived task records, but the migrated operation layer cannot yet create or update task Markdown. Porting these writes is the next required step toward the canonical Rust CLI while keeping existing files readable by the current executable during migration.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust local task storage creates records with IDs allocated across active/archive files in the current project and linked-worktree filename reservations, using source-preserving Markdown serialization and no-overwrite writes.
- [x] #2 Rust updates a uniquely identified active task under the shared entity lock and preserves unknown metadata and unmanaged Markdown.
- [x] #3 Ambiguous, missing, archived-only, malformed-dependency, and lock-contention outcomes match the supported TypeScript behavior.
- [x] #4 Isolated live TypeScript comparisons verify created and updated files, returned identities, and errors.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Compare the allocator and FileSystemOperations saveTask/create/update contracts, including lock paths, file collisions, and active/archive identity behavior. 2. Implement local TaskStore creation with shared create locking, current/linked-worktree ID reservation, source-preserving serialization, and no-overwrite writes. 3. Implement active updates under the compatible per-task entity lock, re-resolving identity after lock acquisition and preserving stored IDs, unknown metadata, and unmanaged Markdown. 4. Compare isolated live TypeScript and Rust results for generated IDs, parsed frontmatter/body, error details, lock contention, and linked-worktree filename reservations. 5. Run focused Bun checks, rustfmt, locked Clippy, the locked Rust suite, and both required reviews.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Scope boundary: this TaskStore slice covers the current project and linked-worktree filename reservations. Stored task identities from scanned branch snapshots and remote refs still need to be supplied by the later Rust operation/index layer; this remains an outstanding parent migration requirement.

Verification: cargo fmt --manifest-path rust/Cargo.toml -- --check; cargo clippy --manifest-path rust/Cargo.toml --locked --all-targets -- -D warnings; BACKLOG_REFERENCE_BUN=/tmp/backlog-bun-1.4.4/bun-linux-x64/bun cargo test --manifest-path rust/Cargo.toml --locked; focused Bun tests for content-identity, task-path, and local-task-command-performance (61 passed); git diff --check. Live comparisons verify new and updated task records, retained unknown frontmatter and unmanaged Markdown, local and linked-worktree ID reservation, ambiguity/missing/archive-only/dependency errors, and exact task-lock contention diagnostics. Cold simplicity and full-context complexity re-reviews found no blockers. Branch snapshot stored identities and remote refs are explicitly deferred to a later Rust operation/index task and remain outstanding for the parent migration.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added Rust TaskStore creation and updates using shared create and entity locks, local plus linked-worktree ID reservations, no-overwrite storage writes, and source-preserving serialization. Live TypeScript comparisons cover file contents and identity/error behavior. Rust formatting, Clippy, the locked suite, and 61 focused Bun tests pass. Branch snapshot and remote-ref ID reservations remain required work in the parent Rust migration.
<!-- SECTION:FINAL_SUMMARY:END -->
