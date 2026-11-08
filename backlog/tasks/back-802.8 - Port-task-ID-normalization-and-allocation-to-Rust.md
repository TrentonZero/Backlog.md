---
id: BACK-802.8
title: Port task ID normalization and allocation to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-08T20:48:00Z'
updated_date: '2026-11-08T20:53:00Z'
labels: []
dependencies:
  - BACK-802.7
references:
  - task-identity
  - src/utils/prefix-config.ts
  - src/utils/task-id.ts
  - src/test/prefix-config.test.ts
  - src/test/task-path.test.ts
  - src/test/cli-custom-prefix-id-resolution.test.ts
  - rust/src/records.rs
modified_files:
  - rust/tests/identity.rs
  - rust/src/lib.rs
  - rust/src/identity.rs
parent_task_id: BACK-802
type: task
ordinal: 416000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Port the pure task-ID rules Rust needs before file-backed identity resolution: configured-prefix normalization, task ID equality and validation, filename ID extraction, and next-ID generation. Keep filesystem scanning, identity ambiguity, and application workflows for later tasks.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust task-ID helpers preserve configured prefixes, case normalization, dotted numeric IDs, and leading-zero equality semantics.
- [x] #2 Rust ID extraction and allocation preserve filename reservations, numeric ordering, and configured zero padding without filesystem scanning.
- [x] #3 Representative valid, invalid, custom-prefix, hierarchical, and allocation cases compare against the live TypeScript helpers.
- [x] #4 The helpers remain pure; Cargo formatting, Clippy, and tests pass.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Read the complete task-ID and prefix helper contracts and their focused tests. 2. Add Rust cases for normalization, canonical equality, valid ID detection, filename ID extraction, and next-ID allocation, with live TypeScript comparison. 3. Implement a focused Rust identity module using string-segment comparisons rather than numeric coercion. 4. Run locked Cargo formatting, Clippy and tests, then obtain the required reviews.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Verification: cargo fmt --check, cargo clippy --locked --all-targets -- -D warnings, and cargo test --locked pass. One integration test compares Rust outputs with live TypeScript normalization, filename extraction, equality, validation, and large-ID allocation. Cold simplicity and full-context complexity reviews found no blocking findings.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added pure Rust task-ID helpers for configured-prefix normalization, filename claims, ID validation/equality, and arbitrary-width top-level and subtask allocation. String-segment logic avoids numeric overflow and matches TypeScript semantics. Verified through live TypeScript comparison and the full locked Cargo suite, formatting, and Clippy. Reviews approved.
<!-- SECTION:FINAL_SUMMARY:END -->
