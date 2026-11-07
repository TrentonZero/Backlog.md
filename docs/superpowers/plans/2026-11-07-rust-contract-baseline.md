# Rust contract baseline implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish the contract inventory and executable comparison gate required before replacing the existing backend.

**Architecture:** Keep the current backend unchanged. A Bun development script runs explicit reference and candidate command vectors in separate temporary projects, comparing process results and durable files. A source-linked inventory identifies the remaining protocol, interactive and packaging gates.

**Tech Stack:** Existing Bun 1.4.4, TypeScript, Node filesystem APIs and Bun subprocesses; no new dependency.

**Spec:** [Approved migration design](../specs/2026-11-07-rust-migration-design.md).

## Global constraints

- React and the npm launcher remain; the native replacement must not delegate to Bun.
- Use isolated fixtures, never the live backlog, for comparison runs.
- Compare exit status, stdout, stderr and durable files; normalize only scenario-owned temporary paths and uncontrolled timestamps.
- Preserve the CI runner's parallel backend and separate serial DOM passes.
- This stage is a prerequisite, not a completed Rust port or cutover.

## Review focus

- A candidate that silently skips a mutation must fail the file comparison even if its output matches.
- A candidate that writes user text incorrectly must fail; timestamp normalization cannot hide body edits.
- Identical failures from both executables must not count as a successful required workflow.
- Ambient BACKLOG_CWD, Git discovery and editor settings must not direct a fixture command at the real repository.
- A hung candidate must be terminated, reported as a failure and have its temporary fixture removed.

## Task 1: Contract inventory and baseline

**Files:** Create `docs/superpowers/specs/rust-contract-inventory.md`.

**Interfaces:** Consumes the current CLI help, HTTP routes, MCP registration, configuration definitions, TUI tests, build and release workflows. Produces a source-linked coverage inventory and baseline record, explicitly distinguishing existing coverage from process parity cases.

- [ ] Inventory all CLI command groups, modes and adapter entry points; use current help as the flag contract rather than freezing incidental prose.
- [ ] Inventory HTTP methods/routes, WebSocket messages, MCP tools/resources, configuration owners, TUI behavior and platform distribution.
- [ ] Run the existing build, type check, Biome and full CI test profile; investigate failures without changing unrelated product code. Record commands and actual results.
- [ ] Verify every source/test/document link resolves.

## Task 2: Executable comparison runner

**Files:** Create `scripts/migration/compare-cli.ts` and `scripts/migration/compare-cli.test.ts`.

**Interfaces:** Export `compareCli(reference: readonly string[], candidate: readonly string[], scenarios?: readonly CliScenario[]): Promise<void>`. `CliScenario` contains `name`, `args`, `exitCode` and an optional `json` boolean. The default scenarios exercise the real CLI. Custom scenarios are for harness fault tests. The command entry point accepts `--reference` and `--candidate`, each a JSON array containing an executable and its fixed arguments.

- [ ] Write harness tests that run real child processes: matching output succeeds; stdout, stderr and exit mismatches fail; a candidate-only file edit fails; a body timestamp edit fails; temp roots are removed on success and failure; a hung child fails and is killed. Before implementation, run `bun test --timeout=10000 scripts/migration/compare-cli.test.ts` and observe missing-runner failures.
- [ ] Implement the runner using isolated temporary roots, fixed hand-written Markdown/config fixtures, closed stdin, bounded process lifetime and deterministic file traversal. Compare each step's process and file results; check the scenario's required exit code independently for both sides. Treat JSON objects structurally while retaining array order and scalar values.
- [ ] Normalize generated timestamps only in owned frontmatter and explicitly owned output fields; preserve seeded timestamps and user bodies. If that cannot be made unambiguous, fail on the timestamp difference rather than ignore content.
- [ ] Run the harness tests. Expected: all fault cases are caught, no leftover fixture roots or live child processes.
- [ ] Run the default scenarios with the current executable on both sides. Expected: every required workflow succeeds and both observable results match.
- [ ] Check types and Biome, then run the task's focused tests. Review the staged diff, commit with the exact Backlog task ID/title, and obtain the required branch review before finalizing.

## Next stages

The approved design contains the remaining seven stages. Their detailed implementation plans are written when picked up, after the contract baseline establishes which behavior must be ported. This avoids guessing Rust APIs, dependency versions, YAML source-preservation strategy and interoperation locks before investigation.
