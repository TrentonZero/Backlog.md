---
id: BACK-802.50
title: Port selected-path Git commits to Rust
status: To Do
assignee:
  - '@codex'
created_date: '2026-11-12T03:17:00Z'
labels: []
dependencies:
  - BACK-802.49
references:
  - src/git/operations.ts
  - docs/superpowers/specs/rust-contract-inventory.md
  - docs/superpowers/specs/2026-11-07-rust-migration-design.md
parent_task_id: BACK-802
type: task
ordinal: 458000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Configured auto_commit is part of the shipped record-write behavior, but the Rust Git module only reads repository metadata. Rust writes need a shared Git commit operation that commits only the Backlog files it owns and preserves the user's other staged work, hooks, signing, and failure state.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The Rust Git adapter commits only the requested project files and preserves unrelated staged entries.
- [ ] #2 Commit signing, hook bypass, configured Git behavior, filesystem-only mode, and non-repository behavior match the supported TypeScript Git contract.
- [ ] #3 Commit failures and partial mutation outcomes preserve and report file and index state without consuming unrelated staged work.
- [ ] #4 Isolated Rust process and Git tests compare selected-path commit results, messages, index state, and refusals against TypeScript.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [ ] #3 bun test (or scoped test) passes
<!-- DOD:END -->
