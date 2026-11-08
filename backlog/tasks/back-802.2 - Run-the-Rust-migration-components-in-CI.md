---
id: BACK-802.2
title: Run the Rust migration components in CI
status: Done
assignee:
  - '@codex'
created_date: '2026-11-07T23:18:00Z'
updated_date: '2026-11-08T18:58:00Z'
labels: []
dependencies:
  - BACK-801
references:
  - .github/workflows/ci.yml
  - rust/Cargo.toml
modified_files:
  - .github/workflows/ci.yml
parent_task_id: BACK-802
type: chore
ordinal: 410000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Verified Rust components now exist on main, but the current CI only exercises the shipped Bun implementation. Add native Cargo checks so subsequent migration components cannot land without formatting, lint and unit coverage on the supported desktop operating systems. Keep the existing trust preparation and separate Bun backend/DOM passes.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 CI runs locked Cargo tests, rustfmt checking and Clippy with warnings denied on Linux, macOS and Windows using the declared Rust 1.98 toolchain.
- [x] #2 The Rust job preserves the existing verified trust preparation and does not alter Bun test grouping or release behavior.
- [x] #3 The workflow parses, action/toolchain references are verified against primary sources, and the equivalent native commands pass locally before push; remote results are reported separately.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Add a three-OS native matrix job after the existing jobs. Reuse secure-tls and checkout; install Node 26.12.1, the checksum-verified Bun runtime, and frozen TypeScript reference dependencies because section differential tests execute the existing implementation. Install the verified pinned dtolnay/rust-toolchain commit at 1.98.0 with rustfmt/clippy. Run Cargo fmt --check, locked Clippy --all-targets -- -D warnings, and locked tests. Parse final YAML and run each equivalent command locally; report remote CI results separately.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Verified Rust 1.98.0 exists and pinned dtolnay/rust-toolchain at commit 683b588c487a69d01f6c6ddf40f036ff682cc2c8. Reuses existing Node 26.12.1/setup-node and checksum-verified Bun setup, then installs frozen dependencies for live TypeScript reference tests. YAML parsing and diff check pass; local fmt, locked Clippy and locked Cargo tests pass with Bun on PATH (37 tests). Push is on origin/main; GitHub lists CI active but currently returns zero workflow runs, so hosted results are unavailable.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added three-OS locked Rust formatting, lint and test checks with existing verified TLS setup. Installed the checksum-verified Bun runtime and project dependencies because the Rust differential tests launch the TypeScript reference. YAML and local checks pass (37 Cargo tests). The push is on origin/main; GitHub reports the workflow active but the Actions API currently returns zero runs, so no hosted result is available yet.
<!-- SECTION:FINAL_SUMMARY:END -->
