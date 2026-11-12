---
id: BACK-802.58
title: Add Rust document lifecycle CLI commands
status: In Progress
assignee:
  - '@codex'
created_date: '2026-11-12T15:05:00Z'
updated_date: '2026-11-12T15:57:00Z'
labels: []
dependencies:
  - BACK-802.57
references:
  - src/cli.ts
  - src/test/cli-doc-archive.test.ts
  - rust/src/document_store.rs
  - rust/tests/document_lifecycle.rs
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
- [ ] #1 Rust exposes doc archive, restore, and delete with TypeScript-compatible output and state errors.
- [ ] #2 Lifecycle commands preserve source and symlink behavior, refuse collisions or ambiguous IDs, and report documentation backlinks as TypeScript does.
- [ ] #3 Isolated process tests compare results, errors, and changed files with TypeScript.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [ ] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Compare TypeScript archive, restore, and delete CLI outputs/errors, linked-documentation reporting, symlink handling, and selected-path auto-commit with the existing DocumentStore lifecycle APIs. Add process parity for state/identity refusals, backlinks, byte-preserving moves, deletion, and Git failures; keep task/draft references unchanged. Run focused/full checks, review, finalize, and continue into decision commands.
<!-- SECTION:PLAN:END -->
