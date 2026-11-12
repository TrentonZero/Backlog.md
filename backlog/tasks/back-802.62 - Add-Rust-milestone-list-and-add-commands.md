---
id: BACK-802.62
title: Add Rust milestone list and add commands
status: Done
assignee:
  - '@codex'
created_date: '2026-11-12T15:23:00Z'
updated_date: '2026-11-12T17:21:00Z'
labels: []
dependencies:
  - BACK-802.61
references:
  - src/cli.ts
  - src/test/cli-milestone-management.test.ts
  - src/test/cli-milestone-filter.test.ts
  - rust/src/milestone_store.rs
parent_task_id: BACK-802
ordinal: 470000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Rust executable has milestone storage but users still cannot list or create milestones through the shipped CLI. Port the existing milestone list and add workflows so milestone planning remains available without the TypeScript runtime.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `milestone` and `milestones` aliases list active milestone buckets with completion counts and due dates; completed milestones remain collapsed unless `--show-completed` is passed.
- [x] #2 `milestone list` preserves `--plain`, paging/count options, and the TypeScript list output contract, including empty sections.
- [x] #3 `milestone add <name>` supports description and due date, trims and validates names, and reports the created milestone.
- [x] #4 Rust CLI process tests cover aliasing, active/completed output, list windows, creation, validation failures, and resulting milestone files.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Port milestone list and add by matching TypeScript bucket projection and presentation over the existing Rust milestone/task stores. Add process parity for aliases and bare list, completion counts/due dates, collapsed/show-completed sections, shared windows/count/footer, creation trimming/description/date validation/duplicate errors, and persisted files. Validate with the milestone CLI and list-window Bun suites plus full Rust checks, formatting, Clippy, review, and finalization before `.63`.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented Rust milestone list/add routes. Isolated process tests compare list aliases, sections, completion counts, due dates, windows/count/footer, empty output, add trimming, multiline description, default description, date validation, duplicate refusal, and resulting files with TypeScript. A Git fixture confirms add commits only the created milestone path and preserves unrelated staged work. Verification: locked Cargo suite passed (88 CLI process cases plus all Rust suites); cargo fmt --check and locked Clippy passed; relevant Bun suites passed (30 tests, 191 assertions across cli-milestone-management, cli-list-window, cli-due-date); Biome checked 481 files; git diff --check passed. TypeScript was unchanged, so the conditional type check did not apply.

A cold review found that the initial duplicate check included archived milestone titles, unlike TypeScript, which checks active milestone files only. Added a process regression: archived title recreation now succeeds while MilestoneStore allocation keeps the archived m-0 reserved and creates m-1. The test was confirmed red before the fix and green after it.

Regression verification after the archived-title fix: process test observed RED before the change and passed after it; full locked Cargo suite passed again (88 CLI tests plus all Rust suites), cargo fmt --check, locked Clippy, Biome (481 files), and git diff --check passed.

Cold review found and verified a parity correction: duplicate names/aliases are checked against active milestones only; archived milestone IDs remain reserved during ID allocation. The regression test was red before and green after the fix. Full-context complexity review cleared the correction.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added Rust milestone list/add commands with TypeScript-parity aliases, sections, completion counts, due dates, paging/count, creation validation, descriptions, and selected-path auto-commit. Matching archived-only title reuse while preserving archived ID reservation. Verified locked Cargo suite (88 CLI cases included), formatting, Clippy, 30 scoped Bun tests, Biome, and diff checks. Independent cold and complexity reviews found no remaining blockers. TypeScript was not changed, so the conditional type check did not apply.
<!-- SECTION:FINAL_SUMMARY:END -->
