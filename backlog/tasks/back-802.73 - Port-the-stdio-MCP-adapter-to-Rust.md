---
id: BACK-802.73
title: Port the stdio MCP adapter to Rust
status: To Do
assignee: []
created_date: '2026-11-12T15:25:00Z'
labels: []
dependencies:
  - BACK-802.72
references:
  - docs/superpowers/specs/rust-contract-inventory.md
  - docs/superpowers/specs/2026-11-07-rust-migration-design.md
  - src/mcp/server.ts
  - src/mcp/workflow-guides.ts
  - src/test/mcp-server.test.ts
  - src/test/mcp-stdio-exit.test.ts
  - src/test/mcp-roots-discovery.test.ts
parent_task_id: BACK-802
ordinal: 481000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
MCP clients need the same shipped tools, schemas, resources, and stdio lifecycle while the implementation moves to Rust. Preserve the current protocol and initialization behavior as an optional adapter.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The Rust MCP command uses stdio only and preserves root discovery, initialization, EOF/signal shutdown, and the rule that diagnostics do not appear on protocol stdout.
- [ ] #2 All shipped task, document, milestone, Definition of Done, and workflow tools retain their names, schemas, annotations, results, and errors.
- [ ] #3 Workflow resources and `backlog://init-required` behavior match the contract inventory; no new HTTP MCP transport is introduced.
- [ ] #4 Process tests cover protocol initialization, each tool/resource contract, roots, EOF/signals, and clean stdout; satisfy the MCP parity gate before cutover.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 bun run check:types passes when TypeScript touched
- [ ] #2 bun run check . passes when formatting/linting touched
- [ ] #3 bun test (or scoped test) passes
<!-- DOD:END -->
