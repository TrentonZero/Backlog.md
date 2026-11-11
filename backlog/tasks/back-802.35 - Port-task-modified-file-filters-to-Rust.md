---
id: BACK-802.35
title: Port task modified-file filters to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-11T23:54:00Z'
updated_date: '2026-11-11T23:56:00Z'
labels: []
dependencies:
  - BACK-802.34
references:
  - src/utils/modified-files.ts
  - src/utils/task-search.ts
parent_task_id: BACK-802
ordinal: 443000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Modified-file filtering has independent normalization and substring rules. It lets task searches narrow the project corpus by the paths recorded on each task without treating empty filter entries as real constraints.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust filters modified-file metadata using case-insensitive substring matching.
- [x] #2 Filter values and task file paths are trimmed, lowercased, and empty values are ignored.
- [x] #3 No effective filter matches every task, while effective filters reject tasks with no modified files.
- [x] #4 Live TypeScript parity covers normalized, multiple, unmatched, and empty path filters.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Inspect the Rust task filter model and the exact TypeScript modified-file normalization and matching helpers. 2. Add live TypeScript parity cases to a focused Rust test. 3. Extend the Rust TaskFilter model and predicate with modified-file substring matching. 4. Run full Rust checks and relevant TypeScript modified-file filter suites. 5. Complete cold simplicity and full-context complexity reviews, finalize the task, commit, and push to main.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Extended TaskFilter with modified-file path substrings. Filter and stored path values are trimmed and lowercased; blank values are ignored. Multiple effective filters use OR semantics. Blank-only filter lists disable that predicate, and tasks without matching paths are excluded for effective filters.

Live TypeScript parity covers case/whitespace normalization, multiple filters, unmatched paths, empty filters, and blank stored paths.

Verification: BACKLOG_REFERENCE_BUN=/tmp/backlog-bun-1.4.4/bun-linux-x64/bun cargo test --manifest-path rust/Cargo.toml --locked (175 passed); cargo fmt --manifest-path rust/Cargo.toml -- --check; cargo clippy --manifest-path rust/Cargo.toml --locked --all-targets -- -D warnings; bun test src/test/task-search-label-filter.test.ts (5 passed); git diff --check.

Cold simplicity and full-context complexity reviews found no blockers. Groma remains unavailable.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added TypeScript-compatible modified-file substring filtering to the shared Rust TaskFilter. It normalizes paths and filter values, ignores empty values, matches multiple filters with OR, and rejects tasks without a matching modified file. Full Rust suite (175), formatting, Clippy, and five focused TypeScript tests passed; required reviews found no blockers.
<!-- SECTION:FINAL_SUMMARY:END -->
