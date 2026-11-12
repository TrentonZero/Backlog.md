---
id: BACK-802.42
title: Add the Rust task detail view command
status: Done
assignee:
  - '@codex'
created_date: '2026-11-12T01:17:00Z'
updated_date: '2026-11-12T01:36:00Z'
labels: []
dependencies:
  - BACK-802.41
references:
  - src/cli.ts
  - src/formatters/json-output.ts
  - src/formatters/task-plain-text.ts
  - src/core/task-detail.ts
  - src/test/cli-json-output.test.ts
parent_task_id: BACK-802
type: task
ordinal: 450000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Rust executable can list tasks, but humans and external agents still need a direct way to read one task’s stored details and derived dependency context, including archived history. Add the plain and versioned JSON detail path available in the current CLI.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust task view help documents explicit plain and JSON modes.
- [x] #2 Local active and archived tasks resolve by supported IDs; missing and ambiguous identities report compatible errors and exit status.
- [x] #3 JSON output matches the versioned task-view schema, including detail fields, dependency graph, and readiness.
- [x] #4 Plain output preserves the current task detail sections and dependency graph for human readers.
- [x] #5 Live TypeScript parity covers active and archived records, sparse and populated details, dependencies, checklists/comments, aliases, errors, stdout/stderr, and exit status.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Compare the task view command, TaskDetail loader, JSON formatter, plain formatter, and current TypeScript coverage with Rust TaskStore and detail models. 2. Add explicit plain and JSON Rust task view modes using the shared identity, detail, dependency, and readiness behavior. 3. Preserve the versioned task-view fields and human detail sections. 4. Add isolated process parity for task records and missing/ambiguous IDs. 5. Run focused and full Rust checks, relevant Bun CLI tests, required reviews, and finalize.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented Rust `task view` with explicit plain/JSON modes, active and archived ID lookup, compatible missing/ambiguous errors, detail serialization, readiness and dependency graph output. Added live TypeScript parity coverage for sparse/populated records, aliases, comments/checklists, archived tasks, errors, and streams/status. Updated the Rust README.

Verification: Rust fmt check, Clippy with warnings denied, full Rust tests (8 integration tests), Cargo build, focused Bun CLI/dependency tests (55 passed), `bun run check .`, and `git diff --check` passed. Both cold simplicity and full-context complexity reviews completed; the only finding, undated comment timestamps serialized as empty strings instead of null, was fixed and re-reviewed. `groma agent-instructions` could not run because Groma is not installed in this environment.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Rust now provides a task detail command with parity for the current TypeScript CLI across active and archived tasks, plain and versioned JSON output, detail fields, readiness, dependency graphs, and error behavior. Verification and reviews passed. The Rust CLI currently supports task list and task view; remaining commands and surfaces have not yet been ported or cut over.
<!-- SECTION:FINAL_SUMMARY:END -->
