---
id: BACK-802.59
title: Add Rust decision list and view commands
status: To Do
assignee:
  - '@codex'
created_date: '2026-11-12T15:16:00Z'
labels: []
dependencies:
  - BACK-802.58
references:
  - src/cli.ts
  - src/formatters/decision-plain-text.ts
  - src/formatters/json-output.ts
  - src/test/cli-doc-decision-board.test.ts
  - src/test/cli-decision-content.test.ts
  - src/test/cli-json-output.test.ts
  - rust/src/decision_store.rs
  - rust/tests/decision_store.rs
parent_task_id: BACK-802
type: task
ordinal: 467000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The native executable can already read decision records, but users still need the TypeScript CLI to browse the active decision log or inspect an active and archived decision. Port the documented read paths so humans and scripts can review the same decision record through Rust.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 decision list prints active decisions in decision ID order as ID, title, status, and any superseded-by link; an empty list prints the documented empty-list message.
- [ ] #2 decision list supports plain output, versioned JSON with schemaVersion 1 and kind decision-list, and shared max-count, skip, count, and continuation behavior; count cannot be combined with JSON.
- [ ] #3 decision view resolves numeric and prefixed IDs for active or archived decisions and prints the documented plain metadata and content, including the archived read-only notice.
- [ ] #4 decision view --json emits the versioned decision-view envelope with curated metadata, updatedAt, archived, and content; missing, ambiguous, and conflicting output-mode inputs fail like TypeScript.
- [ ] #5 Isolated Rust process tests compare list and view output, exit codes, and errors with the TypeScript CLI.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [ ] #3 bun test (or scoped test) passes
<!-- DOD:END -->
