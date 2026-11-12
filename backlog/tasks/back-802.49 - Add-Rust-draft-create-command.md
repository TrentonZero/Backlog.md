---
id: BACK-802.49
title: Add Rust draft create command
status: Done
assignee:
  - '@codex'
created_date: '2026-11-12T03:08:00Z'
updated_date: '2026-11-12T03:19:00Z'
labels: []
dependencies:
  - BACK-802.48
references:
  - src/cli.ts
  - rust/src/draft_store.rs
  - src/test/cli-missing-storage-directories.test.ts
  - BACK-802.50
parent_task_id: BACK-802
type: task
ordinal: 457000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Rust draft store can persist drafts, but the Rust executable has no command for users to create one. Port the existing draft creation entry point so this workflow uses the native CLI and shared storage.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust draft create accepts a title, description alias, repeatable or comma-separated assignees and labels, and the status option with TypeScript-compatible results.
- [x] #2 Creation allocates the next draft ID through DraftStore, writes status Draft, applies configured defaults only when options are omitted, and creates missing storage directories.
- [x] #3 Explicit empty assignees remain unassigned; omitted assignees and Definition of Done use configured defaults like TypeScript.
- [x] #4 With auto_commit unset or false, success and refusal output, resulting Markdown, and no-write failures match TypeScript in process parity tests.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Compare TypeScript draft create options, default assignee and DoD behavior, output, and storage requirements with Rust task creation and DraftStore APIs.
2. Add Rust draft create arguments and construct a Draft task using the shared DraftStore allocator and writer.
3. Add process parity tests for repeatable and comma-separated values, aliases, configured defaults, explicit clears, missing storage, invalid titles, output, and resulting Markdown.
4. Update Rust CLI documentation, run Cargo and scoped Bun checks, obtain required reviews, and finalize.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Review found that configured auto_commit requires a shared selected-path Git commit implementation to preserve unrelated staged files, hooks, signing, and failure state. That cross-writer Git contract is tracked separately in BACK-802.50; this draft-create slice validates with auto_commit unset/false.

Added Rust draft create with TypeScript-compatible title, description aliases, repeatable/comma-separated assignees and labels, accepted status option, configured defaults, explicit empty assignees, DraftStore allocation, directory creation, and refusal handling. Process parity tests cover output and Markdown as well as defaults, aliases, missing storage, and no-write failures.

Verification: full Cargo test suite passed (36 CLI process tests plus Rust unit/integration tests); focused Bun storage/initialization suites passed (9 tests); bun run check ., cargo fmt --check, cargo clippy --all-targets -- -D warnings, and cargo build passed. Cold simplicity and full-context complexity reviews cleared the scoped implementation.

Scope: tested with auto_commit unset/false. Configured auto_commit needs selected-path Git commits that preserve unrelated staged files and configured hooks/signing; BACK-802.50 tracks that shared Rust behavior.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Implemented and verified Rust draft create, including TypeScript-compatible options, defaults, Markdown output, missing-directory creation, and refusal behavior. Cargo suite, focused Bun suites, Biome, rustfmt, Clippy, build, and required reviews passed. Configured auto-commit remains tracked in BACK-802.50.
<!-- SECTION:FINAL_SUMMARY:END -->
