---
id: BACK-802.30
title: Port task readiness queries to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-11T23:19:00Z'
updated_date: '2026-11-11T23:28:00Z'
labels: []
dependencies:
  - BACK-802.29
references:
  - src/utils/readiness.ts
  - src/utils/task-record-index.ts
  - src/utils/terminal-status.ts
  - src/core/task-detail.ts
  - src/test/readiness.test.tsx
  - rust/src/dependency_graph.rs
modified_files:
  - rust/src/lib.rs
  - rust/src/readiness.rs
  - rust/src/task_record_index.rs
  - rust/src/dependency_graph.rs
  - rust/tests/readiness.rs
parent_task_id: BACK-802
type: task
ordinal: 438000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The shared TypeScript task detail/read model answers whether work can start from the task and archived-task corpus. Rust now has the dependency graph read model but lacks readiness, so future adapters need the same verdict and the same visibility boundary: draft records are not part of task detail readiness and therefore do not resolve task dependencies.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust resolves readiness against active and archived task identities plus externally reported ambiguous identities, using configured terminal statuses; draft records remain outside this corpus.
- [x] #2 Rust matches TypeScript verdicts: unfinished tasks are blockers, missing or ambiguous IDs (including IDs found only in drafts) are missing, and archived or terminal tasks are neither ready nor blocked.
- [x] #3 Isolated Rust fixtures compare readiness values with live TypeScript, including empty/default statuses, alias resolution, ambiguity, archived records, draft-only references, and finished or unfinished targets.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Read the readiness index, verdict rules, task detail corpus boundary, public JSON fields, and focused fixtures.
2. Add failing isolated Rust comparisons for ready, blocked, missing/draft-only, ambiguous, archived, terminal, and custom-status cases.
3. Implement the shared Rust readiness model using the same identity and terminal-status rules as dependency graphs while excluding drafts from the detail corpus.
4. Run focused Bun and full Rust checks, obtain required simplicity and complexity reviews, and record evidence.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added Rust readiness queries over active and archived tasks, using a shared canonical TaskRecordIndex also used by dependency graphs. Readiness applies configured/default terminal statuses, reports unfinished blockers and unresolved/ambiguous IDs separately, treats archived or terminal targets as not actionable, and intentionally excludes drafts to match TypeScript task detail.
Verification: cargo fmt --manifest-path rust/Cargo.toml -- --check; cargo clippy --manifest-path rust/Cargo.toml --locked --all-targets -- -D warnings; BACKLOG_REFERENCE_BUN=/tmp/backlog-bun-1.4.4/bun-linux-x64/bun cargo test --manifest-path rust/Cargo.toml --locked (170 tests); /tmp/backlog-bun-1.4.4/bun-linux-x64/bun test --timeout=10000 src/test/readiness.test.tsx src/test/readiness-surface-agreement.test.ts (24 tests). Cold simplicity and full-context complexity reviews found no remaining blockers. Groma agent instructions could not run because groma is not installed.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Rust now computes task readiness with the TypeScript corpus boundary and verdict semantics. Dependency graphs and readiness share one identity, ambiguity, and terminal-status index. Full Rust checks and focused TypeScript readiness tests pass; both required reviews found no blockers.
<!-- SECTION:FINAL_SUMMARY:END -->
