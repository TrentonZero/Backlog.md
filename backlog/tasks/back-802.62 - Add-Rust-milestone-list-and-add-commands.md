---
id: BACK-802.62
title: Add Rust milestone list and add commands
status: To Do
assignee: []
created_date: '2026-11-12T15:23:00Z'
labels: []
dependencies:
  - BACK-802.61
references:
  - src/cli.ts
  - src/test/cli-milestone-management.test.ts
  - src/test/cli-milestone-filter.test.ts
  - rust/src/milestone_store.rs
parent_task_id: BACK-802
ordinal: 470000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Rust executable has milestone storage but users still cannot list or create milestones through the shipped CLI. Port the existing milestone list and add workflows so milestone planning remains available without the TypeScript runtime.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `milestone` and `milestones` aliases list active milestone buckets with completion counts and due dates; completed milestones remain collapsed unless `--show-completed` is passed.
- [ ] #2 `milestone list` preserves `--plain`, paging/count options, and the TypeScript list output contract, including empty sections.
- [ ] #3 `milestone add <name>` supports description and due date, trims and validates names, and reports the created milestone.
- [ ] #4 Rust CLI process tests cover aliasing, active/completed output, list windows, creation, validation failures, and resulting milestone files.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [ ] #3 bun test (or scoped test) passes
<!-- DOD:END -->
