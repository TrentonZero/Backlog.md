---
id: BACK-802.19
title: Port draft archive, promotion and demotion to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-09T12:37:00Z'
updated_date: '2026-11-09T13:04:00Z'
labels: []
dependencies: []
references:
  - src/file-system/operations.ts
  - src/core/backlog.ts
  - rust/src/draft_store.rs
  - rust/src/task_store.rs
modified_files:
  - rust/tests/draft_lifecycle.rs
  - rust/src/draft_store.rs
  - rust/src/task_store.rs
parent_task_id: BACK-802
type: task
ordinal: 427000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Draft archive, promotion, and demotion still use the TypeScript filesystem path. Port these lifecycle operations into the shared Rust draft and task stores while preserving existing filename identity, locking, ID allocation, and Markdown source behavior.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust archives a uniquely resolved draft with its source content intact; missing and ambiguous identities match the current contract, and destination collisions match TypeScript archive behavior.
- [x] #2 Rust promotes a uniquely resolved draft into an active task using configured task ID allocation and lock semantics; success and failure preserve the same source and ID rules as TypeScript.
- [x] #3 Rust demotes an active task into a draft using configured DRAFT ID allocation and lock semantics; success and failure preserve the same source and collision rules as TypeScript.
- [x] #4 Isolated live TypeScript comparisons cover archive, promotion, demotion, missing and ambiguous identities, collisions, and source preservation.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add isolated Rust fixtures comparing archive, promotion, and demotion against the live TypeScript core, including identity, collision, and source-preservation cases.
2. Implement draft archive under its lock, snapshotting source bytes and preserving TypeScript overwrite behavior at the archive destination.
3. Implement promotion with draft-then-create lock order, task ID occupancy and parent-aware allocation, writing the task before removing the source.
4. Implement demotion with task/draft/create locks, DRAFT allocation, source-preserving writes, and vacated-ID link cleanup.
5. Run focused and full Rust checks, scoped Bun lifecycle tests, cold and complexity reviews; record results and push the task commit to main.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented Rust draft archive, promotion, and demotion. Archive snapshots symlink contents and matches TypeScript overwrite behavior for an existing archive path. Promotion uses draft-then-create locking and parent-aware task ID allocation. Demotion preserves TypeScript status/updated_date semantics, removes links to the task ID it releases, and cleans references in other records under task, draft, and allocator locks. Task creation and lifecycle operations share the same occupied-ID scan; task deletion reuses the vacated-link cleanup helper.

Verification: cargo fmt --manifest-path rust/Cargo.toml -- --check; cargo clippy --manifest-path rust/Cargo.toml --locked --all-targets -- -D warnings; BACKLOG_REFERENCE_BUN=/tmp/backlog-bun-1.4.4/bun-linux-x64/bun cargo test --manifest-path rust/Cargo.toml --locked; scoped Bun tests for draft lifecycle auto-commit, draft editing, and released-ID draft links (39 passed, 1 skipped). Live Rust-vs-TypeScript comparisons passed for lifecycle, collisions, and source semantics. Cold and complexity reviews cleared the in-scope findings. Groma instructions could not be loaded because the groma executable is unavailable.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Ported draft archive, promotion, and demotion into the Rust stores, preserving the direct TypeScript lifecycle contract for locks, IDs, source content, archive overwrite behavior, and vacated-ID reference cleanup. Added isolated live TypeScript comparisons and reused shared task/draft identity and cleanup logic.

Verified by formatting, Clippy, the full locked Rust test suite, 39 scoped Bun tests (1 skipped, 0 failed), and cold/complexity reviews. The complete Rust executable cutover remains in later BACK-802 work.
<!-- SECTION:FINAL_SUMMARY:END -->
