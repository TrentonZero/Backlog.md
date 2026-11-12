---
id: BACK-802.46
title: Add Rust task edit for dependencies, references, and docs
status: Done
assignee:
  - '@codex'
created_date: '2026-11-12T02:31:00Z'
updated_date: '2026-11-12T02:39:00Z'
labels: []
dependencies:
  - BACK-802.45
references:
  - src/cli.ts
  - rust/src/task_store.rs
  - src/test/cli-dependency.test.ts
  - src/test/cli-refs-docs.test.ts
parent_task_id: BACK-802
priority: medium
type: task
ordinal: 454000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Rust task edit now covers core scalar fields, but task relationships and attached documentation are also part of the canonical CLI workflow. Add the dependency, reference, and documentation edit options so users can maintain those links from the Rust command while preserving existing identity validation and source handling.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust task edit help documents dependency aliases and clear behavior plus reference and documentation replacement, incremental, and clear flags.
- [x] #2 Dependency edits resolve aliases, deduplicate values, reject missing/self/cyclic targets, and leave files unchanged on validation failures.
- [x] #3 Reference and documentation edits support replacement, incremental add/remove, empty-value validation, and explicit clear behavior matching TypeScript.
- [x] #4 Successful edits preserve unrelated Markdown and emit compatible default and plain output.
- [x] #5 Rust process parity covers aliases, repeated and comma-separated inputs, conflicts, clears, invalid identities, and no-write failures.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Compare TypeScript task edit parsing, validation, mutation input construction, and existing dependency/reference/documentation tests with the Rust TaskRecord and TaskStore contracts.
2. Add Clap flags for dependency replacement/clear and reference/documentation replacement, incremental changes, and clears.
3. Add live TypeScript process parity tests before implementation for successful updates and refusal/no-write cases.
4. Apply supplied fields and persist through TaskStore so dependency validation and locked source-preserving updates remain centralized.
5. Update Rust CLI help/README, run focused and full checks, complete required reviews, and finalize.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added repeatable dependency edits using --depends-on/--dep, plus reference and documentation replacement, add/remove, and clear operations. Dependency validation remains in TaskStore, and all successful changes use its locked, source-preserving update path.

Verification: full Rust test suite passed; focused TypeScript dependency and refs/docs suites passed (75 tests); rustfmt, Clippy with warnings denied, cargo build, Biome, and diff checks passed. Rust process parity covers aliases, repeated/comma-separated values, replacement/incremental/clear operations, default/plain output, invalid missing/self/cyclic dependencies, conflicts, empty values, no-write failures, and preservation of custom frontmatter and body text.

Reviews: cold simplicity and full-context complexity reviews completed with no blockers. Groma instructions were attempted but the executable is unavailable in this environment.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Extended Rust task edit with dependency, reference, and documentation list operations, preserving shared dependency validation, locked writes, and source content. Full Rust tests, 75 focused Bun tests, lint/build checks, and both required reviews passed.
<!-- SECTION:FINAL_SUMMARY:END -->
