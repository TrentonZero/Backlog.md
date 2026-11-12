---
id: BACK-802.43
title: Add the Rust task create command
status: Done
assignee:
  - '@codex'
created_date: '2026-11-12T01:38:00Z'
updated_date: '2026-11-12T02:00:00Z'
labels: []
dependencies:
  - BACK-802.42
references:
  - src/cli.ts
  - rust/src/task_store.rs
parent_task_id: BACK-802
type: task
ordinal: 451000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Rust executable can read tasks but cannot yet create one through the canonical CLI. Users and external agents need task creation to use the same local allocator, shared locks, dependency validation, metadata defaults, and Markdown output already provided by TaskStore.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust task create help exposes the supported noninteractive task creation fields and aliases, with documented output behavior.
- [x] #2 Creating a task through Rust allocates an ID and writes a local Markdown task using the shared TaskStore locking and dependency validation.
- [x] #3 Supported metadata, checklist, description, and structured section inputs match the current TypeScript CLI semantics and serialized record.
- [x] #4 Plain and default confirmation output, invalid input errors, stdout/stderr, exit status, and resulting record match the live TypeScript CLI for covered scenarios.
- [x] #5 Rust integration coverage exercises successful creation, aliases/repeated lists, default assignee and DoD behavior, parent/dependency aliases, and refusal without writes.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Compare TypeScript task create parsing/defaults/output with TaskStore, config, record serialization, and existing CLI fixtures. 2. Define Rust Clap arguments for the noninteractive task create contract, excluding the interactive wizard and draft lifecycle. 3. Add live TypeScript process parity tests before implementation. 4. Construct TaskRecord values using config defaults and shared TaskStore::create_task, then implement confirmation/plain detail output and compatible errors. 5. Update Rust CLI documentation and run focused parity, full Rust checks, reviews, and finalization.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented noninteractive Rust `task create` with current task metadata, checklist and managed-section fields, milestone and parent resolution, dependency aliases, plain detail output, and confirmation output. Uses TaskStore for locked allocation, dependency validation, serialization, and persistence; the Rust CLI selects default ordinals inside the shared create lock. Added live TypeScript process parity for default/populated records, repeated and comma-separated fields, interior blank segments, config assignee/DoD defaults, missing/empty/missing-dependency errors, and no-write refusals. Updated the Rust README.

Verification passed: Rust fmt check, Clippy with warnings denied, full Cargo tests, Cargo build, `bun run check .` (481 files), `git diff --check`, and six focused Bun suites (94 passed, 0 failed). Independent cold simplicity and full-context complexity reviews completed; their identified compatibility findings were fixed and re-reviewed. `groma agent-instructions` could not run because Groma is not installed.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Rust now supports noninteractive task creation with parity for the covered TypeScript CLI fields, defaults, output, validation errors, and Markdown records. ID and dependency validation remain in TaskStore; default ordinals are chosen under the create lock. Rust interactive task creation, `task create --draft`, and auto-commit finalization remain outstanding migration work.
<!-- SECTION:FINAL_SUMMARY:END -->
