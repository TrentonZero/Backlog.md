---
id: BACK-802.38
title: Port task searchable field assembly to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-12T00:17:00Z'
updated_date: '2026-11-12T00:28:00Z'
labels: []
dependencies:
  - BACK-802.37
references:
  - src/utils/task-search.ts
  - src/utils/task-id-search.ts
  - src/test/task-search-parity.test.ts
parent_task_id: BACK-802
ordinal: 446000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Task search currently has one shared searchable-text and field builder, so queries behave the same across the CLI, TUI, MCP, web API, and cross-branch service. Rust needs the same field model before it can own task search without delegating to Bun.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust builds the searchable task body from the same fields and in the same order as TypeScript, including ordered acceptance criteria and comment bodies.
- [x] #2 Rust search fields preserve title, task ID variants, dependency ID variants, and modified-file values from the task record.
- [x] #3 Live TypeScript parity covers populated fields, ordering, and absent optional content.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Inspect the TypeScript searchable body and task ID variant rules, plus the Rust TaskRecord fields already produced by parsing. 2. Add a Rust search field model and body builder that preserve the shared field order and ID variants. 3. Add live TypeScript parity for populated and sparse task records, including criterion ordering and comment bodies. 4. Run focused and full Rust checks and required reviews. 5. Finalize and push the task commit to main.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added public Rust TaskSearchFields and body builder with TypeScript field ordering, ordered acceptance criteria, comment bodies, labels, assignees, and modified files. Task ID and dependency variants match the TypeScript rules, including JavaScript number rounding and Infinity for oversized numeric segments. Live parity covers populated/sparse records and large IDs. Full Rust suite, Clippy, rustfmt, and the existing task-search parity suite (17 tests) pass. Groma agent-instructions is unavailable (command not found).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added a Rust task search field model and body builder matching TypeScript ordering and ID variants. Live parity covers populated and sparse tasks, ordered criteria, comments, dependencies, modified files, large-number rounding, and Infinity overflow. The full Rust suite, Clippy, rustfmt, and task-search parity tests (17) passed. Both required reviews found no blockers. Groma instructions could not be loaded because the command is unavailable.
<!-- SECTION:FINAL_SUMMARY:END -->
