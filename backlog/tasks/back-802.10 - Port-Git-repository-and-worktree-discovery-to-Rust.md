---
id: BACK-802.10
title: Port Git repository and worktree discovery to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-08T21:01:00Z'
updated_date: '2026-11-08T21:05:00Z'
labels: []
dependencies:
  - BACK-802.9
references:
  - task-identity
  - git-operations
  - src/git/operations.ts
  - src/test/git.test.ts
  - src/test/worktree-task-id-allocation.test.ts
  - rust/src/discovery.rs
modified_files:
  - rust/tests/git.rs
  - rust/src/git.rs
  - rust/src/lib.rs
parent_task_id: BACK-802
type: task
ordinal: 418000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Rust migration now has native record storage and identity rules, but it cannot yet identify Git repositories, their root, current branch, or linked worktrees. These read-only facts are needed by later identity allocation and application operations, while repositories without Git must remain supported.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust reports whether a directory is inside a Git repository and resolves its repository root with the Git executable.
- [x] #2 Rust reads the current branch and all linked worktree paths, including a detached HEAD and paths containing spaces.
- [x] #3 Missing Git, invalid directories, and non-repositories produce the same optional-Git results as the current implementation.
- [x] #4 Isolated Rust tests compare representative results against the live TypeScript implementation and pass the locked Rust checks.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Confirm current Git repository, root, branch, and worktree behavior in src/git/operations.ts and its focused tests. 2. Add isolated Rust integration tests that exercise temporary repositories and compare results with the live TypeScript helpers, including detached HEAD, spaces, invalid directories, and a missing Git executable. 3. Implement a small Rust Git module using the installed git executable and export it from the library. 4. Run formatting, locked Clippy, the full locked Cargo suite, focused Bun Git tests, and required reviews.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Verification: cargo fmt --check, locked all-target Clippy with warnings denied, and the full locked Cargo suite passed. Focused Bun tests passed: git.test.ts and worktree-task-id-allocation.test.ts (32 tests). Live Bun comparisons passed for repository roots, branch names, linked worktree paths, detached HEAD, nonexistent directories, and missing Git.

Reviews: cold simplicity review and full-context complexity review found no remaining blockers. Deferred minor: the test fixture helper returns captured git output that callers do not use.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added a read-only Rust Git repository helper for repository detection, repository root, current branch, and linked worktree paths. It runs the installed Git executable and returns the existing optional-Git results for invalid directories and unavailable Git.

Verified through live TypeScript comparisons, 32 focused Bun Git/worktree tests, rustfmt, locked Clippy, and the full locked Rust suite. Detached HEAD yields an empty branch name, matching the existing implementation.
<!-- SECTION:FINAL_SUMMARY:END -->
