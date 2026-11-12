---
id: BACK-802.54
title: Add Rust document list command
status: To Do
assignee:
  - '@codex'
created_date: '2026-11-12T14:49:00Z'
labels: []
dependencies:
  - BACK-802.53
references:
  - src/cli.ts
  - src/test/cli-list-window.test.ts
  - docs/superpowers/specs/rust-contract-inventory.md
  - rust/src/document_store.rs
  - rust/tests/document_store.rs
parent_task_id: BACK-802
type: task
ordinal: 462000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Rust can already list documents through DocumentStore, but the Rust executable has no document CLI group. Expose the human-readable list command so people and scripts can browse local documents, including nested paths, without moving the interactive picker ahead of the TUI stage.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Rust doc list --plain prints document IDs and titles in the same title/path order as TypeScript, including nested documents and excluding README and archived documents.
- [ ] #2 The command matches TypeScript empty-list output and shared --max-count, --skip, and --count behavior, including continuation output and invalid-value errors.
- [ ] #3 Isolated Rust process tests compare output and errors with the TypeScript CLI.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [ ] #3 bun test (or scoped test) passes
<!-- DOD:END -->
