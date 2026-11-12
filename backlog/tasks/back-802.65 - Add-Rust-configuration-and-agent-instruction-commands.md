---
id: BACK-802.65
title: Add Rust configuration and agent instruction commands
status: Done
assignee:
  - '@codex'
created_date: '2026-11-12T15:23:00Z'
updated_date: '2026-11-12T19:49:00Z'
labels: []
dependencies:
  - BACK-802.61
references:
  - src/cli.ts
  - src/test/config-commands.test.ts
  - src/test/cli-agents.test.ts
  - src/test/agent-instructions.test.ts
  - rust/src/config.rs
parent_task_id: BACK-802
ordinal: 473000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Rust executable parses project configuration but cannot inspect or change it, and cannot refresh shipped agent instructions. Restore the supported configuration and instruction-maintenance CLI workflows.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `config get <key>`, `config set <key> <value>`, and `config list` expose supported camelCase keys, validate values, and preserve configuration serialization and migration behavior.
- [x] #2 Configuration writes preserve unrelated and unknown supported data and follow the existing Git auto-commit behavior.
- [x] #3 `agents --update-instructions` updates the same shipped instruction files and reports the outcome; ordinary `agents` behavior remains compatible.
- [x] #4 Process tests cover get/list, valid and invalid set values, file persistence, auto-commit behavior, and instruction update results.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Port config get/list/set and agents instruction refresh against the TypeScript command contract. For config writes, preserve unrelated and unknown YAML source instead of serializing only the typed Config, validate supported keys/values, and match selected config auto-commit behavior. For agents, embed the shipped instruction templates, preserve surrounding managed-file content, reproduce selection/update outcomes, and use the supported human path. Add process parity for keys, errors, source preservation, commit behavior, repeated instruction updates, and outcomes. Run full locked Cargo, config/agents Bun suites, formatting/Clippy/Biome, and reviews before `.66`.

Implement the bare config interactive wizard from the accepted CLI contract inventory; compare TypeScript prompts, defaults, and persistence, add process parity coverage, then run focused and full verification.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented config get/list/set with source-preserving YAML edits and TypeScript validation/default behavior. Process parity covers root and folder config paths, unknown-key/comment preservation, no config auto-commit, boolean and numeric validation, editor PATH checks, milestones, and filesystemOnly side effects. Implemented agents --update-instructions using the shipped CLI nudge embedded at build time; process tests cover managed-block replacement, preserved surrounding content, repeated updates, empty selection, and selected-path auto-commit. Verification: full locked Cargo suite passed (one timestamp-boundary failure on first run passed on isolated rerun and full rerun); scoped Bun config/agents/instruction tests passed (53); cargo fmt --check and locked Clippy passed. No TypeScript source changed.

Follow-up review: verified against live TypeScript process that minimal root and folder configs are migrated on config list, so list outputs defaultPort 6420, autoOpenBrowser true, and remoteOperations true; Rust retains that behavior. Added process parity for those list/get values and config set unknown-key available-keys errors. Windows editor lookup now expands extensionless commands using PATHEXT in declared order, honors explicit extensions without expansion, and falls back to .COM;.EXE;.BAT;.CMD; pure candidate unit test covers these cases. Latest verification: full locked Cargo suite passed (8 unit tests, 103 CLI tests plus other integration suites); scoped Bun suites passed (53 tests, 437 assertions); rustfmt, locked Clippy, Biome, and diff checks passed.

Implemented the TypeScript global schema migration before Rust CLI argument parsing for initialized projects. It applies only when project_name/statuses/default_port/auto_open_browser/remote_operations/auto_commit schema keys are missing, writes the migration defaults through the source-preserving YAML editor, and leaves comments/unknown keys intact. It skips no-argument, init, help, and version invocations as TypeScript does. The new process regression failed before implementation because the config command returned defaults only in memory and left legacy source unchanged; it now passes and verifies persisted values plus preserved source. Existing selected-path auto-commit fixtures were updated with current schema keys to isolate their original behavior from legacy migration. Latest checks: full locked Cargo suite passed (8 lib unit tests, 105 CLI tests and other integration suites); scoped Bun suites passed (53 tests, 437 assertions); rustfmt, locked Clippy, Biome, and diff checks passed.

