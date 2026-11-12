---
id: BACK-802.44
title: Add Rust task edit for core task fields
status: Done
assignee:
  - '@codex'
created_date: '2026-11-12T02:02:00Z'
updated_date: '2026-11-12T02:16:00Z'
labels: []
dependencies:
  - BACK-802.43
references:
  - src/cli.ts
  - rust/src/task_store.rs
parent_task_id: BACK-802
type: task
ordinal: 452000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Rust executable can now create and read tasks, but users still cannot correct an existing task through the canonical CLI. Port the commonly used single-task edit path so local edits retain TaskStore identity checks, locks, source-preserving serialization, and the current CLI field semantics.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust task edit help documents its core scalar fields, aliases, clear operations, and output modes.
- [x] #2 Editing one active local task uses TaskStore identity resolution and entity locking, preserves unrelated Markdown source, and reports missing, ambiguous, archived, and invalid-input outcomes compatibly.
- [x] #3 Title, description, status, assignee, labels, priority, type, project, due date, ordinal, and milestone edits match the covered TypeScript semantics, including replacement, incremental label updates, and explicit clears.
- [x] #4 Default confirmation and --plain output, exit status, stdout/stderr, and resulting Markdown match the live TypeScript CLI for covered scenarios.
- [x] #5 Rust process parity covers field aliases, configured values, clear/conflict errors, source preservation, identity refusals, and no-write failures.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Compare the single-target TypeScript task edit path and its field validators with TaskStore::update_task, record serialization, config, and existing CLI test fixtures. 2. Define Rust Clap arguments for core scalar and label edits, omitting checklist/comment/reference/dependency operations and multi-target editing for later slices. 3. Add live TypeScript process parity tests before implementation. 4. Load one local task, apply only supplied changes, and persist through the existing TaskStore update lock; preserve source when no managed field changes. 5. Implement compatible confirmation and plain detail output, update Rust README, run Rust and focused Bun checks, then complete required reviews.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented Rust task edit for the supported core scalar fields, labels, aliases, clears, and plain/default output. TaskStore resolves the active task and persists through its entity lock and source-preserving serializer.

Verification: Rust full suite passed (BACKLOG_REFERENCE_BUN=/tmp/backlog-bun-1.4.4/bun-linux-x64/bun cargo test --manifest-path rust/Cargo.toml); cargo fmt --check, cargo clippy --all-targets -- -D warnings, cargo build, Biome, and focused Bun edit/date/milestone tests passed. Live TypeScript process parity covers successful edits and clears, default/plain output, canonical/ambiguous/archived/missing identities, invalid input, and preservation of custom frontmatter and Markdown body.

Reviews: cold simplicity and full-context complexity reviews completed; findings were resolved. Groma instructions were attempted but the groma executable is unavailable in this environment.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added the Rust task edit command for core task fields with TypeScript CLI parity, identity checks, locked TaskStore persistence, and source preservation. Rust, Biome, focused Bun, and full Rust test checks passed; both required reviews cleared.
<!-- SECTION:FINAL_SUMMARY:END -->
