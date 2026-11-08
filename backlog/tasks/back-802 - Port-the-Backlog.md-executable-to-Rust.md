---
id: BACK-802
title: Port the Backlog.md executable to Rust
status: In Progress
assignee:
  - '@codex'
created_date: '2026-11-07T21:53:00Z'
updated_date: '2026-11-08T20:47:00Z'
labels: []
dependencies:
  - BACK-801
references:
  - docs/superpowers/specs/2026-11-07-rust-migration-design.md
  - docs/superpowers/specs/rust-contract-inventory.md
  - rust/README.md
  - BACK-802.3
  - BACK-802.4
  - BACK-802.5
  - BACK-802.6
  - BACK-802.7
modified_files:
  - >-
    backlog/tasks/back-802.7 -
    Port-durable-record-storage-and-compatible-locks-to-Rust.md
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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Completed stages: BACK-802.1 managed sections, BACK-802.2 Rust CI, BACK-802.3 config parsing/defaults, and BACK-802.4 native discovery. The Rust library now has reviewed frontmatter, managed-body, config/default and discovery components. Overall executable migration remains In Progress. Next: typed record parsing/serialization, durable storage and Bun-compatible locks before CLI cutover. GitHub reports zero Actions runs; Windows execution is therefore unchecked.

BACK-802.5 is complete and pushed. Native typed parsers now cover all four Markdown record families. BACK-802.6 is active for task, decision and document serialization.

BACK-802.6 is complete and pushed. Pure Rust serializers now cover task, decision, and document records. Next: continue storage, locking, and CLI cutover stages in the approved migration plan.

BACK-802.6 is complete and pushed. BACK-802.7 is active for durable record storage and locks compatible with the current executable.

BACK-802.7 is complete and pushed. Rust now provides source-preserving explicit-path storage, no-overwrite creation/moves, and proper-lockfile-compatible create/entity locks. Next: port identity, Git integration, and application operations.
<!-- SECTION:NOTES:END -->
