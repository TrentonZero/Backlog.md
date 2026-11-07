---
id: BACK-799
title: Plan the Rust migration with the React browser retained
status: Done
assignee:
  - '@codex'
created_date: '2026-11-07T20:47:00Z'
updated_date: '2026-11-07T21:14:00Z'
labels: []
dependencies: []
references:
  - src/cli.ts
  - src/core/backlog.ts
  - src/server/index.ts
documentation:
  - docs/superpowers/specs/2026-11-07-rust-migration-design.md
modified_files:
  - docs/superpowers/specs/2026-11-07-rust-migration-design.md
type: task
ordinal: 405000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The maintainer requested a complete Rust port and accepted the recommendation to retain the existing React browser. Establish a reviewed migration design and staged compatibility plan before replacing the shipped CLI, TUI, HTTP server, MCP adapter, persistence, and distribution. This task covers planning; implementation is separate dependent work.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The design defines retained behavior, runtime boundaries, migration stages, and cutover criteria.
- [x] #2 The plan maps current source responsibilities to Rust deliverables and concrete verification gates.
- [x] #3 A cold builder review is addressed and the maintainer has approved the design before implementation.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 bun run check:types passes when TypeScript touched
- [x] #2 bun run check . passes when formatting/linting touched
- [x] #3 bun test (or scoped test) passes
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Inspect the current product contracts and required workflows. 2. Record the Rust executable architecture with React retained, compatibility boundaries, delivery order and cutover gates. 3. Obtain a cold builder review and address material gaps. 4. Present the concrete design for the required maintainer review before implementation planning and product code changes.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Restored checksum-verified Bun 1.4.4 in /tmp, installed lockfile dependencies, and read the required Backlog and Groma guides. Recorded the proposed design and eight delivery gates. Baseline smoke: src/test/markdown.test.ts passes (35 tests, 140 assertions). This is not a full-suite baseline. No Rust implementation or product behavior changes. Awaiting cold builder review and required maintainer design approval.

Cold builder review complete. Its one finding concerned npm launcher runtime requirements; resolved in the design by explicitly preserving the existing Node-based launcher while making direct native execution independent of Bun/Node. No other material planning questions were raised. Detailed implementation planning and product changes remain gated on maintainer review of the design.

Maintainer approved the concrete design with yes do it on 2026-11-07. No TypeScript changed; type/lint obligations are not applicable to this design-only task. Markdown smoke suite: 35 passed. Local design link exists and git diff --check passes. The connected GitHub service can read the fork but rejected issue creation with HTTP 403; gh is unauthenticated. The explicit design discussion and approval supply the pre-implementation review point.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Recorded and cold-reviewed the Rust migration design, including the retained React browser and npm launcher, shared Rust core, compatibility requirements, eight delivery stages and cutover gates. Maintainer approved implementation. No product behavior changed. Design link and whitespace checks pass; 35 Markdown smoke tests pass.
<!-- SECTION:FINAL_SUMMARY:END -->
