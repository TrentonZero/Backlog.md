---
id: BACK-802.17
title: Port archived task deletion and link cleanup to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-09T00:20:00Z'
updated_date: '2026-11-09T00:42:00Z'
labels: []
dependencies: []
references:
  - src/core/backlog.ts
  - src/utils/task-links.ts
  - src/utils/task-subtasks.ts
  - rust/src/task_store.rs
  - rust/src/frontmatter.rs
modified_files:
  - rust/tests/task_store_mutations.rs
parent_task_id: BACK-802
ordinal: 425000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Deleting an archived task releases its ID, so the existing TypeScript operation first removes dependency and reference links other tasks and drafts hold to that ID. It refuses active tasks, ambiguous identities, and tasks with active subtasks. Rust needs the same safe deletion lifecycle before its local operation layer can replace the TypeScript path.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust deletes only archived tasks, returns false for missing local records, and refuses active or ambiguous IDs with the supported TaskStore outcomes.
- [x] #2 Deletion refuses a task while any active or archived task or draft directly references it as a parent.
- [x] #3 Successful deletion cleans dependency/reference links from remaining active and archived tasks and drafts before removing the archived file under the shared create lock.
- [x] #4 Isolated live TypeScript comparisons verify safe deletion, refusal outcomes, link cleanup/source preservation, and freed ID allocation.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add live TypeScript comparison tests for archived-only deletion, missing/active/ambiguous outcomes, parent blockers across tasks and drafts, cleanup of dependencies and exact task references, source preservation, and ID reuse.
2. Run the focused Rust comparison test and confirm it fails because deletion is not implemented.
3. Implement TaskStore deletion with task and draft link scans, parent-child refusal, source-preserving frontmatter cleanup, and removal of the archived record last under shared locks.
4. Run focused comparisons, Rust formatting, Clippy, the full locked Rust suite, scoped TypeScript suites, and required reviews.
5. Record evidence, finalize BACK-802.17, and push its commit directly to main.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
The required Groma guide could not be loaded because the groma executable is not installed in this workspace. Continuing against the repository task/spec and established migration workflow.

Implemented archived-only TaskStore deletion with active/ambiguous refusal, direct child refusal across active tasks, archived tasks, and drafts, and source-preserving link cleanup under task, draft, and shared create locks.

Verification: live TypeScript comparison tests cover missing/active/ambiguous IDs, task and draft child blockers, active/archive/draft link cleanup, unmanaged metadata/body preservation, and ID reuse. `cargo fmt --manifest-path rust/Cargo.toml -- --check`, locked Clippy with warnings denied, full locked Rust tests, and scoped Bun delete/reference tests (50 passed) all passed. Cold simplicity and full-context complexity reviews found no remaining blockers; the comparison assertion gap was fixed and rechecked. Groma could not run because it is not installed.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added Rust archived task deletion with safe local link cleanup and parent-child refusals. Rust matches the live TypeScript outcomes and preserves unmanaged Markdown while releasing the ID only after cleanup. Verified with full Rust checks and scoped Bun suites; reviews are clear.
<!-- SECTION:FINAL_SUMMARY:END -->
