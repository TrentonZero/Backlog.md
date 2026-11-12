---
id: BACK-802.72
title: Port the HTTP and WebSocket server to Rust with the React browser retained
status: To Do
assignee: []
created_date: '2026-11-12T15:25:00Z'
labels: []
dependencies:
  - BACK-802.71
references:
  - docs/superpowers/specs/rust-contract-inventory.md
  - docs/superpowers/specs/2026-11-07-rust-migration-design.md
  - src/server/index.ts
  - src/web/lib/api.ts
  - src/test/server-assets.test.ts
  - src/test/server-reorder-publication.test.ts
  - src/test/server-loading-progress.test.ts
parent_task_id: BACK-802
ordinal: 480000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The current browser depends on the TypeScript server contract. Replace the server implementation in Rust while retaining the React application and its API expectations, with no data conversion for existing projects.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 All documented HTTP methods, paths, request/response shapes, partial-success behavior, static assets, and SPA routes in the contract inventory work through the Rust server.
- [ ] #2 WebSocket loading-state, task/milestone/config update, ping/pong, debounce, and scope-widening behavior matches the established protocol.
- [ ] #3 The existing React browser is built and served unchanged against the Rust server, with its required types and tests retained; existing project data needs no conversion.
- [ ] #4 Process-level tests cover routes, WebSocket events/lifecycle, asset restrictions, browser loading/publication, and runtime cwd/root discovery; do not advance cutover until this server/browser parity gate passes.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [ ] #3 bun test (or scoped test) passes
<!-- DOD:END -->
