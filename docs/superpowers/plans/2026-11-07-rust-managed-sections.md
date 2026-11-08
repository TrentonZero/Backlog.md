# Rust managed Markdown implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement the bounded task, followed by task review and the required final complexity review.

**Goal:** Port the managed task/decision body semantics required by BACK-802.1.

**Architecture:** One Rust sections module owns recognized section ranges, Markdown context, safe replacement and checklist/comment formats. Existing TypeScript remains the behavioral reference. Preserve unmanaged source; do not introduce filesystem or CLI behavior in this component.

**Tech Stack:** The existing Rust 1.98 Cargo library. Use a Markdown parser dependency only if current semantics require its source spans; research official documentation and pin any added dependency. No Bun delegation in native code.

**Spec:** [Approved migration design](../specs/2026-11-07-rust-migration-design.md), stage 2; source contracts in `src/markdown/structured-sections.ts` and `src/markdown/section-titles.ts`.

## Global constraints

- Match supported current marked and legacy section behavior, including the existing family-specific treatment of marker-like text in code, lists and block quotes. Do not broaden context protections beyond current contracts as an unrelated parser fix.
- Preserve refusal rules for ambiguous legacy boundaries, malformed markers, invalid checklist marks and decision editor structure.
- Keep task and decision distinctions supported by the current code; introduce no new formats or aliases.
- Do not write files or change the shipped executable in this component.
- The maintainer authorized direct commits/pushes to main and waived GitHub issue/PR creation; do not ask again.

## Review focus

- Fenced code, indented code and nested containers must receive the same family-specific marker/heading treatment as the reference; compare the actual operations rather than assuming one global Markdown policy.
- Empty strings clear requested managed sections; omitted fields preserve them.
- Indexed checklist edits must not reinterpret unsupported marks or discard surrounding prose.
- Comments retain multiline bodies, authors, written UTC dates and unmanaged text.
- Duplicate/mismatched markers or ambiguous legacy headings fail under the same supported operations as the reference.

## Task 1: Managed sections, checklists and comments

**Files:** Create `rust/src/sections.rs`, `rust/tests/sections.rs`; modify `rust/src/lib.rs`; update Cargo files only for a justified pinned parser dependency. Keep internal helpers together; a separate private Markdown-context module is allowed if it makes the existing parser responsibility substantially easier to follow. Update `rust/README.md` with this component.

**Interfaces:**

- `SectionKey` is an ordered enum for Description, ImplementationPlan, ImplementationNotes, FinalSummary, Context, Decision, Consequences, Alternatives.
- `extract_section(content: &str, key: SectionKey) -> Option<String>` reads marked/legacy section content according to the current family rules.
- `update_sections(content: &str, changes: &BTreeMap<SectionKey, Option<String>>) -> Result<String, SectionError>` preserves omitted sections and safely replaces/clears requested ones.
- `ChecklistKind` distinguishes AcceptanceCriteria and DefinitionOfDone.
- `Criterion { index: usize, text: String, checked: bool }`; `parse_criteria(content: &str, kind: ChecklistKind) -> Vec<Criterion>`; `update_criteria(content: &str, kind: ChecklistKind, criteria: &[Criterion]) -> Result<String, SectionError>`; `assert_valid_checklist_marks(content: &str, kind: ChecklistKind) -> Result<(), SectionError>`. The future application layer will use parsed items for indexed changes and call validation before mutation.
- `Comment { index: usize, body: String, created_date: String, author: Option<String> }`; `parse_comments(content: &str) -> Vec<Comment>` and `update_comments(content: &str, comments: &[Comment]) -> Result<String, SectionError>`.
- `assert_section_input_safe(value: &str, key: SectionKey) -> Result<(), SectionError>` and `validate_decision_editor_structure(original: &str, edited: &str) -> Result<(), SectionError>` preserve the shipped validation paths.
- `SectionError` implements Display/Error with actionable explanations. Keep helper APIs private. Routine internal choices belong to the implementer; report any needed public signature change before dependent work starts.

- [x] Read the complete owning TypeScript files and relevant existing tests, including marker-context, malformed/ambiguous sections, AC/DoD and comment tests. Map each supported branch to the smallest native test that proves it.
- [x] Write focused Rust tests first and observe expected failures. Port the behavior using the existing canonical formats, boundary decisions, Markdown context and safe update rules.
- [x] Check extraction and mutation against actual TypeScript examples, covering both task and decision sections, code/container markers, stable checklist states and comments. Compare observable data and returned source/refusals rather than private implementation shape.
- [x] Run all Cargo tests, Cargo formatting check and Clippy with warnings denied. Run the affected TypeScript reference suites only where they establish the comparison baseline; no product TypeScript mutation is expected.
- [x] Self-review the entry-point flow and preservation/validation ownership. Record TDD and comparison evidence in the task/report. Commit only this task's files, with subject `BACK-802.1 - Port managed Markdown sections and checklists to Rust`; do not push until the controller completes required reviews.

## Following tasks

Configuration, durable storage and typed records consume this component after its signatures and behavior are verified. Their research can proceed independently, but no other agent should mutate these files while this task is running.

## Implementation record

Implemented the complete planned interface in one native sections module. The
pinned markdown 1.0.0 dependency supplies CommonMark AST heading spans; the module
retains source-specific marker, checklist, comment and normalization rules. A
private parser projection expands visual tab columns and maps heading positions
back to original byte offsets to match the current TypeScript parser.

Final verification: 37 Cargo tests pass (18 existing frontmatter, 19 sections),
including 5,166 actual TypeScript comparisons across three corpora; Cargo
formatting and Clippy with warnings denied pass. The seven affected TypeScript
reference suites pass: 154 tests, zero failures. No shipped TypeScript or
frontmatter implementation changed. Required controller reviews and task
finalization remain separate from this implementation record.

The preserved reference distinctions include sequential parsed checklist indices,
legacy task refusal/append behavior, canonical task spacing and ordering, in-place
decision migration, known foreign sentinel masking, comment author normalization,
and the existing greedy-whitespace behavior of empty legacy task sections.
