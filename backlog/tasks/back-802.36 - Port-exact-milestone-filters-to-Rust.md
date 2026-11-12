---
id: BACK-802.36
title: Port exact milestone filters to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-11T23:57:00Z'
updated_date: '2026-11-12T00:05:00Z'
labels: []
dependencies:
  - BACK-802.35
references:
  - src/utils/milestone-filter.ts
  - src/test/milestone-filter.test.ts
parent_task_id: BACK-802
ordinal: 444000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Rust task filter currently lacks milestone matching. Port the exact resolution rules first; fuzzy closest-title behavior remains a separate query capability.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust resolves active and archived milestones by case-insensitive stored ID, numeric aliases, and exact title.
- [x] #2 Exact ID matches take precedence over title matches, while duplicated exact titles match all tasks carrying that title.
- [x] #3 The no-milestone filter matches blank or absent milestone values.
- [x] #4 Live TypeScript parity covers active/archive resolution, numeric aliases, title matching, collisions, and no-milestone behavior.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Inspect the Rust milestone record/store APIs and exact TypeScript milestone filter resolution, including active and archived records. 2. Add a live TypeScript parity test for exact IDs, numeric aliases, exact titles, duplicate-title behavior, and no-milestone matching. 3. Add milestone selection to TaskFilter using the existing active/archive records, without implementing closest-title fuzzy fallback. 4. Run full Rust checks and focused TypeScript milestone filter suites. 5. Complete cold simplicity and full-context complexity reviews, finalize the task, commit, and push to main.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added exact Rust milestone filtering using the complete active and archived milestone corpus. Live TypeScript parity covers stored IDs, numeric aliases including M-000, ID/title collisions, duplicate titles, archive resolution, absent and blank assignments, and blank inactive queries. Verification: cargo test --manifest-path rust/Cargo.toml passed; cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings passed; Rust formatting and git diff --check passed; bun test src/test/milestone-filter.test.ts src/test/cli-milestone-filter.test.ts passed (27 tests). Cold simplicity and full-context complexity reviews found no remaining blockers. Groma agent-instructions is unavailable in this environment (command not found).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added exact Rust milestone filters to TaskFilter. ID and numeric alias resolution considers active and archived records, gives exact IDs precedence over colliding titles, matches duplicate titles, preserves the no-milestone behavior, and treats blank queries as inactive. Full Rust suite and clippy passed; the TypeScript milestone suites passed (27 tests). Reviewers found no blockers. Groma instructions could not be read because the command is unavailable.
<!-- SECTION:FINAL_SUMMARY:END -->
