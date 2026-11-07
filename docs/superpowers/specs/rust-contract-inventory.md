# Rust migration contract inventory

This inventory belongs to [the approved migration design](2026-11-07-rust-migration-design.md)
and BACK-800. It identifies what must survive the migration; it does not claim
that a Rust implementation or the comparison harness covers every entry.

## CLI

The complete option/default/input contract is the current output of
`backlog <command> --help`, including its input schema, interpreted alongside
[README](../../../README.md), [CLI instructions](../../../CLI-INSTRUCTIONS.md)
and [advanced configuration](../../../ADVANCED-CONFIG.md). Preserve meaningful
output and documented refusals; incidental help wrapping is not an API.

| Entry points | Required behavior | Existing coverage examples |
| --- | --- | --- |
| root, `--version`, `--help`, runtime cwd | Root experience, version, aliases, BACKLOG_CWD and cwd discovery | [runtime cwd](../../../src/test/runtime-cwd.test.ts), [launcher](../../../src/test/cli-launcher.test.ts) |
| `init` | Interactive/default setup, optional Git, repeat init, instructions, editor and completion choices | [init/create](../../../src/test/cli-init-create.test.ts), [no Git](../../../src/test/cli-init-no-git.test.ts) |
| `task` / `tasks`; ID shorthand; `create`, `edit`, `view`, `list` | Semantic mutations, multi-edit, prefixed/numeric IDs, filters, ordering, readiness, list windows and plain/JSON output | [JSON](../../../src/test/cli-json-output.test.ts), [windows](../../../src/test/cli-list-window.test.ts), [prefix resolution](../../../src/test/cli-custom-prefix-id-resolution.test.ts) |
| `task archive`, `restore`, `delete`, `demote` | Archive before deletion, archive read-only, promotion/demotion identity and reference cleanup | [task lifecycle](../../../src/test/tui-task-lifecycle.test.ts), [released IDs](../../../src/test/cli-released-id-draft-links.test.ts) |
| hidden `task complete` | Existing removal guidance/refusal, not restoration of an obsolete workflow | [source](../../../src/cli.ts) |
| `draft create`, `edit`, `view`, `list`, `archive`, `promote` | Draft identities, ambiguity refusal, JSON list, task conversion | [draft editing](../../../src/test/cli-draft-edit.test.ts), [draft create](../../../src/test/draft-create-consistency.test.ts) |
| `milestone` / `milestones`; `list`, `add`, `rename`, `remove`, `archive` | File-backed milestone identities, legacy config migration, task links, filters and archived milestones | [milestones](../../../src/test/cli-milestone-management.test.ts) |
| `doc create`, `update`, `view`, `list`, `search`, `archive`, `restore`, `delete` | Nested paths, metadata/content edits, lifecycle, reference reporting | [docs](../../../src/test/documentation.test.ts), [archive](../../../src/test/document-archive.test.ts) |
| `decision create`, `edit`, `view`, `list`, `archive`, `restore`, `delete` | Sections, status, supersession, unmanaged content, JSON views/lists and lifecycle | [content](../../../src/test/cli-decision-content.test.ts), [archive](../../../src/test/cli-decision-archive.test.ts) |
| `search` | Task/document/decision ranking, filters, literal arguments after `--`, versioned JSON | [search](../../../src/test/cli-search-command.test.ts), [parity](../../../src/test/task-search-parity.test.ts) |
| `board`, `board view`, `board export`; `overview` | Interactive views, plain board and Markdown/README export, statistics | [board](../../../src/test/board.test.ts), [overview source](../../../src/commands/overview.ts) |
| `config`, `get`, `set`, `list`; `agents --update-instructions` | Validated configuration, advanced wizard, shipped agent instructions | [config](../../../src/test/config-commands.test.ts), [instructions](../../../src/test/agent-instructions.test.ts) |
| `doctor`, `cleanup` | Diagnoses, explicit preview/review, stale-preview refusal, repair evidence and noninteractive handling | [repair](../../../src/test/duplicate-task-repair.test.ts), [cleanup](../../../src/test/cli-cleanup-input.test.ts) |
| `browser` | Host/port configuration, optional browser launch, shutdown | [ports](../../../src/test/cli-browser-port.test.ts), [hostname](../../../src/test/server-hostname.test.ts) |
| `mcp start` | Stdio only, cwd/root discovery, EOF/signals and no diagnostic stdout | [stdio](../../../src/test/mcp-stdio-exit.test.ts), [roots](../../../src/test/mcp-roots-discovery.test.ts) |
| `instructions [guide]`, `--list` | overview, task-creation, task-execution, task-finalization, init-required; configured task-ID examples | [source](../../../src/commands/instructions.ts), [instructions](../../../src/test/agent-instructions.test.ts) |
| `completion install`, hidden `__complete` | bash/zsh/fish/PowerShell installation and completion protocol | [completion](../../../src/commands/completion.test.ts), [source](../../../src/commands/completion.ts) |

