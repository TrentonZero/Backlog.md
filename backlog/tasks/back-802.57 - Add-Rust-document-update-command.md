---
id: BACK-802.57
title: Add Rust document update command
status: To Do
assignee:
  - '@codex'
created_date: '2026-11-12T15:05:00Z'
labels: []
dependencies:
  - BACK-802.56
references:
  - src/cli.ts
  - src/test/cli-doc-decision-board.test.ts
  - rust/src/document_store.rs
  - rust/tests/document_store.rs
  - rust/tests/document_lifecycle.rs
parent_task_id: BACK-802
type: task
ordinal: 465000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Rust DocumentStore can update a document while preserving unmanaged content, but the CLI cannot yet make those edits. Expose the supported TypeScript update workflow so users can change document metadata or content from the canonical Rust CLI.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Rust doc update supports the TypeScript fields and preserves omitted metadata and unmanaged Markdown.
- [ ] #2 Title, type, path, tags and content validation match TypeScript, including archive refusals and no-write failures.
- [ ] #3 Isolated process tests compare output, errors, and resulting document files with TypeScript.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [ ] #3 bun test (or scoped test) passes
<!-- DOD:END -->
