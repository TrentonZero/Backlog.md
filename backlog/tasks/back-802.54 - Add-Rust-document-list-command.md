---
id: BACK-802.54
title: Add Rust document list command
status: Done
assignee:
  - '@codex'
created_date: '2026-11-12T14:49:00Z'
updated_date: '2026-11-12T15:13:00Z'
labels: []
dependencies:
  - BACK-802.53
references:
  - src/cli.ts
  - src/test/cli-list-window.test.ts
  - docs/superpowers/specs/rust-contract-inventory.md
  - rust/src/document_store.rs
  - rust/tests/document_store.rs
modified_files:
  - rust/tests/cli.rs
  - rust/src/cli.rs
parent_task_id: BACK-802
type: task
ordinal: 462000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Rust can already list documents through DocumentStore, but the Rust executable has no document CLI group. Expose the human-readable list command so people and scripts can browse local documents, including nested paths, without moving the interactive picker ahead of the TUI stage.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust doc list --plain prints document IDs and titles in the same title/path order as TypeScript, including nested documents and excluding README and archived documents.
- [x] #2 The command matches TypeScript empty-list output and shared --max-count, --skip, and --count behavior, including continuation output and invalid-value errors.
- [x] #3 Isolated Rust process tests compare output and errors with the TypeScript CLI.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Compare TypeScript doc list output and list-window parsing with DocumentStore ordering and the existing Rust task/draft list helpers.
2. Add `doc list --plain` using DocumentStore and the shared list-window behavior; keep interactive listing for the later TUI stage.
3. Add isolated process parity tests for nested/title ordering, empty results, list windows, continuation output, and invalid values.
4. Run full Rust and scoped Bun checks, review, and finalize.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented Rust `doc list --plain` with DocumentStore ordering, active nested documents, and shared list-window parsing/footer behavior. The isolated process test failed red before implementation because Rust rejected the missing `doc` command; it now matches TypeScript for ordering, empty output, counts, paging/continuation, and invalid values, with and without `--plain`. Existing draft-list process parity still matches TypeScript after extracting shared window parsing. Full Cargo suite, Rust formatting and Clippy, Biome, the focused draft/document process tests, and scoped `src/test/cli-list-window.test.ts` (17/17) passed. Independent cold simplicity and full-context reviews found no blockers.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Implemented Rust doc list with TypeScript-compatible ordering, paging, counts, empty output, and validation. Full Cargo suite, formatting, Clippy, Biome, scoped Bun list-window tests (17/17), and isolated Rust/TypeScript process parity passed. No TypeScript source changed, so the conditional type check was not applicable.
<!-- SECTION:FINAL_SUMMARY:END -->
