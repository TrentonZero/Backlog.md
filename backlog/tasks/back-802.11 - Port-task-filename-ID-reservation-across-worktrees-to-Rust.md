---
id: BACK-802.11
title: Port task filename ID reservation across worktrees to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-08T21:07:00Z'
updated_date: '2026-11-08T21:18:00Z'
labels: []
dependencies:
  - BACK-802.10
references:
  - core
  - file-storage
  - cross-branch-loading
  - src/core/backlog.ts
  - src/file-system/operations.ts
  - src/utils/task-path.ts
  - src/utils/prefix-config.ts
  - src/test/worktree-task-id-allocation.test.ts
  - rust/src/git.rs
  - rust/src/identity.rs
modified_files:
  - rust/tests/task_file_ids.rs
  - rust/src/task_file_ids.rs
  - rust/src/lib.rs
parent_task_id: BACK-802
type: task
ordinal: 419000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Task allocation must reserve IDs claimed by filenames even when their frontmatter is unreadable or inconsistent. The working copy also shares its task ID namespace with sibling Git worktrees before files are committed; archived task filenames continue reserving their IDs.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust collects task IDs from filenames in active and archived task directories, including the legacy completed directory, without reading frontmatter.
- [x] #2 Sibling worktrees contribute task filename IDs from the same nested project path, while non-Git projects use only their own files.
- [x] #3 Configured task prefixes, direct-directory filename matching, and filename-derived IDs follow the existing rules.
- [x] #4 Isolated Rust tests compare collected identities and next allocation with live TypeScript behavior, including uncommitted sibling and archived files.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Confirm the existing filename patterns, directory layouts, archive compatibility, and worktree project-relative path rules in the current core and file-system implementation. 2. Add isolated Rust tests comparing filename ID collection and next allocation against Bun using temporary projects, malformed frontmatter, custom prefixes, and sibling worktrees. 3. Implement the smallest read-only Rust filename identity scanner using the existing Git and task-ID helpers. 4. Run formatting, locked Clippy, full locked Cargo tests, focused Bun worktree allocation tests, and required reviews.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Verification: cargo fmt --check, locked all-target Clippy with warnings denied, and the full locked Cargo suite passed. Focused Bun worktree task allocation tests passed (6 tests). Live Bun comparisons passed for malformed frontmatter, custom prefixes, direct file pattern matching, legacy archives, nested project paths, uncommitted sibling worktrees, symlinked task files on Unix, and next ID allocation.

Reviews: cold simplicity and full-context complexity reviews found no remaining blockers. The cold review identified symlinked task files as a coverage gap; this was fixed with a failing Bun parity comparison before updating the scanner, and the focused regression test passed.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added a Rust filename-based task ID reservation scan for the current project and sibling Git worktrees. It reads active tasks, current and legacy archive directories, honors custom prefixes and the existing direct-file glob behavior, follows linked task files, and never reads frontmatter.

Verified collected IDs and next allocation against the live TypeScript core, plus focused Bun allocation tests, rustfmt, locked Clippy, and the full locked Rust suite.
<!-- SECTION:FINAL_SUMMARY:END -->
