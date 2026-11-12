---
id: BACK-802.56
title: Add Rust document create command
status: Done
assignee:
  - '@codex'
created_date: '2026-11-12T15:05:00Z'
updated_date: '2026-11-12T15:34:00Z'
labels: []
dependencies:
  - BACK-802.55
references:
  - src/cli.ts
  - src/test/cli-doc-decision-board.test.ts
  - src/types/index.ts
  - rust/src/document_store.rs
  - rust/tests/document_store.rs
modified_files:
  - rust/tests/cli.rs
  - rust/src/document_store.rs
  - rust/src/cli.rs
  - rust/src/serialization.rs
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
- [x] #1 Rust doc create accepts title, optional path and type values, and assigns the same default document type and ID format as TypeScript.
- [x] #2 Invalid titles, types, absolute paths, and traversal paths match TypeScript refusal behavior without creating a file.
- [x] #3 Successful creation output and resulting Markdown match TypeScript through isolated process parity tests.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Compare TypeScript doc create parsing/output and DocumentStore allocation/validation, then implement matching CLI arguments and isolated process parity for success and refusal cases. Run full Rust and scoped Bun checks, review, finalize, and continue to document update.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented Rust `doc create` with TypeScript-compatible title/type/path validation, defaults, output, and Markdown serialization. TDD process parity was red before implementation (Rust rejected the missing command while TypeScript succeeded); it passed after the implementation and empty-body serialization adjustment. Verified the focused Rust process parity test, full Cargo test suite, cargo fmt check, Clippy with warnings denied, scoped Bun document CLI suite (20 tests / 86 assertions), and git diff --check. No TypeScript source changed. Task remains In Progress for orchestrator review.

Follow-up parity review added `doc create EmptyType --type ""`; the new process case failed before the fix (TypeScript exit 0, Rust exit 1). Rust now treats an explicitly empty document type as omitted and defaults to `other`. The focused parity test, full Cargo suite, fmt check, Clippy with warnings denied, scoped Bun document CLI suite (20 tests / 86 assertions), and `git diff --check` all pass after the fix. Task remains In Progress.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Implemented Rust doc create with TypeScript-compatible title, type and path behavior, default ID/type, output and Markdown serialization. Empty --type defaults to other as in TypeScript. Full Cargo suite, rustfmt, Clippy, scoped Bun document CLI tests (20/86 assertions), and isolated process parity passed. Cold and full-context reviews cleared the fix. No TypeScript source changed, so the conditional type check was not applicable.
<!-- SECTION:FINAL_SUMMARY:END -->
