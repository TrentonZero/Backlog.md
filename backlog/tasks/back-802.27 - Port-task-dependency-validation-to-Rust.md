---
id: BACK-802.27
title: Port task dependency validation to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-11T22:50:00Z'
updated_date: '2026-11-11T22:59:00Z'
labels: []
dependencies:
  - BACK-802.26
references:
  - src/utils/task-builders.ts
  - src/utils/dependency-graph.ts
  - rust/src/task_store.rs
  - rust/src/identity.rs
modified_files:
  - rust/src/lib.rs
  - rust/src/task_store.rs
  - rust/src/dependencies.rs
  - rust/tests/task_dependencies.rs
parent_task_id: BACK-802
type: task
ordinal: 435000
---

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust task creation and dependency edits resolve local task, archived task, and draft identities to canonical IDs, deduplicate aliases, and refuse missing, ambiguous, or self dependencies.
- [x] #2 Rust rejects dependency cycles before writing and preserves the existing task on refused edits.
- [x] #3 Isolated Rust fixtures compare validated dependencies, refusal messages, cycle paths, and resulting Markdown against live TypeScript.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Read task dependency validation, identity corpus construction, cycle detection, TaskStore create/update behavior, and focused tests. 2. Add failing isolated Rust tests with live TypeScript comparisons for canonicalization, missing/self/ambiguous inputs, and cycles. 3. Implement a shared Rust dependency validator for task create/update while keeping file locking and persistence in TaskStore. 4. Run focused Bun and full Rust verification, obtain required reviews, and record evidence.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented shared Rust dependency validation for TaskStore create/update. It resolves active, archived, and draft IDs, canonicalizes and deduplicates aliases, rejects missing, ambiguous, self, and cyclic references, and validates before persistence. Added isolated live-TypeScript comparisons for successful create/edit Markdown fields and refusal messages/file preservation.
Verification: cargo fmt --manifest-path rust/Cargo.toml -- --check; cargo clippy --manifest-path rust/Cargo.toml --locked --all-targets -- -D warnings; BACKLOG_REFERENCE_BUN=/tmp/backlog-bun-1.4.4/bun-linux-x64/bun cargo test --manifest-path rust/Cargo.toml --locked (163 tests); /tmp/backlog-bun-1.4.4/bun-linux-x64/bun test --timeout=10000 src/test/dependency.test.ts (24 tests). Cold simplicity and full-context complexity reviews found no blockers. Groma agent instructions could not run because groma is not installed.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Task creation and dependency edits now use Rust validation with TypeScript-parity behavior for canonical identities, alias deduplication, refusals, cycles, and persisted dependency Markdown. Full Rust checks and the focused TypeScript dependency suite pass; both required reviews found no blockers.
<!-- SECTION:FINAL_SUMMARY:END -->
