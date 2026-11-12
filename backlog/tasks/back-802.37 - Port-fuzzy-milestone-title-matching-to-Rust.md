---
id: BACK-802.37
title: Port fuzzy milestone title matching to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-12T00:07:00Z'
updated_date: '2026-11-12T00:16:00Z'
labels: []
dependencies:
  - BACK-802.36
references:
  - src/utils/milestone-filter.ts
  - src/test/milestone-filter.test.ts
parent_task_id: BACK-802
ordinal: 445000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Users who search tasks with a shortened or misspelled milestone title need the same closest-title result in Rust as they receive from the current TypeScript matcher. The exact milestone filter task handles ID and exact-title resolution; this task covers its remaining closest-title behavior.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust milestone filtering resolves partial and misspelled titles with the same closest-title behavior as the TypeScript matcher.
- [x] #2 Exact ID, exact title, collision, blank-query, and no-milestone semantics remain unchanged when fuzzy matching is added.
- [x] #3 Live TypeScript parity covers representative partial, typo, normalized punctuation/case, and unmatched queries.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Inspect Fuse.js scoring, normalization, candidate keys, threshold, and fallback behavior alongside the Rust filter. 2. Add Fuse-compatible closest-title matching while preserving exact ID/title and no-milestone precedence. 3. Pass a separate full task corpus so fuzzy resolution stays stable when other filters narrow returned rows. 4. Add live TypeScript parity for partial, typo, punctuation/case, unmatched, and hidden-candidate queries. 5. Run Rust and focused TypeScript checks, complete the required reviews, finalize, and push to main.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented closest-title matching with the TypeScript Fuse settings: normalized title and compact keys (0.7/0.3 weights), 0.45 threshold, minimum match length 2, and compact-query fallback. The filter now accepts a separate full milestone_corpus so candidate resolution remains stable after status/visibility filters narrow the returned rows. Live TypeScript parity verifies partial/typo matches, normalization, no-match fallback, and a hidden closer title. Full Rust suite, clippy, formatting, and focused TypeScript milestone suites pass. Complexity review's full-corpus finding was fixed and cleared. Groma agent-instructions remains unavailable (command not found).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added Fuse-compatible closest milestone title matching to Rust using the existing normalized and compact keys, field weights, threshold, short-match rule, and fallback. Exact ID/title and no-milestone handling remain ahead of fuzzy matching. Filtering accepts a separate full task corpus so hidden rows cannot change title resolution. Full Rust suite, clippy, rustfmt, and the focused TypeScript milestone suites (27 tests) passed. Both required reviews found no blockers. Groma instructions could not be loaded because the command is unavailable.
<!-- SECTION:FINAL_SUMMARY:END -->
