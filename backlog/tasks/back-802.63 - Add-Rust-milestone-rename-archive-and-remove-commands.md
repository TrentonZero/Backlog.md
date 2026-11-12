---
id: BACK-802.63
title: Add Rust milestone rename, archive, and remove commands
status: Done
assignee:
  - '@codex'
created_date: '2026-11-12T15:23:00Z'
updated_date: '2026-11-12T17:53:00Z'
labels: []
dependencies:
  - BACK-802.62
references:
  - src/cli.ts
  - src/test/cli-milestone-management.test.ts
  - rust/src/milestone_store.rs
  - rust/src/milestone_service.rs
parent_task_id: BACK-802
ordinal: 471000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Milestone files can be renamed, archived, and removed with task-reference handling in the existing product CLI and Rust services. Expose those established lifecycle operations in the Rust executable while preserving their identity and task-update semantics.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `milestone rename <from> <to>` resolves active milestone IDs/titles, updates matching local task references by default, supports `--no-update-tasks`, and preserves due-date set/clear behavior.
- [x] #2 `milestone archive <name>` moves an active milestone to archived storage and reports success or the established missing/ambiguous error.
- [x] #3 `milestone remove <name>` supports clear, keep, and reassign task handling, requiring an active reassignment target when selected.
- [x] #4 Process tests cover task reference outcomes, due dates, archived state, invalid combinations, and errors; existing metadata and task content are preserved.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Port milestone rename, archive, and remove using existing Rust MilestoneStore and removal service. Add TypeScript process parity for ID/title resolution, task-reference update vs preserve, due-date set/clear/conflict, archive path and identity errors, remove clear/keep/reassign including target validation, content/metadata preservation, and selected-path commits if documented. Run focused milestone-management and lifecycle tests, full Rust checks, formatting/Clippy/Biome, review, and finalization before `.64`.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added isolated Rust process tests first; rename/archive/remove initially failed because the Rust routes were absent. Routed rename/archive/remove through MilestoneStore and the existing milestone removal service, with TaskStore reference updates, due-date/handling validation, and selected-path auto-commit. Focused `rust_milestone_*` process tests: 7 passed; full locked Cargo suite passed (92 CLI process tests plus all Rust suites); cargo fmt --check and locked Clippy passed; scoped Bun suites passed (17 tests, 90 assertions across cli-milestone-management, cli-task-milestone, cli-due-date); Biome checked 481 files; git diff --check passed. TypeScript was unchanged.

Completed the deferred command-group contract too: bare milestone/milestones invocations now send the complete group help to stderr and exit 1, matching TypeScript once all milestone routes exist. Added parity coverage for both aliases. After this change, full locked Cargo passed (93 CLI process tests plus all Rust suites), fmt and locked Clippy passed; scoped Bun suites remained green (17 tests/90 assertions), Biome (481 files), and diff check passed.

Addressed cold-review rollback finding with a pre-commit hook regression: the hook replaces the original milestone path with a directory and fails, causing restoration to fail. The test was red because Rust swallowed the rollback error; rollback_milestone_file now returns failure and rename reports the rollback suffix while preserving the underlying Git error. The task-update failure path reports the same file rollback detail. Verified full locked Cargo suite (94 CLI process tests plus all Rust suites), fmt, Clippy, 17 scoped Bun tests/90 assertions, Biome (481 files), and diff check.

Fixed the cold-review data-loss finding in rollback order. The strengthened pre-commit-hook test was red when Rust deleted the renamed file before discovering the old path could not be restored. Rollback now attempts to rename the target back first and preserves the target on failure; same-path due-date rollback restores original bytes directly. Verified locked Cargo (94 CLI process tests and all Rust suites), fmt, Clippy, 17 scoped Bun tests/90 assertions, Biome 481, and diff check.

Closed the second rollback review finding: when rename commit fails but path restoration succeeds, rollback now moves the file back and rewrites the captured original bytes. Added a process parity regression for normal hook failure asserting the old path/title/body are restored and the renamed path is absent; retained the forced restore-failure regression asserting the renamed path and milestone body survive. The two regressions were observed red before the corresponding fixes. Final verification: cargo fmt --check; cargo clippy --locked --all-targets -- -D warnings; BACKLOG_REFERENCE_BUN=/tmp/backlog-bun-1.4.4/bun-linux-x64/bun cargo test --locked (95 CLI process tests plus all Rust suites); scoped Bun milestone/task-milestone/due-date suites (17 tests, 90 assertions); Biome checked 481 files; git diff --check. No TypeScript files changed.

Strengthened both rollback process regressions to compare exact bytes: successful restoration equals the original fixture byte-for-byte, and failed restoration preserves bytes identical to the TypeScript renamed target. Focused  passed (4 tests);  passed.

Correction: the previous note command expanded inline backticks in the shell, so its command name and diff-check text were omitted. The actual focused command `BACKLOG_REFERENCE_BUN=/tmp/backlog-bun-1.4.4/bun-linux-x64/bun cargo test --locked --test cli rust_milestone_rename_` passed (4 tests), and `git diff --check` passed.

Cold review found and cleared two rollback defects. Rename rollback now reports file-restore failures and preserves the renamed source if move-back fails; after successful move-back it restores the captured original bytes. Process tests compare exact bytes for both paths. Full-context complexity review found no remaining blocker.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added Rust milestone rename, archive, and remove routes with TypeScript-parity task-reference updates, due-date behavior, removal handling, and command help. Rollback now reports failures, preserves the renamed file when restoration cannot proceed, and restores original bytes when it can. Verified locked Cargo suite (95 CLI tests plus all Rust suites), formatting, Clippy, 17 scoped Bun tests, Biome, diff checks, and focused rollback tests with exact-byte assertions. Independent reviews cleared all findings. TypeScript was not changed, so the conditional type check did not apply.
<!-- SECTION:FINAL_SUMMARY:END -->
