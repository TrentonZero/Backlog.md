---
id: BACK-802.53
title: Add Rust task ID shorthand and plural alias
status: Done
assignee:
  - '@codex'
created_date: '2026-11-12T14:45:00Z'
updated_date: '2026-11-12T15:01:00Z'
labels: []
dependencies:
  - BACK-802.52
references:
  - src/cli.ts
  - src/test/cli-custom-prefix-id-resolution.test.ts
  - docs/superpowers/specs/rust-contract-inventory.md
  - rust/src/cli.rs
parent_task_id: BACK-802
type: task
ordinal: 461000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Rust task details are currently available only through the explicit task view subcommand, while the canonical CLI also accepts `backlog task <id>` and the `tasks` alias. Add those established read paths so Rust users and scripts can reach the same detail output without changing how existing subcommands behave.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The Rust CLI accepts task and tasks ID shorthand and resolves numeric and prefixed IDs using the same task detail model as task view.
- [x] #2 Plain and JSON shorthand outputs, missing IDs, reserved subcommand words, the removed complete command guidance, and no-ID help behavior match the TypeScript CLI.
- [x] #3 Isolated process tests compare shorthand output and errors with TypeScript.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Compare the TypeScript task shorthand, tasks alias, output-mode resolver, and Rust task view detail path, including how Clap selects subcommands versus the positional ID.
2. Add the Rust shorthand and alias while keeping all existing task subcommands unchanged.
3. Add isolated process parity tests for numeric/prefixed IDs, plain/JSON modes, missing IDs, reserved words, and no-ID help.
4. Run the full Rust suite and affected Bun checks, review the diff, and finalize the task.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented task ID shorthand and the tasks alias in Rust, reusing task view details. Added process parity coverage for numeric and prefixed IDs, plain/JSON/default output, missing IDs, conflicting output flags, reserved list/view subcommands, and no-ID help. TDD: the new test failed before implementation because task shorthand exited 2 in Rust and 0 in TypeScript; it passes after the change. Verification: BACKLOG_REFERENCE_BUN=<cached Bun 1.4.4> cargo test (all Rust tests passed) and cargo fmt --check passed. Groma instructions unavailable; the Groma executable is not installed. Standard bun commands are absent from PATH; invoked cached Bun directly.

Follow-up parity review found `task complete` was being treated as an ID shorthand and returning not found. Rust now returns the same removal guidance and nonzero status as TypeScript. The process parity test covers this refusal; cargo test --test cli passes all 65 tests and cargo fmt --check passes.

Follow-up contract check confirmed hidden task complete accepts ignored trailing arguments (`task complete foo`) and rejects parent-level `--json` before running the removal action (`task --json complete`). Rust models this as a hidden variadic Complete subcommand; parity cases now cover both behaviors. Focused parity and all 65 Rust CLI tests pass; cargo fmt --check passes.

Compatibility review also covered the hidden removed `task complete` command: arbitrary ignored arguments preserve its removal notice, and parent --json retains the TypeScript unknown-option response. Independent cold and full-context reviews found no blockers.

Additional parity cases found that `task complete --json` and `task complete foo --json` were treated as ignored variadic arguments in Rust, while TypeScript rejects `--json`. The hidden Complete route now scans ignored arguments for `--json` and returns the same unknown-option guidance. Focused parity and all 65 Rust CLI tests pass; cargo fmt --check passes.

Rust now accepts `task <id>` and the `tasks` alias through the existing task detail output path. Process parity covers numeric/prefixed IDs, plain/JSON/default modes, missing IDs, reserved subcommands, no-ID help, and the hidden removed `task complete` behavior including arbitrary arguments and `--json` ordering. Verification: full serial Rust suite passed (65 CLI process tests plus all unit, integration, and doc tests); scoped TypeScript suites passed (54 tests); cargo fmt --check, Clippy with -D warnings, cargo build, bun run check ., and git diff --check passed. Cold simplicity and full-context complexity reviews found no remaining blockers. Groma remains unavailable.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added Rust task ID shorthand and the tasks alias with TypeScript-compatible output modes, help and refusal behavior. Process parity tests and the full Rust suite pass.
<!-- SECTION:FINAL_SUMMARY:END -->
