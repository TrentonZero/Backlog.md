---
id: BACK-802.12
title: Port local task record listing and lookup to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-08T21:18:00Z'
updated_date: '2026-11-08T21:30:00Z'
labels: []
dependencies:
  - BACK-802.11
references:
  - file-storage
  - core
  - task-identity
  - src/file-system/operations.ts
  - src/utils/task-path.ts
  - src/test/content-identity.test.ts
  - src/test/task-path.test.ts
  - rust/src/records.rs
  - rust/src/storage.rs
  - rust/src/identity.rs
modified_files:
  - rust/tests/task_store.rs
  - rust/src/task_file_ids.rs
  - rust/src/task_store.rs
  - rust/src/lib.rs
parent_task_id: BACK-802
type: task
ordinal: 420000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Filename reservation now covers the working copy and sibling worktrees, but Rust cannot yet load local task records from the Markdown store. The first application read path must preserve the current active/archive distinctions and task identity rules before write operations and CLI adapters are added.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust lists active task files from the configured task directory, parses records, normalizes task and parent IDs, and associates each result with its source path.
- [x] #2 Rust lists current and legacy archived task directories without normalizing stored IDs, skipping unreadable or malformed records as the current store does.
- [x] #3 Local lookup resolves active and archived identities with the current ambiguity behavior and does not return an archived task as an active task.
- [x] #4 Live TypeScript comparisons cover active, archived, malformed, missing, and ambiguous cases; locked Rust checks pass.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Confirm the exact FileSystemOperations and TaskPath lookup contracts, including active versus archived normalization, malformed files, sorting, and duplicate diagnostics. 2. Add isolated Rust comparisons against live Bun Core filesystem methods for active/archive lists and local ID lookup. 3. Implement a small Rust task store on the existing discovery, record parser, storage, and identity modules. 4. Run focused Bun task lookup tests, rustfmt, locked Clippy, the full locked Rust suite, and required reviews.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Verification: cargo fmt --manifest-path rust/Cargo.toml -- --check; cargo clippy --manifest-path rust/Cargo.toml --locked --all-targets -- -D warnings; BACKLOG_REFERENCE_BUN=/tmp/backlog-bun-1.4.4/bun-linux-x64/bun cargo test --manifest-path rust/Cargo.toml --locked; focused Bun tests for content-identity, task-path, and local-task-command-performance (61 passed); git diff --check. Live Bun parity includes dependency parse diagnostics. Cold simplicity review and full-context complexity review found no blocking issues; both noted Rust lexical candidate ordering can differ from TypeScript localeCompare for locale-sensitive paths, a previously recorded deterministic-ordering tradeoff.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added Rust TaskStore listing, archive listing, active lookup, and fail-closed ambiguity handling using the existing record parser, storage, identity, and filename discovery. Active IDs normalize while archive IDs remain stored; malformed listing entries are skipped and invalid dependency metadata preserves the TypeScript lookup error. Live Bun comparisons, the locked Rust suite, Clippy, rustfmt, and focused Bun tests passed. Required cold and complexity reviews found no blocking issues.
<!-- SECTION:FINAL_SUMMARY:END -->
