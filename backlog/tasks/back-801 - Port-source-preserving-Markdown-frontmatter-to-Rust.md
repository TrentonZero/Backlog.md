---
id: BACK-801
title: Port source-preserving Markdown frontmatter to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-07T21:30:00Z'
updated_date: '2026-11-07T21:52:00Z'
labels: []
dependencies:
  - BACK-800
references:
  - src/markdown/frontmatter.ts
  - src/test/task-frontmatter-preservation.test.ts
  - src/test/content-frontmatter-preservation.test.ts
documentation:
  - docs/superpowers/plans/2026-11-07-rust-frontmatter.md
modified_files:
  - docs/superpowers/plans/2026-11-07-rust-frontmatter.md
  - rust/tests/frontmatter.rs
  - rust/Cargo.toml
  - rust/src/lib.rs
  - .gitignore
  - rust/src/frontmatter.rs
  - rust/Cargo.lock
  - rust/README.md
type: task
ordinal: 407000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The approved Rust migration must preserve existing Markdown and unknown YAML metadata before any Rust mutation can be trusted. Implement the first Rust library component for frontmatter parsing and source-preserving metadata updates, with no shipping executable cutover. Reuse one implementation across future task, draft, document and decision flows. Retain bodies exactly and refuse updates that invalidate preserved YAML references.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A pinned Cargo library parses frontmatter and preserves the original Markdown source and body without requiring Bun at runtime.
- [x] #2 Metadata updates preserve unknown values, comments, styles and unmanaged Markdown, including aliases and anchors; unsafe updates fail without returning changed source.
- [x] #3 Existing unquoted assignee conventions, due-date text, LF/CRLF and supported YAML mapping forms have focused Rust compatibility tests.
- [x] #4 Rust tests, formatting and Clippy pass, and parity cases are compared with the current frontmatter behavior before the component is used for product mutations.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Execute docs/superpowers/plans/2026-11-07-rust-frontmatter.md inline. Pin the evaluated Rust YAML libraries, first prove parsing and source-preserving metadata updates with Rust tests, preserve body/delimiter source, and validate semantic intent before returning any edited source. No filesystem writes or executable cutover in this component. Run Cargo tests/fmt/Clippy and compare representative behavior with the current implementation; obtain required reviews.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Initial Rust implementation passes 17 focused tests. Replaced serde-saphyr after an observed supported multiline-quoted-scalar failure; pinned serde_yaml_ng 0.10.0 accepts it. Added untouched root-entry source comparison after observing yaml-edit anchor relocation on removal preserve meaning while changing unknown alias source. Both original failures now pass. Also observed and fixed comment-only header creation. Clippy passes; final formatting and cross-language checks pending.

Fresh final component checks pass: 18 Cargo tests, cargo fmt --check and Clippy --all-targets -- -D warnings. Eight representative native Rust/current-TypeScript cases match parsed metadata, exact body and refusal results. Corrected the cross-language probe to preprocess only the header rather than normalize CRLF body text. Added an observed red-green bare-007 integer compatibility test and semantic-input integer canonicalization; quoted/tagged values and original bytes stay intact. Five local documentation links resolve; rust/target is ignored. Cold simplicity review independently ran18 passing tests and reported no blocking finding. Full-context complexity review pending. This is only the frontmatter component, with no file writes, Rust CLI, adapter or packaging cutover.

Separate full-context complexity review found no supported blocker or removable architectural complexity and independently passed all18 Rust tests, formatting and Clippy. Its additional prefix/trailing-comment probes passed. Verified implementation plan steps are checked. No TypeScript was changed by BACK-801, so conditional Bun type/Biome obligations do not apply; Cargo tests/fmt/Clippy cover the native component. The complete migration remains unfinished: filesystem/concurrency, domain operations, CLI, TUI, HTTP/MCP and distribution/cutover remain.

Final staged diff check identified one extra blank line at Cargo.toml EOF. Removed it before pushing; no manifest value changed.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added the pinned internal Rust frontmatter library with source-preserving metadata updates, exact body/delimiter handling, legacy scalar conventions and refusal of unsafe reference changes. Eighteen Rust tests, formatting and Clippy pass; eight native/current-TypeScript comparison cases match. Required cold and full-context reviews found no remaining blocker. This is a library foundation only; the shipped TypeScript executable remains in place and the complete Rust port is not done.
<!-- SECTION:FINAL_SUMMARY:END -->
