---
id: BACK-802.2
title: Run the Rust migration components in CI
status: In Progress
assignee:
  - '@codex'
created_date: '2026-11-07T23:18:00Z'
updated_date: '2026-11-08T18:55:00Z'
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
- [ ] #3 The workflow parses, action/toolchain references are verified against primary sources, and the equivalent native commands pass locally before push; remote results are reported separately.
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
Verified Rust 1.98.0 exists and pinned dtolnay/rust-toolchain at observed commit 683b588c487a69d01f6c6ddf40f036ff682cc2c8. The three-OS job reuses existing Node 26.12.1/setup-node and checksum-verified Bun setup, then installs frozen dependencies for the Rust tests' live TypeScript reference. YAML parsing and diff check pass. Locally, cargo fmt --check, locked Clippy -D warnings, and locked Cargo tests pass; Cargo tests require the verified Bun 1.4.4 binary on PATH and then pass all 37 tests. Remote CI will be checked after push.
<!-- SECTION:NOTES:END -->
