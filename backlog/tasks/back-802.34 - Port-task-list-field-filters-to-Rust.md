---
id: BACK-802.34
title: Port task list field filters to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-11T23:50:00Z'
updated_date: '2026-11-11T23:53:00Z'
labels: []
dependencies:
  - BACK-802.33
references:
  - src/utils/task-search.ts
  - src/utils/status-filter.ts
  - src/utils/task-type-config.ts
  - src/utils/project-config.ts
  - src/utils/priority-config.ts
  - src/utils/label-filter.ts
  - src/utils/task-id.ts
  - src/test/task-search-label-filter.test.ts
  - src/test/task-type-filtering.test.ts
parent_task_id: BACK-802
ordinal: 442000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Task list consumers need consistent field predicates in the shared Rust model before filter options can serve the CLI, TUI, MCP, and browser. This slice covers stored task fields whose matching rules need neither another corpus nor a fuzzy search index.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust filters tasks by included and excluded statuses, task type, project, priority, assignee, and unassigned state with TypeScript-compatible normalization.
- [x] #2 Label filters preserve any/all matching semantics, and parent filters use canonical task identity.
- [x] #3 Combined filters are ANDed and retain the input order of matching task records.
- [x] #4 Live TypeScript parity covers normalized matching, empty values, and combined predicates.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Inspect the reference task matcher and helper normalization contracts plus TaskRecord fields. 2. Add live TypeScript parity tests for the selected field predicates and combinations. 3. Implement a Rust TaskFilter value and a pure filter operation that preserves input order. 4. Run full Rust checks and focused TypeScript filter suites. 5. Complete cold simplicity and full-context complexity reviews, finalize the task, commit, and push to main.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Rust added TaskFilter, LabelMatch, and filter_tasks. The predicate covers included/excluded statuses, task type, project, priority, assignee/unassigned, any/all labels, and canonical parent IDs. It ANDs predicates and returns matching references in their input order. Fuzzy search, modified-file matching, and milestone resolution remain separate query work.

Live TypeScript parity covers normalized values, whitespace-only filters, custom labels, canonical parent IDs, unassigned rows, exclusion, and combined predicates.

Verification: BACKLOG_REFERENCE_BUN=/tmp/backlog-bun-1.4.4/bun-linux-x64/bun cargo test --manifest-path rust/Cargo.toml --locked (175 passed); cargo fmt --manifest-path rust/Cargo.toml -- --check; cargo clippy --manifest-path rust/Cargo.toml --locked --all-targets -- -D warnings; bun test src/test/task-search-label-filter.test.ts src/test/task-type-filtering.test.ts src/test/task-project-filtering.test.ts src/test/status-filter.test.ts src/test/label-filter.test.ts (32 passed); git diff --check.

Cold simplicity and full-context complexity reviews found no blockers. Groma remains unavailable.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added shared Rust exact field filters for task lists, including normalized statuses, types, projects, priorities, assignees, labels, and parent IDs. Matching predicates are ANDed and preserve input order. Live TypeScript parity covers all scoped rules. Full Rust suite (175), formatting, Clippy, and 32 focused TypeScript tests passed; required reviews found no blockers.
<!-- SECTION:FINAL_SUMMARY:END -->
