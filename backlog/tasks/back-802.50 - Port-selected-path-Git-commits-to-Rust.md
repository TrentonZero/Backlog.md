---
id: BACK-802.50
title: Port selected-path Git commits to Rust
status: Done
assignee:
  - '@codex'
created_date: '2026-11-12T03:17:00Z'
updated_date: '2026-11-12T03:56:00Z'
labels: []
dependencies:
  - BACK-802.49
references:
  - src/git/operations.ts
  - docs/superpowers/specs/rust-contract-inventory.md
  - docs/superpowers/specs/2026-11-07-rust-migration-design.md
parent_task_id: BACK-802
type: task
ordinal: 458000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Configured auto_commit is part of the shipped record-write behavior, but the Rust Git module only reads repository metadata. Rust writes need a shared Git commit operation that commits only the Backlog files it owns and preserves the user's other staged work, hooks, signing, and failure state.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The Rust Git adapter commits only the requested project files and preserves unrelated staged entries.
- [x] #2 Commit signing, hook bypass, configured Git behavior, filesystem-only mode, and non-repository behavior match the supported TypeScript Git contract.
- [x] #3 Commit failures and partial mutation outcomes preserve and report file and index state without consuming unrelated staged work.
- [x] #4 Isolated Rust process and Git tests compare selected-path commit results, messages, index state, and refusals against TypeScript.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Trace TypeScript selected-path commit behavior, config switches, failure handling, and current Git tests; inspect the Rust Git adapter and record-writing call sites.
2. Add isolated Git parity tests for requested paths, unrelated staged changes, hooks/signing/configuration switches, filesystem-only/non-repository behavior, and commit failures; verify each new expectation fails before implementation.
3. Implement the smallest shared Rust selected-path commit API and connect supported record-write flows, preserving and reporting index and file state on failures.
4. Run focused and full Rust/Bun checks, required cold simplicity and complexity reviews, update docs/task notes, finalize, and push the scoped commit to main.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented selected-path Git commits for Rust task create/edit/archive/restore/delete and draft create. Commits preserve unrelated staged files, configured hooks and signing, honor bypass_git_hooks and filesystemOnly, roll back selected index entries on refusal, and remove a newly created record after a refused commit when the file is still owned. Repository selection follows symlinked record parents and supports linked records and symlinked Backlog repositories.

Verification: full Cargo suite passed (47 CLI process tests plus all Rust tests); focused TypeScript auto-commit suites passed (20 tests across 3 files, rerun individually after one concurrent-run timeout); bun run check ., cargo fmt --check, Clippy, build, and diff checks passed. Groma is unavailable (command not found).

Final verification: serial full Cargo suite passed (49 CLI process tests and all Rust unit, integration, and doc tests); cargo fmt --check, Clippy with warnings denied, cargo build, bun run check ., three scoped auto-commit TypeScript suites (20 tests), and git diff --check passed. The parallel Cargo run had one unrelated subprocess-spawn failure in the draft-list test; the exact test passed alone and the full serial suite passed. Cold simplicity and complexity re-reviews found no blocking issues. Groma remains unavailable in this environment.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Implemented Rust selected-path Git commits for task and draft writes. Commits preserve unrelated staged entries, honor hooks, signing, bypass and filesystem-only settings, and match TypeScript behavior for failures, deletion cleanup, linked repositories, and record symlinks. Verified with the full serial Rust suite, scoped TypeScript auto-commit suites, formatting, lint, build, and diff checks. TypeScript source was not changed, so the conditional type-check DOD item does not apply.
<!-- SECTION:FINAL_SUMMARY:END -->
