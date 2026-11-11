---
id: BACK-802.26
title: Port milestone removal and task assignment handling to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-11T22:39:00Z'
updated_date: '2026-11-11T22:49:00Z'
labels: []
dependencies:
  - BACK-802.25
references:
  - src/mcp/tools/milestones/handlers.ts
  - rust/src/milestone_store.rs
  - rust/src/task_store.rs
modified_files:
  - rust/src/lib.rs
  - rust/src/task_store.rs
  - rust/src/milestone_service.rs
  - rust/tests/milestone_removal.rs
parent_task_id: BACK-802
type: task
ordinal: 434000
---

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust removes an active milestone by archiving it and supports keep, clear, and reassignment of matching local task milestone values with existing target validation.
- [x] #2 Task and milestone files remain unchanged on invalid requests or when archiving fails; failures after task edits restore prior task milestone values.
- [x] #3 Isolated fixtures compare task matching, validation, resulting files, removal summary, and failure behavior against live TypeScript.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Read the milestone removal handler, TaskStore mutation contract, exact identifier matching utilities, and focused tests. 2. Add isolated fixtures and live TypeScript comparisons for keep/clear/reassign, invalid targets, matching aliases, and rollback-relevant refusal cases. 3. Implement the smallest Rust application operation coordinating task updates and milestone archival through existing stores and locks. 4. Run focused Bun and full Rust checks, obtain required independent reviews, and record evidence.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Verification: cargo fmt --check, cargo clippy --locked --all-targets -D warnings, and full cargo test passed (158 tests). Focused Bun milestone/due-date tests passed 13 tests with 75 assertions. Isolated live TypeScript comparisons cover clear/keep/reassign, matching task aliases, validation refusal, serialized task frontmatter/body, summaries, archive failure, and rollback. Cold simplicity and full-context complexity reviews found no remaining blockers.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added a Rust milestone removal operation coordinating task updates and milestone archival. It supports keep, clear, and reassign, validates targets before writes, restores changed tasks on archive failure, and reports the matching removal summary. Full Rust and focused Bun checks passed; independent reviews found no remaining blockers.
<!-- SECTION:FINAL_SUMMARY:END -->
