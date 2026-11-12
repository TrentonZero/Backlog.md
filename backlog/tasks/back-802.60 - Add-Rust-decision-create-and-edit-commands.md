---
id: BACK-802.60
title: Add Rust decision create and edit commands
status: Done
assignee:
  - '@codex'
created_date: '2026-11-12T15:16:00Z'
updated_date: '2026-11-12T16:49:00Z'
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
- [x] #1 decision create requires a non-empty title, defaults status to proposed, accepts free-form status and the context, decision, consequences, and alternatives sections, and prints the created decision ID.
- [x] #2 Create section flags preserve real newlines, leave omitted sections empty, accept --plain, and refuse reserved section markers without creating a record.
- [x] #3 decision edit requires at least one field; omitted fields remain unchanged, section values replace their sections, and empty section values clear them.
- [x] #4 Edit supports title, free-form status, and superseded-by: a replacement must resolve to an existing different decision, an empty value clears the link, and changing the link does not implicitly change status.
- [x] #5 Edits preserve unknown frontmatter and unmanaged content, retain updated_date on no-op updates, and refuse missing, ambiguous, or archived identities without writing.
- [x] #6 Isolated Rust process tests compare create and edit output, errors, and resulting files with the TypeScript CLI.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Compare decision create/edit behavior, output, serialization, and refusal paths with the TypeScript CLI and DecisionStore. Add isolated Rust process parity coverage for defaults, sections, supersession links, partial/no-op edits, identity and archive errors, source preservation, and auto-commit where documented. Implement using the existing DecisionStore and shared commit semantics; run full Rust checks and focused TypeScript parity suites, review, and finalize before continuing to decision lifecycle.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented native decision create/edit routes over DecisionStore. Isolated process parity covers default and free-form status, multiline/omitted sections, plain output, reserved-marker refusal, partial section replacement and clearing, title rename, supersession validation and clearing without status changes, unknown metadata and unmanaged body preservation, identical-value no-op timestamp stability, and missing/ambiguous/archived/error paths without mutation. Focused `rust_decision_*` process tests: 5 passed; full locked Cargo suite passed (80 CLI integration tests plus all Rust suites); cargo fmt and locked Clippy passed; scoped Bun suites passed (48 tests, 339 assertions); Biome checked 481 files; git diff check passed. TypeScript was unchanged, so the conditional type check did not apply.

Cold simplicity and full-context complexity reviews found no blockers. The no-op edit still passes the selected path to shared commit handling, matching the TypeScript mutation route while preserving the timestamp and source.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added Rust decision create/edit commands on DecisionStore with TypeScript-parity defaults, section handling, validation, source preservation, supersession links, identity refusals, output, and selected-path commit behavior. Verified locked Cargo suite (80 CLI process tests included), formatting, Clippy, 48 scoped Bun tests, Biome, and diff checks. Both independent reviews cleared the implementation. TypeScript was not changed, so the conditional type check did not apply.
<!-- SECTION:FINAL_SUMMARY:END -->
