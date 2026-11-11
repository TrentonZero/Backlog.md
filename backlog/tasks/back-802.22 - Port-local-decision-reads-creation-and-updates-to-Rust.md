---
id: BACK-802.22
title: Port local decision reads, creation and updates to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-11T21:54:00Z'
updated_date: '2026-11-11T22:07:00Z'
labels: []
dependencies:
  - BACK-802.21
references:
  - src/core/backlog.ts
  - src/file-system/operations.ts
  - src/markdown/serializer.ts
  - rust/src/records.rs
  - rust/src/serialization.rs
modified_files:
  - rust/src/lib.rs
  - rust/src/decision_store.rs
  - rust/tests/decision_store.rs
parent_task_id: BACK-802
type: task
ordinal: 430000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Port the local decision store operations to Rust. Decisions are structured Markdown records with managed sections and human-readable IDs; preserve existing TypeScript ordering, identity resolution, source text, validation, and write behavior. Keep lifecycle moves and deletion for a follow-up slice.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust lists readable active and archived decisions in TypeScript-compatible order and resolves IDs across both locations, failing closed on ambiguous identities.
- [x] #2 Rust creates decisions with the existing DEC ID allocation and file naming rules, shared create locking, and no-overwrite behavior.
- [x] #3 Rust applies validated partial updates under the decision entity lock, rechecks identity, refuses archived records, and preserves unknown frontmatter and unmanaged Markdown.
- [x] #4 Decision sections, status, supersession links, timestamps, and unchanged-source behavior match the current Core and serializer contracts.
- [x] #5 Isolated Rust tests compare reads, create, update, validation refusals, identity ambiguity, locking, and source preservation against live TypeScript.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Re-read the decision Core and FileSystem methods, record/parser/serializer contracts, and focused tests; inventory identity, path, input, and source-preservation semantics.
2. Add isolated failing Rust tests and live TypeScript comparisons for decision listing/lookup, creation, and updates before implementation.
3. Implement DecisionStore using shared identity resolution, ID allocation, source-preserving serialization, create locking, and entity locking; keep archive lifecycle out of scope.
4. Run Rust fmt, Clippy, full Rust tests, and focused Bun decision content/identity tests; inspect the diff and record review findings.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented DecisionStore reads, creation, and locked partial updates using the shared parser, source-preserving serializer, identity lookup, create/entity locks, and DEC allocation across local and Git branch records. Live TypeScript comparisons cover order, active/archive lookup, creation, updates, validation refusals, ambiguity, archive read-only, lock contention, source preservation, and unchanged updates. Verification: cargo fmt --check; cargo clippy --locked --all-targets -- -D warnings; full Rust suite passed 144/144; focused Bun decision suites passed 46/46. Cold simplicity and full-context complexity reviews found and verified fixes for comparison coverage and trailing-number ordering. Groma was unavailable because it is not installed.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added Rust decision listing, archive-aware lookup, DEC ID allocation and creation, and locked, validated partial updates with supersession handling and source preservation. Rust formatting, Clippy, and all 144 Rust tests passed; 46 focused Bun tests passed. Independent reviews confirmed their findings were resolved. Groma was unavailable.
<!-- SECTION:FINAL_SUMMARY:END -->
