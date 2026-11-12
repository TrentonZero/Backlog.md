---
id: BACK-802.63
title: Add Rust milestone rename, archive, and remove commands
status: To Do
assignee: []
created_date: '2026-11-12T15:23:00Z'
labels: []
dependencies:
  - BACK-802.62
references:
  - src/cli.ts
  - src/test/cli-milestone-management.test.ts
  - rust/src/milestone_store.rs
  - rust/src/milestone_service.rs
parent_task_id: BACK-802
ordinal: 471000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Milestone files can be renamed, archived, and removed with task-reference handling in the existing product CLI and Rust services. Expose those established lifecycle operations in the Rust executable while preserving their identity and task-update semantics.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `milestone rename <from> <to>` resolves active milestone IDs/titles, updates matching local task references by default, supports `--no-update-tasks`, and preserves due-date set/clear behavior.
- [ ] #2 `milestone archive <name>` moves an active milestone to archived storage and reports success or the established missing/ambiguous error.
- [ ] #3 `milestone remove <name>` supports clear, keep, and reassign task handling, requiring an active reassignment target when selected.
- [ ] #4 Process tests cover task reference outcomes, due dates, archived state, invalid combinations, and errors; existing metadata and task content are preserved.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [ ] #3 bun test (or scoped test) passes
<!-- DOD:END -->
