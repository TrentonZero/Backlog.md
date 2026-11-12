---
id: BACK-802.56
title: Add Rust document create command
status: To Do
assignee:
  - '@codex'
created_date: '2026-11-12T15:05:00Z'
labels: []
dependencies:
  - BACK-802.55
references:
  - src/cli.ts
  - src/test/cli-doc-decision-board.test.ts
  - src/types/index.ts
  - rust/src/document_store.rs
  - rust/tests/document_store.rs
parent_task_id: BACK-802
type: task
ordinal: 464000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
After users can list and read documents in Rust, they need to create a Markdown document from the canonical CLI. The Rust DocumentStore already owns local paths and ID allocation; expose its supported create workflow with the same title, type, path, and output behavior as TypeScript.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Rust doc create accepts title, optional path and type values, and assigns the same default document type and ID format as TypeScript.
- [ ] #2 Invalid titles, types, absolute paths, and traversal paths match TypeScript refusal behavior without creating a file.
- [ ] #3 Successful creation output and resulting Markdown match TypeScript through isolated process parity tests.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [ ] #3 bun test (or scoped test) passes
<!-- DOD:END -->
