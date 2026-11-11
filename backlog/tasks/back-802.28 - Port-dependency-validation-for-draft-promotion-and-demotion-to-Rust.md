---
id: BACK-802.28
title: Port task status transition dependency validation to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-11T23:00:00Z'
updated_date: '2026-11-11T23:09:00Z'
labels: []
dependencies:
  - BACK-802.27
references:
  - rust/src/task_store.rs
  - rust/src/dependencies.rs
  - src/core/backlog.ts
  - src/test/dependency.test.ts
modified_files:
  - rust/src/dependencies.rs
  - rust/src/task_store.rs
  - rust/tests/task_dependencies.rs
parent_task_id: BACK-802
type: task
ordinal: 436000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The TypeScript core validates dependencies when editing a task into or out of Draft because the transition allocates a new record identity. The Rust record stores already support direct promotion and demotion, but the future Rust CLI and other adapters will also need the status-edit behavior to reject self references or cycles created by the new identity while preserving legacy unresolved dependency entries.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust task status transitions that promote a draft validate the allocated task identity against stored dependencies and refuse self or cycle-forming links before writing, while preserving unrelated unresolved dependency entries.
- [x] #2 Rust task status transitions that demote a task validate the allocated draft identity against stored dependencies and refuse self or cycle-forming links before moving the task or cleaning linked records.
- [x] #3 Isolated Rust fixtures compare status transition results, refusal messages, cycle cases, and source preservation with live TypeScript.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Inspect the Rust promotion/demotion flow and TypeScript status-edit dependency contract, including allocated identities and cleanup ordering.
2. Add failing isolated Rust parity tests for status-driven promotion and demotion self/cycle refusal before changing implementation.
3. Add the smallest Rust TaskStore operations needed for status-driven transitions, reusing existing identity, locking, cleanup, and cycle logic while keeping direct lifecycle commands behavior-compatible.
4. Run focused Bun and full Rust checks, obtain required simplicity and complexity reviews, and record evidence.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added TaskStore status-transition operations for promotion/demotion. They validate dependencies against the allocated identity while holding the existing locks, reject self/cycle-forming links before writes or cleanup, and preserve unresolved legacy dependencies. Existing direct promote/demote methods retain their prior behavior.
Verification: cargo fmt --manifest-path rust/Cargo.toml -- --check; cargo clippy --manifest-path rust/Cargo.toml --locked --all-targets -- -D warnings; BACKLOG_REFERENCE_BUN=/tmp/backlog-bun-1.4.4/bun-linux-x64/bun cargo test --manifest-path rust/Cargo.toml --locked (166 tests); /tmp/backlog-bun-1.4.4/bun-linux-x64/bun test --timeout=10000 src/test/dependency.test.ts (24 tests). Cold simplicity and full-context complexity reviews found no remaining blockers. Groma agent instructions could not run because groma is not installed.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Rust now exposes status-driven task promotion and demotion operations that validate dependencies against the newly allocated identity before mutation, matching TypeScript self/cycle refusals and preserving unresolved legacy links. Direct lifecycle behavior remains unchanged. Full Rust checks and focused TypeScript dependency tests pass; required reviews found no blockers.
<!-- SECTION:FINAL_SUMMARY:END -->
