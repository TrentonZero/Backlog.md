---
id: BACK-802.1
title: Port managed Markdown sections and checklists to Rust
status: In Progress
assignee:
  - '@codex'
created_date: '2026-11-07T23:12:00Z'
updated_date: '2026-11-08T18:52:00Z'
labels: []
dependencies:
  - BACK-801
references:
  - src/markdown/structured-sections.ts
  - src/markdown/section-titles.ts
  - src/test/acceptance-criteria-structured.test.ts
documentation:
  - docs/superpowers/plans/2026-11-07-rust-managed-sections.md
modified_files:
  - docs/superpowers/plans/2026-11-07-rust-managed-sections.md
  - rust/tests/sections.rs
  - rust/src/sections.rs
  - rust/src/lib.rs
  - rust/Cargo.toml
  - rust/Cargo.lock
  - rust/README.md
  - .superpowers/sdd/2026-11-07-rust-managed-sections/task-1-report.md
parent_task_id: BACK-802
type: task
ordinal: 409000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Rust migration needs the same managed Markdown body semantics as the shipped CLI before typed records or mutations can use native storage. Port the existing marked and legacy sections, acceptance criteria, Definition of Done and comments as one shared Rust component, retaining unmanaged content and supported refusal rules.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Rust extraction and updates match the supported marked and legacy task and decision section semantics, preserving unmanaged Markdown and fenced or nested marker-like text.
- [ ] #2 Acceptance criteria and Definition of Done preserve stable indices, check state, indentation and supported validation/refusal behavior through reads and indexed edits.
- [ ] #3 Comment parsing and updates retain bodies, authors and timestamps with the existing managed format and refuse invalid marker inputs.
- [ ] #4 Focused Rust regression and current-TypeScript comparison cases cover the supported rules; Cargo tests, formatting and Clippy pass, with no executable cutover.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [ ] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Execute docs/superpowers/plans/2026-11-07-rust-managed-sections.md with one focused implementer and task-scoped review. Port the current managed Markdown semantics, with TDD, native/reference comparisons, complete Cargo checks and no executable cutover. User authorized direct main work and waived issue/PR creation; preserve unrelated edits and push only after verification/reviews.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Clarified context requirement before dependent implementation: preserve the reference family-specific treatment of markers/headings in code and containers. The original broad wording did not authorize changing operations whose current supported behavior differs. parse_criteria will match parseAllCriteria sequential indexing used by indexed application edits.

Implemented the complete native managed-sections interface in rust/src/sections.rs with canonical task serialization, in-place decision migration, shared AC/DoD handling, comments and explicit validation APIs. Pinned markdown=1.0.0 for CommonMark heading spans; preserved reference-specific marker rules. Self-review complete. Verified 37 Cargo tests (including 5,166 live TypeScript comparisons), Cargo fmt and Clippy -D warnings; seven TypeScript reference suites pass 154 tests/929 assertions. No TypeScript/frontmatter implementation changes or executable cutover. Implementation report: .superpowers/sdd/2026-11-07-rust-managed-sections/task-1-report.md. Controller owns required separate reviews, acceptance checks, final summary, Done status and pushing.
<!-- SECTION:NOTES:END -->
