---
id: BACK-802.31
title: Port task detail assembly to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-11T23:30:00Z'
updated_date: '2026-11-11T23:33:00Z'
labels: []
dependencies:
  - BACK-802.30
parent_task_id: BACK-802
type: task
ordinal: 439000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Task detail reads combine the stored task with its dependency graph and readiness verdict. The Rust migration has those queries independently; adapters need one shared result assembled from one corpus so their relationship data cannot disagree.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust exposes a task detail read model containing the stored task, dependency graph, and readiness verdict.
- [x] #2 All derived fields are computed from the same active and archived task corpus with the existing identity and status rules.
- [x] #3 Task detail assembly matches the TypeScript core for representative resolved, blocked, missing, ambiguous, archived, and terminal cases.
- [x] #4 Derived fields remain read-only and are not serialized into Markdown task records.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Inspect the Rust record, dependency graph, readiness models, and existing parity harness. 2. Add focused Rust parity coverage for assembling task detail from a shared corpus. 3. Implement the smallest Rust detail model that composes graph and readiness without duplicating identity rules. 4. Run required Rust checks and relevant TypeScript parity suites. 5. Complete cold simplicity and full-context complexity reviews, finalize the task, commit, and push to main.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Rust added a public TaskDetail model and build_task_detail function. They preserve the stored TaskRecord and compose dependency graph and readiness from the same active and archived records, statuses, ambiguity IDs, and prefix.

A live TypeScript parity test covers resolved, blocked, missing, ambiguous, archived, and terminal readiness cases. A serialization test confirms derived values stay outside Markdown.

Verification: BACKLOG_REFERENCE_BUN=/tmp/backlog-bun-1.4.4/bun-linux-x64/bun cargo test --manifest-path rust/Cargo.toml --locked (172 passed); cargo fmt --manifest-path rust/Cargo.toml -- --check; cargo clippy --manifest-path rust/Cargo.toml --locked --all-targets -- -D warnings; bun test src/test/readiness.test.tsx src/test/readiness-surface-agreement.test.ts (24 passed); git diff --check.

Cold simplicity and full-context complexity reviews found no blocking issue. Groma instructions could not be loaded because groma is not installed.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added the Rust task detail read model, which keeps the stored task intact and assembles its dependency graph and readiness from one corpus. Live TypeScript parity covers resolved, blocked, missing, ambiguous, archived, and terminal cases; Markdown serialization remains limited to TaskRecord. Rust tests (172), formatting, Clippy, and 24 focused TypeScript tests passed. Required reviews found no blocking issues.
<!-- SECTION:FINAL_SUMMARY:END -->
