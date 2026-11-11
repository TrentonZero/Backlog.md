---
id: BACK-802.32
title: Port task list readiness projection to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-11T23:34:00Z'
updated_date: '2026-11-11T23:36:00Z'
labels: []
dependencies:
  - BACK-802.31
parent_task_id: BACK-802
type: task
ordinal: 440000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Rust can answer readiness for one task, but list, search, and board reads need a consistent readiness value on each returned task. A shared list projection should reuse one readiness index for the full corpus, including dependencies hidden by display filters.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust exposes a task list item projection that preserves each stored task and adds its readiness boolean.
- [x] #2 Projection computes readiness against one full active and archived corpus, regardless of which tasks are included in the displayed list.
- [x] #3 Projection matches TypeScript behavior for ready, blocked, missing, ambiguous, archived, and terminal dependencies and tasks.
- [x] #4 Derived readiness remains read-only and does not alter serialized task Markdown.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Inspect existing Rust readiness APIs and TypeScript withReadiness behavior, including list filtering and full-corpus semantics. 2. Add focused live-TypeScript parity coverage for list projection results. 3. Implement the smallest reusable Rust projection around ReadinessGraph. 4. Run Rust fmt, Clippy, full tests, and relevant TypeScript suites. 5. Complete cold simplicity and full-context complexity reviews, finalize the task, commit, and push to main.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Rust added TaskListItem and with_readiness. The projection preserves each TaskRecord, builds one ReadinessGraph over the full active and archived corpora, and uses it for displayed rows only. The API documents that corpus_tasks must include active records hidden by display filters.

Live TypeScript parity covers ready rows, blocked rows with a hidden dependency, missing and ambiguous dependencies, archived and terminal task rows. The test also serializes the stored TaskRecord and verifies derived readiness is not written to Markdown.

Verification: BACKLOG_REFERENCE_BUN=/tmp/backlog-bun-1.4.4/bun-linux-x64/bun cargo test --manifest-path rust/Cargo.toml --locked (173 passed); cargo fmt --manifest-path rust/Cargo.toml -- --check; cargo clippy --manifest-path rust/Cargo.toml --locked --all-targets -- -D warnings; bun test src/test/readiness.test.tsx src/test/readiness-surface-agreement.test.ts (24 passed); git diff --check.

Cold simplicity and full-context complexity reviews found no blockers. Groma is unavailable (command not found).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added a Rust task list readiness projection that preserves stored records, builds one readiness index from the full active and archived corpus, and attaches readiness to displayed tasks. Live TypeScript parity covers filtered dependencies and ready, blocked, missing, ambiguous, archived, and terminal cases. Rust tests (173), formatting, Clippy, and 24 focused TypeScript tests passed. Reviews found no blocking issues.
<!-- SECTION:FINAL_SUMMARY:END -->
