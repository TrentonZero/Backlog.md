---
id: BACK-802.61
title: Add Rust decision archive, restore, and delete commands
status: Done
assignee:
  - '@codex'
created_date: '2026-11-12T15:16:00Z'
updated_date: '2026-11-12T16:58:00Z'
labels: []
dependencies:
  - BACK-802.60
references:
  - src/cli.ts
  - src/test/cli-decision-archive.test.ts
  - rust/src/decision_store.rs
  - rust/tests/decision_lifecycle.rs
parent_task_id: BACK-802
type: task
ordinal: 469000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
DecisionStore already implements the decision lifecycle, but users of the native executable cannot archive, restore, or remove decision files. Expose the existing lifecycle through the CLI while preserving identity, reviewable file changes, and documentation backlinks.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 decision archive and restore move the decision file byte-for-byte under the matching archive path, keep its ID reserved, refuse path collisions, and print TypeScript-compatible confirmations and linked task/draft reports.
- [x] #2 Archived decisions remain readable and read-only; archive and restore refuse missing, ambiguous, or wrong-state identities without changing unrelated records.
- [x] #3 decision delete requires an archived decision, asks no prompt, removes a symlink as a link, reports matching task and draft documentation references without changing them, and releases the ID only when no other file or committed branch claims it.
- [x] #4 Lifecycle operations preserve configured auto-commit behavior and commit only their selected decision paths.
- [x] #5 Isolated Rust process tests compare exit codes, output, errors, and filesystem changes with TypeScript, including collisions, backlinks, and symlink behavior.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Port archive, restore, and delete CLI behavior using DecisionStore and the document lifecycle runner as the model. Add isolated TypeScript process parity for byte-preserving moves, identity/state errors, path collisions, task/draft backlinks, archived deletion and ID release, symlinks, selected-path auto-commit, and commit-failure state. Run focused parity first, then full Rust and scoped Bun checks, formatting, Clippy, and independent reviews before finalizing and proceeding to milestones.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented Rust decision archive, restore, and delete routes through DecisionStore. Process parity covers byte-preserving moves, confirmations/backlink output without owner mutation, ID release, missing/wrong-state/ambiguous errors, archive and restore collisions, symlink moves/deletion, auto-commit selected paths with unrelated staged work, and index.lock commit failure state. Verified: focused Rust lifecycle tests 5 passed; full locked Cargo suite passed (85 CLI process tests plus all Rust suites); cargo fmt --check; locked Clippy with -D warnings; decision archive Bun suite 36 passed / 241 assertions; Biome checked 481 files; git diff --check.

Independent cold simplicity and full-context complexity reviews found no blocking issues. The lifecycle routes closely parallel document lifecycle routing, while retaining command-specific store calls and user-facing messages.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added Rust decision archive, restore, and delete commands through DecisionStore with TypeScript-parity moves, refusals, backlink reporting, symlink handling, ID behavior, and selected-path auto-commit. Verified full locked Cargo suite (85 CLI process tests included), formatting, Clippy, 36 decision lifecycle Bun tests, Biome, and diff checks. Both required reviews found no blockers. TypeScript was not changed, so the conditional type check did not apply.
<!-- SECTION:FINAL_SUMMARY:END -->
