---
id: BACK-802.40
title: Port task statistics aggregation to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-12T00:41:00Z'
updated_date: '2026-11-12T01:00:00Z'
labels: []
dependencies:
  - BACK-802.39
references:
  - src/core/statistics.ts
  - src/utils/utc-timestamp.ts
  - src/test/statistics.test.ts
parent_task_id: BACK-802
ordinal: 448000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The CLI overview, web statistics endpoint, and TUI share one TypeScript task statistics calculation. Rust has task storage and read models but no equivalent aggregation, so those surfaces cannot move to the Rust shared model until these counts and activity summaries are ported.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Rust produces the same status and priority counts, completed totals, completion percentage, and draft count as the TypeScript statistics model.
- [x] #2 Recent created and updated tasks, stale tasks, and blocked tasks match TypeScript inclusion rules and ordering, including archived final-status handling.
- [x] #3 Average task age and recency cutoffs use the same stored UTC date parsing and day rounding as TypeScript, with deterministic time supplied for parity checks.
- [x] #4 Live TypeScript parity covers empty and populated inputs, malformed and timezone-bearing dates, custom priorities and statuses, archived tasks, blocked dependencies, and five-item result limits.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Inspect src/core/statistics.ts, src/utils/utc-timestamp.ts, and existing statistics parity fixtures; identify every observable output and sort/tie rule. 2. Add a typed Rust TaskStatistics result and a pure aggregation function, with an explicit current-time input so date boundaries and average ages are deterministic. 3. Port stored UTC timestamp parsing semantics needed by statistics without changing record parsing. 4. Add live TypeScript parity for all returned counts, date-based buckets, archived/final-status inclusion, priorities, blockers, invalid dates, and limits. 5. Run Rust formatting, Clippy, full Cargo tests, existing Bun statistics tests, the new live parity test, required reviews, finalize the task, and push to main.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added a pure Rust statistics model and stored UTC parser. Its input preserves the combined corpus order and marks archive state per record; ordered count pairs retain the TypeScript Map iteration order. Live TypeScript parity covers empty/populated data, status/priority order, archive and duplicate-ID source cases, malformed and timezone-offset dates, five-item caps, and fixed time.

Verification: cargo fmt --manifest-path rust/Cargo.toml -- --check; cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings; BACKLOG_REFERENCE_BUN=/tmp/backlog-bun-1.4.4/bun-linux-x64/bun cargo test --manifest-path rust/Cargo.toml; bun test src/test/statistics.test.ts (12 passed); bun run check .; git diff --check. Cold simplicity and complexity reviews found no blocking issues. Groma agent-instructions is unavailable in this environment (command not found).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added a Rust task statistics model with ordered status/priority counts, archive-aware totals, recent activity, stale and blocked task summaries, average age, and stored UTC date parsing. Live TypeScript parity covers empty and populated inputs, custom order, archive sources including duplicate IDs, malformed and offset dates, result caps, and deterministic time. Rust formatting, Clippy, full Cargo tests, Bun statistics tests (12), Biome, and diff checks passed; required reviews found no blockers. The CLI overview, browser endpoint, and TUI are not wired to this Rust model yet.
<!-- SECTION:FINAL_SUMMARY:END -->
