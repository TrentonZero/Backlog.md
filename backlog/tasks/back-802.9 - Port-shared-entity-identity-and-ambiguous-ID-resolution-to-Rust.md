---
id: BACK-802.9
title: Port shared entity identity and ambiguous ID resolution to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-08T20:54:00Z'
updated_date: '2026-11-08T21:00:00Z'
labels: []
dependencies:
  - BACK-802.8
references:
  - task-identity
  - src/utils/entity-id.ts
  - src/utils/decision-id.ts
  - src/utils/document-id.ts
  - src/utils/task-id.ts
  - src/test/content-identity.test.ts
  - src/test/task-path.test.ts
  - src/test/cli-custom-prefix-id-resolution.test.ts
  - rust/src/identity.rs
modified_files:
  - rust/src/identity.rs
  - rust/tests/entity_identity.rs
parent_task_id: BACK-802
type: task
ordinal: 417000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add pure Rust shared identity and ambiguity resolution for prefixed records. Reuse task-ID semantics, resolve document/decision IDs with the shipped numeric alias rules, and fail closed with deterministic diagnostics when multiple files claim an ID. Keep filesystem scanning and application workflows outside this slice.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust shared entity keys and equality preserve document/decision numeric aliases, case-insensitive text IDs, and fail-closed empty IDs.
- [x] #2 Unique entity lookup returns a single match, null, or a deterministic ambiguity error naming sorted candidate files and doctor guidance.
- [x] #3 Task list lookup distinguishes invalid, not-found, found, and ambiguous results using Rust task-ID semantics.
- [x] #4 Live TypeScript comparisons cover success, no match, duplicate candidates, empty IDs, and normalization; Rust checks pass.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Read the generic entity and task lookup contracts and focused ambiguity tests. 2. Add isolated Rust tests and live TypeScript comparisons for normalized keys, unique matches, ambiguity messages, and task lookup result states. 3. Implement a shared pure entity resolver and small decision/document wrappers, reusing the task ID functions from BACK-802.8. 4. Run locked Cargo formatting, Clippy, tests, and required reviews.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Review ruling: Keep deterministic Rust lexical path ordering for this slice. The accepted requirement says sorted candidates; localeCompare order depends on ICU locale/runtime behavior, and adding ICU collation would add a large dependency without a cross-platform locale contract. Cost if wrong: mixed-case or punctuation-heavy paths can appear in a different candidate order than the current TypeScript CLI.

Verification: cargo fmt --check, locked all-target Clippy with warnings denied, and full locked Cargo tests passed. Focused Bun contract tests passed: content-identity, task-path, cli-custom-prefix-id-resolution, and web-ambiguous-id (97 tests). Live Bun comparisons run from the Rust integration test passed.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Implemented pure Rust document/decision identity helpers, fail-closed unique lookup with sorted ambiguity diagnostics, and task list resolution for found, ambiguous, invalid, and not-found IDs.

Verified with live Bun comparisons, 97 focused TypeScript tests, rustfmt, locked Clippy, and the full locked Rust test suite. Diagnostic path ordering uses deterministic Rust lexical order; mixed-case or punctuation-heavy paths can order differently from TypeScript localeCompare.
<!-- SECTION:FINAL_SUMMARY:END -->
