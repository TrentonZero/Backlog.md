---
id: BACK-802.47
title: Add Rust task edit for acceptance criteria and DoD
status: Done
assignee:
  - '@codex'
created_date: '2026-11-12T02:41:00Z'
updated_date: '2026-11-12T02:52:00Z'
labels: []
dependencies:
  - BACK-802.46
references:
  - src/cli.ts
  - rust/src/sections.rs
  - src/test/cli-task-view-edit.test.ts
  - src/test/definition-of-done.test.ts
parent_task_id: BACK-802
priority: medium
type: task
ordinal: 455000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Rust task edit now covers fields and links, but users also need to maintain the checklist sections that define acceptance and completion. Port the task CLI operations for Acceptance Criteria and Definition of Done so indexed edits keep the Markdown checklist rules and checked state already provided by the Rust section module.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust task edit help documents acceptance-criteria replacement/addition, indexed check/uncheck/remove operations, clear behavior, and Definition of Done operations.
- [x] #2 Acceptance Criteria and Definition of Done additions and indexed edits preserve sequential indices, checked marks, and surrounding Markdown according to the existing section rules.
- [x] #3 Acceptance-criteria replacement preserves checked state for matching trimmed text and commas within criteria; clear and replacement conflicts match TypeScript.
- [x] #4 Invalid indices, unsupported checklist marks, and ambiguous legacy sections fail compatibly without modifying task files.
- [x] #5 Rust process parity covers successful checklist mutations, aliases/repeated options, help, refusal output, and source preservation.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Compare TypeScript task edit checklist parsing, conflict validation, indexed updates, and existing CLI tests with Rust TaskRecord and sections APIs.
2. Add Clap options for AC and DoD additions, replacement, clear, and indexed check/uncheck/remove operations supported by the canonical CLI.
3. Write process parity tests against live TypeScript for index semantics, checked-state retention, conflict handling, and no-write refusals.
4. Apply checklist mutations through sections helpers and persist with TaskStore source-preserving updates.
5. Update Rust help/README, run focused and full checks, obtain required reviews, and finalize.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented Rust task edit for acceptance criteria and Definition of Done. Added replacement/add/clear/check/uncheck/remove options, conflict and index validation, checked-state preservation, sequential reindexing, and source-preserving TaskStore updates. Indexed edits validate the original checklist body before mutations, including no-op edits.

Verification: Rust full suite passed (33 CLI tests plus all library/integration tests); scoped TypeScript suite passed (49 tests across task edit, DoD, and malformed checklist marks); cargo fmt --check, Clippy with -D warnings, cargo build, bun run check . and git diff --check passed. Cold simplicity and full-context complexity reviews found no remaining blockers. Groma instructions were unavailable (`groma: command not found`).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Ported task acceptance criteria and Definition of Done editing to the Rust CLI, including replacement, additions, clear, indexed checks/removals, checked-state retention, and safe refusal for invalid or ambiguous checklists. Verified TypeScript process parity, source preservation, no-write failures, the full Rust suite, focused TypeScript regressions, formatting, Clippy, build, and Biome checks.
<!-- SECTION:FINAL_SUMMARY:END -->
