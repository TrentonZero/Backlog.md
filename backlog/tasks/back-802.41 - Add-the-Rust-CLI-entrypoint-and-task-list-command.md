---
id: BACK-802.41
title: Add the Rust CLI entrypoint and task list command
status: Done
assignee:
  - '@codex'
created_date: '2026-11-12T01:02:00Z'
updated_date: '2026-11-12T01:15:00Z'
labels: []
dependencies:
  - BACK-802.40
references:
  - src/cli.ts
  - src/formatters/json-output.ts
  - src/core/task-detail.ts
  - src/test/cli-status-filtering.test.ts
  - src/test/cli-milestone-filter.test.ts
parent_task_id: BACK-802
ordinal: 449000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add the Rust backlog binary with root and task-list help, version output, and an initial noninteractive task list that supports plain and JSON output with the agreed status, search, label, sort, and limit filters.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A Rust `backlog` executable exposes root help/version and `task list` help, plus explicit `--plain` and `--json` task-list modes.
- [x] #2 Task listing reads the configured local backlog and preserves the current default ordering, status sections, empty-result output, and JSON task-list schema.
- [x] #3 The initial noninteractive list supports status, exclude-status, search, labels, sort, and limit filters with the same composition and ordering as the shared Rust query modules.
- [x] #4 Live TypeScript parity covers empty and populated projects, both output modes, filters, relevance/order, and stdout/stderr/exit status.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Inspect the current task list option schema, text renderer, JSON summary schema, project discovery/config/store APIs, and Rust task filter/search/sort/readiness models. 2. Add a Rust binary entrypoint with root/task help, version output, and a local read-only task-list command. 3. Wire explicit plain/JSON modes to the shared Rust filter/search/sort/readiness model and preserve the initial documented filters and output shape. 4. Add live TypeScript process parity on isolated project fixtures for output modes, status/search/label filters, ordering, empty results, and process streams. 5. Run Cargo formatting, Clippy, full Rust tests and binary checks, relevant Bun CLI tests, required reviews, finalize, and push to main.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented a Clap-based backlog binary backed by project discovery/configuration and the shared Rust task store, filter/search/sort/readiness modules. Added plain status-section output and the versioned task-list JSON shape. Live process comparisons cover empty and populated fixtures, output modes, search result ordering, known and unknown statuses, exclude-status, labels, priority sort, limits, stdout/stderr, and exit codes.

Verification: cargo fmt --check; cargo clippy --all-targets -D warnings; full cargo test; cargo build; 40 focused Bun CLI tests across JSON output, list windows, and exclude-status; bun run check .; git diff --check. Cold simplicity and complexity reviews found no remaining blockers. Groma instructions unavailable (`groma` command not found), so no Groma guide was available. The Rust README documents the partial CLI scope and invocation.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added the Rust backlog executable with root/task help and version output plus noninteractive plain and JSON task listing. It reads the configured local backlog and uses the shared Rust filtering, fuzzy search, sorting, status grouping, and readiness models. Live TypeScript comparisons cover empty/populated projects, output schema/text, filters, search order, and process streams. Full Rust tests, build, formatting, Clippy, 40 focused Bun CLI tests, and `bun run check .` pass. Cold simplicity and complexity reviews found no remaining blockers. The shipped launcher, TUI, browser server, and MCP adapter remain on TypeScript pending later migration slices; Groma guidance could not be loaded because the `groma` command is unavailable.
<!-- SECTION:FINAL_SUMMARY:END -->
