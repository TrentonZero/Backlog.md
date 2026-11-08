# Rust Markdown Record Serialization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Add pure Rust serializers for task, decision, and document Markdown with the source preservation and selective managed-body updates of the current TypeScript serializers.

**Architecture:** Build serialization on the typed records, source-preserving frontmatter, and managed-section components already in Rust. Accept the original full Markdown source explicitly so serializers can retain unknown metadata and existing structure; keep disk and application operations outside this module.

**Tech Stack:** Rust 1.98, serde_json, serde_yaml_ng, existing frontmatter and sections modules, Bun for live TypeScript comparisons.

**Spec:** `backlog/tasks/back-802.6 - Port-Markdown-record-serialization-to-Rust.md`

## Global Constraints

- Match supported behavior in `src/markdown/serializer.ts` and `src/markdown/frontmatter.ts`.
- Preserve unknown frontmatter and unmanaged Markdown source when updating records.
- Apply edits through existing source-preserving Rust frontmatter and managed-section APIs.
- Do not add filesystem, persistence, CLI, or milestone serialization behavior.

## Review Focus

- Omitted optional fields must clear owned metadata while preserving unknown entries.
- Unchanged task body fields must retain their original source formatting.
- Changed task checklist/comment/section content must preserve managed-section safety and line endings.
- Decision serializers must preserve untouched frontmatter and the full untrimmed body.
- New records and existing records must use the correct header/body separator and document defaults.

---

### Task 1: Serialize typed Markdown records

**Files:**
- Create: `rust/src/serialization.rs`
- Modify: `rust/src/lib.rs`
- Test: `rust/tests/serialization.rs`

**Interfaces:**
- Consumes: `records::{TaskRecord, DecisionRecord, DocumentRecord}`, `frontmatter::update_frontmatter`, and existing `sections` update/validation APIs.
- Produces: `SerializeError`; pure `serialize_task(&TaskRecord, &str)`, `serialize_decision(&DecisionRecord, &str)`, and `serialize_document(&DocumentRecord, &str)` functions returning serialized Markdown or a `SerializeError`. The second argument is the original full Markdown source and may be empty for new records.

- [x] Write failing tests for new and existing task, decision, and document records; unknown metadata; optional-field clearing; unchanged-body preservation; changed sections/checklists/comments; line endings; and safety errors.
- [x] Run the focused Cargo test and confirm new API/tests fail before implementation.
- [x] Implement frontmatter owned-field updates and body replacement while retaining original unknown entries and unmanaged text.
- [x] Implement task managed-body updates using existing section/checklist/comment APIs; preserve source when parsed values are unchanged.
- [x] Implement decision and document serialization and compare representative output and errors with live TypeScript serializers through Bun.
- [x] Run the locked Cargo suite, rustfmt check, Clippy with warnings denied, and the affected Bun serializer/frontmatter suites.
- [x] Review the final diff against this task and serializer contracts.
