---
id: BACK-802.64
title: Add Rust cross-entity search command
status: To Do
assignee: []
created_date: '2026-11-12T15:23:00Z'
updated_date: '2026-11-12T15:23:00Z'
labels: []
dependencies:
  - BACK-802.61
references:
  - src/cli.ts
  - src/test/cli-search-command.test.ts
  - src/test/task-search-parity.test.ts
  - docs/superpowers/specs/rust-contract-inventory.md
  - rust/src/task_search.rs
parent_task_id: BACK-802
ordinal: 472000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Users currently have to search tasks, documents, and decisions separately, so relevant records can be missed. Add the existing shared search workflow to the Rust CLI after document and decision commands are available.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `search [query]` searches tasks, documents, and decisions and returns the established cross-entity ranking.
- [ ] #2 Search supports `--type`, task `--task-type`, `--status`, and `--exclude-status` filters, repeated or comma-separated where documented, plus list-window options.
- [ ] #3 Arguments after `--` are treated as literal query text; no-query and empty-result behavior match the TypeScript CLI.
- [ ] #4 Plain output and schemaVersion 1 JSON use the documented search result envelope; process tests cover all three entity kinds, filters, literal arguments, ordering, and errors.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [ ] #3 bun test (or scoped test) passes
<!-- DOD:END -->
