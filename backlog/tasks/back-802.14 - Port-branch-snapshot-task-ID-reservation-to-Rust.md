---
id: BACK-802.14
title: Port branch snapshot task ID reservation to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-08T21:52:00Z'
updated_date: '2026-11-08T23:46:00Z'
labels: []
dependencies:
  - BACK-802.13
references:
  - src/core/backlog.ts
  - src/core/task-loader.ts
  - src/core/task-identity-index.ts
  - src/git/operations.ts
  - rust/src/git.rs
  - rust/src/task_store.rs
modified_files:
  - >-
    backlog/tasks/back-802.14 -
    Port-branch-snapshot-task-ID-reservation-to-Rust.md
  - rust/src/git.rs
  - rust/src/task_store.rs
  - rust/tests/task_store_mutations.rs
parent_task_id: BACK-802
type: task
ordinal: 422000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Rust local task creation now reserves IDs from the current project and linked worktrees. TypeScript also treats stored task identities in configured branch snapshots as occupied. Rust needs the same reservation source before its task creator can safely replace the canonical allocation path.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust reads configured recent local branch snapshots and their active/archive task filename claims, excluding the current and stale branch refs as TypeScript does.
- [x] #2 Task ID allocation combines branch snapshot filename claims with current-project and linked-worktree claims while holding the shared create lock.
- [x] #3 Branch scanning settings, malformed task records, and duplicate/canonicalized branch filename claims preserve the existing allocation behavior.
- [x] #4 Isolated live TypeScript comparisons verify branch-only and duplicate filename claims, stale branches, and disabled scanning.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Compare Core.getOccupiedTaskIds and BranchTaskLoader branch-tip selection, branch task filename indexing, and the check_active_branches/filesystem_only/active_branch_days settings. 2. Add GitRepository helpers for recent local branch tips and committed task paths, preserving the current-branch and age cutoff rules. 3. Add selected branch filename IDs to TaskStore creation while the shared create lock is held and pass the existing Config settings through the call. 4. Add live Bun comparisons for branch-only claims, malformed branch records, recent/stale branches, and disabled scanning. 5. Run rustfmt, locked Clippy, the locked Rust suite, focused Bun tests, and required reviews.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Verification: cargo fmt --manifest-path rust/Cargo.toml -- --check; cargo clippy --manifest-path rust/Cargo.toml --locked --all-targets -- -D warnings; BACKLOG_REFERENCE_BUN=... cargo test --manifest-path rust/Cargo.toml --locked; scoped Bun identity-index test (12 passed). Cold simplicity and full-context complexity reviews found no blockers. The slice covers local refs only; remote-ref reservations remain outstanding.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Ported recent local branch snapshot filename claims into Rust task ID allocation under the shared create lock, honoring branch scan settings, current-branch exclusion, and active-branch age. Live TypeScript comparisons and Rust checks pass. Remote refs remain for a later slice.
<!-- SECTION:FINAL_SUMMARY:END -->
