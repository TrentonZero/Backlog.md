---
id: BACK-802.61
title: Add Rust decision archive, restore, and delete commands
status: To Do
assignee:
  - '@codex'
created_date: '2026-11-12T15:16:00Z'
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
- [ ] #1 decision archive and restore move the decision file byte-for-byte under the matching archive path, keep its ID reserved, refuse path collisions, and print TypeScript-compatible confirmations and linked task/draft reports.
- [ ] #2 Archived decisions remain readable and read-only; archive and restore refuse missing, ambiguous, or wrong-state identities without changing unrelated records.
- [ ] #3 decision delete requires an archived decision, asks no prompt, removes a symlink as a link, reports matching task and draft documentation references without changing them, and releases the ID only when no other file or committed branch claims it.
- [ ] #4 Lifecycle operations preserve configured auto-commit behavior and commit only their selected decision paths.
- [ ] #5 Isolated Rust process tests compare exit codes, output, errors, and filesystem changes with TypeScript, including collisions, backlinks, and symlink behavior.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [ ] #3 bun test (or scoped test) passes
<!-- DOD:END -->
