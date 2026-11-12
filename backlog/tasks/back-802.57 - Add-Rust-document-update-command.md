---
id: BACK-802.57
title: Add Rust document update command
status: Done
assignee:
  - '@codex'
created_date: '2026-11-12T15:05:00Z'
updated_date: '2026-11-12T15:57:00Z'
labels: []
dependencies:
  - BACK-802.56
references:
  - src/cli.ts
  - src/test/cli-doc-decision-board.test.ts
  - rust/src/document_store.rs
  - rust/tests/document_store.rs
  - rust/tests/document_lifecycle.rs
modified_files:
  - rust/tests/cli.rs
  - rust/src/document_store.rs
  - rust/src/cli.rs
  - rust/src/serialization.rs
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
- [x] #1 Rust doc update supports the TypeScript fields and preserves omitted metadata and unmanaged Markdown.
- [x] #2 Title, type, path, tags and content validation match TypeScript, including archive refusals and no-write failures.
- [x] #3 Isolated process tests compare output, errors, and resulting document files with TypeScript.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Compare TypeScript doc update options, validation, path moves, source-preserving serialization, archive refusals and auto-commit behavior with the existing Rust DocumentStore. Add isolated Rust/TypeScript process parity for success, omitted fields, content preservation, validation and no-write failures. Run focused/full checks, review and finalize, then continue to document lifecycle.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented Rust `doc update` for title, Markdown content, type, tags and docs-relative moves. The path-aware DocumentStore update preserves omitted fields and unmanaged frontmatter/body. Process parity covers full metadata/content replacement, omitted-field preservation, tag deduplication, title/type/path failures, missing and archived IDs, no-write refusals, and move output/content. TDD caught and fixed two parity defects: updated_date placement/newline in serialized Markdown, and duplicate tags (now deduplicated first-seen, matching TypeScript). An auto-commit index-lock regression demonstrated TypeScript keeps an applied update after commit failure while the create helper deleted it; update commits now use the update-safe selected-path commit path and retain the file. Final checks passed: focused process parity; full `cargo fmt --check`, Clippy with warnings denied, and full Cargo suite; scoped Bun document CLI suites (57 tests / 343 assertions); `bun run check`; and `git diff --check`. No TypeScript source changed. Task remains In Progress.

Review follow-up added an auto-commit/index-lock regression: TypeScript retains the updated document while commit fails. Rust update now bypasses the creation rollback helper and preserves the applied file; an update-specific error adapter makes this single-path stage failure stderr match TypeScript exactly. Duplicate-tag parity is covered (`ops,ops` becomes one `ops`). Re-ran final gates after these changes: full fmt/Clippy/Cargo suite (70 CLI integration tests and all suites pass), focused document process parity including commit-failure stderr, scoped Bun docs suites (57 tests / 343 assertions), Biome (481 files checked), and `git diff --check`; all pass. Task remains In Progress.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Implemented Rust doc update for title, content, type, tags, and path moves while preserving omitted fields and unmanaged Markdown. Process parity covers validation/refusals, missing and archived records, duplicate-tag normalization, and auto-commit failure retention with matching stderr. Full Cargo suite, rustfmt, Clippy, Biome, scoped Bun document suites (61 tests / 355 assertions), and diff check passed; cold and full-context reviews cleared the findings. No TypeScript source changed, so the conditional type check was not applicable.
<!-- SECTION:FINAL_SUMMARY:END -->
