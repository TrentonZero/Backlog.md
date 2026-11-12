---
id: BACK-802.75
title: Cut over Rust executable distribution across platforms
status: To Do
assignee: []
created_date: '2026-11-12T15:26:00Z'
labels: []
dependencies:
  - BACK-802.69
  - BACK-802.70
  - BACK-802.71
  - BACK-802.72
  - BACK-802.73
  - BACK-802.74
references:
  - docs/superpowers/specs/rust-contract-inventory.md
  - docs/superpowers/specs/2026-11-07-rust-migration-design.md
  - scripts/build.ts
  - scripts/smoke-compiled-build.ts
  - scripts/cli.cjs
  - flake.nix
  - .github/workflows/ci.yml
  - .github/workflows/release.yml
parent_task_id: BACK-802
ordinal: 483000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Rust implementation becomes the shipped executable only after CLI, browser server, MCP, and TUI parity gates pass. Keep the React browser and existing project data compatible while replacing the TypeScript runtime in published packages.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Build and package native Linux, macOS, and Windows targets for x64 and arm64; preserve npm platform selection and direct native execution without Node or Bun.
- [ ] #2 Preserve Nix support for x86_64-linux, aarch64-linux, and aarch64-darwin; browser assets are built and embedded/served through the supported packaging flow.
- [ ] #3 Preserve version provenance, installation outside the source checkout, CI checks, and GitHub Release-driven publishing without a raw-tag release path.
- [ ] #4 Cutover occurs only after the CLI comparison harness and HTTP/WebSocket, MCP, TUI, Git/worktree, and platform packaging gates in the migration design and contract inventory pass; retain React sources, required types, and browser tests.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [ ] #3 bun test (or scoped test) passes
<!-- DOD:END -->
