---
id: BACK-802.68
title: Add Rust fingerprint-checked cleanup repair
status: To Do
assignee: []
created_date: '2026-11-12T15:23:00Z'
updated_date: '2026-11-12T15:23:00Z'
labels: []
dependencies:
  - BACK-802.67
references:
  - src/cli.ts
  - src/test/cli-cleanup-input.test.ts
  - src/test/duplicate-task-repair.test.ts
  - docs/superpowers/specs/rust-contract-inventory.md
parent_task_id: BACK-802
ordinal: 476000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A cleanup preview can become unsafe if the backlog changes before repair. Add the explicit repair step that accepts reviewed preview evidence and refuses to mutate records when its fingerprint is stale.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The repair command accepts only the reviewed preview fingerprint/evidence and refuses missing, malformed, or stale evidence without changing files.
- [ ] #2 A valid reviewed repair performs the established duplicate cleanup and reference updates, preserving remaining records and reporting the repair result.
- [ ] #3 Process tests verify valid repair filesystem results, stale-fingerprint refusal, and no mutation on invalid or stale input.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [ ] #3 bun test (or scoped test) passes
<!-- DOD:END -->
