---
id: BACK-802.69
title: Add Rust workflow instructions and shell completion commands
status: To Do
assignee: []
created_date: '2026-11-12T15:25:00Z'
labels: []
dependencies:
  - BACK-802.68
references:
  - docs/superpowers/specs/rust-contract-inventory.md
  - docs/superpowers/specs/2026-11-07-rust-migration-design.md
  - src/commands/instructions.ts
  - src/commands/completion.ts
  - src/commands/completion.test.ts
parent_task_id: BACK-802
ordinal: 477000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
External users rely on the CLI to explain Backlog workflows and install shell completion. Restore these built-in guidance surfaces so the native executable remains usable without source-tree context.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `instructions [guide]` and `instructions --list` expose the overview, task-creation, task-execution, task-finalization, and init-required guidance with configured task-ID examples.
- [ ] #2 `completion install` supports bash, zsh, fish, and PowerShell installation behavior; hidden `__complete` preserves the existing completion protocol.
- [ ] #3 Commands preserve documented stdout/stderr, exit status, shell configuration changes, and no unintended diagnostic output.
- [ ] #4 Process tests cover guide selection/listing, completion installation by shell, and completion protocol cases; satisfy the CLI contract inventory gate before proceeding to later cutover.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [ ] #3 bun test (or scoped test) passes
<!-- DOD:END -->