JSON envelopes have `schemaVersion: 1`; kinds include task-list, task-view,
draft-list, decision-list, decision-view and search. Curated record fields,
nullability, timestamps, direct dependencies, readiness and paging are owned by
[the formatter](../../../src/formatters/json-output.ts). JSON errors go to
stderr with failing exit codes and empty stdout. `task list --json --watch`
emits successive complete replacement values; consumers must not assume NDJSON.
Preserve cancellation and parent-process lifetime behavior from
[watch coverage](../../../src/test/cli-json-watch.test.ts).

## State, configuration and shared semantics

Authoritative owners: [configuration and filesystem](../../../src/file-system/operations.ts),
[types](../../../src/types/index.ts), [core](../../../src/core/backlog.ts),
[content store](../../../src/core/content-store.ts), [parser](../../../src/markdown/parser.ts),
[frontmatter](../../../src/markdown/frontmatter.ts) and [Git](../../../src/git/operations.ts).

Configuration includes projectName, defaultAssignee/defaultReporter, statuses,
labels, types, priorities, projects, legacy milestones, definitionOfDone,
defaultStatus, dateFormat, maxColumnWidth, taskResolutionStrategy, defaultEditor,
autoOpenBrowser, defaultPort, hideEmptyColumns, remoteOperations, autoCommit,
filesystemOnly, zeroPaddedIds, includeDateTimeInDates, bypassGitHooks,
checkActiveBranches, activeBranchDays, backlogDirectory, onStatusChange and
prefixes/taskPrefix. Preserve supported snake_case file keys and camelCase CLI
keys, defaults, validation and migration. The type's legacy `mcp.http` shape does
not authorize a new HTTP MCP transport; the shipped MCP command is stdio only.

Discovery includes `backlog/`, `.backlog/`, project-root configuration and custom
relative paths. Records include tasks, drafts, completed/archived tasks,
documents, decisions and milestones. Preserve numeric/subtask identity,
unknown YAML text and comments, aliases/anchors, managed section markers,
unmanaged text, checks, comments, UTC instants and date-only due dates.

Concurrency and failure gates include allocation locks, worktree/branch IDs,
read/modify/write locks, atomic creation, no-overwrite moves, ambiguous identity
refusals, dependencies/cycles, archived read-only state, reference cleanup,
Git trust and partial mutation/commit reporting. Relevant existing suites:
[atomic creation](../../../src/test/atomic-task-create.test.ts),
[atomic edits](../../../src/test/atomic-task-edit.test.ts),
[worktree allocation](../../../src/test/worktree-task-id-allocation.test.ts),
[task YAML](../../../src/test/task-frontmatter-preservation.test.ts),
[content YAML](../../../src/test/content-frontmatter-preservation.test.ts),
[identity](../../../src/test/task-identity-index.test.ts),
[Git trust](../../../src/test/git-certificate-trust.test.ts).

## HTTP and browser

The method/path inventory below comes from [the server](../../../src/server/index.ts).
Request/response fields and partial-success handling are consumed by
[the browser API](../../../src/web/lib/api.ts); inspect both when porting a route.

| Path | Methods |
| --- | --- |
| `/api/tasks` | GET, POST |
| `/api/task/:id` | GET |
| `/api/tasks/:id` | GET, PUT |
| `/api/tasks/:id/archive`, `/api/tasks/:id/demote` | POST |
| `/api/statuses` | GET |
| `/api/config` | GET, PUT |
| `/api/docs` | GET, POST |
| `/api/doc/:id` | GET |
| `/api/docs/:id` | GET, PUT |
| `/api/docs/:id/archive` | POST |
| `/api/decisions` | GET, POST |
| `/api/decision/:id` | GET |
| `/api/decisions/:id` | GET, PUT |
| `/api/decisions/:id/archive` | POST |
| `/api/drafts` | GET |
| `/api/drafts/:id/promote` | POST |
| `/api/milestones`, `/api/milestones/archived` | GET; POST also on `/api/milestones` |
| `/api/milestones/:id` | GET, PUT, DELETE |
| `/api/milestones/:id/archive` | POST |
| `/api/tasks/reorder`, `/api/tasks/move` | POST |
| `/api/tasks/cleanup` | GET |
| `/api/tasks/cleanup/execute` | POST |
| `/api/tasks/duplicates` | GET, POST |
| `/api/version`, `/api/statistics`, `/api/status`, `/api/search` | GET |
| `/api/init` | POST |
| `/assets/*` | GET with existing path restrictions |

Preserve SPA routes `/`, `/tasks`, `/tasks/*`, `/board`, `/board/*`,
`/milestones`, `/drafts`, `/documentation`, `/documentation/*`, `/decisions`,
`/decisions/*`, `/statistics`, `/settings`, bundled resources and cache behavior.

