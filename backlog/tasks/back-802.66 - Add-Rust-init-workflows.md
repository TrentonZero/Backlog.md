---
id: BACK-802.66
title: Add Rust init workflows
status: To Do
assignee: []
created_date: '2026-11-12T15:23:00Z'
updated_date: '2026-11-12T15:23:00Z'
labels: []
dependencies:
  - BACK-802.61
references:
  - src/cli.ts
  - src/test/cli-init-create.test.ts
  - src/test/cli-init-no-git.test.ts
  - src/test/cli-init-input.test.ts
  - src/test/enhanced-init.test.ts
  - docs/superpowers/specs/rust-contract-inventory.md
parent_task_id: BACK-802
ordinal: 474000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The native executable needs the established setup path so a new project can create its backlog with the same defaults and choices as the current CLI, including projects without Git.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `init [projectName]` supports the documented interactive and defaults/noninteractive workflows, repeat initialization, and existing-directory behavior.
- [ ] #2 Initialization creates the configured backlog structure and initial configuration/instructions with TypeScript-compatible defaults and project-name handling.
- [ ] #3 Optional Git setup, agent instruction selection, editor selection, and completion choices preserve the supported prompts and results.
- [ ] #4 Process tests cover default setup, no-Git setup, repeat initialization, prompt choices, and created files.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [ ] #3 bun test (or scoped test) passes
<!-- DOD:END -->
