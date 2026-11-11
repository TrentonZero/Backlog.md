---
id: BACK-802.24
title: Port local milestone reads and creation to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-11T22:21:00Z'
updated_date: '2026-11-11T22:28:00Z'
labels: []
dependencies:
  - BACK-802.23
references:
  - rust/src/records.rs
  - src/file-system/operations.ts
modified_files:
  - rust/src/lib.rs
  - rust/src/milestone_store.rs
  - rust/tests/milestone_store.rs
parent_task_id: BACK-802
type: task
ordinal: 432000
---

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust lists active and archived milestones in deterministic ID order and resolves active milestones using the existing identifier contract.
- [x] #2 Rust creates milestones with the next ID reserved across active and archived files, honoring title, description, and due-date behavior.
- [x] #3 Isolated Rust tests compare listing, lookup, allocation, serialization, and validation outcomes against live TypeScript.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Read the Rust migration design, milestone parser, Core/FileSystem operations, and focused TypeScript tests; document exact list, lookup, creation, ordering, and validation semantics. 2. Add temporary-fixture Rust tests with live TypeScript comparisons for those outcomes. 3. Implement the smallest Rust MilestoneStore for list, lookup, and create using existing source-preserving parsers, serialization, storage, and lock conventions. 4. Run focused Bun and full Rust checks, review the flow and diff, and record evidence.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Verification: full cargo test passed 152 tests; cargo fmt --check and cargo clippy --locked --all-targets -D warnings passed. Focused Bun milestone management and due-date suites passed 13 tests with 75 assertions. Isolated Rust tests compare active/archive lists, identifier priority including zero-padded collisions, title and due-date creation, invalid-date errors, allocation, and serialized bytes against live TypeScript. Cold simplicity and full-context complexity reviews confirmed the reported blockers were closed.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added Rust milestone listing, archived listing, ordered identifier resolution, and locked creation with active/archive ID allocation, title/description preservation, filename generation, and TypeScript-compatible due-date normalization. Full Rust and focused Bun checks passed; independent reviews found no remaining blockers.
<!-- SECTION:FINAL_SUMMARY:END -->
