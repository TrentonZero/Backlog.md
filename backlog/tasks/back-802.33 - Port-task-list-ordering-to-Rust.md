---
id: BACK-802.33
title: Port task list ordering to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-11T23:37:00Z'
updated_date: '2026-11-11T23:49:00Z'
labels: []
dependencies:
  - BACK-802.32
references:
  - src/utils/task-sorting.ts
  - src/test/task-sorting.test.ts
parent_task_id: BACK-802
ordinal: 441000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Task lists use a stable default order plus named ID, ordinal, and priority sorts. Status grouping also keeps configured statuses and rows in a defined order. A shared Rust query helper prevents the future CLI and human-facing adapters from each implementing subtly different ordering.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust sorts task records in the same default order and named ID, ordinal, and priority orders as the TypeScript implementation.
- [x] #2 Configured priority order and numeric hierarchical task IDs retain their existing tie-breaking behavior.
- [x] #3 Rust status grouping preserves configured status order and keeps each group’s task order; explicit priority order remains a flat list.
- [x] #4 Parity tests compare representative ordering and grouping results with the TypeScript implementation.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Inspect the task sorting helpers, priority ranking, Rust TaskRecord fields, and existing ordering fixtures. 2. Add focused Rust tests against the live TypeScript sort and section helpers. 3. Implement the smallest Rust ordering and grouping API for task records. 4. Run full Rust tests, formatting, Clippy, and the relevant TypeScript sorting/order suites. 5. Complete cold simplicity and full-context complexity reviews, finalize the task, commit, and push to main.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Rust added task list sorting and status grouping. Default order applies ordinal, then configured priority rank, then numeric task ID; named ID, ordinal, and priority sorts match the existing helpers. Priority inputs are normalized and deduplicated, with the existing defaults when configuration is empty. Status groups follow configured status order and retain row order; explicit priority sort stays flat.

Parity coverage compares Rust with live TypeScript for all sort fields, configured and fallback priorities, numeric hierarchical IDs, status sections, equivalent numeric spellings, and malformed NaN ordinal cases. The NaN path uses stable insertion sorting instead of passing a non-transitive comparator to Rust slice sorting.

Verification: BACKLOG_REFERENCE_BUN=/tmp/backlog-bun-1.4.4/bun-linux-x64/bun cargo test --manifest-path rust/Cargo.toml --locked (174 tests); cargo fmt --manifest-path rust/Cargo.toml -- --check; cargo clippy --manifest-path rust/Cargo.toml --locked --all-targets -- -D warnings; bun test src/test/task-sorting.test.ts src/test/task-list-order.test.ts src/test/cli-task-list-ordinal-sort.test.ts (38 passed); git diff --check.

Cold simplicity and full-context complexity reviews found no remaining blockers. Groma remains unavailable.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added Rust task list sorting and status grouping with TypeScript-compatible default and named orders, configured priority ranking, stable numeric ID ties, configured status sections, and flat priority sections. Live parity also covers malformed NaN ordinals without giving Rust an invalid comparator. Full Rust suite (174), formatting, Clippy, and 38 focused TypeScript tests passed; required reviews found no blockers.
<!-- SECTION:FINAL_SUMMARY:END -->
