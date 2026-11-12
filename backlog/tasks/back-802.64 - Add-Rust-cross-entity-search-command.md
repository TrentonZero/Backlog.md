---
id: BACK-802.64
title: Add Rust cross-entity search command
status: Done
assignee:
  - '@codex'
created_date: '2026-11-12T15:23:00Z'
updated_date: '2026-11-12T18:26:00Z'
labels: []
dependencies:
  - BACK-802.61
references:
  - src/cli.ts
  - src/test/cli-search-command.test.ts
  - src/test/task-search-parity.test.ts
  - docs/superpowers/specs/rust-contract-inventory.md
  - rust/src/task_search.rs
parent_task_id: BACK-802
ordinal: 472000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Users currently have to search tasks, documents, and decisions separately, so relevant records can be missed. Add the existing shared search workflow to the Rust CLI after document and decision commands are available.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `search [query]` searches tasks, documents, and decisions and returns the established cross-entity ranking.
- [x] #2 Search supports `--type`, task `--task-type`, `--status`, and `--exclude-status` filters, repeated or comma-separated where documented, plus list-window options.
- [x] #3 Arguments after `--` are treated as literal query text; no-query and empty-result behavior match the TypeScript CLI.
- [x] #4 Plain output and schemaVersion 1 JSON use the documented search result envelope; process tests cover all three entity kinds, filters, literal arguments, ordering, and errors.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Port shared cross-entity search using current TypeScript SearchService and existing Rust task-search primitives. Add isolated process parity for task/document/decision ranking, grouped plain output, JSON result projections, type/task filters, windows/count/continuation, no query, empty matches, and option-looking terms after `--`. Match documented filtering rules and errors; keep local corpus semantics for plain output. Run cli-search/json/list-window Bun suites, full locked Cargo, formatting/Clippy/Biome and independent reviews; finalize then proceed to config and instructions.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented Rust cross-entity search with TypeScript-matched ranking, task filters, output modes, and list windows. Verification: focused process parity test passed; full locked Cargo suite passed; cargo fmt --check and Clippy with warnings denied passed; 40 focused Bun CLI search/JSON/window tests passed; Biome and git diff --check passed. JSON parity compares parsed schema and ordered result arrays because serde_json object key order differs from TypeScript output.

Review follow-up: added process parity for unsupported  and literal-query pagination. TypeScript warns and returns all types when every explicit type is unsupported, matching Rust's existing empty-types fallback. Fixed shared next-page command generation to insert  before a literal ; the test executes the emitted command and verifies page two against TypeScript. Full locked Cargo tests, fmt, Clippy, 40 focused Bun tests, Biome, and diff check pass.

Clarification: process tests cover `--type invalid` and a query after `--`. The shared paginator places its `--skip` option before the separator so the emitted next-page command preserves the literal query.

Independent reviews cleared the search contract and a footer pagination finding. The shared next-page command now inserts --skip before the first literal -- and retains all following query arguments; the process test executes the generated command and verifies page 2. Unsupported entity types retain the TypeScript warning and empty-types fallback to all entity types.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added Rust cross-entity task/document/decision search with TypeScript-compatible ranking, plain grouping, versioned JSON projections, documented filters, list windows, count, empty/no-query behavior, and literal `--` query support. Fixed next-page command construction to keep window flags before literal query terms. Verified locked Cargo suite (97 CLI tests plus all Rust suites), formatting, Clippy, 40 scoped Bun tests, Biome, and diff checks. Both independent reviews cleared the implementation. TypeScript was not changed, so the conditional type check did not apply.
<!-- SECTION:FINAL_SUMMARY:END -->