WebSocket `/ws` sends loading-state JSON (`loading` with nullable message,
`loaded`, or `error` with message), plus `tasks-updated`, `milestones-updated`
and `config-updated` text events. Incoming messages receive `pong`. Preserve
debounced publication after state changes and milestone scope widening.
Coverage includes [assets](../../../src/test/server-assets.test.ts),
[publication](../../../src/test/server-reorder-publication.test.ts),
[loading](../../../src/test/server-loading-progress.test.ts),
[milestone events](../../../src/test/server-milestone-broadcast.test.ts),
[browser reconciliation](../../../src/web/utils/reconcile.test.ts).

## MCP

The shipped tool registrations are in [tasks](../../../src/mcp/tools/tasks/index.ts),
[documents](../../../src/mcp/tools/documents/index.ts),
[milestones](../../../src/mcp/tools/milestones/index.ts),
[Definition of Done](../../../src/mcp/tools/definition-of-done/index.ts) and
[workflow](../../../src/mcp/tools/workflow/index.ts):

- task_create, task_list, task_search, task_edit, task_view, task_archive,
  task_delete, task_restore.
- document_list, document_view, document_create, document_update,
  document_archive, document_restore, document_delete, document_search.
- milestone_list, milestone_add, milestone_rename, milestone_remove,
  milestone_archive.
- definition_of_done_defaults_get, definition_of_done_defaults_upsert.
- get_backlog_instructions.

Preserve each registration's schemas, annotations, error/result payloads and
initialization behavior; do not infer tools for every CLI feature. Workflow
resources are `backlog://workflow/overview`, `task-creation`, `task-execution`,
`task-finalization` under that URI prefix, and `backlog://init-required` when
appropriate. See [guides](../../../src/mcp/workflow-guides.ts),
[server](../../../src/mcp/server.ts), [MCP tests](../../../src/test/mcp-server.test.ts),
[task tests](../../../src/test/mcp-tasks.test.ts).

## TUI and distribution

Preserve board/list switching, popup details, keyboard/vim and mouse navigation,
focus/scroll, filters/search, readiness, task composition, checklists, task/draft
editing and lifecycle, multi-selection moves, external editor handoff, Unicode
display and terminal cleanup. Existing anchors include
[multi-move PTY](../../../src/test/board-tui-multi-move-pty.test.ts),
[editor PTY](../../../src/test/tui-interactive-editor-handoff.test.ts),
[ready filter PTY](../../../src/test/tui-ready-filter-pty.test.ts),
[input lifecycle](../../../src/test/tui-input-lifecycle.test.ts),
[task composer](../../../src/test/tui-task-composer.test.ts).

Distribution is six native targets: Linux, macOS and Windows, each x64/arm64.
Preserve npm platform resolution and the Node launcher; direct native execution
must need neither Node nor Bun. Nix currently supports x86_64-linux,
aarch64-linux and aarch64-darwin. Preserve version provenance, supported CPU
behavior, embedded browser assets, installation outside the source checkout and
the GitHub Release-driven publishing flow. Owners:
[build](../../../scripts/build.ts), [smoke](../../../scripts/smoke-compiled-build.ts),
[npm launcher](../../../scripts/cli.cjs), [Nix](../../../flake.nix),
[CI](../../../.github/workflows/ci.yml), [release](../../../.github/workflows/release.yml).

## Verification status

On 2026-11-07 the unchanged reference bundle builds and the type check passes
(558 files). Biome passes (479 files). The full CI profile passes: 3,286 backend
tests, 406 DOM tests, zero failures and 13 skips. These counts predate migration
harness additions. Commands used (with Bun 1.4.4 on PATH):

```sh
BACKLOG_BUILD_OUTDIR=/tmp/backlog-rust-migration/reference bun scripts/build.ts
bun run check:types
bun run check
BACKLOG_TEST_CLI_BUNDLE=/tmp/backlog-rust-migration/reference/cli.js bun scripts/run-ci-tests.ts --profile=full --parallel=2 --isolate --timeout=10000 --max-concurrency=2
```

The comparison entry point takes JSON argument vectors, not shell commands:

```sh
bun scripts/migration/compare-cli.ts --reference '["/absolute/bun","/absolute/reference/cli.js"]' --candidate '["/absolute/rust/target/debug/backlog"]'
bun test --timeout=10000 scripts/migration/compare-cli.test.ts
```

Both vectors must use absolute paths for file arguments, because processes run
inside their disposable projects. The default comparison exercises 29 CLI
scenarios, including JSON/plain reads, filtering, missing-ID and output-mode
refusals, task creation/edit/archive/restore/delete, drafts, decisions,
documents and milestones. Every step checks its required exit code and compares
both process output and the resulting filesystem. Generated timestamps are
normalized only in owned metadata fields within the run's clock window, with
their formatting retained; seeded timestamps, unknown metadata and body text
remain exact. Stderr and non-JSON output remain exact apart from fixture roots.

The executable comparison runner is a starting gate for representative CLI
workflows, not a substitute for the inventory above. HTTP, WebSocket, MCP,
interactive TUI, concurrency, Git/worktrees and platform packaging require their
own process-level cases as those stages are implemented. No Rust parity or
cross-platform result has been established at this point.
