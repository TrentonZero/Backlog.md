---
id: BACK-802.52
title: Add Rust draft lifecycle CLI commands
status: Done
assignee:
  - '@codex'
created_date: '2026-11-12T14:31:00Z'
updated_date: '2026-11-12T14:43:00Z'
labels: []
dependencies:
  - BACK-802.51
references:
  - src/cli.ts
  - src/test/cli-task-state.test.ts
  - src/test/cli-custom-prefix-id-resolution.test.ts
  - docs/superpowers/specs/rust-contract-inventory.md
  - rust/src/draft_store.rs
  - rust/src/task_store.rs
parent_task_id: BACK-802
type: task
ordinal: 460000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Drafts can be listed, created, viewed, and edited in the Rust CLI, but lifecycle transitions still require the TypeScript executable. The Rust stores already implement draft archive and promotion plus task demotion; expose those operations through the canonical CLI so users can complete the draft lifecycle in Rust.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust exposes draft archive and promote commands with TypeScript-compatible output, exit behavior, and identity resolution.
- [x] #2 Rust exposes task demote with TypeScript-compatible output, dependency/reference cleanup, and safe handling of missing or archived tasks.
- [x] #3 Lifecycle mutations preserve record contents and correctly commit all changed paths when auto_commit is enabled.
- [x] #4 Isolated Rust process tests compare supported outputs, errors, and resulting files against TypeScript.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Compare TypeScript draft archive/promote and task demote behavior with the existing Rust DraftStore, TaskStore, and selected-path Git commit helpers.
2. Add Rust CLI commands that call those stores, preserve TypeScript identity and error semantics, and commit every changed record path when configured.
3. Add isolated process parity tests for successful transitions, missing/ambiguous/archived cases, link cleanup, byte preservation, and auto-commit scope.
4. Run focused and full Rust/Bun checks, review the diff, and finalize the task.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added Rust draft archive and promotion commands plus task demotion. TypeScript-parity tests cover output, missing/ambiguous/drifted identities, archived tasks, preserved record content, dependency/reference cleanup, and exact auto-commit scope. Full serial Rust suite passed (64 CLI process tests plus all unit, integration, and doc tests); three scoped Bun suites passed (60 tests); bun run check ., cargo fmt --check, Clippy with -D warnings, cargo build, and git diff --check passed. Corrected an existing fixture assertion that compared commit hashes made at different times; it now checks each fixture remains on the Baseline commit. Cold simplicity and complexity reviews found no blockers. Groma remains unavailable in this environment.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Exposed draft archive/promote and task demote through the Rust CLI, returning the exact changed paths needed for selected-path auto-commit. Verified TypeScript parity, record/link preservation, identity refusals, and full Rust/Bun checks.
<!-- SECTION:FINAL_SUMMARY:END -->
