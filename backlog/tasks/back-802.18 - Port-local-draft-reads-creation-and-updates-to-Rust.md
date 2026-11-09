---
id: BACK-802.18
title: Port local draft reads, creation and updates to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-09T12:19:00Z'
updated_date: '2026-11-09T12:36:00Z'
labels: []
dependencies: []
references:
  - src/file-system/operations.ts
  - src/core/backlog.ts
  - src/utils/task-path.ts
  - rust/src/task_store.rs
  - rust/src/identity.rs
modified_files:
  - rust/tests/draft_store.rs
  - rust/src/lib.rs
  - rust/src/draft_store.rs
  - rust/src/task_store.rs
parent_task_id: BACK-802
type: task
ordinal: 426000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Drafts are a supported local workflow with their own DRAFT IDs and filename-based identity rules. Reads, creation, and editing still belong to the TypeScript filesystem/core path, so the Rust shared model cannot yet own those operations. This slice covers local listing, lookup, creation, and updates; archive, promotion, and demotion remain separate lifecycle work.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust lists readable local drafts in stable task-ID order and resolves a unique draft ID to its matching filename/frontmatter record; missing drafts return no record and duplicate identities fail closed.
- [x] #2 Rust creates drafts with the DRAFT prefix under configured ID padding and the shared create lock, reserving filename claims even when a file is malformed and refusing destination overwrite.
- [x] #3 Rust updates a uniquely resolved draft under its draft lock, rechecks the same file, preserves its ID and unmanaged Markdown, keeps status Draft, and safely handles title-based filename changes.
- [x] #4 Isolated live TypeScript comparisons verify listing and lookup, missing and ambiguous identity outcomes, ID allocation/collisions, update preservation, and title rename behavior.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add isolated live TypeScript comparisons for healthy draft listing and numeric ordering, filename-based lookup and identity drift, missing and ambiguous IDs, configured ID padding and malformed filename reservations, source-preserving updates, and title-based path changes.
2. Run the focused Rust draft comparison test and confirm it fails because the DraftStore API is absent.
3. Add a DraftStore module for local draft identity, listing, lookup, creation, and updates; reuse the shared create lock for allocation and the draft entity lock for updates.
4. Reuse DraftStore healthy identity scans from task reference cleanup, keeping one authority for readable unique draft records.
5. Run focused comparisons, Rust formatting, Clippy, the full locked Rust suite, scoped Bun draft tests, and required reviews; record results and push the completed task commit to main.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented Rust DraftStore for local draft list/lookup, allocation, and updates. Draft IDs include malformed filename claims and parseable frontmatter claims; strict identity checks protect mutations while read behavior follows the existing TypeScript contract. Task deletion cleanup now reuses DraftStore healthy unique records.

Verification: cargo fmt --manifest-path rust/Cargo.toml -- --check; cargo clippy --manifest-path rust/Cargo.toml --locked --all-targets -- -D warnings; BACKLOG_REFERENCE_BUN=/tmp/backlog-bun-1.4.4/bun-linux-x64/bun cargo test --manifest-path rust/Cargo.toml --locked; focused Bun tests in cli-draft-edit, draft-create-consistency, and cli-released-id-draft-links (38 passed, 1 skipped). Review findings resolved. Groma instructions could not be loaded because the groma executable is unavailable in this environment.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added Rust local draft list, lookup, create, and update operations with shared allocation/entity locks, collision-safe writes, stable ordering, source preservation, and title-based renames. Reused the validated draft scan for task deletion cleanup.

Verified by Rust formatting, Clippy, the full locked Rust suite, the focused live TypeScript comparison tests, and 38 scoped Bun tests (1 skipped, 0 failed). Full Rust executable cutover remains in later BACK-802 work.
<!-- SECTION:FINAL_SUMMARY:END -->
