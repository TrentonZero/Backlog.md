---
id: BACK-802.45
title: Add Rust task lifecycle commands
status: Done
assignee:
  - '@codex'
created_date: '2026-11-12T02:18:00Z'
updated_date: '2026-11-12T02:29:00Z'
labels: []
dependencies:
  - BACK-802.44
references:
  - src/cli.ts
  - rust/src/task_store.rs
  - src/test/cli-task-state.test.ts
  - src/test/cli-task-restore.test.ts
  - src/test/cli-task-delete.test.ts
parent_task_id: BACK-802
priority: medium
type: task
ordinal: 453000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Rust task store already supports archive, restore, and archived deletion, but Rust CLI users cannot invoke the lifecycle operations documented by the canonical CLI. Add the local task lifecycle command adapters so users can move finished work off the board, restore it, and permanently remove archived work with the same identity and reference-cleanup behavior.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust task archive and restore help and confirmation output match the canonical CLI for local tasks.
- [x] #2 Archive, restore, and delete resolve unique local task identities and report missing, ambiguous, and wrong-state outcomes compatibly without unintended writes.
- [x] #3 Archived task deletion preserves existing subtask guards and cleans task and draft links with compatible reporting.
- [x] #4 Lifecycle mutations preserve task Markdown bytes across archive and restore and follow existing TaskStore locking and collision rules.
- [x] #5 Rust process parity covers help, successful lifecycle transitions, cleanup reporting, identity refusals, and no-write failures.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Compare the TypeScript archive/restore/delete command adapters, their tests, help contracts, and Rust TaskStore lifecycle methods.
2. Add Clap subcommands and help text for the local task lifecycle operations without changing storage semantics.
3. Add Rust process parity tests against the live TypeScript CLI for normal transitions, wrong states, ambiguous/missing IDs, and deletion cleanup.
4. Wire each operation through TaskStore and preserve canonical IDs, errors, exit codes, and output.
5. Update Rust README, run focused and full checks, complete cold simplicity and complexity reviews, then finalize and push.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added Rust task archive, restore, and delete CLI adapters using the existing TaskStore lifecycle operations. Archive and restore preserve task bytes, and archived deletion cleans task and draft links while retaining subtask guards. Command help documents identity, status, read-only, cleanup, and no-prompt behavior.

Verification: full Cargo test suite passed; rustfmt, Clippy with warnings denied, cargo build, Biome, and diff checks passed. Focused TypeScript lifecycle tests passed (47 tests across archive/state, restore, and delete). Rust process parity covers lifecycle transitions, help semantics, repeated/wrong-state/missing operations, ambiguous identities with no writes, dependent task/draft cleanup reporting, byte-preserving moves, and subtask guards.

Reviews: cold simplicity and full-context complexity reviews completed; all findings resolved. Groma instructions were attempted but the groma executable is unavailable in this environment.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added Rust task archive, restore, and delete commands with TypeScript-compatible confirmations, identity refusals, locked storage operations, cleanup reporting, and help. Full Rust tests and focused TypeScript lifecycle tests passed; formatting, lint, build, and both required reviews passed.
<!-- SECTION:FINAL_SUMMARY:END -->
