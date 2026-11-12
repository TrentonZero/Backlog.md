---
id: BACK-802.55
title: Add Rust document view command
status: Done
assignee:
  - '@codex'
created_date: '2026-11-12T15:05:00Z'
updated_date: '2026-11-12T15:19:00Z'
labels: []
dependencies:
  - BACK-802.54
references:
  - src/cli.ts
  - src/test/cli-doc-view.test.ts
  - src/test/cli-doc-archive.test.ts
  - rust/src/document_store.rs
  - docs/superpowers/specs/rust-contract-inventory.md
modified_files:
  - rust/tests/cli.rs
  - rust/src/document_store.rs
  - rust/src/cli.rs
parent_task_id: BACK-802
type: task
ordinal: 463000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Once documents can be listed in Rust, users need to open an individual document from the same CLI. Add the plain document detail path using the existing DocumentStore identity model, including archived records, without waiting for the interactive TUI.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust doc view resolves numeric and prefixed IDs and prints the same stored Markdown detail as TypeScript for active and archived documents.
- [x] #2 Missing and ambiguous document IDs match TypeScript behavior and fail without guessing.
- [x] #3 Isolated process tests compare output, errors, and archived notices with TypeScript.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Compare Rust DocumentStore and TypeScript doc view behavior, then implement active and archived reads with isolated process parity. Run focused and full Rust checks, review, finalize, and continue to the next Rust CLI task.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented Rust `doc view` through the existing document command group. DocumentStore exposes read-only lookup across active and archived folders; the CLI prints stored Markdown, adds the archived read-only notice, and preserves missing and ambiguity behavior. The isolated parity test failed red before implementation because Rust rejected `doc view`, then passed for numeric and prefixed IDs, active and archived records, no --plain output, missing IDs, and an active/archive ID conflict. Full Cargo tests, rustfmt check, Clippy, Biome, and focused Bun doc-view/archive tests (41/41) passed. Independent cold and full-context reviews found no blockers.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Implemented Rust doc view for active and archived documents with TypeScript-compatible IDs, stored Markdown, archived notice, missing errors, and ambiguity refusal. Full Cargo suite, Rust formatting and Clippy, Biome, focused Bun document view/archive tests (41/41), and isolated Rust/TypeScript process parity passed. Independent cold and full-context reviews found no blockers. No TypeScript source changed, so the conditional type check was not applicable.
<!-- SECTION:FINAL_SUMMARY:END -->
