---
id: BACK-802.60
title: Add Rust decision create and edit commands
status: To Do
assignee:
  - '@codex'
created_date: '2026-11-12T15:16:00Z'
labels: []
dependencies:
  - BACK-802.59
references:
  - src/cli.ts
  - src/test/cli-decision-content.test.ts
  - src/test/cli-decision-status.test.ts
  - rust/src/decision_store.rs
  - rust/tests/decision_store.rs
  - rust/tests/serialization.rs
parent_task_id: BACK-802
type: task
ordinal: 468000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
DecisionStore already creates and updates decisions safely, but the native executable does not expose the authoring workflow. Port the existing section-based CLI so people can record choices and revise them without losing unmanaged Markdown or metadata.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 decision create requires a non-empty title, defaults status to proposed, accepts free-form status and the context, decision, consequences, and alternatives sections, and prints the created decision ID.
- [ ] #2 Create section flags preserve real newlines, leave omitted sections empty, accept --plain, and refuse reserved section markers without creating a record.
- [ ] #3 decision edit requires at least one field; omitted fields remain unchanged, section values replace their sections, and empty section values clear them.
- [ ] #4 Edit supports title, free-form status, and superseded-by: a replacement must resolve to an existing different decision, an empty value clears the link, and changing the link does not implicitly change status.
- [ ] #5 Edits preserve unknown frontmatter and unmanaged content, retain updated_date on no-op updates, and refuse missing, ambiguous, or archived identities without writing.
- [ ] #6 Isolated Rust process tests compare create and edit output, errors, and resulting files with the TypeScript CLI.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [ ] #3 bun test (or scoped test) passes
<!-- DOD:END -->
