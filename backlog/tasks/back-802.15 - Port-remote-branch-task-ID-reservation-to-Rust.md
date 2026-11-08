---
id: BACK-802.15
title: Port remote branch task ID reservation to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-08T23:48:00Z'
updated_date: '2026-11-08T23:57:00Z'
labels: []
dependencies: []
modified_files:
  - rust/src/git.rs
  - rust/src/task_store.rs
  - rust/tests/task_store_mutations.rs
parent_task_id: BACK-802
ordinal: 423000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Rust task creation currently includes current-project, linked-worktree, and local branch snapshot filename claims. TypeScript also reserves IDs found in configured remote branch snapshots, so moving task creation to Rust without this source can allocate an ID already present on a remote branch. Add remote snapshot reservation with the existing freshness and remote-operation settings preserved.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust allocation includes eligible remote branch task and archive filename IDs alongside local and linked-worktree claims under the shared create lock.
- [x] #2 Remote refresh, remoteOperations, active-branch age, current-branch exclusion, and filesystem-only settings match the supported TypeScript behavior.
- [x] #3 Malformed and duplicate/canonicalized remote filename claims reserve IDs without failing allocation.
- [x] #4 Isolated live TypeScript comparisons cover remote-only claims, remote operations disabled, stale refs, and refresh behavior.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add a GitRepository origin refresh method that skips absent origins and disabled remote operations, fetches with the existing origin/prune/no-submodules behavior, and does not fail task allocation on refresh errors.
2. Extend recent branch-tip discovery to include origin remote refs when remote operations are enabled, excluding origin/HEAD and stale tips while retaining local/current branch behavior.
3. Invoke refresh before branch snapshot ID collection under the existing create lock; keep check_active_branches and filesystem_only as complete scan bypasses.
4. Add isolated live TypeScript comparisons with local bare remotes for fresh remote-only and post-clone pushed claims, remote operations disabled, stale commits, duplicate/malformed filenames, and refresh behavior.
5. Run focused tests, rustfmt, Clippy, locked Rust suite, relevant Bun checks, and cold simplicity plus full-context complexity reviews.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
The required Groma guide could not be loaded because the groma executable is not installed in this workspace. Continuing against the repository task/spec and established migration workflow.

Verification: cargo fmt --manifest-path rust/Cargo.toml -- --check; cargo clippy --manifest-path rust/Cargo.toml --locked --all-targets -- -D warnings; BACKLOG_REFERENCE_BUN=... cargo test --manifest-path rust/Cargo.toml --locked; bun test ./src/test/core-task-corpus-regressions.test.ts (12 passed). Rust live TypeScript comparisons verify fresh post-clone remote pushes, remote operations disabled, stale refs, filesystem-only mode, duplicate/malformed active claims, and archived filename claims. Cold simplicity and full-context complexity reviews found no blocking findings. Groma was unavailable in the workspace.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Rust task ID allocation now refreshes the configured origin under the shared create lock, scans recent origin branch task and archive filename claims, and honors remote, branch-age, and filesystem-only settings. Fetch failures remain nonfatal and use the existing refs, matching TypeScript behavior. Full Rust checks and focused Bun compatibility tests pass.
<!-- SECTION:FINAL_SUMMARY:END -->
