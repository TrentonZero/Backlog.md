---
id: BACK-802.3
title: Port native configuration parsing and defaults to Rust
status: In Progress
assignee: []
created_date: '2026-11-08T18:59:00Z'
updated_date: '2026-11-08T19:00:00Z'
labels: []
dependencies:
  - BACK-801
references:
  - rust/src/frontmatter.rs
  - rust/src/config.rs
  - docs/superpowers/plans/2026-11-08-rust-config-discovery.md
modified_files:
  - docs/superpowers/plans/2026-11-08-rust-config-discovery.md
parent_task_id: BACK-802
type: task
ordinal: 411000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement the next native configuration boundary for BACK-802. Reuse the shared YAML semantics in the Rust library without changing the shipped TypeScript product. Preserve sparse stored values and distinct parse, fresh-init, and schema-migration defaults. Keep project discovery, filesystem mutation, cache/watchers, and CLI integration for later bounded work.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A crate-private raw YAML mapping parser shares merge and bare-integer semantics without record-specific @assignee/reporter or due_date preprocessing; existing record tests remain green.
- [ ] #2 Native config parsing, documented aliases, list/DoD coercions, strict errors, and deterministic save serialization match current persisted behavior; absent values remain sparse.
- [ ] #3 Fresh-init defaults, prefix/filesystem-only rules, and schema migration defaults remain separate pure functions and preserve explicit false/empty values.
- [ ] #4 Source-mapped Rust tests cover malformed selected keys, anchors, duplicates, scalar/list distinctions, defaults, and serializer behavior; Cargo tests, formatting, and Clippy pass. No CLI or filesystem integration is added.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [ ] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Split the current frontmatter raw YAML parser from record-only preprocessing; add failing boundary tests first. 2. Port sparse config parsing/serialization and its compatibility coercions with source-mapped tests. 3. Add fresh-init and schema-migration defaults as separate pure operations. 4. Run focused and complete Cargo checks; review against the approved plan and current persisted behavior.
<!-- SECTION:PLAN:END -->
