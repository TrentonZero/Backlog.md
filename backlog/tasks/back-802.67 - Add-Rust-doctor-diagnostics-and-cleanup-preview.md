---
id: BACK-802.67
title: Add Rust doctor diagnostics and cleanup preview
status: To Do
assignee: []
created_date: '2026-11-12T15:23:00Z'
updated_date: '2026-11-12T15:24:00Z'
labels: []
dependencies:
  - BACK-802.61
references:
  - src/cli.ts
  - src/test/cli-doctor.test.ts
  - src/test/cli-cleanup-input.test.ts
  - src/test/duplicate-task-repair.test.ts
  - docs/superpowers/specs/rust-contract-inventory.md
parent_task_id: BACK-802
ordinal: 475000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Users need a reliable way to inspect backlog health before choosing a repair. Restore the doctor diagnostics and read-only cleanup preview in the Rust executable, preserving the current evidence and refusal behavior.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `doctor` reports the supported backlog diagnostics with actionable findings and exit behavior matching the TypeScript CLI.
- [ ] #2 `cleanup` produces the existing read-only duplicate/stale-record preview and requires explicit user review before any mutation.
- [ ] #3 The cleanup preview exposes the fingerprint/evidence needed for a later reviewed repair without mutating records.
- [ ] #4 Process tests cover clean and problematic diagnostics, preview-only behavior, noninteractive handling, and stable evidence generation.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [ ] #3 bun test (or scoped test) passes
<!-- DOD:END -->
