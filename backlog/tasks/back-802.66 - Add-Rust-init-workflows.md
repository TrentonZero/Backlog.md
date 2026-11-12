---
id: BACK-802.66
title: Add Rust init workflows
status: Done
assignee:
  - '@codex'
created_date: '2026-11-12T15:23:00Z'
updated_date: '2026-11-12T20:34:00Z'
labels: []
dependencies:
  - BACK-802.61
references:
  - src/cli.ts
  - src/test/cli-init-create.test.ts
  - src/test/cli-init-no-git.test.ts
  - src/test/cli-init-input.test.ts
  - src/test/enhanced-init.test.ts
  - docs/superpowers/specs/rust-contract-inventory.md
parent_task_id: BACK-802
ordinal: 474000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The native executable needs the established setup path so a new project can create its backlog with the same defaults and choices as the current CLI, including projects without Git.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `init [projectName]` supports the documented interactive and defaults/noninteractive workflows, repeat initialization, and existing-directory behavior.
- [x] #2 Initialization creates the configured backlog structure and initial configuration/instructions with TypeScript-compatible defaults and project-name handling.
- [x] #3 Optional Git setup, agent instruction selection, editor selection, and completion choices preserve the supported prompts and results.
- [x] #4 Process tests cover default setup, no-Git setup, repeat initialization, prompt choices, and created files.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Trace the current TypeScript init commands, config defaults, prompt contracts, and existing Rust init/storage/agent integrations. Implement init [projectName] and repeat/existing-directory behavior with TypeScript-compatible defaults, optional Git setup, agent instruction selection, editor choice, and shell completion choices. Add process parity for default, no-Git, repeat, prompt choices, and created files; verify the full Rust suite, formatting, Clippy, scoped TypeScript tests, Biome, and diff.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Groma agent-instructions was attempted before implementation but the groma executable is unavailable in this environment.

Rust init implementation is complete in the shared working tree. Process parity covers no-Git defaults, Git projects, BACKLOG_CWD, root/custom paths, immutable repeat-init paths and preserved supported settings, integration modes, instruction file selection/update, advanced options, and PTY prompt choices. Verification: cargo test --manifest-path rust/Cargo.toml --locked (all passed); cargo fmt --manifest-path rust/Cargo.toml --all -- --check; cargo clippy --manifest-path rust/Cargo.toml --locked --all-targets -- -D warnings; Bun init suites (77 passed across four files); bun run check (481 files, no fixes). Groma is unavailable in this environment.

Complexity-review parity fixes added: interactive MCP initialization now offers Claude/Codex/Gemini/Kiro/guide setup, runs selected client commands with manual fallback on failure, writes MCP reminders, and opens the guide; shell completions are offered in the advanced wizard for MCP and other modes, while the Claude-agent prompt remains CLI-only. PTY coverage verifies empty selection retry, Codex arguments, guide URL launch, MCP reminder creation, and completion install under MCP. Final verification after these changes: cargo test --manifest-path rust/Cargo.toml --locked passed all suites (127 CLI process tests plus unit/integration suites); cargo fmt --manifest-path rust/Cargo.toml --all -- --check passed; cargo clippy --manifest-path rust/Cargo.toml --locked --all-targets -- -D warnings passed; scoped Bun init tests passed 77/77 across four files; bun run check passed (481 files, no fixes).

Final complexity parity fixes: the advanced wizard now asks about shell completions before other advanced settings in every integration mode; MCP PTY asserts this ordering. Cancel/back navigation is not implemented by the current line-prompt interaction and remains a known limitation. Final checks after this change: cargo fmt --manifest-path rust/Cargo.toml --all -- --check passed; cargo clippy --manifest-path rust/Cargo.toml --locked --all-targets -- -D warnings passed; cargo test --manifest-path rust/Cargo.toml --locked passed all suites (127 CLI tests plus all unit/integration suites); scoped Bun init tests passed 77/77 across four files; bun run check passed (481 files, no fixes). A preceding full cargo run hit a pre-existing current-minute timestamp boundary in rust_draft_edit_matches_typescript_and_preserves_unmanaged_source (TS 20:30 UTC, Rust 20:31 UTC); the focused test then passed 1/1, and the subsequent full Cargo rerun passed. No assertions were changed.

Final verification after prompt ordering and MCP changes: full locked Cargo suite passed (127 CLI process tests plus all other Rust suites); rustfmt, all-target Clippy with -D warnings, scoped Bun init suites (77 tests), Biome (481 files), and git diff --check passed. One full run transiently crossed a UTC minute during an unrelated TypeScript/Rust draft timestamp comparison; focused rerun and subsequent full suite passed without changing the assertion. Reviewed residual prompt difference: the line-based Rust selections do not support Clack Escape/back navigation; selecting a choice, retrying empty selections, and setup results are covered.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Ported `init [projectName]` to Rust with TypeScript-compatible no-Git/default setup, Git setup, path/config selection, repeat-init behavior, config defaults and overrides, agent/MCP setup, editor fallback, shell completion, Claude setup, and summaries. Added process parity including PTY interactive CLI and MCP flows. Verified full locked Cargo, formatting, Clippy, scoped Bun (77 tests), Biome (481 files), and diff checks. Remaining UI limitation: line-based prompts do not support Clack Escape/back navigation.
<!-- SECTION:FINAL_SUMMARY:END -->
