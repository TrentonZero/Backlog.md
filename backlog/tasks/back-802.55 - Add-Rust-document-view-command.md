---
id: BACK-802.55
title: Add Rust document view command
status: In Progress
assignee:
  - '@codex'
created_date: '2026-11-12T15:05:00Z'
updated_date: '2026-11-12T15:12:00Z'
labels: []
dependencies:
  - BACK-802.54
references:
  - src/cli.ts
  - src/test/cli-doc-view.test.ts
  - src/test/cli-doc-archive.test.ts
  - rust/src/document_store.rs
  - docs/superpowers/specs/rust-contract-inventory.md
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
- [ ] #1 Rust doc view resolves numeric and prefixed IDs and prints the same stored Markdown detail as TypeScript for active and archived documents.
- [ ] #2 Missing and ambiguous document IDs match TypeScript behavior and fail without guessing.
- [ ] #3 Isolated process tests compare output, errors, and archived notices with TypeScript.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [ ] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Compare Rust DocumentStore and TypeScript doc view behavior, then implement active and archived reads with isolated process parity. Run focused and full Rust checks, review, finalize, and continue to the next Rust CLI task.
<!-- SECTION:PLAN:END -->
