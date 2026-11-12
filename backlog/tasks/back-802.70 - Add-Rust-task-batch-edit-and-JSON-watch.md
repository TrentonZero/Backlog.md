---
id: BACK-802.70
title: Add Rust task batch edit and JSON watch
status: To Do
assignee: []
created_date: '2026-11-12T15:25:00Z'
labels: []
dependencies:
  - BACK-802.69
references:
  - docs/superpowers/specs/rust-contract-inventory.md
  - docs/superpowers/specs/2026-11-07-rust-migration-design.md
  - src/test/cli-task-batch-edit.test.ts
  - src/test/cli-json-watch.test.ts
  - src/formatters/json-output.ts
parent_task_id: BACK-802
ordinal: 478000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The single-task Rust CLI is missing two established task workflows: applying shared edits to several task IDs and watching live JSON list updates. Restore both without changing the task data model.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `task edit [taskIds...]` applies the supported shared-field changes to every requested task, preserving per-task validation and reporting failures consistently with the TypeScript CLI.
- [ ] #2 `task list --json --watch` emits successive complete schemaVersion 1 replacement values rather than NDJSON lines and preserves cancellation and parent-process lifetime behavior.
- [ ] #3 Process tests cover multiple IDs, mixed valid/invalid edits, watch updates, cancellation, and child-process termination.
- [ ] #4 Comparison cases and CLI-inventory coverage demonstrate parity for both workflows.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [ ] #3 bun test (or scoped test) passes
<!-- DOD:END -->
