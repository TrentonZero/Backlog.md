---
id: BACK-802.6
title: Port Markdown record serialization to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-08T19:54:00Z'
updated_date: '2026-11-08T20:26:00Z'
labels: []
dependencies:
  - BACK-802.1
  - BACK-802.5
references:
  - src/markdown/serializer.ts
  - src/markdown/frontmatter.ts
  - src/markdown/structured-sections.ts
  - rust/src/records.rs
  - rust/src/frontmatter.rs
  - rust/src/sections.rs
  - docs/superpowers/plans/2026-11-08-rust-serialization.md
modified_files:
  - docs/superpowers/plans/2026-11-08-rust-serialization.md
  - rust/tests/serialization.rs
  - rust/src/serialization.rs
  - rust/src/lib.rs
  - rust/src/sections.rs
  - rust/src/frontmatter.rs
parent_task_id: BACK-802
ordinal: 414000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add pure Rust serializers for supported task, decision and document Markdown records, preserving unknown frontmatter and applying changes through the existing managed-section and frontmatter components. Keep paths, disk storage and CLI behavior in later tasks.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Task serialization preserves unknown metadata and applies typed field and managed-body changes with the current semantics.
- [x] #2 Decision and document serialization preserve source structure, optional fields and existing behavior.
- [x] #3 Serializers are pure; no filesystem, storage or CLI behavior is introduced.
- [x] #4 Rust tests compare representative serialization and safety cases against the actual TypeScript serializers; Cargo tests, formatting and Clippy pass.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Define serialization input/output contracts. 2. Add failing tests for task, decision and document serialization, source preservation and managed-body changes. 3. Implement typed serializers with existing frontmatter and section APIs. 4. Compare outputs and errors against TypeScript and run Cargo checks and review.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Verification: cargo fmt --check, cargo clippy --locked --all-targets -- -D warnings, and cargo test --locked pass. bun run check:types, bun run check, and the four affected Bun suites pass (81 tests, 449 assertions). Cold and full-context scoped reviews approved.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added pure Rust serializers for task, decision, and document Markdown. They preserve unknown frontmatter and unmanaged content, update managed sections, handle optional field clears and non-finite ordinals, and match TypeScript line-ending behavior. Verified with locked Cargo checks, TypeScript/Biome checks, and 81 focused Bun tests. Cold and complexity reviews approved.
<!-- SECTION:FINAL_SUMMARY:END -->
