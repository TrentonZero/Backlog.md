---
id: BACK-800
title: Establish executable compatibility gates for the Rust migration
status: Done
assignee:
  - '@codex'
created_date: '2026-11-07T21:15:00Z'
updated_date: '2026-11-07T21:33:00Z'
labels: []
dependencies:
  - BACK-799
references:
  - docs/superpowers/specs/2026-11-07-rust-migration-design.md
  - src/cli.ts
documentation:
  - docs/superpowers/plans/2026-11-07-rust-contract-baseline.md
  - docs/superpowers/specs/rust-contract-inventory.md
modified_files:
  - docs/superpowers/plans/2026-11-07-rust-contract-baseline.md
  - docs/superpowers/specs/rust-contract-inventory.md
  - scripts/migration/compare-cli.test.ts
  - scripts/migration/compare-cli.ts
  - scripts/run-ci-tests.ts
type: task
ordinal: 406000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Before replacing the shipped executable, the Rust migration needs a concrete inventory of public behavior and a process-level harness that detects differences between the reference and candidate without importing either implementation. The approved design retains React and the npm launcher. Use isolated fixture projects rather than the live backlog.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The inventory identifies CLI command groups and modes, configuration, HTTP and WebSocket contracts, MCP tools/resources, TUI flows and distribution targets, with source and existing-test references.
- [x] #2 A runner accepts explicit reference and candidate argument vectors and compares exit status, stdout, stderr and durable fixture files through representative read, mutation and refusal scenarios.
- [x] #3 Harness tests prove it catches output, exit-code and file-state mismatches and cleans up isolated projects on success and failure without normalizing user content.
- [x] #4 The current implementation build, type check and CI-profile baseline results are recorded with failures investigated and limitations stated.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Implement docs/superpowers/plans/2026-11-07-rust-contract-baseline.md inline: inventory the complete public surface and record the baseline, then TDD a two-executable process comparison runner with isolated fixtures, mutation/refusal cases and fault-detection tests. Preserve the current product implementation and serial DOM test pass. The maintainer approved staged implementation; routine plan choices require no additional review round.

Include the migration harness tests in the full CI runner discovery; keep them in the backend pass and preserve the separate serial DOM pass.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Unchanged baseline: bundle builds; types pass in 558 files; Biome passes in 479 files; full CI-profile runner exits 0 with 3286 backend passes, 406 DOM passes, 13 skips and zero failures. The same reference executable compares successfully through all 29 default CLI scenarios. Harness red-green tests cover process output, exits, durable state, isolation, cleanup, timeouts and timestamp over-normalization; one test reproduced invalid dates being hidden, corrected by requiring finite parsed timestamps.

Final checks: full CI profile passes 3304 backend tests plus 406 DOM tests, zero failures and 13 skips. All 29 reference/reference CLI scenarios pass. Type check passes560 files; Biome481 files; 81 local links resolve. Cold simplicity review and separate full-context complexity review both performed. Fixed reproduced timestamp-normalization findings with observed red-green regressions, including unknown task date fields, nested JSON, precision, multiline YAML strings and missing JSON terminal newline. The complexity reviewer verified only its original finding and fix regressions and reported resolved. GitHub issue and PR creation are blocked by integration HTTP403; SSH branch pushes work.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Established the Rust migration contract inventory, a 29-scenario two-executable comparison gate, and 18 harness fault/normalization tests integrated into the existing backend CI pass. Retained the separate serial DOM pass. Full suite, types, Biome, links and reference comparison pass. No Rust replacement or product cutover is claimed.
<!-- SECTION:FINAL_SUMMARY:END -->
