---
id: BACK-802.71
title: Add Rust overview and board export commands
status: To Do
assignee: []
created_date: '2026-11-12T15:25:00Z'
labels: []
dependencies:
  - BACK-802.70
references:
  - docs/superpowers/specs/rust-contract-inventory.md
  - docs/superpowers/specs/2026-11-07-rust-migration-design.md
  - src/commands/overview.ts
  - src/test/board.test.ts
  - rust/src/task_statistics.rs
parent_task_id: BACK-802
ordinal: 479000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Users need the existing statistics summary and a noninteractive way to export board data for sharing. Expose overview and board export in Rust while keeping interactive board rendering within the later TUI work.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `overview` reports the established task statistics and status summaries using the Rust aggregation semantics.
- [ ] #2 `board export [filename]` writes the compatible Markdown/README export, honors the documented default destination and overwrite/error behavior, and reports the resulting path.
- [ ] #3 Process tests cover populated and empty backlogs, configured statuses, default/custom destinations, and write failures.
- [ ] #4 These CLI routes pass their contract-inventory cases without introducing interactive board behavior.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [ ] #3 bun test (or scoped test) passes
<!-- DOD:END -->
