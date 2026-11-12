---
id: BACK-802.59
title: Add Rust decision list and view commands
status: Done
assignee:
  - '@codex'
created_date: '2026-11-12T15:16:00Z'
updated_date: '2026-11-12T16:36:00Z'
labels: []
dependencies:
  - BACK-802.58
references:
  - src/cli.ts
  - src/formatters/decision-plain-text.ts
  - src/formatters/json-output.ts
  - src/test/cli-doc-decision-board.test.ts
  - src/test/cli-decision-content.test.ts
  - src/test/cli-json-output.test.ts
  - rust/src/decision_store.rs
  - rust/tests/decision_store.rs
modified_files:
  - rust/src/cli.rs
  - rust/tests/cli.rs
parent_task_id: BACK-802
type: task
ordinal: 467000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The native executable can already read decision records, but users still need the TypeScript CLI to browse the active decision log or inspect an active and archived decision. Port the documented read paths so humans and scripts can review the same decision record through Rust.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 decision list prints active decisions in decision ID order as ID, title, status, and any superseded-by link; an empty list prints the documented empty-list message.
- [x] #2 decision list supports plain output, versioned JSON with schemaVersion 1 and kind decision-list, and shared max-count, skip, count, and continuation behavior; count cannot be combined with JSON.
- [x] #3 decision view resolves numeric and prefixed IDs for active or archived decisions and prints the documented plain metadata and content, including the archived read-only notice.
- [x] #4 decision view --json emits the versioned decision-view envelope with curated metadata, updatedAt, archived, and content; missing, ambiguous, and conflicting output-mode inputs fail like TypeScript.
- [x] #5 Isolated Rust process tests compare list and view output, exit codes, and errors with the TypeScript CLI.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Compare decision list/view output, JSON envelopes, formatting, shared window flags, archived lookup, identity errors and output-mode conflicts against TypeScript and DecisionStore. Add process parity for active/archived, empty, paging/count, JSON and refusals. Run full Rust and scoped Bun checks, review, finalize, then continue to decision create/edit.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented Rust decision list and decision view over DecisionStore. Process parity covers sorted active list, empty output, plain and versioned JSON, list windows, count and continuation, conflicting output modes, active and archived numeric and prefixed aliases, archived notice, missing and ambiguous IDs, and view metadata and content. A cold review regression added unmanaged marker and CRLF coverage; Rust now removes only paired markers belonging to recognized decision sections using source offsets, preserving all unrelated body bytes and line endings. Verified: cargo fmt --manifest-path rust/Cargo.toml --check; cargo clippy --manifest-path rust/Cargo.toml --locked --all-targets -- -D warnings; BACKLOG_REFERENCE_BUN=/tmp/backlog-bun-1.4.4/bun-linux-x64/bun cargo test --manifest-path rust/Cargo.toml --locked; decision-scoped Bun tests (23 passed, 0 failed); Biome (481 files); git diff --check. One broad Bun selection timed out only on unrelated remote-branch board-view coverage while Cargo builds ran; the decision-filtered rerun passed.

Independent cold simplicity and full-context complexity reviews cleared the marker-preservation fix with no blockers.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added Rust decision list and view commands with TypeScript-parity plain and JSON output, paging/count behavior, active/archive identity resolution, and matching errors. Decision content stripping now removes only managed section markers and preserves unmanaged marker blocks and CRLF. Verified full Cargo tests (78 CLI process tests included), cargo fmt, Clippy, 48 scoped Bun tests, Biome, and git diff checks. TypeScript was not changed, so the conditional type check did not apply.
<!-- SECTION:FINAL_SUMMARY:END -->
