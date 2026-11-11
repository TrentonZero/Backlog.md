---
id: BACK-802.25
title: Port milestone rename and archive operations to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-11T22:29:00Z'
updated_date: '2026-11-11T22:38:00Z'
labels: []
dependencies:
  - BACK-802.24
references:
  - rust/src/milestone_store.rs
  - src/file-system/operations.ts
  - src/core/backlog.ts
modified_files:
  - rust/src/milestone_store.rs
  - rust/tests/milestone_lifecycle.rs
parent_task_id: BACK-802
type: task
ordinal: 433000
---

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust renames active milestones by the existing ID/title resolution rules, preserves default description semantics and omitted due dates, and refuses collisions without losing source bytes.
- [x] #2 Rust archives uniquely resolved active milestones to the matching archive path with existing return and refusal behavior.
- [x] #3 Isolated tests compare rename/archive successes, invalid input, collisions, paths, bytes, and due-date behavior against live TypeScript.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Re-read the filesystem and Core milestone rename/archive contracts and focused tests; separate store behavior from later CLI/Git orchestration. 2. Add isolated lifecycle fixtures and live TypeScript comparisons for rename, archive, failures, and byte preservation. 3. Implement the smallest MilestoneStore mutation methods using shared storage semantics and source-preserving file operations. 4. Run focused Bun and full Rust verification, obtain cold simplicity and complexity reviews, and record evidence.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Verification: cargo fmt --check and cargo clippy --locked --all-targets -D warnings passed; full Rust suite passed 154 tests. Focused Bun milestone/due-date suites passed 13 tests with 75 assertions. Isolated live TypeScript comparisons cover rename/archive paths, data, default descriptions, due-date preservation/validation, destination collisions, and repeat archive refusal. Cold simplicity and full-context complexity reviews found no remaining blockers. Groma was unavailable.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Implemented Rust milestone rename and archive operations with existing identifier resolution, default-description updates, optional due-date semantics, no-overwrite moves, and false-result refusal behavior. Full Rust and focused Bun checks passed; both independent reviews cleared the findings.
<!-- SECTION:FINAL_SUMMARY:END -->
