---
id: BACK-802.4
title: Port project directory discovery to Rust
status: Done
assignee: []
created_date: '2026-11-08T19:20:00Z'
updated_date: '2026-11-08T19:34:00Z'
labels: []
dependencies:
  - BACK-802.3
references:
  - docs/superpowers/plans/2026-11-08-rust-config-discovery.md
  - src/utils/backlog-directory.ts
  - src/utils/runtime-cwd.ts
  - src/utils/find-backlog-root.ts
modified_files:
  - docs/superpowers/plans/2026-11-08-rust-config-discovery.md
  - rust/tests/discovery.rs
  - rust/src/discovery.rs
  - rust/src/lib.rs
  - rust/README.md
parent_task_id: BACK-802
type: task
ordinal: 412000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Port root/folder backlog directory selection, runtime cwd precedence, lexical custom-path validation, and nearest marked project discovery to the Rust library. Reuse BACK-802.3 configuration types. Keep Git optional; discovery remains read-only and adds no CLI, watcher, cache, or record migration behavior.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Runtime cwd selection preserves option, environment, and process precedence; selected invalid paths fail without fallback.
- [x] #2 Directory normalization and root/folder config precedence preserve current lexical, alias, and built-in behavior, including accepted descendant symlinks.
- [x] #3 Nearest project search recognizes current config markers and optional Git-root fallback without requiring Git for normal Markdown projects.
- [x] #4 Source-mapped Rust tests cover path selection, precedence, markers, fallback, and platform rules; Cargo tests, formatting, and Clippy pass.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add failing test tables for runtime cwd, lexical paths, and folder/root precedence. 2. Implement read-only discovery functions consuming ConfigLocation and existing Config metadata behavior. 3. Add nearest-root and optional-Git fallback tests using isolated fixtures and an injected internal fallback. 4. Run focused/full Cargo verification and required reviews; commit/push directly to main under the existing authorization.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented read-only cwd, backlog-directory, root-config and nearest-project discovery using ConfigLocation. Root metadata stays lightweight; normal marker discovery precedes optional Git. Matching Unix root-path double normalization was added after review and verified by target-access regression. 73 Cargo tests, 35 scoped Bun tests, fmt, Clippy and Biome pass. Task review and focused fix re-review approved; separate full-context complexity review found no blocker. Windows-specific cfg fixtures exist but hosted CI currently has zero runs, so Windows execution remains unchecked.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Ported runtime cwd and project/backlog directory discovery to Rust with metadata-only config recognition, lexical path behavior and optional Git fallback. Verified 73 Cargo tests, 35 Bun discovery tests, formatting, Clippy and Biome; required reviews approved. Windows-specific behavior remains unverified on a Windows runner.
<!-- SECTION:FINAL_SUMMARY:END -->
