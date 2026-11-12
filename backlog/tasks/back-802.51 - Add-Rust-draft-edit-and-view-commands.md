---
id: BACK-802.51
title: Add Rust draft edit and view commands
status: Done
assignee:
  - '@codex'
created_date: '2026-11-12T03:58:00Z'
updated_date: '2026-11-12T04:23:00Z'
labels: []
dependencies:
  - BACK-802.50
references:
  - src/cli.ts
  - src/test/cli-draft-edit.test.ts
  - docs/superpowers/specs/rust-contract-inventory.md
parent_task_id: BACK-802
type: task
ordinal: 459000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Draft records can already be created and listed through the Rust CLI, while people still need the TypeScript executable to inspect or revise a draft. Complete these ordinary draft workflows through the same Rust stores so the command path can stay on the shared Rust model.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust exposes draft view and the documented draft ID shorthand with plain output, correct identity resolution, and missing or ambiguous behavior.
- [x] #2 Rust draft edit supports the fields and section operations exposed by the TypeScript draft edit command, keeps status Draft, and preserves unknown metadata and unmanaged Markdown.
- [x] #3 Edit validation and persistence match TypeScript behavior, including refusal cases that leave draft bytes unchanged.
- [x] #4 Isolated Rust process tests compare supported output, errors, and resulting draft files against TypeScript.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Compare TypeScript draft edit/view behavior and fixtures with Rust TaskEditArgs, TaskViewArgs, DraftStore lookup, update, and serialization.
2. Add Rust CLI draft view and edit using the shared draft store and task validation/output helpers.
3. Add isolated process parity tests for plain views, edits, identity errors, refusals, and source preservation.
4. Run focused and full Rust/Bun checks, reviews, and finalize before pushing to main.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented Rust draft view, ID shorthand, and draft edit through the shared record mutation flow. Verified isolated process parity and preservation/refusal behavior. Verification: full serial Rust suite passed (60 CLI process tests plus unit, integration, and doc tests); TypeScript cli-draft-edit suite passed (31 passed, 1 skipped); bun run check . passed; cargo fmt --check, clippy with -D warnings, cargo build, and git diff --check passed. Cold simplicity and full-context complexity reviews found no blockers. Groma architecture instructions could not be run because groma is unavailable.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added Rust draft view and edit commands with the documented ID shorthand, TypeScript-compatible behavior, and isolated parity coverage. Full Rust and scoped TypeScript suites and required formatting, lint, build, and diff checks passed.
<!-- SECTION:FINAL_SUMMARY:END -->
