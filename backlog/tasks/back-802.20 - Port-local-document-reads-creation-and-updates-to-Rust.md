---
id: BACK-802.20
title: Port local document reads, creation and updates to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-09T13:05:00Z'
updated_date: '2026-11-09T13:23:00Z'
labels: []
dependencies: []
references:
  - src/file-system/operations.ts
  - src/core/backlog.ts
  - rust/src/records.rs
  - rust/src/serialization.rs
modified_files:
  - rust/src/document_store.rs
  - rust/src/lib.rs
  - rust/tests/document_store.rs
parent_task_id: BACK-802
type: task
ordinal: 428000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Documents are stored Markdown records with their own DOC IDs, identity rules, and title-based filenames. Rust already has shared document parsing and serialization, but local document reads, creation, and editing still use the TypeScript filesystem path. This slice covers local list, lookup, create, and update; archive, restore, and delete remain separate lifecycle work.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust lists readable local documents in the TypeScript title-then-path order and resolves unique IDs to records; missing and duplicate identities match the existing contract.
- [x] #2 Rust creates documents with configured ID padding, shared create-lock coordination, filename/frontmatter ID reservations, and no destination overwrite.
- [x] #3 Rust updates a uniquely resolved document under its entity lock, rechecks the same file, preserves its ID and unmanaged Markdown, and handles title-based filename changes safely.
- [x] #4 Isolated live TypeScript comparisons verify list/lookup, missing/ambiguous identities, allocation/collisions, source preservation, and title rename behavior.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Mirror TypeScript local document traversal, skipping README and unreadable entries, with title/path ordering and shared document ID resolution.
2. Add Rust store tests and isolated Bun live comparisons before implementation for reads, allocation, writes, ambiguity, preservation, and title rename.
3. Implement create-lock coordination, ID reservation across active/archive documents, filename construction, safe no-overwrite writes, and locked identity-rechecked updates using existing Rust record, identity, serialization, and storage modules.
4. Run focused Rust and Bun comparisons, then formatting, Clippy, and full Rust verification; record results and review the diff.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Live TypeScript comparisons exposed and resolved parity details: document listing uses title/path locale ordering; README files do not reserve IDs; ambiguous errors use project-relative paths; inward directory symlinks are not listed through aliases; active/archive duplicate IDs fail closed; missing loads and archived updates retain existing messages.
Verification: cargo fmt --check and Clippy with warnings denied passed; the locked Rust suite passed 132 tests; focused document store comparisons passed 4/4; Bun doc decision/create, archive, and restore/delete tests passed 96/96. Groma is unavailable in this workspace (command not found).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added a Rust DocumentStore for local listing, unique lookup, padded creation and locked source-preserving updates, using shared identity, serialization, storage and locks. It preserves TypeScript ordering, path/error behavior, README and filename/frontmatter ID reservations, archive read-only semantics, and symlink traversal behavior. Verified with four live TypeScript comparison tests, 132 locked Rust tests, and 96 scoped Bun tests; Rust formatting and Clippy passed.
<!-- SECTION:FINAL_SUMMARY:END -->
