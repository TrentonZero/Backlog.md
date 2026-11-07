---
id: BACK-802
title: Port the Backlog.md executable to Rust
status: In Progress
assignee:
  - '@codex'
created_date: '2026-11-07T21:53:00Z'
labels: []
dependencies:
  - BACK-801
references:
  - docs/superpowers/specs/2026-11-07-rust-migration-design.md
  - docs/superpowers/specs/rust-contract-inventory.md
  - rust/README.md
type: feature
ordinal: 408000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The maintainer requested and approved a complete executable migration to Rust while retaining the React browser and existing npm launcher. BACK-799 established the approved design; BACK-800 established the compatibility inventory and comparison harness; BACK-801 implemented the first native frontmatter component. Those foundations do not complete the port. This task preserves the outstanding initiative and completion gates for continued implementation.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Rust shared records, configuration, storage, locks, allocation, identity and Git operations preserve supported Markdown behavior, including old/new writer coordination during migration.
- [ ] #2 The complete documented CLI, output schemas, workflow instructions, editor and watch behavior run through the Rust shared model without Bun delegation.
- [ ] #3 The retained React browser works with the Rust HTTP/WebSocket server with existing behavior and reviewed desktop/narrow-screen interaction.
- [ ] #4 The shipped MCP stdio tools, schemas, resources, root discovery and lifecycle work through the Rust shared model.
- [ ] #5 The Rust TUI preserves supported human workflows and passes required interactive and visual checks.
- [ ] #6 All six native distribution targets, npm launcher, Nix packaging, version derivation and release flow are verified before cutover.
- [ ] #7 Required compatibility, platform and review gates pass before the old backend is retired; no compiling scaffold or partial CLI is reported as a complete port.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [ ] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Continue the eight stages in docs/superpowers/specs/2026-11-07-rust-migration-design.md. Completed foundations: design, contract inventory/harness, initial source-preserving Rust frontmatter. Next: remaining record parsing and structured sections, configuration and durable storage with compatible locks, then domain/Git operations and the complete CLI before adapters and distribution. Research and record each bounded implementation plan when picked up. Do not cut over or mark this initiative Done until every accepted surface and platform gate is verified.
<!-- SECTION:PLAN:END -->
