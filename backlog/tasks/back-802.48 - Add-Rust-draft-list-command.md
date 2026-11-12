---
id: BACK-802.48
title: Add Rust draft list command
status: Done
assignee:
  - '@codex'
created_date: '2026-11-12T02:54:00Z'
updated_date: '2026-11-12T03:06:00Z'
labels: []
dependencies:
  - BACK-802.47
parent_task_id: BACK-802
type: task
ordinal: 456000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Draft records already have a Rust store, but users cannot list drafts through the Rust executable. Implement the read-only list command so draft management can move onto the canonical Rust CLI.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust `draft list --plain` matches TypeScript output for empty and populated draft directories.
- [x] #2 Rust `draft list --json` matches the versioned TypeScript response, including pagination metadata.
- [x] #3 Draft sorting, default ordering, invalid sort fields, and list windows match TypeScript behavior.
- [x] #4 The command uses local draft records and does not modify files.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Compare TypeScript draft list flags, output formats, sort behavior, and pagination with existing Rust list and JSON helpers.
2. Add Rust draft list arguments and a read path using DraftStore, shared sorting, and established plain and JSON contracts.
3. Add Rust versus TypeScript process tests for empty and populated drafts, sorting, invalid sort fields, pagination, help, and no writes.
4. Update Rust CLI documentation, run Cargo and scoped Bun checks, obtain required reviews, and finalize.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added Rust `draft list` with local DraftStore reads, TypeScript-compatible priority/ID/ordinal sorting, plain and versioned JSON output, pagination windows, count mode, and matching validation. JSON IDs preserve TypeScript normalization, including leading zeroes; large valid window values behave as out-of-range values.

Verification: Full Rust test suite passed; focused TypeScript list-window and JSON-output suites passed (35 tests); cargo fmt --check, Clippy with -D warnings, cargo build, bun run check ., and git diff --check passed. Cold simplicity and full-context complexity reviews found no blockers. Groma instructions were unavailable (`groma: command not found`).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added the Rust draft list command with TypeScript-compatible plain and JSON output, sorting, count, and pagination behavior. Verified empty and populated projects, zero-padded IDs, large and invalid window values, no-write behavior, help, and output parity. Full Rust tests, 35 focused Bun tests, formatting, Clippy, build, and Biome checks passed; required reviews cleared.
<!-- SECTION:FINAL_SUMMARY:END -->
