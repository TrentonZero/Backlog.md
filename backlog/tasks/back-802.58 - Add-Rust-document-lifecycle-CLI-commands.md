---
id: BACK-802.58
title: Add Rust document lifecycle CLI commands
status: Done
assignee:
  - '@codex'
created_date: '2026-11-12T15:05:00Z'
updated_date: '2026-11-12T16:13:00Z'
labels: []
dependencies:
  - BACK-802.57
references:
  - src/cli.ts
  - src/test/cli-doc-archive.test.ts
  - rust/src/document_store.rs
  - rust/tests/document_lifecycle.rs
modified_files:
  - rust/src/cli.rs
  - rust/tests/cli.rs
parent_task_id: BACK-802
type: task
ordinal: 466000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Documents can be created and updated through the Rust CLI, but archiving, restoring, and deleting still require TypeScript. Expose the existing Rust DocumentStore lifecycle operations while preserving backlinks, identity checks, and path semantics.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust exposes doc archive, restore, and delete with TypeScript-compatible output and state errors.
- [x] #2 Lifecycle commands preserve source and symlink behavior, refuse collisions or ambiguous IDs, and report documentation backlinks as TypeScript does.
- [x] #3 Isolated process tests compare results, errors, and changed files with TypeScript.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Compare TypeScript archive, restore, and delete CLI outputs/errors, linked-documentation reporting, symlink handling, and selected-path auto-commit with the existing DocumentStore lifecycle APIs. Add process parity for state/identity refusals, backlinks, byte-preserving moves, deletion, and Git failures; keep task/draft references unchanged. Run focused/full checks, review, finalize, and continue into decision commands.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented Rust doc archive/restore/delete CLI adapters over DocumentStore. Added process parity for aliases, confirmations, backlinks, missing/wrong-state refusals, byte-preserving moves, collisions/ambiguity, symlink unlink behavior, ID release, and auto-commit path selection/failure behavior. Git index-lock failures match TypeScript stderr and preserve the applied lifecycle change. Verification: cargo fmt --check, cargo clippy --locked --all-targets -D warnings, full cargo test --locked, scoped Bun archive/restore-delete/view tests (100 passed), Biome check, and git diff --check all passed.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Implemented Rust doc archive, restore, and delete commands over DocumentStore. They preserve identity/state refusals, byte-preserving moves, backlinks without mutating owners, symlink targets, selected-path auto-commit, and TypeScript-compatible failure reporting. Full Cargo suite, scoped Bun docs suites (100 tests / 638 assertions), rustfmt, Clippy, Biome, and diff check passed; cold simplicity and full-context reviews found no blockers. No TypeScript source changed, so the conditional type check was not applicable.
<!-- SECTION:FINAL_SUMMARY:END -->
