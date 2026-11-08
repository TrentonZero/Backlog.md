# Rust Typed Markdown Record Parsers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add native typed parsing for task, decision, document, and milestone Markdown records with the observable behavior of the current TypeScript parsers.

**Architecture:** Keep parsing pure and in the Rust library, using the existing source-preserving frontmatter and managed-section modules. Model parser outputs explicitly; keep persistence, filesystem metadata, serialization, and CLI work out of this slice.

**Tech Stack:** Rust 1.98, serde, serde_json, serde_yaml_ng, existing markdown crate, Bun for differential reference tests.

**Spec:** `backlog/tasks/back-802.5 - Port-typed-Markdown-record-parsers-to-Rust.md`

## Global Constraints

- Match supported parser output and refusals in `src/markdown/parser.ts`.
- Keep Markdown task state human-readable and do not add filesystem or persistence behavior.
- Use the existing Rust frontmatter and section APIs.
- Preserve dates as written where the TypeScript parser does; normalize task priority as it does.

## Review Focus

- Dependency entries containing nested arrays or mappings must return the existing actionable refusal, not silently stringify.
- Due-date validation must reject invalid calendar/time/offset values while retaining the written day for valid offsets.
- Missing and wrongly shaped optional fields must follow the TypeScript defaults and coercions.
- Managed section markers, legacy sections, comments, and checklist states must use existing section semantics.
- Decision and milestone bodies must preserve their established raw-body behavior.

---

### Task 1: Parse typed Markdown records

**Files:**
- Create: `rust/src/records.rs`
- Modify: `rust/src/lib.rs`
- Test: `rust/tests/records.rs`

**Interfaces:**
- Consumes: `frontmatter::parse_frontmatter`, `sections` APIs, and existing Rust dependencies.
- Produces: `ParsedMarkdown`, `TaskRecord`, `DecisionRecord`, `DocumentRecord`, `MilestoneRecord`, `RecordError`, and pure `parse_markdown`, `parse_task`, `parse_decision`, `parse_document`, `parse_milestone` functions. All parser functions accept `&str`; `parse_markdown` returns `ParsedMarkdown`, record parsers return their record or `RecordError`.

- [x] Write failing source-mapped tests for body trimming, date spellings, task defaults/coercions/priority, sections/checklists/comments, dependency refusal, and decision/document/milestone fields and body behavior.
- [x] Run the focused Cargo test and confirm the new API/tests fail before implementation.
- [x] Implement the explicit record types and pure parser functions in `rust/src/records.rs`, following the specified TypeScript coercions and existing Rust section APIs.
- [x] Compare representative records and dependency/due-date refusals against the actual TypeScript parser using Bun-backed tests.
- [x] Run `cargo test --manifest-path rust/Cargo.toml --locked`, `cargo fmt --manifest-path rust/Cargo.toml --check`, `cargo clippy --manifest-path rust/Cargo.toml --locked --all-targets -- -D warnings`, and the affected Bun parser suites.
- [x] Review the final diff against this task and the parser contracts.
