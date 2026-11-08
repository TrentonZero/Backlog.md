---
id: BACK-802.5
title: Port typed Markdown record parsers to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-08T19:37:00Z'
updated_date: '2026-11-08T19:52:00Z'
labels: []
dependencies:
  - BACK-802.1
  - BACK-802.3
references:
  - src/markdown/parser.ts
  - src/types/index.ts
  - src/utils/due-date.ts
  - src/utils/priority-config.ts
  - rust/src/frontmatter.rs
  - rust/src/sections.rs
  - docs/superpowers/plans/2026-11-08-rust-records.md
modified_files:
  - docs/superpowers/plans/2026-11-08-rust-records.md
  - rust/tests/records.rs
  - rust/src/records.rs
  - rust/src/lib.rs
parent_task_id: BACK-802
type: task
ordinal: 413000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add native typed parsing for tasks, decisions, documents and milestones using the Rust frontmatter and managed-sections components. The public parser contract takes Markdown text and returns human-readable typed records or actionable parse errors. Keep disk paths, storage, CLI behavior and serialization in later tasks.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Task parsing preserves current required/optional values, coercions, date spellings, priority normalization, section/checklist/comment extraction, and dependency-specific refusal behavior.
- [x] #2 Decision, document and milestone parsers preserve current defaults, dates, section fields and raw body semantics.
- [x] #3 Typed record models and parse errors are explicit; no filesystem metadata, persistence, serializer or CLI behavior is added.
- [x] #4 Source-mapped Rust tests cover required parser branches and actual-TypeScript comparisons; Cargo tests, formatting and Clippy pass.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Define small native record structs and parse error types matching the current domain-owned parser output. 2. Add failing parser tests, then implement Markdown/frontmatter date handling and task dependency/due-date rules. 3. Implement decision, document and milestone parsing through existing section APIs. 4. Compare observable records and refusals with the TypeScript reference; run full Cargo checks and review.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Native typed record parsers now cover tasks, decisions, documents and milestones. Full locked Cargo tests, fmt check, Clippy, and focused Bun parser suites pass. Cold and full-context reviews found parity issues; all identified issues are fixed and scoped review is pending.

Cold review and scoped full-context complexity re-review approved. Final verification: cargo fmt --check, locked Clippy, locked Cargo tests, 72 focused Bun tests with 271 assertions, and git diff --check passed.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added pure Rust parsers and explicit typed records for tasks, decisions, documents, and milestones, reusing the Rust frontmatter and managed-section components. Parser coercions, due-date validation, dependency refusals, and body extraction are covered by Rust tests compared against the live TypeScript parsers. Full locked Cargo tests, formatting, Clippy, and focused Bun suites pass; both required reviews approved.
<!-- SECTION:FINAL_SUMMARY:END -->