Migration edge follow-up: process parity showed TypeScript writes  when the folder config lacks project_name and leaves task_prefix absent when no prefix was configured. Rust migration now preserves the empty name and only emits task_prefix when the source had one; the regression compares live output and parses persisted YAML values. Latest full locked Cargo suite passed (8 lib tests, 106 CLI tests and other integration suites); scoped Bun suites passed (53 tests, 437 assertions); rustfmt, locked Clippy, Biome, and diff checks passed.

Correction to migration edge note: TypeScript writes an empty project_name value when folder config lacks project_name, and omits task_prefix when none was configured. Rust migration now preserves that behavior. The new process regression compares live output and parsed persisted YAML. Latest full locked Cargo suite passed (8 lib tests, 106 CLI tests and remaining integration suites); scoped Bun suites passed (53 tests, 437 assertions); rustfmt, locked Clippy, Biome, and diff checks passed.

Review follow-up: a process regression confirmed TypeScript removes an empty task_prefix during legacy migration and uses the default task prefix in config list. Rust now removes that empty YAML key during source-preserving migration. The regression failed before the fix and passes after it. Latest full locked Cargo suite passed (8 library tests, 107 CLI tests, and remaining integration suites); scoped Bun tests passed (53 tests, 437 assertions); rustfmt, locked Clippy, Biome, and diff checks passed.

Corrected migration trigger parity: missing project_name and statuses alone no longer trigger schema persistence because TypeScript parseConfig fills them before needsMigration. Added a process regression with task_prefix and all four actual trigger fields present; TypeScript and Rust leave the source unchanged. Added independent legacy milestones migration parity: when schema defaults are already present, config list still creates milestone files, reports generated IDs, and removes the legacy key. Rust edits only the milestones key, preserving comments and unknown fields. Regression was RED before implementation (Rust reported no milestones while TypeScript reported m-0 and m-1), then GREEN. Legacy milestone migration deduplicates case-insensitively by existing ID/title and retains the legacy key if any create operation fails.

Fixed empty legacy milestones migration parity. With schema migration triggered, both milestones: [] and milestones: now have the legacy key removed as migrated config is written; when all schema fields are present, the empty key remains unchanged. The process regression was initially red: blank milestones: was parsed as YAML null and Rust incorrectly created a milestone titled null. Empty/null now maps to no legacy milestone values. Focused regression passes.

Added the bare config interactive wizard to the Rust CLI. It persists branch/remote settings, active-branch age, Git hook/auto-commit options, zero-padded IDs, editor choice and validation, Definition of Done add/remove/reorder/clear actions, web UI settings, optional isolated-shell completion installation, and optional Claude agent installation. Process tests cover settings and DoD addition, editor clearing, rejecting unavailable DoD actions, Claude agent output, and Bash completion installation under an isolated HOME. Verification after these changes: cargo fmt --check, cargo clippy --locked --all-targets -- -D warnings, full cargo test --locked (113 CLI tests plus all Rust suites), scoped Bun config/agents tests (53 tests, 437 assertions), bun run check (481 files), and git diff --check all passed. Interactive wizard uses line-based input in the Rust CLI; the TypeScript wizard uses Clack terminal controls.

Fixed empty legacy milestone parity for scalar and inline-list empty strings. With all schema trigger fields present, milestones: "" and milestones: [""] now remain unchanged, matching TypeScript. The new process test was RED before the fix (Rust removed the key; TypeScript preserved it) and is GREEN afterward. Verification: cargo fmt --check, cargo clippy --locked --all-targets -- -D warnings, all 114 Rust CLI process tests, Bun config-commands suite (32 tests / 320 assertions), bun run check (481 files), and git diff --check passed.

Final edge verification: milestones: "" and milestones: [""] remain unchanged byte-for-byte when schema migration is not triggered and are filtered as empty legacy names. The process regression passed. Final full cargo test --locked passed (114 CLI tests and all Rust suites). cargo fmt, all-target Clippy with -D warnings, scoped Bun config/agents/instructions suites (53 tests / 437 assertions), Biome (481 files), and git diff --check passed.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Ported config get/list/set, source-preserving schema and legacy milestone migration, agents --update-instructions, and the interactive bare config wizard to Rust. Added process parity for config validation/persistence, instruction updates, wizard settings and edge behavior. Verified against TypeScript process behavior. Full locked Cargo suite, Clippy, formatting, scoped Bun suites, Biome, and diff checks pass. The Rust wizard uses line-based prompts rather than Clack controls.
<!-- SECTION:FINAL_SUMMARY:END -->
