---
id: BACK-802.65
title: Add Rust configuration and agent instruction commands
status: To Do
assignee: []
created_date: '2026-11-12T15:23:00Z'
updated_date: '2026-11-12T15:23:00Z'
labels: []
dependencies:
  - BACK-802.61
references:
  - src/cli.ts
  - src/test/config-commands.test.ts
  - src/test/cli-agents.test.ts
  - src/test/agent-instructions.test.ts
  - rust/src/config.rs
parent_task_id: BACK-802
ordinal: 473000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Rust executable parses project configuration but cannot inspect or change it, and cannot refresh shipped agent instructions. Restore the supported configuration and instruction-maintenance CLI workflows.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `config get <key>`, `config set <key> <value>`, and `config list` expose supported camelCase keys, validate values, and preserve configuration serialization and migration behavior.
- [ ] #2 Configuration writes preserve unrelated and unknown supported data and follow the existing Git auto-commit behavior.
- [ ] #3 `agents --update-instructions` updates the same shipped instruction files and reports the outcome; ordinary `agents` behavior remains compatible.
- [ ] #4 Process tests cover get/list, valid and invalid set values, file persistence, auto-commit behavior, and instruction update results.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [ ] #3 bun test (or scoped test) passes
<!-- DOD:END -->
