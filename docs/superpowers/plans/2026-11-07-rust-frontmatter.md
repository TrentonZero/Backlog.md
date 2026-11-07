# Rust frontmatter implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Implement the source-preserving Markdown frontmatter component needed before Rust record mutations.

**Architecture:** One internal Cargo library under `rust/`. Parse semantic metadata separately from its concrete YAML syntax. Apply requested root-field changes to a fresh lossless syntax tree, then reparse and compare against the intended metadata before returning source. Keep the Markdown body and delimiter bytes outside YAML editing.

**Tech Stack:** Rust 1.98 (the installed, tested compiler), edition 2024; yaml-edit `=0.3.6`, serde_yaml_ng `=0.10.0`, serde_json `=1.0.154`; Cargo.lock. No native executable cutover in this task.

**Spec:** [Approved migration design](../specs/2026-11-07-rust-migration-design.md), stage 2; BACK-801.

## Global constraints

- No Bun or Node runtime dependency for the Rust library.
- Keep unknown metadata source, comments, anchors/aliases and unmanaged Markdown; unsafe reference changes fail.
- Preserve LF/CRLF, existing unquoted @assignee conventions and due-date text.
- Expose no file-writing function in this component; the future storage layer owns locking and writes.
- Existing TypeScript behavior remains the shipped implementation until all migration gates pass.

## Review focus

- Updating an anchored owned field must not silently change an unknown alias value.
- Removing a root field must not succeed if a YAML merge restores it.
- A Markdown body beginning with another `---` block must remain body text.
- Text resembling metadata inside an unknown multiline value must remain exact.
- Semantic preprocessing for @assignees and due dates must never rewrite preserved source.

## Task 1: Parsing and source-preserving updates

**Files:** Create `rust/Cargo.toml`, `rust/Cargo.lock`, `rust/src/lib.rs`, `rust/src/frontmatter.rs`, `rust/tests/frontmatter.rs`, `rust/README.md`. Modify `.gitignore` to exclude `rust/target/`.

**Interfaces:**

- `ParsedMarkdown { pub data: serde_json::Map<String, Value>, pub content: String }` contains semantic root metadata and the exact body after the closing delimiter line.
- `parse_frontmatter(source: &str) -> Result<ParsedMarkdown, FrontmatterError>` reads one Markdown record. No header means empty metadata and the complete source as body. A present malformed header is an error.
- `update_frontmatter(source: &str, changes: &BTreeMap<String, Option<Value>>) -> Result<String, FrontmatterError>` applies explicit field changes; None removes a field, Some(Value::Null) stores null. An empty or semantically unchanged change set returns the original bytes. Unknown metadata semantics must remain unchanged; failures return no modified source.

- [x] Write tests for no header, empty header, body preservation, scalar/list metadata, supported @ syntax, date-only/offset due-date text, CRLF/BOM, repeated malformed parses and independent parse results. Run `cargo test --manifest-path rust/Cargo.toml`; observe the absent component failing before implementation.
- [x] Add the pinned library package and parser. Use a single delimiter splitter for parsing and updates. Semantic preprocessing follows the existing assignee/reporter/due_date conventions without changing source. Verify the parsing tests pass.
- [x] Add failing update tests for unknown nested/flow/block/quoted metadata, explicit null keys, indented and flow roots, optional-field clearing, unchanged edits, body-leading delimiters, anchors and aliases, reference-changing updates and YAML merges that undo clearing.
- [x] Implement updates using yaml-edit's lossless YamlFile and the existing semantic parser. Construct changed values through serde_yaml_ng serialization, apply them to the root mapping, and require the reparsed result to equal the original metadata with exactly the requested changes. Also compare untouched root-entry source: the editor can relocate an anchor when deleting its original entry, preserving meaning while changing unknown source. Return original delimiters/body plus the edited header only after validation.
- [x] Run `cargo test --manifest-path rust/Cargo.toml`, `cargo fmt --manifest-path rust/Cargo.toml --check` and `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings`. Expected: all pass.
- [x] Compare representative cases against the current TypeScript frontmatter implementation. Known intentional internal representation differences (opaque timestamps as strings rather than JavaScript Date instances) must not change persisted source or documented record values. Record observed limitations rather than claiming complete backend parity.
- [x] Document the library's ownership and verification commands. Check changed links and repository formatting. Obtain the required cold simplicity and separate full-context complexity reviews, address supported findings, then commit/push this task branch.

## Dependency evidence

The official [yaml-edit documentation](https://docs.rs/yaml-edit/0.3.6/yaml_edit/) describes a lossless concrete syntax tree; its Document/Mapping handles are not Send/Sync. Parse and edit inside the operation and retain source strings rather than storing mutable syntax handles across asynchronous boundaries. The official [serde_yaml_ng documentation](https://docs.rs/serde_yaml_ng/0.10.0/serde_yaml_ng/) supplies semantic parsing/serialization and merge application. Disable unused yaml-edit default features.

A disposable evaluation confirmed preservation of ordinary anchors, flow roots and indentation, semantic merge handling, and the need to reject unknown alias changes when replacing an anchored title. The initially evaluated serde-saphyr `2.1.1` was rejected after a repository test reproduced its refusal of an unindented multiline quoted value accepted by the current product. serde_yaml_ng accepts that case. Its tests are not a substitute for the repository tests above.
