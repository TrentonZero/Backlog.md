---
id: BACK-802.29
title: Port task dependency graph queries to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-11T23:10:00Z'
updated_date: '2026-11-11T23:18:00Z'
labels: []
dependencies:
  - BACK-802.28
references:
  - src/utils/dependency-graph.ts
  - src/utils/task-record-index.ts
  - src/core/task-detail.ts
  - src/test/dependency-graph.test.ts
  - src/test/cli-dependency-graph.test.ts
  - rust/src/dependencies.rs
modified_files:
  - rust/src/lib.rs
  - rust/src/dependency_graph.rs
  - rust/tests/dependency_graph.rs
parent_task_id: BACK-802
type: task
ordinal: 437000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Task detail already presents the transitive dependency context, including unresolved identities, archived records, and cycles. The Rust shared model has dependency mutation validation but no graph read model yet, so future CLI, TUI, browser, and MCP adapters cannot return the same task relationships from one canonical implementation.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust builds the dependency graph with the documented root, nodes, directed edges, resolution states, completion flags, and dependency/dependent depths.
- [x] #2 Graph traversal matches TypeScript for aliases, active/draft/archived records, missing and ambiguous IDs, cycles, branches, and diamonds, and remains deterministic.
- [x] #3 Isolated Rust fixtures compare graph results with live TypeScript without writing derived fields into Markdown.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Read the TypeScript graph builder, identity index, task detail corpus, public graph shape, and focused fixtures.
2. Add failing isolated Rust parity fixtures for graph traversal and unresolved identities.
3. Implement the dependency graph read model in Rust using the shared identity rules.
4. Run focused Bun and full Rust checks, obtain required simplicity and complexity reviews, and record evidence.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added a Rust dependency graph read model with canonical identity indexing, active/draft/archive resolution, ambiguity and missing states, archived completion status, separate breadth-first depth calculations, and deterministic output. It accepts externally reported ambiguous IDs and keeps unresolved nodes as leaves. The builder does not mutate records.
Verification: cargo fmt --manifest-path rust/Cargo.toml -- --check; cargo clippy --manifest-path rust/Cargo.toml --locked --all-targets -- -D warnings; BACKLOG_REFERENCE_BUN=/tmp/backlog-bun-1.4.4/bun-linux-x64/bun cargo test --manifest-path rust/Cargo.toml --locked (168 tests); /tmp/backlog-bun-1.4.4/bun-linux-x64/bun test --timeout=10000 src/test/dependency-graph.test.ts src/test/cli-dependency-graph.test.ts (37 tests). Cold simplicity and full-context complexity reviews found no blockers. Removed unused temp-directory setup from the pure in-memory parity fixtures. Groma agent instructions could not run because groma is not installed.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Rust now builds the same dependency graph read model as TypeScript, including resolution states, archived completion, forward/reverse depths, cycles, and deterministic node/edge ordering. Parity tests cover archived, draft, missing, ambiguous, diamond, and cycle cases. Full Rust and focused TypeScript graph checks pass; required reviews found no blockers.
<!-- SECTION:FINAL_SUMMARY:END -->
