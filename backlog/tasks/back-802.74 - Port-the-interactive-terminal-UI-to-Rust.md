---
id: BACK-802.74
title: Port the interactive terminal UI to Rust
status: To Do
assignee: []
created_date: '2026-11-12T15:26:00Z'
labels: []
dependencies:
  - BACK-802.73
references:
  - docs/superpowers/specs/rust-contract-inventory.md
  - docs/superpowers/specs/2026-11-07-rust-migration-design.md
  - src/test/board-tui-multi-move-pty.test.ts
  - src/test/tui-interactive-editor-handoff.test.ts
  - src/test/tui-ready-filter-pty.test.ts
  - src/test/tui-input-lifecycle.test.ts
  - src/test/tui-task-composer.test.ts
parent_task_id: BACK-802
ordinal: 482000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The terminal UI is a canonical human workflow. Port it after the shared CLI and service semantics are in place, preserving navigation and editing behavior rather than reducing it to a read-only view.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Rust TUI preserves board/list switching, detail popups, keyboard/vim and mouse navigation, focus/scroll, filters/search, and readiness behavior.
- [ ] #2 Task composition, checklists, task/draft editing and lifecycle, multi-selection moves, and external editor handoff preserve existing data semantics.
- [ ] #3 Unicode rendering, terminal cleanup, cancellation, and input lifecycle match the supported interactive behavior.
- [ ] #4 PTY/process tests cover the contract inventory anchors; TUI parity is a required gate before distribution cutover.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [ ] #3 bun test (or scoped test) passes
<!-- DOD:END -->
