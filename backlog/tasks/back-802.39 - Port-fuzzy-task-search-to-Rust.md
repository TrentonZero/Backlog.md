---
id: BACK-802.39
title: Port fuzzy task search to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-12T00:29:00Z'
updated_date: '2026-11-12T00:39:00Z'
labels: []
dependencies:
  - BACK-802.38
references:
  - src/utils/task-search.ts
  - src/test/task-search-parity.test.ts
  - src/test/task-search-label-filter.test.ts
parent_task_id: BACK-802
ordinal: 447000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Rust task-search field builder now matches TypeScript, but Rust cannot yet answer free-text queries or preserve Fuse ranking across task fields. Porting the shared index behavior is required before search surfaces can move off the TypeScript model.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust searches title, searchable body, ID, ID variants, dependency IDs, and modified files with the same Fuse weights, threshold, location behavior, and minimum match length as TypeScript.
- [x] #2 Search results preserve Fuse relevance order while all exact field and milestone filters compose with the same semantics and full-corpus resolution.
- [x] #3 Empty or whitespace-only queries preserve input order, and unmatched queries return no results.
- [x] #4 Live TypeScript parity covers fuzzy title/body matches, ID aliases including one-character queries, modified files, ranking, and combined filters.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Inspect Fuse 7.5 indexing, key-weight normalization, field norms, score ordering, and existing task filter behavior. 2. Extract the shared Bitap scorer used by milestone matching so both searches use one implementation. 3. Add a Rust in-memory task search index that ranks indexed fields and applies the shared TaskFilter predicates against the full corpus. 4. Add live TypeScript parity for scores/order, empty queries, one-character ID matches, and combined filters. 5. Run full Rust and focused TypeScript checks, complete cold and complexity reviews, finalize, and push to main.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Groma setup: attempted groma agent-instructions before implementation; unavailable in this environment (command not found), so the architecture guide could not be loaded.

Rust verification: cargo fmt --manifest-path rust/Cargo.toml -- --check; cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings; BACKLOG_REFERENCE_BUN=/tmp/backlog-bun-1.4.4/bun-linux-x64/bun cargo test --manifest-path rust/Cargo.toml (full suite passed); bun test src/test/task-search-parity.test.ts src/test/task-search-label-filter.test.ts src/test/milestone-filter.test.ts src/test/cli-milestone-filter.test.ts (49 passed); bun run check .; bun run check:types; git diff --check. Cold simplicity and complexity reviews found no blocking findings.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added Fuse-compatible fuzzy task search in Rust with stable relevance order, full-corpus exact and milestone filter composition, and order-preserving empty queries. Extracted the shared Bitap scorer for task and milestone search and reused the shared task filter matcher. Live TypeScript parity covers fuzzy title/body queries, one-character IDs, dependencies, modified files, unmatched and empty queries, ranking, and combined filters. Full Rust tests, Clippy, rustfmt, focused TypeScript tests (49), Biome check, and diff check passed. Groma remains unavailable in this environment.
<!-- SECTION:FINAL_SUMMARY:END -->
