# Rust migration components

This internal Cargo library is the start of the [approved executable migration](../docs/superpowers/specs/2026-11-07-rust-migration-design.md).
The shipped CLI, TUI, browser server and MCP adapter still use TypeScript. There
is no Rust command-line executable or installation cutover yet.

Use Rust 1.98 or later:

```sh
cargo test --manifest-path rust/Cargo.toml
cargo fmt --manifest-path rust/Cargo.toml --check
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
```

The [frontmatter module](src/frontmatter.rs) owns parsing Markdown headers and
returning source with explicit metadata changes. It accepts the existing
unquoted assignee/reporter conventions and keeps due-date text. Its semantic
parser input canonicalizes unquoted integer spellings such as `007` to match
the current product; quoted/tagged strings and the original source stay intact.
Opaque timestamp scalars are represented as strings rather than JavaScript
Date objects; this internal representation is not a public wire format.

Updates edit a fresh YAML concrete syntax tree, check untouched root-entry
source, and reparse the result against exactly the intended metadata. These
checks refuse changes that would rewrite unknown aliases or let a YAML merge
undo a requested clear. Delimiters, BOM, body text and existing line endings
remain outside semantic serialization. No function writes files: the future
storage/application operations will own locks, identity checks and persistence.

The [tests](tests/frontmatter.rs) exercise parsing, source preservation,
references and malformed input. The [implementation plan](../docs/superpowers/plans/2026-11-07-rust-frontmatter.md)
records dependency choices and verification. This component alone does not
establish CLI, server, protocol, TUI, concurrency or platform parity.
