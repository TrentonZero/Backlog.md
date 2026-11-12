//! Native Rust CLI for noninteractive local backlog commands.

use crate::{
    config::{Config, ConfigLocation, initial_config, parse_config, serialize_config},
    decision_store::DecisionStore,
    dependency_graph::{DependencyGraph, DependencyGraphNode},
    discovery::{
        BacklogDirectorySource, find_project_root, normalize_backlog_directory,
        resolve_backlog_directory, resolve_runtime_cwd,
    },
    document_store::DocumentStore,
    draft_store::DraftStore,
    identity::{canonical_task_id, normalize_task_id, task_ids_equal},
    milestone_store::MilestoneStore,
    records::TaskRecord,
    sections::{ChecklistKind, Comment, Criterion, SectionKey},
    task_detail::{TaskDetail, build_task_detail},
    task_filtering::{LabelMatch, TaskFilter, TaskFilterMatcher},
    task_list::{TaskListItem, with_readiness},
    task_search::{
        TaskSearchFields, TaskSearchIndex, build_task_search_fields, search_fields_score,
    },
    task_sorting::{TaskSection, section_tasks, sort_tasks},
    task_store::TaskStore,
};
use clap::{ArgAction, Args, CommandFactory, Parser, Subcommand, ValueEnum};
use markdown::{ParseOptions, mdast::Node, to_mdast};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{self, IsTerminal, Write},
    path::PathBuf,
};

#[derive(Debug, Parser)]
#[command(name = "backlog", version, about = "Manage a Markdown task backlog")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Task commands.
    #[command(alias = "tasks")]
    Task {
        #[command(subcommand)]
        command: Option<TaskCommand>,
        /// Task ID shorthand; equivalent to `task view <taskId>`.
        task_id: Option<String>,
        /// Use human-readable output for the task ID shorthand.
        #[arg(long)]
        plain: bool,
        /// Use versioned JSON output for the task ID shorthand.
        #[arg(long)]
        json: bool,
    },
    /// Draft commands.
    Draft {
        #[command(subcommand)]
        command: Option<DraftCommand>,
        /// Draft ID shorthand; equivalent to `draft view <draftId>`.
        task_id: Option<String>,
        /// Use human-readable output for the draft ID shorthand.
        #[arg(long)]
        plain: bool,
    },
    /// Document commands.
    Doc {
        #[command(subcommand)]
        command: DocCommand,
    },
    /// Decision commands.
    Decision {
        #[command(subcommand)]
        command: DecisionCommand,
    },
    /// List and create milestones.
    #[command(alias = "milestones")]
    Milestone {
        #[command(subcommand)]
        command: Option<MilestoneCommand>,
    },
    /// Search tasks, documents, and decisions.
    Search(SearchArgs),
    /// Read or change project configuration.
    Config {
        #[command(subcommand)]
        command: Option<ConfigCommand>,
    },
    /// Initialize a Backlog.md project.
    Init(InitArgs),
    /// Refresh shipped agent instructions.
    Agents {
        #[arg(long)]
        update_instructions: bool,
    },
}

#[derive(Debug, Subcommand)]
enum ConfigCommand {
    Get { key: String },
    Set { key: String, value: String },
    List,
}

#[derive(Debug, Args)]
struct InitArgs {
    /// Project name; prompted when omitted.
    project_name: Option<String>,
    /// Comma-separated agent instruction files to create.
    #[arg(long)]
    agent_instructions: Option<String>,
    /// Check task states across active branches.
    #[arg(long)]
    check_branches: Option<String>,
    /// Include remote branches when checking.
    #[arg(long)]
    include_remote: Option<String>,
    /// Days to consider a branch active.
    #[arg(long)]
    branch_days: Option<String>,
    /// Bypass Git hooks when committing.
    #[arg(long)]
    bypass_git_hooks: Option<String>,
    /// Number of digits for zero-padded IDs (0 disables).
    #[arg(long)]
    zero_padded_ids: Option<String>,
    /// Default editor command.
    #[arg(long)]
    default_editor: Option<String>,
    /// Default web UI port.
    #[arg(long)]
    web_port: Option<String>,
    /// Automatically open the browser when starting the web UI.
    #[arg(long)]
    auto_open_browser: Option<String>,
    /// Install the Claude Code agent.
    #[arg(long)]
    install_claude_agent: Option<String>,
    /// AI integration mode: cli, mcp, or none.
    #[arg(long)]
    integration_mode: Option<String>,
    /// Backlog folder path, relative to the project root.
    #[arg(long)]
    backlog_dir: Option<String>,
    /// Config location: folder or root.
    #[arg(long)]
    config_location: Option<String>,
    /// Task ID prefix; letters only.
    #[arg(long)]
    task_prefix: Option<String>,
    /// Skip Git integration.
    #[arg(long)]
    no_git: bool,
    /// Use default values for setup prompts.
    #[arg(long)]
    defaults: bool,
}

#[derive(Debug, Subcommand)]
enum DecisionCommand {
    /// Create a decision.
    Create(DecisionCreateArgs),
    /// Edit an active decision.
    Edit(DecisionEditArgs),
    /// List active decisions.
    List(DecisionListArgs),
    /// View an active or archived decision.
    View {
        decision_id: String,
        #[command(flatten)]
        options: DecisionViewArgs,
    },
    /// Move a decision into the archive, keeping its ID reserved.
    Archive { decision_id: String },
    /// Move an archived decision back to the decisions folder.
    Restore { decision_id: String },
    /// Permanently delete an archived decision without prompting.
    Delete { decision_id: String },
}

#[derive(Debug, Subcommand)]
enum MilestoneCommand {
    /// List milestones with completion status.
    List(MilestoneListArgs),
    /// Add a milestone file.
    Add(MilestoneAddArgs),
    /// Rename an active milestone and optionally update task references.
    Rename(MilestoneRenameArgs),
    /// Archive an active milestone.
    Archive { name: String },
    /// Remove an active milestone and handle task references.
    Remove(MilestoneRemoveArgs),
}

#[derive(Debug, Args)]
struct MilestoneListArgs {
    /// Include completed milestones.
    #[arg(long)]
    show_completed: bool,
    /// Use human-readable text output.
    #[arg(long)]
    plain: bool,
    #[command(flatten)]
    window: ListWindowArgs,
}

#[derive(Debug, Args)]
struct MilestoneAddArgs {
    /// Milestone name.
    name: String,
    /// Milestone description.
    #[arg(short, long)]
    description: Option<String>,
    /// Set due date (YYYY-MM-DD).
    #[arg(long)]
    due_date: Option<String>,
}

#[derive(Debug, Args)]
struct MilestoneRenameArgs {
    /// Active milestone ID or title.
    from: String,
    /// New title.
    to: String,
    /// Do not update local task milestone references.
    #[arg(long = "no-update-tasks", action = ArgAction::SetFalse, default_value_t = true)]
    update_tasks: bool,
    /// Set the milestone due date.
    #[arg(long)]
    due_date: Option<String>,
    /// Clear the milestone due date.
    #[arg(long)]
    clear_due_date: bool,
}

#[derive(Debug, Args)]
struct MilestoneRemoveArgs {
    /// Active milestone ID or title.
    name: String,
    /// How to handle matching tasks.
    #[arg(long, default_value = "clear")]
    task_handling: String,
    /// Active target milestone when task handling is reassign.
    #[arg(long)]
    reassign_to: Option<String>,
}

#[derive(Debug, Args)]
struct SearchArgs {
    /// Fuzzy search text.
    #[arg(allow_hyphen_values = true)]
    query: Option<String>,
    /// Limit results to task, document, or decision; repeat the option to select multiple types.
    #[arg(long = "type", action = ArgAction::Append)]
    result_types: Option<Vec<String>>,
    /// Filter task results by configured type; repeat or comma-separate values.
    #[arg(long, value_delimiter = ',', action = ArgAction::Append)]
    task_type: Option<Vec<String>>,
    /// Filter task results by status; repeat or comma-separate values.
    #[arg(long, value_delimiter = ',', action = ArgAction::Append)]
    status: Option<Vec<String>>,
    /// Exclude task results by status; repeat or comma-separate values.
    #[arg(long, value_delimiter = ',', action = ArgAction::Append)]
    exclude_status: Option<Vec<String>>,
    /// Filter task results by configured priority.
    #[arg(long)]
    priority: Option<String>,
    /// Filter task results by configured project; repeat or comma-separate values.
    #[arg(long, value_delimiter = ',', action = ArgAction::Append)]
    project: Option<Vec<String>>,
    /// Filter task results by modified-file path substring; repeat or comma-separate values.
    #[arg(long = "modified-file", value_delimiter = ',', action = ArgAction::Append)]
    modified_file: Option<Vec<String>>,
    /// Maximum total search results before applying the output window.
    #[arg(long, allow_hyphen_values = true)]
    limit: Option<String>,
    #[command(flatten)]
    window: ListWindowArgs,
    /// Use human-readable text output.
    #[arg(long)]
    plain: bool,
    /// Use versioned JSON output.
    #[arg(long)]
    json: bool,
}

#[derive(Clone, Copy)]
enum SearchResultRef<'a> {
    Task(&'a TaskRecord),
    Document(&'a crate::document_store::DocumentFileEntry),
    Decision(&'a crate::decision_store::DecisionFileEntry),
}

impl SearchResultRef<'_> {
    fn kind(self) -> &'static str {
        match self {
            Self::Task(_) => "task",
            Self::Document(_) => "document",
            Self::Decision(_) => "decision",
        }
    }
}

struct SearchIndexedResult<'a> {
    result: SearchResultRef<'a>,
    fields: TaskSearchFields,
}

struct RankedSearchResult<'a> {
    result: SearchResultRef<'a>,
    score: Option<f64>,
}

#[derive(Debug, Args)]
struct DecisionCreateArgs {
    /// Decision title.
    title: String,
    /// Decision status; defaults to proposed.
    #[arg(short, long)]
    status: Option<String>,
    /// Replace the Context section.
    #[arg(long)]
    context: Option<String>,
    /// Replace the Decision section.
    #[arg(long)]
    decision: Option<String>,
    /// Replace the Consequences section.
    #[arg(long)]
    consequences: Option<String>,
    /// Replace the Alternatives section.
    #[arg(long)]
    alternatives: Option<String>,
    /// Use human-readable output.
    #[arg(long)]
    plain: bool,
}

#[derive(Debug, Args)]
struct DecisionEditArgs {
    /// Decision ID.
    decision_id: String,
    /// Replace the decision title.
    #[arg(short, long)]
    title: Option<String>,
    /// Replace the decision status.
    #[arg(short, long)]
    status: Option<String>,
    /// Replace the Context section.
    #[arg(long)]
    context: Option<String>,
    /// Replace the Decision section.
    #[arg(long)]
    decision: Option<String>,
    /// Replace the Consequences section.
    #[arg(long)]
    consequences: Option<String>,
    /// Replace the Alternatives section.
    #[arg(long)]
    alternatives: Option<String>,
    /// Set the replacing decision; pass an empty value to clear.
    #[arg(long)]
    superseded_by: Option<String>,
    /// Use human-readable output.
    #[arg(long)]
    plain: bool,
}

#[derive(Debug, Args)]
struct DecisionListArgs {
    #[command(flatten)]
    window: ListWindowArgs,
    /// Use human-readable text output.
    #[arg(long)]
    plain: bool,
    /// Use versioned JSON output.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct DecisionViewArgs {
    /// Use human-readable text output.
    #[arg(long)]
    plain: bool,
    /// Use versioned JSON output.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Subcommand)]
enum DocCommand {
    /// Create a document.
    Create(DocCreateArgs),
    /// Update a document.
    Update(DocUpdateArgs),
    /// List documents in the configured docs directory.
    List(DocListArgs),
    /// Display a document's Markdown content.
    View {
        document_id: String,
        #[command(flatten)]
        options: DocumentViewArgs,
    },
    /// Move a document into the archive.
    Archive { document_id: String },
    /// Restore an archived document.
    Restore { document_id: String },
    /// Permanently delete an archived document.
    Delete { document_id: String },
}

#[derive(Debug, Args)]
struct DocCreateArgs {
    /// Document title.
    title: String,
    /// Subdirectory under the docs directory.
    #[arg(short, long)]
    path: Option<String>,
    /// Document type.
    #[arg(short = 't', long = "type")]
    document_type: Option<String>,
    /// Use human-readable text output.
    #[arg(long)]
    plain: bool,
}

#[derive(Debug, Args)]
struct DocUpdateArgs {
    /// Document ID.
    document_id: String,
    /// Replace the document title.
    #[arg(long)]
    title: Option<String>,
    /// Replace the document's Markdown body.
    #[arg(long)]
    content: Option<String>,
    /// Move the document under a docs-relative path.
    #[arg(short, long)]
    path: Option<String>,
    /// Replace the document type.
    #[arg(short = 't', long = "type")]
    document_type: Option<String>,
    /// Replace tags (comma-separated; repeatable).
    #[arg(long, value_delimiter = ',', action = ArgAction::Append)]
    tags: Option<Vec<String>>,
}

#[derive(Debug, Args)]
struct DocumentViewArgs {
    /// Use human-readable text output.
    #[arg(long)]
    plain: bool,
}

#[derive(Debug, Args)]
struct ListWindowArgs {
    /// Print at most this many items after sorting.
    #[arg(long, allow_hyphen_values = true)]
    max_count: Option<String>,
    /// Leave out this many items after sorting.
    #[arg(long, allow_hyphen_values = true)]
    skip: Option<String>,
    /// Print only the number of items in the selected window.
    #[arg(long)]
    count: bool,
}

#[derive(Clone, Copy, Debug)]
struct ListWindow {
    max_count: Option<usize>,
    skip: usize,
    count: bool,
}

#[derive(Debug, Args)]
struct DocListArgs {
    #[command(flatten)]
    window: ListWindowArgs,
    /// Use human-readable text output.
    #[arg(long)]
    plain: bool,
}

#[derive(Debug, Subcommand)]
enum DraftCommand {
    /// List local drafts.
    List(DraftListArgs),
    /// Create a local draft.
    Create(DraftCreateArgs),
    /// Display one draft's details.
    View {
        task_id: String,
        #[command(flatten)]
        options: DraftViewArgs,
    },
    /// Edit one local draft.
    Edit(Box<TaskEditArgs>),
    /// Archive a draft.
    Archive { task_id: String },
    /// Promote a draft to an active task.
    Promote { task_id: String },
}

#[derive(Debug, Args)]
struct DraftViewArgs {
    /// Use human-readable output.
    #[arg(long)]
    plain: bool,
}

#[derive(Debug, Args)]
struct DraftCreateArgs {
    /// Draft title.
    title: String,
    /// Draft description.
    #[arg(short, long)]
    description: Option<String>,
    /// Description alias.
    #[arg(long)]
    desc: Option<String>,
    /// Assign one or more @names; repeat or comma-separate values.
    #[arg(short, long, value_delimiter = ',', action = ArgAction::Append)]
    assignee: Option<Vec<String>>,
    /// Accepted for task create compatibility; drafts always have Draft status.
    #[arg(short, long)]
    status: Option<String>,
    /// Draft labels; repeat or comma-separate values.
    #[arg(short, long, value_delimiter = ',', action = ArgAction::Append)]
    labels: Option<Vec<String>>,
}

#[derive(Debug, Args)]
struct DraftListArgs {
    /// Use human-readable text output.
    #[arg(long)]
    plain: bool,
    /// Use versioned JSON output.
    #[arg(long)]
    json: bool,
    /// Sort drafts by priority, ID, or ordinal.
    #[arg(long)]
    sort: Option<String>,
    #[command(flatten)]
    window: ListWindowArgs,
}

#[derive(Debug, Subcommand)]
enum TaskCommand {
    /// List tasks grouped by status.
    List(TaskListArgs),
    /// Display one task's details.
    View {
        task_id: String,
        #[command(flatten)]
        options: TaskViewArgs,
    },
    /// Create a task in the local working copy.
    Create(Box<TaskCreateArgs>),
    /// Edit one task in the local working copy.
    Edit(Box<TaskEditArgs>),
    /// Moves a task off the board into the archive. Its ID stays reserved, and its record, dependencies, and references are kept; no other task is changed.
    Archive { task_id: String },
    /// Moves an archived task back to the board with the same ID and the status it had. Archived tasks are read-only, so restore a task before editing it. A task that is already on the board is left as it is.
    Restore { task_id: String },
    /// Permanently removes an archived task and its ID when no other branch holds it. Removes links other tasks and drafts hold; requires archive first, refuses existing subtasks, and has no prompt.
    Delete { task_id: String },
    /// Move an active task back to drafts.
    Demote { task_id: String },
    /// Removed command; directs callers to archive.
    #[command(hide = true, disable_help_flag = true)]
    Complete {
        /// Accepted for compatibility with the removed command.
        #[arg(num_args = 0.., allow_hyphen_values = true)]
        ignored: Vec<String>,
    },
}

#[derive(Debug, Args)]
struct TaskCreateArgs {
    /// Task title.
    title: Option<String>,
    /// Task description.
    #[arg(short, long, visible_alias = "desc")]
    description: Option<String>,
    /// Initial task status.
    #[arg(short, long)]
    status: Option<String>,
    /// Assign one or more @names; repeat or comma-separate values.
    #[arg(short, long, value_delimiter = ',', action = ArgAction::Append)]
    assignee: Vec<String>,
    /// Task labels; repeat or comma-separate values.
    #[arg(short, long, value_delimiter = ',', action = ArgAction::Append)]
    labels: Vec<String>,
    /// Configured task priority.
    #[arg(long)]
    priority: Option<String>,
    /// Configured task type.
    #[arg(long = "type")]
    task_type: Option<String>,
    /// Configured project.
    #[arg(long)]
    project: Option<String>,
    /// Milestone ID or title.
    #[arg(short, long)]
    milestone: Option<String>,
    /// Due date in YYYY-MM-DD format.
    #[arg(long)]
    due_date: Option<String>,
    /// Acceptance criterion; repeat for multiple criteria.
    #[arg(long = "ac", visible_alias = "acceptance-criteria", action = ArgAction::Append)]
    acceptance_criteria: Vec<String>,
    /// Definition of Done item; repeat for multiple items.
    #[arg(long = "dod", action = ArgAction::Append)]
    definition_of_done: Vec<String>,
    /// Do not include configured Definition of Done defaults.
    #[arg(long)]
    no_dod_defaults: bool,
    /// Implementation plan for already-started work.
    #[arg(long)]
    plan: Option<String>,
    /// Implementation notes.
    #[arg(long)]
    notes: Option<String>,
    /// Final summary for completed work.
    #[arg(long)]
    final_summary: Option<String>,
    /// Manual non-negative ordinal.
    #[arg(long)]
    ordinal: Option<f64>,
    /// Existing parent task ID.
    #[arg(short, long)]
    parent: Option<String>,
    /// Task dependency; repeat or comma-separate values.
    #[arg(long = "depends-on", action = ArgAction::Append)]
    depends_on: Vec<String>,
    /// Alias for --depends-on.
    #[arg(long = "dep", action = ArgAction::Append)]
    dep: Vec<String>,
    /// Reference URL or path; repeat for multiple references.
    #[arg(long = "ref", action = ArgAction::Append)]
    references: Vec<String>,
    /// Documentation URL or path; repeat for multiple entries.
    #[arg(long = "doc", action = ArgAction::Append)]
    documentation: Vec<String>,
    /// Modified file path; repeat for multiple paths.
    #[arg(long = "modified-file", action = ArgAction::Append)]
    modified_files: Vec<String>,
    /// Print the full task detail after creation.
    #[arg(long)]
    plain: bool,
}

#[derive(Clone, Debug, Args)]
struct TaskEditArgs {
    /// Task ID to edit.
    task_id: String,
    /// Replacement title.
    #[arg(short, long)]
    title: Option<String>,
    /// Replacement description.
    #[arg(short, long, visible_alias = "desc")]
    description: Option<String>,
    /// Replace the implementation plan.
    #[arg(long)]
    plan: Option<String>,
    /// Append implementation plan text; repeat for multiple blocks.
    #[arg(long = "append-plan", action = ArgAction::Append)]
    append_plan: Vec<String>,
    /// Replace implementation notes.
    #[arg(long)]
    notes: Option<String>,
    /// Append implementation notes; repeat for multiple blocks.
    #[arg(long = "append-notes", action = ArgAction::Append)]
    append_notes: Vec<String>,
    /// Append a comment; repeat for multiple comments.
    #[arg(long, action = ArgAction::Append)]
    comment: Vec<String>,
    /// Author to record on appended comments.
    #[arg(long)]
    comment_author: Option<String>,
    /// Replace the final summary.
    #[arg(long)]
    final_summary: Option<String>,
    /// Append final summary text; repeat for multiple blocks.
    #[arg(long = "append-final-summary", action = ArgAction::Append)]
    append_final_summary: Vec<String>,
    /// Remove the final summary.
    #[arg(long)]
    clear_final_summary: bool,
    /// Replace modified file paths; repeat or comma-separate values.
    #[arg(long = "modified-file", value_delimiter = ',', action = ArgAction::Append)]
    modified_files: Option<Vec<String>>,
    /// Replacement status.
    #[arg(
        short,
        long,
        help = "Replacement status; draft edits only accept Draft."
    )]
    status: Option<String>,
    /// Replace all assignees; repeat or comma-separate values.
    #[arg(short, long, value_delimiter = ',', action = ArgAction::Append)]
    assignee: Option<Vec<String>>,
    /// Replace all labels; repeat or comma-separate values.
    #[arg(short = 'l', long = "label", value_delimiter = ',', action = ArgAction::Append)]
    labels: Option<Vec<String>>,
    /// Add labels without replacing existing labels.
    #[arg(long = "add-label", value_delimiter = ',', action = ArgAction::Append)]
    add_labels: Vec<String>,
    /// Remove labels without replacing other labels.
    #[arg(long = "remove-label", value_delimiter = ',', action = ArgAction::Append)]
    remove_labels: Vec<String>,
    /// Remove all labels.
    #[arg(long)]
    clear_labels: bool,
    /// Configured task priority.
    #[arg(long)]
    priority: Option<String>,
    /// Configured task type; pass an empty value to clear.
    #[arg(long = "type")]
    task_type: Option<String>,
    /// Configured project; pass an empty value to clear.
    #[arg(long)]
    project: Option<String>,
    /// Set due date in YYYY-MM-DD format.
    #[arg(long)]
    due_date: Option<String>,
    /// Clear the due date.
    #[arg(long)]
    clear_due_date: bool,
    /// Set manual non-negative ordinal.
    #[arg(long)]
    ordinal: Option<f64>,
    /// Assign milestone by ID or title.
    #[arg(short, long)]
    milestone: Option<String>,
    /// Clear the milestone assignment.
    #[arg(long)]
    clear_milestone: bool,
    /// Set dependencies; repeat or comma-separate IDs. Pass an empty value to clear.
    #[arg(long = "depends-on", visible_alias = "dep", value_delimiter = ',', action = ArgAction::Append)]
    dependencies: Option<Vec<String>>,
    /// Clear all dependencies.
    #[arg(long)]
    clear_deps: bool,
    /// Replace all references; repeat or comma-separate values. Pass an empty value to clear.
    #[arg(long = "ref", value_delimiter = ',', action = ArgAction::Append)]
    references: Option<Vec<String>>,
    /// Add references without replacing existing references.
    #[arg(long = "add-ref", value_delimiter = ',', action = ArgAction::Append)]
    add_references: Vec<String>,
    /// Remove references without replacing other references.
    #[arg(long = "remove-ref", value_delimiter = ',', action = ArgAction::Append)]
    remove_references: Vec<String>,
    /// Clear all references.
    #[arg(long)]
    clear_refs: bool,
    /// Replace all documentation; repeat or comma-separate values. Pass an empty value to clear.
    #[arg(long = "doc", value_delimiter = ',', action = ArgAction::Append)]
    documentation: Option<Vec<String>>,
    /// Add documentation without replacing existing documentation.
    #[arg(long = "add-doc", value_delimiter = ',', action = ArgAction::Append)]
    add_documentation: Vec<String>,
    /// Remove documentation without replacing other entries.
    #[arg(long = "remove-doc", value_delimiter = ',', action = ArgAction::Append)]
    remove_documentation: Vec<String>,
    /// Clear all documentation.
    #[arg(long)]
    clear_docs: bool,
    /// Add an acceptance criterion; repeat for multiple criteria.
    #[arg(long = "ac", action = ArgAction::Append)]
    add_ac: Vec<String>,
    /// Replace all acceptance criteria; repeat for multiple criteria.
    #[arg(long = "acceptance-criteria", action = ArgAction::Append)]
    acceptance_criteria: Option<Vec<String>>,
    /// Clear all acceptance criteria.
    #[arg(long)]
    clear_ac: bool,
    /// Remove acceptance criteria by 1-based index.
    #[arg(long = "remove-ac", action = ArgAction::Append)]
    remove_ac: Vec<String>,
    /// Check acceptance criteria by 1-based index.
    #[arg(long = "check-ac", action = ArgAction::Append)]
    check_ac: Vec<String>,
    /// Uncheck acceptance criteria by 1-based index.
    #[arg(long = "uncheck-ac", action = ArgAction::Append)]
    uncheck_ac: Vec<String>,
    /// Add a Definition of Done item; repeat for multiple items.
    #[arg(long = "dod", action = ArgAction::Append)]
    add_dod: Vec<String>,
    /// Remove Definition of Done items by 1-based index.
    #[arg(long = "remove-dod", action = ArgAction::Append)]
    remove_dod: Vec<String>,
    /// Check Definition of Done items by 1-based index.
    #[arg(long = "check-dod", action = ArgAction::Append)]
    check_dod: Vec<String>,
    /// Uncheck Definition of Done items by 1-based index.
    #[arg(long = "uncheck-dod", action = ArgAction::Append)]
    uncheck_dod: Vec<String>,
    /// Print full task detail after editing.
    #[arg(long)]
    plain: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum SortField {
    Priority,
    Id,
    Ordinal,
}

#[derive(Debug, Args)]
#[command(group(clap::ArgGroup::new("output").required(true).args(["plain", "json"])))]
struct TaskListArgs {
    /// Use human-readable text output.
    #[arg(long, conflicts_with = "json")]
    plain: bool,
    /// Use versioned JSON output.
    #[arg(long, conflicts_with = "plain")]
    json: bool,
    /// Filter by one or more statuses; repeat or comma-separate values.
    #[arg(short, long, value_delimiter = ',', action = ArgAction::Append)]
    status: Vec<String>,
    /// Exclude one or more statuses; repeat or comma-separate values.
    #[arg(long, value_delimiter = ',', action = ArgAction::Append)]
    exclude_status: Vec<String>,
    /// Search task titles and searchable task content.
    #[arg(long)]
    search: Option<String>,
    /// Require every comma-separated label.
    #[arg(long, value_delimiter = ',', action = ArgAction::Append)]
    labels: Vec<String>,
    /// Order by priority, ID, or ordinal.
    #[arg(long, value_enum)]
    sort: Option<SortField>,
    /// Limit the displayed results after filtering and sorting.
    #[arg(long, value_parser = parse_positive_limit)]
    limit: Option<usize>,
}

#[derive(Debug, Args)]
#[command(group(clap::ArgGroup::new("output").required(true).args(["plain", "json"])))]
struct TaskViewArgs {
    /// Use human-readable text output.
    #[arg(long, conflicts_with = "json")]
    plain: bool,
    /// Use versioned JSON output.
    #[arg(long, conflicts_with = "plain")]
    json: bool,
}

fn parse_positive_limit(value: &str) -> Result<usize, String> {
    let limit = value
        .parse::<usize>()
        .map_err(|_| "limit must be a positive integer".to_owned())?;
    if limit == 0 {
        return Err("limit must be a positive integer".to_owned());
    }
    Ok(limit)
}

struct ProjectContext {
    root: PathBuf,
    backlog_directory: PathBuf,
    config_path: Option<PathBuf>,
    config: Config,
    store: TaskStore,
    prefix: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EditTarget {
    Task,
    Draft,
}

#[derive(Clone, Debug)]
struct EditableRecord {
    task: TaskRecord,
    path: PathBuf,
}

pub fn run() -> Result<(), String> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let excluded = arguments.iter().any(|argument| {
        matches!(
            argument.as_str(),
            "init" | "--help" | "-h" | "--version" | "-v"
        )
    });
    if !arguments.is_empty() && !excluded {
        migrate_project_config();
    }
    let cli = Cli::parse();
    match cli.command {
        Command::Task {
            command: Some(TaskCommand::List(options)),
            ..
        } => run_task_list(options),
        Command::Task {
            command: Some(TaskCommand::View { task_id, options }),
            ..
        } => run_task_view(&task_id, options),
        Command::Task {
            command: Some(TaskCommand::Create(options)),
            ..
        } => run_task_create(*options),
        Command::Task {
            command: Some(TaskCommand::Edit(options)),
            ..
        } => run_task_edit(*options),
        Command::Task {
            command: Some(TaskCommand::Archive { task_id }),
            ..
        } => run_task_archive(&task_id),
        Command::Task {
            command: Some(TaskCommand::Restore { task_id }),
            ..
        } => run_task_restore(&task_id),
        Command::Task {
            command: Some(TaskCommand::Delete { task_id }),
            ..
        } => run_task_delete(&task_id),
        Command::Task {
            command: Some(TaskCommand::Demote { task_id }),
            ..
        } => run_task_demote(&task_id),
        Command::Task {
            command: Some(TaskCommand::Complete { ignored }),
            json,
            ..
        } => {
            if json || ignored.iter().any(|argument| argument == "--json") {
                return Err("error: unknown option '--json'\nRun with --help to see accepted fields and examples.".into());
            }
            Err("backlog task complete was removed. Use backlog task archive <id>.".into())
        }
        Command::Task {
            command: None,
            task_id: Some(task_id),
            plain,
            json,
        } => {
            if plain && json {
                return Err("--json cannot be combined with --plain.".into());
            }
            run_task_view(
                &task_id,
                TaskViewArgs {
                    plain: plain || !json,
                    json,
                },
            )
        }
        Command::Task {
            command: None,
            task_id: None,
            ..
        } => {
            let mut command = Cli::command();
            let task = command.find_subcommand_mut("task").unwrap();
            task.print_help().map_err(|error| error.to_string())?;
            write_stdout("\n")
        }
        Command::Draft {
            command: Some(DraftCommand::List(options)),
            ..
        } => run_draft_list(options),
        Command::Draft {
            command: Some(DraftCommand::Create(options)),
            ..
        } => run_draft_create(options),
        Command::Draft {
            command: Some(DraftCommand::View { task_id, options }),
            ..
        } => run_draft_view(&task_id, options),
        Command::Draft {
            command: Some(DraftCommand::Edit(options)),
            ..
        } => run_draft_edit(*options),
        Command::Draft {
            command: Some(DraftCommand::Archive { task_id }),
            ..
        } => run_draft_archive(&task_id),
        Command::Draft {
            command: Some(DraftCommand::Promote { task_id }),
            ..
        } => run_draft_promote(&task_id),
        Command::Draft {
            command: None,
            task_id: Some(task_id),
            plain: _,
        } => run_draft_view(&task_id, DraftViewArgs { plain: true }),
        Command::Draft {
            command: None,
            task_id: None,
            ..
        } => Err("Usage: backlog draft <command> or backlog draft <draftId>".into()),
        Command::Doc {
            command: DocCommand::Create(options),
        } => run_document_create(options),
        Command::Doc {
            command: DocCommand::Update(options),
        } => run_document_update(options),
        Command::Doc {
            command: DocCommand::List(options),
        } => run_document_list(options),
        Command::Doc {
            command:
                DocCommand::View {
                    document_id,
                    options,
                },
        } => run_document_view(&document_id, options),
        Command::Doc {
            command: DocCommand::Archive { document_id },
        } => run_document_lifecycle(&document_id, DocumentLifecycleOperation::Archive),
        Command::Doc {
            command: DocCommand::Restore { document_id },
        } => run_document_lifecycle(&document_id, DocumentLifecycleOperation::Restore),
        Command::Doc {
            command: DocCommand::Delete { document_id },
        } => run_document_lifecycle(&document_id, DocumentLifecycleOperation::Delete),
        Command::Decision {
            command: DecisionCommand::Create(options),
        } => run_decision_create(options),
        Command::Decision {
            command: DecisionCommand::Edit(options),
        } => run_decision_edit(options),
        Command::Decision {
            command: DecisionCommand::List(options),
        } => run_decision_list(options),
        Command::Decision {
            command:
                DecisionCommand::View {
                    decision_id,
                    options,
                },
        } => run_decision_view(&decision_id, options),
        Command::Decision {
            command: DecisionCommand::Archive { decision_id },
        } => run_decision_lifecycle(&decision_id, DecisionLifecycleOperation::Archive),
        Command::Decision {
            command: DecisionCommand::Restore { decision_id },
        } => run_decision_lifecycle(&decision_id, DecisionLifecycleOperation::Restore),
        Command::Decision {
            command: DecisionCommand::Delete { decision_id },
        } => run_decision_lifecycle(&decision_id, DecisionLifecycleOperation::Delete),
        Command::Milestone { command } => match command {
            Some(MilestoneCommand::List(options)) => run_milestone_list(options),
            Some(MilestoneCommand::Add(options)) => run_milestone_add(options),
            Some(MilestoneCommand::Rename(options)) => run_milestone_rename(options),
            Some(MilestoneCommand::Archive { name }) => run_milestone_archive(&name),
            Some(MilestoneCommand::Remove(options)) => run_milestone_remove(options),
            None => {
                let mut command = Cli::command();
                let milestone = command.find_subcommand_mut("milestone").unwrap();
                let mut help = Vec::new();
                milestone
                    .write_help(&mut help)
                    .map_err(|error| error.to_string())?;
                Err(String::from_utf8_lossy(&help).trim_end().to_owned())
            }
        },
        Command::Search(options) => run_search(options),
        Command::Config { command } => match command {
            Some(ConfigCommand::Get { key }) => run_config_get(&key),
            Some(ConfigCommand::Set { key, value }) => run_config_set(&key, &value),
            Some(ConfigCommand::List) => run_config_list(),
            None => run_config_wizard(),
        },
        Command::Init(options) => run_init(options),
        Command::Agents {
            update_instructions,
        } => {
            if update_instructions {
                run_agents_update()
            } else {
                let mut command = Cli::command();
                let agents = command.find_subcommand_mut("agents").unwrap();
                let mut help = Vec::new();
                agents
                    .write_help(&mut help)
                    .map_err(|error| error.to_string())?;
                write_stdout(&format!("{}\n", String::from_utf8_lossy(&help).trim_end()))
            }
        }
    }
}

#[derive(Clone, Copy)]
enum DocumentLifecycleOperation {
    Archive,
    Restore,
    Delete,
}

#[derive(Clone, Copy)]
enum DecisionLifecycleOperation {
    Archive,
    Restore,
    Delete,
}

fn project_context() -> Result<ProjectContext, String> {
    let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
    let root = find_project_root(&cwd).ok_or_else(|| {
        "No Backlog.md project found. Run `backlog init` to initialize.".to_owned()
    })?;
    let resolution = resolve_backlog_directory(&root);
    let backlog_directory = resolution.backlog_directory.ok_or_else(|| {
        "No Backlog.md project found. Run `backlog init` to initialize.".to_owned()
    })?;
    let config_path = resolution.config_path.clone();
    let config = match resolution.config_path {
        Some(path) => {
            let source = fs::read_to_string(&path).map_err(|error| error.to_string())?;
            parse_config(&source, &path).map_err(|error| error.to_string())?
        }
        None => Config::default(),
    };
    let prefix = config.task_prefix.as_deref().unwrap_or("task").to_owned();
    let backlog_directory = PathBuf::from(backlog_directory);
    let store = TaskStore::new(&root, &backlog_directory, &prefix);
    Ok(ProjectContext {
        root,
        backlog_directory,
        config_path,
        config,
        store,
        prefix,
    })
}

/// Apply the TypeScript CLI's pre-command schema migration without rewriting unrelated YAML.
/// Clap exits while parsing --help/--version and rejects init before `run` continues, matching
/// the TypeScript migration exclusions for those invocations.
fn migrate_project_config() {
    let Ok(cwd) = std::env::current_dir() else {
        return;
    };
    let Some(root) = find_project_root(&cwd) else {
        return;
    };
    let resolution = resolve_backlog_directory(&root);
    let Some(path) = resolution.config_path else {
        return;
    };
    let Ok(source) = fs::read_to_string(&path) else {
        return;
    };
    let Ok(mapping) = crate::frontmatter::parse_yaml_mapping(&source) else {
        return;
    };
    let present = |snake: &str, camel: &str| {
        mapping.contains_key(snake) || (snake == "backlog_directory" && mapping.contains_key(camel))
    };
    let schema_keys = [
        ("default_port", "defaultPort"),
        ("auto_open_browser", "autoOpenBrowser"),
        ("remote_operations", "remoteOperations"),
        ("auto_commit", "autoCommit"),
    ];
    let needs_schema_migration = schema_keys
        .iter()
        .any(|(snake, camel)| !present(snake, camel));
    let Ok(config) = parse_config(&source, &path) else {
        return;
    };
    let legacy_milestones = mapping
        .get("milestones")
        .map(|value| match value {
            Value::Array(values) => values.iter().map(legacy_milestone_name).collect(),
            Value::String(_) | Value::Number(_) | Value::Bool(_) => {
                vec![legacy_milestone_name(value)]
            }
            Value::Null => Vec::new(),
            _ => Vec::new(),
        })
        .unwrap_or_default()
        .into_iter()
        .filter(|name| !name.trim().is_empty())
        .collect::<Vec<_>>();
    let migrated =
        needs_schema_migration.then(|| crate::config::migrate_config(Some(config.clone())));
    let mut changes = std::collections::BTreeMap::new();
    let mut add = |snake: &str, camel: &str, value: Value| {
        if !present(snake, camel) {
            changes.insert(snake.to_owned(), Some(value));
        }
    };
    if let Some(migrated) = &migrated {
        add(
            "project_name",
            "projectName",
            Value::String(config.project_name.clone()),
        );
        add(
            "statuses",
            "statuses",
            serde_json::to_value(&migrated.statuses).unwrap_or(Value::Null),
        );
        add(
            "labels",
            "labels",
            serde_json::to_value(&migrated.labels).unwrap_or(Value::Null),
        );
        add(
            "date_format",
            "dateFormat",
            Value::String(migrated.date_format.clone()),
        );
        add(
            "max_column_width",
            "maxColumnWidth",
            Value::from(migrated.max_column_width.unwrap_or(80.0) as i64),
        );
        add(
            "auto_open_browser",
            "autoOpenBrowser",
            Value::Bool(migrated.auto_open_browser.unwrap_or(true)),
        );
        add(
            "default_port",
            "defaultPort",
            Value::from(migrated.default_port.unwrap_or(6420.0) as i64),
        );
        add(
            "remote_operations",
            "remoteOperations",
            Value::Bool(migrated.remote_operations.unwrap_or(true)),
        );
        add(
            "auto_commit",
            "autoCommit",
            Value::Bool(migrated.auto_commit.unwrap_or(false)),
        );
        add(
            "bypass_git_hooks",
            "bypassGitHooks",
            Value::Bool(migrated.bypass_git_hooks.unwrap_or(false)),
        );
        add(
            "check_active_branches",
            "checkActiveBranches",
            Value::Bool(migrated.check_active_branches.unwrap_or(true)),
        );
        add(
            "active_branch_days",
            "activeBranchDays",
            Value::from(migrated.active_branch_days.unwrap_or(30.0) as i64),
        );
        if resolution.config_location == Some(crate::config::ConfigLocation::Root) {
            add(
                "backlog_directory",
                "backlogDirectory",
                Value::String(
                    resolution
                        .backlog_directory
                        .clone()
                        .unwrap_or_else(|| "backlog".into()),
                ),
            );
        }
        if let Some(task_prefix) = config.task_prefix.as_deref() {
            if task_prefix.is_empty() {
                changes.insert("task_prefix".into(), None);
            } else if !present("task_prefix", "taskPrefix") {
                changes.insert(
                    "task_prefix".into(),
                    Some(Value::String(task_prefix.to_owned())),
                );
            }
        }
    }
    if !legacy_milestones.is_empty() {
        let store = MilestoneStore::new(
            &root,
            resolution
                .backlog_directory
                .clone()
                .unwrap_or_else(|| "backlog".into()),
        );
        let mut keys = store
            .list_active()
            .iter()
            .flat_map(|milestone| [milestone.id.as_str(), milestone.title.as_str()])
            .map(|value| value.trim().to_lowercase())
            .filter(|value| !value.is_empty())
            .collect::<std::collections::HashSet<_>>();
        let mut succeeded = true;
        for name in legacy_milestones {
            let title = name.trim();
            let key = title.to_lowercase();
            if title.is_empty() || keys.contains(&key) {
                continue;
            }
            match store.create(title, None, None) {
                Ok(created) => {
                    keys.insert(created.id.trim().to_lowercase());
                    keys.insert(created.title.trim().to_lowercase());
                }
                Err(_) => succeeded = false,
            }
        }
        if succeeded {
            changes.insert("milestones".into(), None);
        }
    } else if needs_schema_migration && mapping.contains_key("milestones") {
        changes.insert("milestones".into(), None);
    }
    if !changes.is_empty()
        && let Ok(updated) = crate::config::update_config_source(&source, &changes)
    {
        let _ = fs::write(path, updated);
    }
}

fn legacy_milestone_name(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Null => "null".into(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        _ => String::new(),
    }
}

const CONFIG_AVAILABLE_KEYS: &str = "Available keys: defaultEditor, projectName, defaultAssignee, defaultStatus, statuses, labels, priorities, types, projects, milestones, definitionOfDone, dateFormat, maxColumnWidth, defaultPort, autoOpenBrowser, hideEmptyColumns, remoteOperations, autoCommit, filesystemOnly, bypassGitHooks, zeroPaddedIds, checkActiveBranches, activeBranchDays";

fn run_config_get(key: &str) -> Result<(), String> {
    let context = project_context()?;
    let c = &context.config;
    let value = match key {
        "projectName" => c.project_name.clone(),
        "defaultEditor" => match c.default_editor.clone().filter(|v| !v.is_empty()) {
            Some(value) => value,
            None => {
                write_stdout("defaultEditor is not set\n")?;
                std::process::exit(1);
            }
        },
        "defaultAssignee" => c.default_assignee.clone().unwrap_or_default().join(", "),
        "defaultStatus" => c.default_status.clone().unwrap_or_default(),
        "statuses" => c.statuses.join(", "),
        "labels" => c.labels.join(", "),
        "priorities" => c
            .priorities
            .clone()
            .unwrap_or_else(|| vec!["High".into(), "Medium".into(), "Low".into()])
            .join(", "),
        "types" => c
            .types
            .clone()
            .unwrap_or_else(|| {
                vec![
                    "bug".into(),
                    "feature".into(),
                    "enhancement".into(),
                    "task".into(),
                    "chore".into(),
                    "docs".into(),
                    "spike".into(),
                ]
            })
            .join(", "),
        "projects" => c.projects.clone().unwrap_or_default().join(", "),
        "milestones" => {
            crate::milestone_store::MilestoneStore::new(&context.root, &context.backlog_directory)
                .list_active()
                .iter()
                .map(|item| item.id.clone())
                .collect::<Vec<_>>()
                .join(", ")
        }
        "definitionOfDone" => c.definition_of_done.clone().unwrap_or_default().join(", "),
        "dateFormat" => c.date_format.clone(),
        "maxColumnWidth" => c.max_column_width.map(num_string).unwrap_or_default(),
        "defaultPort" => c.default_port.map(num_string).unwrap_or_default(),
        "zeroPaddedIds" => c
            .zero_padded_ids
            .map(num_string)
            .unwrap_or_else(|| "(disabled)".into()),
        "activeBranchDays" => c
            .active_branch_days
            .map(num_string)
            .unwrap_or_else(|| "30".into()),
        "autoOpenBrowser" => c
            .auto_open_browser
            .map(|v| v.to_string())
            .unwrap_or_default(),
        "hideEmptyColumns" => c.hide_empty_columns.unwrap_or(false).to_string(),
        "remoteOperations" => c
            .remote_operations
            .map(|v| v.to_string())
            .unwrap_or_default(),
        "autoCommit" => c.auto_commit.map(|v| v.to_string()).unwrap_or_default(),
        "filesystemOnly" => c.filesystem_only.unwrap_or(false).to_string(),
        "bypassGitHooks" => c
            .bypass_git_hooks
            .map(|v| v.to_string())
            .unwrap_or_default(),
        "checkActiveBranches" => c.check_active_branches.unwrap_or(true).to_string(),
        _ => {
            return Err(format!(
                "Unknown config key: {key}\n{CONFIG_AVAILABLE_KEYS}"
            ));
        }
    };
    write_stdout(&format!("{value}\n"))
}

fn num_string(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

fn parse_int_prefix(value: &str) -> Option<i64> {
    let trimmed = value.trim_start();
    let sign_len = usize::from(trimmed.starts_with(['+', '-']));
    let rest = &trimmed[sign_len..];
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    trimmed[..sign_len + digits].parse().ok()
}

fn numeric_error(key: &str) -> String {
    match key {
        "maxColumnWidth" => "maxColumnWidth must be a positive number".into(),
        "defaultPort" => "defaultPort must be a valid port number (1-65535)".into(),
        "zeroPaddedIds" => "zeroPaddedIds must be a non-negative number.".into(),
        _ => "activeBranchDays must be a non-negative number.".into(),
    }
}

fn editor_exists(value: &str) -> bool {
    let executable = value.split(' ').next().unwrap_or("");
    #[cfg(windows)]
    let candidates = editor_candidate_names(
        executable,
        &std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into()),
        true,
    );
    #[cfg(not(windows))]
    let candidates = editor_candidate_names(executable, "", false);
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .any(|path| {
            candidates.iter().any(|name| {
                let candidate = path.join(name);
                if !candidate.is_file() {
                    return false;
                }
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    candidate
                        .metadata()
                        .is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
                }
                #[cfg(not(unix))]
                {
                    true
                }
            })
        })
}

fn editor_candidate_names(command: &str, pathext: &str, windows: bool) -> Vec<String> {
    if command.is_empty() {
        return Vec::new();
    }
    if !windows || std::path::Path::new(command).extension().is_some() {
        return vec![command.to_owned()];
    }
    pathext
        .split(';')
        .filter(|extension| !extension.is_empty())
        .map(|extension| format!("{command}{extension}"))
        .collect()
}

fn run_config_set(key: &str, value: &str) -> Result<(), String> {
    let context = project_context()?;
    let (yaml_key, json) = match key {
        "defaultEditor" => {
            if !value.is_empty() && !editor_exists(value) { return Err(format!("Editor command not found: {value}\nPlease ensure the editor is installed and available in your PATH")); }
            ("default_editor", if value.is_empty() { None } else { Some(Value::String(value.into())) })
        },
        "defaultAssignee" => {
            let mut seen = HashSet::new();
            let values = value.split(',').map(str::trim).filter(|v| !v.is_empty()).filter(|v| seen.insert((*v).to_owned())).map(|v| Value::String(v.into())).collect::<Vec<_>>();
            ("default_assignee", if values.is_empty() { None } else { Some(Value::Array(values)) })
        },
        "projectName" => ("project_name", Some(Value::String(value.into()))),
        "defaultStatus" => ("default_status", Some(Value::String(value.into()))),
        "dateFormat" => ("date_format", Some(Value::String(value.into()))),
        "maxColumnWidth" | "defaultPort" | "zeroPaddedIds" | "activeBranchDays" => {
            let number = parse_int_prefix(value).ok_or_else(|| numeric_error(key))?;
            if number < 0 || (key == "maxColumnWidth" && number == 0) || (key == "defaultPort" && !(1..=65535).contains(&number)) { return Err(numeric_error(key)); }
            let yaml_key = match key { "maxColumnWidth"=>"max_column_width", "defaultPort"=>"default_port", "zeroPaddedIds"=>"zero_padded_ids", _=>"active_branch_days" };
            (yaml_key, if key == "zeroPaddedIds" && number == 0 { None } else { Some(Value::Number(number.into())) })
        },
        "autoOpenBrowser" | "hideEmptyColumns" | "remoteOperations" | "autoCommit" | "filesystemOnly" | "bypassGitHooks" | "checkActiveBranches" => {
            let b = match value.to_ascii_lowercase().as_str() { "true"|"1"|"yes"=>true, "false"|"0"|"no"=>false, _=>return Err(format!("{key} must be true or false")) };
            if key == "filesystemOnly" && b { /* match TypeScript side effects below */ }
            let yaml_key = match key { "autoOpenBrowser"=>"auto_open_browser", "hideEmptyColumns"=>"hide_empty_columns", "remoteOperations"=>"remote_operations", "autoCommit"=>"auto_commit", "filesystemOnly"=>"filesystem_only", "bypassGitHooks"=>"bypass_git_hooks", _=>"check_active_branches" };
            (yaml_key, Some(Value::Bool(b)))
        },
        "milestones" => return Err("milestones cannot be set directly.\nUse milestone files via milestone commands (e.g. `backlog milestone list`, `backlog milestone add`).".into()),
        "statuses"|"labels"|"priorities"|"types"|"projects" => return Err(format!("{key} cannot be set directly. View current values with 'backlog config get {key}'.\nEdit the list in the project config file (`backlog/config.yml`, `.backlog/config.yml`, or `backlog.config.yml`) directly.")),
        "definitionOfDone" => return Err("definitionOfDone cannot be set directly.\nUse `backlog config` for interactive editing, update the project config file (`backlog/config.yml`, `.backlog/config.yml`, or `backlog.config.yml`), or use Web UI Settings.".into()),
        "taskPrefix"|"prefixes" => return Err("Task prefix cannot be changed after initialization.\nThe prefix is set during 'backlog init' and is permanent to avoid breaking existing task IDs.".into()),
        _ => return Err(format!("Unknown config key: {key}\n{CONFIG_AVAILABLE_KEYS}")),
    };
    let path = context
        .config_path
        .ok_or("No backlog project found. Initialize one first with: backlog init")?;
    let source = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut changes = std::collections::BTreeMap::new();
    changes.insert(yaml_key.to_owned(), json);
    if key == "filesystemOnly"
        && matches!(value.to_ascii_lowercase().as_str(), "true" | "yes" | "1")
    {
        changes.insert("check_active_branches".into(), Some(Value::Bool(false)));
        changes.insert("remote_operations".into(), Some(Value::Bool(false)));
        changes.insert("auto_commit".into(), Some(Value::Bool(false)));
        changes.insert("bypass_git_hooks".into(), Some(Value::Bool(false)));
    }
    let edited = crate::config::update_config_source(&source, &changes)?;
    fs::write(path, edited).map_err(|e| e.to_string())?;
    write_stdout(&format!("Set {key} = {value}\n"))
}

fn run_config_list() -> Result<(), String> {
    let context = project_context()?;
    let c = &context.config;
    let milestones =
        crate::milestone_store::MilestoneStore::new(&context.root, &context.backlog_directory)
            .list_active()
            .iter()
            .map(|item| item.id.clone())
            .collect::<Vec<_>>();
    let list = |key: &str, value: String| format!("  {key}: {value}\n");
    let mut output = String::from("Configuration:\n");
    for key in [
        "projectName",
        "defaultEditor",
        "defaultAssignee",
        "defaultStatus",
        "statuses",
        "labels",
        "priorities",
        "types",
        "projects",
        "milestones",
        "definitionOfDone",
        "dateFormat",
        "maxColumnWidth",
        "autoOpenBrowser",
        "hideEmptyColumns",
        "defaultPort",
        "remoteOperations",
        "autoCommit",
        "filesystemOnly",
        "bypassGitHooks",
        "zeroPaddedIds",
        "checkActiveBranches",
        "activeBranchDays",
    ] {
        let mut value = config_display(c, key);
        if key == "milestones" {
            value = format!("[{}]", milestones.join(", "));
        }
        if [
            "statuses",
            "labels",
            "priorities",
            "types",
            "projects",
            "definitionOfDone",
        ]
        .contains(&key)
        {
            if key == "statuses" {
                value = format!("[{}]", c.statuses.join(", "));
            }
            if key == "labels" {
                value = format!("[{}]", c.labels.join(", "));
            }
            if key == "priorities" {
                value = format!(
                    "[{}]",
                    c.priorities
                        .as_ref()
                        .cloned()
                        .unwrap_or_else(|| vec!["High".into(), "Medium".into(), "Low".into()])
                        .join(", ")
                );
            }
            if key == "types" {
                value = format!(
                    "[{}]",
                    c.types
                        .as_ref()
                        .cloned()
                        .unwrap_or_else(|| vec![
                            "bug".into(),
                            "feature".into(),
                            "enhancement".into(),
                            "task".into(),
                            "chore".into(),
                            "docs".into(),
                            "spike".into()
                        ])
                        .join(", ")
                );
            }
            if key == "projects" {
                value = format!(
                    "[{}]",
                    c.projects.as_ref().cloned().unwrap_or_default().join(", ")
                );
            }
            if key == "definitionOfDone" {
                value = format!(
                    "[{}]",
                    c.definition_of_done
                        .as_ref()
                        .cloned()
                        .unwrap_or_default()
                        .join(", ")
                );
            }
        }
        if key == "defaultAssignee" {
            value = format!(
                "[{}]",
                c.default_assignee
                    .as_ref()
                    .cloned()
                    .unwrap_or_default()
                    .join(", ")
            );
        }
        output.push_str(&list(key, value));
    }
    // taskPrefix is printed between zeroPaddedIds and the branch settings.
    let branch_marker = "  checkActiveBranches:";
    if let Some(index) = output.find(branch_marker) {
        output.insert_str(
            index,
            &format!(
                "  taskPrefix: {} (read-only)\n",
                c.task_prefix.as_deref().unwrap_or("task")
            ),
        );
    }
    write_stdout(&output)
}

fn run_config_wizard() -> Result<(), String> {
    let context = project_context()?;
    let config_path = context
        .config_path
        .as_ref()
        .ok_or("No backlog project found. Initialize one first with: backlog init")?;
    let config = &context.config;
    let completion = wizard_confirm("Install shell completions now?", true)?;
    let mut check_active_branches = config.check_active_branches.unwrap_or(true);
    let mut remote_operations = config.remote_operations.unwrap_or(true);
    let mut active_branch_days = config.active_branch_days.unwrap_or(30.0);
    if wizard_confirm(
        "Check task states across active branches?",
        check_active_branches,
    )? {
        check_active_branches = true;
        remote_operations =
            wizard_confirm("Check task states in remote branches?", remote_operations)?;
        active_branch_days = wizard_number(
            "How many days should a branch be considered active?",
            active_branch_days,
            1.0,
            365.0,
        )?;
    } else {
        check_active_branches = false;
        remote_operations = false;
    }
    let bypass_git_hooks = wizard_confirm(
        "Bypass git hooks when committing?",
        config.bypass_git_hooks.unwrap_or(false),
    )?;
    let auto_commit = wizard_confirm(
        "Enable automatic commits for Backlog operations?",
        config.auto_commit.unwrap_or(false),
    )?;
    let zero_padded_ids = if wizard_confirm(
        "Enable zero-padded IDs for consistent formatting?",
        config.zero_padded_ids.unwrap_or(0.0) > 0.0,
    )? {
        Some(wizard_number(
            "Number of digits for zero-padding:",
            config.zero_padded_ids.unwrap_or(3.0),
            1.0,
            10.0,
        )?)
    } else {
        None
    };

    let editor = wizard_line(
        "Default editor command (leave blank to use system default):",
        config.default_editor.as_deref().unwrap_or(""),
    )?;
    let default_editor = if editor.is_empty() {
        None
    } else if editor_exists(&editor) {
        Some(editor)
    } else {
        write_stderr(&format!(
            "Warning: Editor command '{editor}' not found in PATH\n"
        ))?;
        wizard_confirm("Editor not found. Set it anyway?", false)?.then_some(editor)
    };

    let mut definition_of_done = config
        .definition_of_done
        .clone()
        .unwrap_or_default()
        .into_iter()
        .map(|item| item.trim().to_owned())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    loop {
        let preview = if definition_of_done.is_empty() {
            "(none)".to_owned()
        } else {
            definition_of_done
                .iter()
                .enumerate()
                .map(|(index, item)| format!("{}. {item}", index + 1))
                .collect::<Vec<_>>()
                .join(" | ")
        };
        let available_actions = if definition_of_done.is_empty() {
            vec![1, 5]
        } else if definition_of_done.len() == 1 {
            vec![1, 2, 4, 5]
        } else {
            vec![1, 2, 3, 4, 5]
        };
        let action = loop {
            let default = if definition_of_done.is_empty() { 1 } else { 5 };
            let actions = available_actions
                .iter()
                .map(|action| match action {
                    1 => "1 Add",
                    2 => "2 Remove",
                    3 => "3 Reorder",
                    4 => "4 Clear",
                    _ => "5 Done",
                })
                .collect::<Vec<_>>()
                .join(", ");
            let value = wizard_line(
                &format!("Definition of Done defaults: {preview}\nChoose: {actions}"),
                &default.to_string(),
            )?;
            let action = if value.is_empty() {
                Some(default)
            } else {
                value.parse::<usize>().ok()
            };
            if let Some(action) = action
                && available_actions.contains(&action)
            {
                break action;
            }
            write_stderr("Choose one of the available actions.\n")?;
        };
        match action {
            1 => {
                let item = wizard_text("New Definition of Done item:", "")?;
                if !item.is_empty() {
                    definition_of_done.push(item);
                }
            }
            2 if !definition_of_done.is_empty() => {
                let index = wizard_choice(
                    &format!("Remove which item number (1-{})?", definition_of_done.len()),
                    definition_of_done.len(),
                    1,
                    definition_of_done.len(),
                )? - 1;
                definition_of_done.remove(index);
            }
            3 if definition_of_done.len() >= 2 => {
                let from = wizard_choice(
                    &format!("Move which item number (1-{})?", definition_of_done.len()),
                    definition_of_done.len(),
                    1,
                    definition_of_done.len(),
                )? - 1;
                let to = wizard_choice(
                    &format!("Move to which position (1-{})?", definition_of_done.len()),
                    1,
                    1,
                    definition_of_done.len(),
                )? - 1;
                if from != to {
                    let item = definition_of_done.remove(from);
                    definition_of_done.insert(to, item);
                }
            }
            4 if !definition_of_done.is_empty() => {
                if wizard_confirm("Clear all Definition of Done defaults?", false)? {
                    definition_of_done.clear();
                }
            }
            _ => break,
        }
    }

    let mut default_port = config.default_port.unwrap_or(6420.0);
    let mut auto_open_browser = config.auto_open_browser.unwrap_or(true);
    if wizard_confirm("Configure web UI settings now?", false)? {
        default_port = wizard_number("Default web UI port:", default_port, 1.0, 65535.0)?;
        auto_open_browser = wizard_confirm(
            "Automatically open browser when starting web UI?",
            auto_open_browser,
        )?;
    }
    let install_claude = wizard_confirm("Install Claude Code Backlog.md agent?", false)?;

    let mut changes = std::collections::BTreeMap::new();
    changes.insert(
        "check_active_branches".into(),
        Some(Value::Bool(check_active_branches)),
    );
    changes.insert(
        "remote_operations".into(),
        Some(Value::Bool(remote_operations)),
    );
    changes.insert(
        "active_branch_days".into(),
        Some(Value::from(active_branch_days)),
    );
    changes.insert(
        "bypass_git_hooks".into(),
        Some(Value::Bool(bypass_git_hooks)),
    );
    changes.insert("auto_commit".into(), Some(Value::Bool(auto_commit)));
    changes.insert("zero_padded_ids".into(), zero_padded_ids.map(Value::from));
    changes.insert(
        "default_editor".into(),
        default_editor.clone().map(Value::String),
    );
    changes.insert(
        "definition_of_done".into(),
        if definition_of_done.is_empty() {
            None
        } else {
            Some(serde_json::to_value(&definition_of_done).unwrap_or(Value::Null))
        },
    );
    changes.insert("default_port".into(), Some(Value::from(default_port)));
    changes.insert(
        "auto_open_browser".into(),
        Some(Value::Bool(auto_open_browser)),
    );
    let source = fs::read_to_string(config_path).map_err(|error| error.to_string())?;
    let updated = crate::config::update_config_source(&source, &changes)?;
    fs::write(config_path, updated).map_err(|error| error.to_string())?;

    write_stdout("\nAdvanced configuration updated.\n")?;
    write_stdout(&format!(
        "  Check active branches: {check_active_branches}\n"
    ))?;
    write_stdout(&format!("  Remote operations: {remote_operations}\n"))?;
    write_stdout(&format!(
        "  Zero-padded IDs: {}\n",
        zero_padded_ids.map_or_else(|| "disabled".into(), |digits| format!("{digits} digits"))
    ))?;
    write_stdout(&format!("  Web UI port: {default_port}\n"))?;
    write_stdout(&format!("  Auto open browser: {auto_open_browser}\n"))?;
    write_stdout(&format!("  Bypass git hooks: {bypass_git_hooks}\n"))?;
    write_stdout(&format!("  Auto commit: {auto_commit}\n"))?;
    write_stdout(&format!(
        "  Definition of Done defaults: {}\n",
        if definition_of_done.is_empty() {
            "(none)".into()
        } else {
            definition_of_done.join(" | ")
        }
    ))?;
    if completion {
        match install_shell_completion() {
            Ok((path, instructions)) => {
                write_stdout(&format!(
                    "  Shell completions: installed to {}\n",
                    path.display()
                ))?;
                write_stdout(&instructions)?;
            }
            Err(error) => {
                write_stdout("  Shell completions: installation failed (see warning below)\n")?;
                write_stderr(&format!("Warning: {error}\n"))?;
            }
        }
    } else {
        write_stdout("  Shell completions: skipped\n")?;
    }
    if let Some(editor) = default_editor {
        write_stdout(&format!("  Default editor: {editor}\n"))?;
    }
    if install_claude {
        install_claude_agent(&context.root)?;
        write_stdout("✓ Claude Code Backlog.md agent installed to .claude/agents/\n")?;
    }
    write_stdout("\nUse `backlog config list` to review all configuration values.\n")
}

fn wizard_line(prompt: &str, initial: &str) -> Result<String, String> {
    write_stdout(&format!("{prompt} [{initial}]: "))?;
    io::stdout().flush().map_err(|error| error.to_string())?;
    let mut line = String::new();
    let read = io::stdin()
        .read_line(&mut line)
        .map_err(|error| error.to_string())?;
    if read == 0 {
        return Err("Configuration cancelled.".into());
    }
    Ok(line.trim().to_owned())
}

fn wizard_confirm(prompt: &str, initial: bool) -> Result<bool, String> {
    loop {
        let value = wizard_line(prompt, if initial { "Y/n" } else { "y/N" })?;
        match value.to_ascii_lowercase().as_str() {
            "" => return Ok(initial),
            "y" | "yes" | "true" | "1" => return Ok(true),
            "n" | "no" | "false" | "0" => return Ok(false),
            _ => write_stderr("Enter y or n.\n")?,
        }
    }
}

fn wizard_text(prompt: &str, initial: &str) -> Result<String, String> {
    let value = wizard_line(prompt, initial)?;
    Ok(if value.is_empty() {
        initial.to_owned()
    } else {
        value
    })
}

fn wizard_number(prompt: &str, initial: f64, min: f64, max: f64) -> Result<f64, String> {
    loop {
        let value = wizard_line(prompt, &initial.to_string())?;
        if value.is_empty() {
            return Ok(initial);
        }
        if let Ok(number) = value.parse::<f64>()
            && number.is_finite()
            && (min..=max).contains(&number)
        {
            return Ok(number);
        }
        write_stderr(&format!("Enter a number from {min} to {max}.\n"))?;
    }
}

fn wizard_choice(prompt: &str, initial: usize, min: usize, max: usize) -> Result<usize, String> {
    loop {
        let value = wizard_line(prompt, &initial.to_string())?;
        if value.is_empty() {
            return Ok(initial);
        }
        if let Ok(choice) = value.parse::<usize>()
            && (min..=max).contains(&choice)
        {
            return Ok(choice);
        }
        write_stderr(&format!("Choose a number from {min} to {max}.\n"))?;
    }
}

fn install_claude_agent(project_root: &std::path::Path) -> Result<(), String> {
    let agent_dir = project_root.join(".claude/agents");
    fs::create_dir_all(&agent_dir).map_err(|error| error.to_string())?;
    let agent = include_str!("../../src/guidelines/project-manager-backlog.md");
    let version = env!("CARGO_PKG_VERSION");
    let content = format!(
        "{}\n\n<!-- backlog.md-instructions-version: {version} -->\n",
        agent.trim_end()
    );
    fs::write(agent_dir.join("project-manager-backlog.md"), content)
        .map_err(|error| error.to_string())
}

fn mcp_client_setup(
    client: &str,
) -> Option<(&'static str, &'static str, Vec<&'static str>, &'static str)> {
    match client {
        "claude" => Some((
            "Claude Code",
            "claude",
            vec![
                "mcp", "add", "-s", "user", "backlog", "--", "backlog", "mcp", "start",
            ],
            "CLAUDE.md",
        )),
        "codex" => Some((
            "OpenAI Codex",
            "codex",
            vec!["mcp", "add", "backlog", "--", "backlog", "mcp", "start"],
            "AGENTS.md",
        )),
        "gemini" => Some((
            "Gemini CLI",
            "gemini",
            vec![
                "mcp", "add", "-s", "user", "backlog", "backlog", "mcp", "start",
            ],
            "GEMINI.md",
        )),
        "kiro" => Some((
            "Kiro",
            "kiro-cli",
            vec![
                "mcp",
                "add",
                "--scope",
                "global",
                "--name",
                "backlog",
                "--command",
                "backlog",
                "--args",
                "mcp,start",
            ],
            "AGENTS.md",
        )),
        _ => None,
    }
}

fn open_init_url(url: &str) -> Result<(), String> {
    let mut command = if cfg!(windows) {
        let mut command = std::process::Command::new("cmd");
        command.args(["/C", "start", "", url]);
        command
    } else if cfg!(target_os = "macos") {
        let mut command = std::process::Command::new("open");
        command.arg(url);
        command
    } else {
        let mut command = std::process::Command::new("xdg-open");
        command.arg(url);
        command
    };
    let status = command.status().map_err(|error| error.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("browser command exited with {status}"))
    }
}

fn write_mcp_guidelines(root: &std::path::Path, name: &str) -> Result<(bool, bool), String> {
    const NUDGE: &str = include_str!("../../src/guidelines/mcp/agent-nudge.md");
    let path = root.join(name);
    let existed = path.exists();
    let original = if existed {
        fs::read_to_string(&path).map_err(|error| error.to_string())?
    } else {
        String::new()
    };
    let (without_cli, cli_removed, cli_insert) = strip_managed(&original, "BACKLOG.MD GUIDELINES");
    let (content, mcp_removed, mcp_insert) =
        strip_managed(&without_cli, "BACKLOG.MD MCP GUIDELINES");
    let block = format!(
        "\n<!-- BACKLOG.MD MCP GUIDELINES START -->\n<!-- backlog.md-instructions-version: {} -->\n{}\n<!-- BACKLOG.MD MCP GUIDELINES END -->\n",
        env!("CARGO_PKG_VERSION"),
        NUDGE.trim()
    );
    let updated = if mcp_removed || cli_removed {
        let index = mcp_insert
            .or(cli_insert)
            .unwrap_or(content.len())
            .min(content.len());
        let mut updated = content;
        updated.insert_str(index, &block);
        updated
    } else if content.contains("<!-- BACKLOG.MD MCP GUIDELINES START -->") {
        content
    } else {
        let mut updated = content;
        if !updated.is_empty() && !updated.ends_with('\n') {
            updated.push('\n');
        }
        updated.push_str(&block);
        updated
    };
    let changed = updated != original;
    if changed {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(path, updated).map_err(|error| error.to_string())?;
    }
    Ok((!existed, changed))
}

fn install_shell_completion() -> Result<(PathBuf, String), String> {
    let shell = std::env::var("SHELL").unwrap_or_default();
    let profile_path;
    let (script, relative, enable) = if shell.contains("bash") {
        (
            include_str!("../../completions/backlog.bash"),
            ".local/share/bash-completion/completions/backlog",
            "bash",
        )
    } else if shell.contains("zsh") {
        (
            include_str!("../../completions/_backlog"),
            ".zsh/completions/_backlog",
            "zsh",
        )
    } else if shell.contains("fish") {
        (
            include_str!("../../completions/backlog.fish"),
            ".config/fish/completions/backlog.fish",
            "fish",
        )
    } else if shell.contains("pwsh") {
        let output = std::process::Command::new("pwsh")
            .args(["-NoProfile", "-Command", "$PROFILE.CurrentUserAllHosts"])
            .output()
            .map_err(|error| format!("Could not resolve your PowerShell profile path: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "Could not resolve your PowerShell profile path: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        profile_path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if profile_path.is_empty() {
            return Err("PowerShell returned an empty profile path".into());
        }
        let profile_path_buf = PathBuf::from(&profile_path);
        let profile_dir = profile_path_buf
            .parent()
            .ok_or_else(|| "Could not determine the PowerShell profile directory".to_owned())?;
        let path = profile_dir.join("Completions/backlog-completion.ps1");
        let script = include_str!("../../completions/backlog.ps1");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(&path, script).map_err(|error| error.to_string())?;
        let instructions = format!(
            "\nTo enable completions, add this to your PowerShell profile ({profile_path}):\n$completionScript = Join-Path (Split-Path -Parent $PROFILE.CurrentUserAllHosts) \"Completions/backlog-completion.ps1\"\nif (Test-Path $completionScript) {{ . $completionScript }}\n\nThen restart PowerShell or run:\n. $PROFILE.CurrentUserAllHosts\n\n"
        );
        return Ok((path, instructions));
    } else {
        return Err("Could not detect your shell. Run `backlog completion install --shell bash|zsh|fish|pwsh` later.".into());
    };
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .ok_or_else(|| "Could not determine the user home directory".to_owned())?;
    let path = home.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(&path, script).map_err(|error| error.to_string())?;
    let instructions = match enable {
        "bash" => format!(
            "\nTo enable completions, add this to your ~/.bashrc:\nsource {}\n\nThen restart your shell or run:\nsource ~/.bashrc\n\n",
            path.display()
        ),
        "zsh" => format!(
            "\nTo enable completions, ensure the directory is in your fpath.\nAdd this to your ~/.zshrc:\nfpath=({} $fpath)\nautoload -Uz compinit && compinit\n\nThen restart your shell or run:\nsource ~/.zshrc\n\n",
            path.parent().unwrap_or(&path).display()
        ),
        _ => "\nCompletions should be automatically loaded by fish.\nRestart your shell or run:\nexec fish\n\n".to_owned(),
    };
    Ok((path, instructions))
}

fn run_init(options: InitArgs) -> Result<(), String> {
    let process_cwd = std::env::current_dir().map_err(|error| error.to_string())?;
    let backlog_cwd = std::env::var("BACKLOG_CWD").ok();
    let root = resolve_runtime_cwd(None, backlog_cwd.as_deref(), &process_cwd)
        .map_err(|error| error.to_string())?
        .path;
    let interactive = io::stdin().is_terminal() && io::stdout().is_terminal();
    let mut filesystem_only = options.no_git;
    let git = crate::git::GitRepository::new(&root);
    let mut git_enabled = git.is_repository();
    if !git_enabled && !filesystem_only {
        if !interactive {
            return Err("No Git repository found and no interactive terminal is available. Run `git init` first, pass --no-git, or rerun in an interactive terminal to choose.".into());
        }
        if wizard_choice(
            "No git repository found. Choose: 1 Initialize a Git repository, 2 Continue without Git",
            1,
            1,
            2,
        )? == 1
        {
            let output = std::process::Command::new("git")
                .arg("init")
                .current_dir(&root)
                .output()
                .map_err(|error| format!("Failed to initialize git repository: {error}"))?;
            if !output.status.success() {
                return Err(format!(
                    "Failed to initialize git repository: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
            git_enabled = true;
        } else {
            filesystem_only = true;
        }
    }

    let initial_resolution = resolve_backlog_directory(&root);
    let existing_config_path = initial_resolution
        .config_path
        .as_ref()
        .filter(|path| path.is_file());
    let existing_config = if let Some(path) = existing_config_path {
        let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
        Some(parse_config(&source, path).map_err(|error| error.to_string())?)
    } else {
        None
    };
    let is_re_initialization = existing_config.is_some();
    if is_re_initialization {
        write_stdout(
            "Existing backlog project detected. Current configuration will be preserved where not specified.\n",
        )?;
        if options.backlog_dir.is_some() {
            return Err("The backlog directory is fixed after initialization. Re-run init without --backlog-dir for this project.".into());
        }
        if options.config_location.is_some() {
            return Err("The config location is fixed after initialization. Re-run init without --config-location for this project.".into());
        }
    }

    let is_non_interactive = options.defaults
        || options.agent_instructions.is_some()
        || options.check_branches.is_some()
        || options.include_remote.is_some()
        || options.branch_days.is_some()
        || options.bypass_git_hooks.is_some()
        || options.zero_padded_ids.is_some()
        || options.default_editor.is_some()
        || options.web_port.is_some()
        || options.auto_open_browser.is_some()
        || options.install_claude_agent.is_some()
        || options.integration_mode.is_some()
        || options.backlog_dir.is_some()
        || options.config_location.is_some()
        || options.task_prefix.is_some()
        || options.no_git;

    let mut project_name = options.project_name.clone().filter(|name| !name.is_empty());
    if project_name.is_none() {
        if !interactive {
            return Err("Project name is required when no interactive terminal is available. Supply it with `backlog init \"My Project\" --defaults`.".into());
        }
        let default_name = existing_config
            .as_ref()
            .map(|config| config.project_name.as_str())
            .unwrap_or("");
        let prompt = if is_re_initialization && !default_name.is_empty() {
            format!("Project name ({default_name}):")
        } else {
            "Project name:".into()
        };
        let entered = wizard_line(&prompt, default_name)?;
        project_name = Some(if entered.is_empty() {
            default_name.to_owned()
        } else {
            entered
        });
    }
    let project_name = project_name.unwrap_or_default();
    if project_name.trim().is_empty() {
        return Err("Project name is required.".into());
    }
    if !is_non_interactive && !interactive {
        return Err("Initialization needs an interactive terminal for setup choices. Pass --defaults to use default settings.".into());
    }

    let mut backlog_directory = initial_resolution
        .backlog_directory
        .clone()
        .unwrap_or_else(|| "backlog".into());
    let mut config_location = initial_resolution
        .config_location
        .unwrap_or(ConfigLocation::Folder);
    let mut directory_source = initial_resolution
        .source
        .unwrap_or(BacklogDirectorySource::Backlog);
    if !is_re_initialization {
        if let Some(value) = options.backlog_dir.as_deref() {
            let Some(normalized) = normalize_backlog_directory(value) else {
                return Err("Invalid --backlog-dir value. Use 'backlog', '.backlog', or a project-relative path inside the project.".into());
            };
            directory_source = match normalized.as_str() {
                "backlog" => BacklogDirectorySource::Backlog,
                ".backlog" => BacklogDirectorySource::Hidden,
                _ => BacklogDirectorySource::Custom,
            };
            backlog_directory = normalized;
        }
        if let Some(value) = options.config_location.as_deref() {
            config_location = match value.trim().to_ascii_lowercase().as_str() {
                "folder" => ConfigLocation::Folder,
                "root" => ConfigLocation::Root,
                _ => return Err("Invalid --config-location value. Use 'folder' or 'root'.".into()),
            };
        } else if directory_source == BacklogDirectorySource::Custom {
            config_location = ConfigLocation::Root;
        }
        if directory_source == BacklogDirectorySource::Custom
            && config_location != ConfigLocation::Root
        {
            return Err("Custom backlog directories require --config-location root.".into());
        }
        if !is_non_interactive {
            let default_choice = match directory_source {
                BacklogDirectorySource::Backlog => 1,
                BacklogDirectorySource::Hidden => 2,
                BacklogDirectorySource::Custom => 3,
            };
            let choice = wizard_choice(
                "Where should Backlog.md store project files? Choose: 1 backlog/, 2 .backlog/, 3 Custom project-relative path",
                default_choice,
                1,
                3,
            )?;
            if choice == 3 {
                let entered =
                    wizard_line("Project-relative backlog directory:", &backlog_directory)?;
                backlog_directory = normalize_backlog_directory(&entered)
                    .ok_or("Enter a project-relative path inside the project.")?;
                config_location = ConfigLocation::Root;
            } else {
                backlog_directory = if choice == 1 { "backlog" } else { ".backlog" }.into();
                config_location = if wizard_choice(
                    "Where should Backlog.md store project configuration? Choose: 1 folder config.yml, 2 root backlog.config.yml",
                    if config_location == ConfigLocation::Root {
                        2
                    } else {
                        1
                    },
                    1,
                    2,
                )? == 1
                {
                    ConfigLocation::Folder
                } else {
                    ConfigLocation::Root
                };
            }
        }
    }

    let mut task_prefix = options
        .task_prefix
        .clone()
        .filter(|prefix| !prefix.is_empty())
        .unwrap_or_else(|| "task".into());
    if !is_re_initialization && !is_non_interactive {
        task_prefix = wizard_line("Task prefix (default: task):", "")?;
        if task_prefix.is_empty() {
            task_prefix = "task".into();
        }
    }
    if let Some(error) = task_prefix_error(&task_prefix) {
        return Err(error);
    }

    let mut integration_mode = options
        .integration_mode
        .as_deref()
        .map(normalize_init_integration_mode)
        .transpose()?
        .unwrap_or_else(|| "cli".into());
    if !is_non_interactive {
        integration_mode = match wizard_choice(
            "How would you like your AI tools to connect to Backlog.md? Choose: 1 CLI instructions, 2 MCP connector, 3 Skip for now",
            1,
            1,
            3,
        )? {
            1 => "cli",
            2 => "mcp",
            _ => "none",
        }
        .into();
    }
    if integration_mode != "cli"
        && (options.agent_instructions.is_some() || options.install_claude_agent.is_some())
    {
        let message = if integration_mode == "mcp" {
            "The MCP connector option cannot be combined with --agent-instructions or --install-claude-agent."
        } else {
            "Skipping AI integration cannot be combined with --agent-instructions or --install-claude-agent."
        };
        return Err(message.into());
    }

    let mut mcp_client_setup_summary = None;
    if integration_mode == "mcp" {
        if is_non_interactive {
            mcp_client_setup_summary = Some("skipped (non-interactive)".to_owned());
        } else {
            write_stdout("MCP server name: backlog\n")?;
            let selected_clients = loop {
                let answer = wizard_line(
                    "Which AI tools should we configure right now? (comma-separated: claude, codex, gemini, kiro, guide):",
                    "",
                )?;
                let mut clients = Vec::new();
                let mut valid = true;
                for client in answer
                    .split(',')
                    .map(|client| client.trim().to_ascii_lowercase())
                {
                    if client.is_empty() {
                        continue;
                    }
                    if !["claude", "codex", "gemini", "kiro", "guide"].contains(&client.as_str()) {
                        write_stderr(&format!(
                            "Invalid MCP client: {client}. Choose claude, codex, gemini, kiro, or guide.\n"
                        ))?;
                        valid = false;
                        break;
                    }
                    if !clients.contains(&client) {
                        clients.push(client);
                    }
                }
                if !valid {
                    continue;
                }
                if clients.is_empty() {
                    write_stdout("Please select at least one AI tool before continuing.\n")?;
                    continue;
                }
                break clients;
            };

            const GUIDE_URL: &str =
                "https://github.com/MrLesk/Backlog.md#-mcp-integration-model-context-protocol";
            let mut setup_results = Vec::new();
            let mut reminder_files = Vec::new();
            let mut created_reminders = Vec::new();
            let mut updated_reminders = Vec::new();
            for client in selected_clients {
                if client == "guide" {
                    write_stdout("Opening MCP setup guide in your browser...\n")?;
                    if let Err(error) = open_init_url(GUIDE_URL) {
                        write_stderr(&format!(
                            "Unable to open the MCP setup guide automatically ({error}). Please visit {GUIDE_URL}\n"
                        ))?;
                    }
                    setup_results.push("Setup guide opened".to_owned());
                    reminder_files.push("AGENTS.md".to_owned());
                    continue;
                }

                let (label, command, arguments, reminder) = mcp_client_setup(&client)
                    .ok_or_else(|| format!("Unsupported MCP client selection: {client}"))?;
                write_stdout(&format!("Configuring {label}...\n"))?;
                let result = match std::process::Command::new(command)
                    .args(&arguments)
                    .status()
                {
                    Ok(status) if status.success() => {
                        write_stdout(&format!("Added Backlog MCP server to {label}\n"))?;
                        label.to_owned()
                    }
                    Ok(status) => {
                        let manual = format!("{command} {}", arguments.join(" "));
                        write_stderr(&format!(
                            "Unable to configure {label} automatically (command exited with {status}).\nRun manually: {manual}\n"
                        ))?;
                        format!("{label} (manual setup required)")
                    }
                    Err(error) => {
                        let manual = format!("{command} {}", arguments.join(" "));
                        write_stderr(&format!(
                            "Unable to configure {label} automatically ({error}).\nRun manually: {manual}\n"
                        ))?;
                        format!("{label} (manual setup required)")
                    }
                };
                setup_results.push(result);
                reminder_files.push(reminder.to_owned());
            }
            let mut unique_reminders = Vec::new();
            for file in reminder_files {
                if !unique_reminders.contains(&file) {
                    unique_reminders.push(file);
                }
            }
            for file in unique_reminders {
                let (created, changed) = write_mcp_guidelines(&root, &file)?;
                if changed {
                    if created {
                        created_reminders.push(file);
                    } else {
                        updated_reminders.push(file);
                    }
                }
            }
            if !created_reminders.is_empty() {
                write_stdout(&format!(
                    "Created MCP reminder file(s): {}\n",
                    created_reminders.join(", ")
                ))?;
            }
            if !updated_reminders.is_empty() {
                write_stdout(&format!(
                    "Added MCP reminder to {}\n",
                    updated_reminders.join(", ")
                ))?;
            }
            mcp_client_setup_summary = Some(setup_results.join(", "));
        }
    }

    let mut agent_files = Vec::new();
    let mut agent_skipped = false;
    if integration_mode == "cli" {
        if let Some(selection) = options.agent_instructions.as_deref() {
            (agent_files, agent_skipped) = parse_init_agent_selection(selection)?;
        } else if is_non_interactive {
            agent_files.push("AGENTS.md".to_owned());
        } else {
            loop {
                let selection = wizard_line(
                    "Select instruction files for CLI-based AI tools (comma-separated; CLAUDE.md, AGENTS.md, GEMINI.md, .github/copilot-instructions.md; 'none' skips):",
                    "",
                )?;
                if selection.is_empty() {
                    write_stderr(
                        "Select at least one instruction file, or enter 'none' to skip.\n",
                    )?;
                    continue;
                }
                (agent_files, agent_skipped) = parse_init_agent_selection(&selection)?;
                break;
            }
        }
    }

    let existing = existing_config.as_ref();
    let mut check_active_branches = existing
        .and_then(|config| config.check_active_branches)
        .unwrap_or(true);
    let mut remote_operations = existing
        .and_then(|config| config.remote_operations)
        .unwrap_or(true);
    let mut active_branch_days = existing
        .and_then(|config| config.active_branch_days)
        .unwrap_or(30.0);
    let mut bypass_git_hooks = existing
        .and_then(|config| config.bypass_git_hooks)
        .unwrap_or(false);
    let mut auto_commit = existing
        .and_then(|config| config.auto_commit)
        .unwrap_or(false);
    let mut zero_padded_ids = existing.and_then(|config| config.zero_padded_ids);
    let mut default_editor = existing
        .and_then(|config| config.default_editor.clone())
        .or_else(|| {
            if is_re_initialization {
                None
            } else {
                std::env::var("EDITOR")
                    .ok()
                    .filter(|editor| !editor.is_empty())
                    .or_else(|| {
                        std::env::var("VISUAL")
                            .ok()
                            .filter(|editor| !editor.is_empty())
                    })
            }
        });
    let mut default_port = existing
        .and_then(|config| config.default_port)
        .unwrap_or(6420.0);
    let mut auto_open_browser = existing
        .and_then(|config| config.auto_open_browser)
        .unwrap_or(true);
    let mut definition_of_done = existing
        .and_then(|config| config.definition_of_done.clone())
        .unwrap_or_default();
    let mut advanced_configured = false;
    let mut install_completions = false;
    let mut should_install_claude_agent = integration_mode == "cli"
        && parse_init_bool(options.install_claude_agent.as_deref(), false);
    if !is_non_interactive
        && wizard_confirm(
            "Configure advanced settings now? (Runs the advanced backlog config wizard)",
            false,
        )?
    {
        advanced_configured = true;
        install_completions = wizard_confirm("Install shell completions now?", true)?;
        check_active_branches = wizard_confirm(
            "Check task states across active branches?",
            check_active_branches,
        )?;
        if check_active_branches {
            remote_operations =
                wizard_confirm("Check task states in remote branches?", remote_operations)?;
            active_branch_days = wizard_number(
                "How many days should a branch be considered active?",
                active_branch_days,
                1.0,
                365.0,
            )?;
        } else {
            remote_operations = false;
        }
        bypass_git_hooks = wizard_confirm("Bypass git hooks when committing?", bypass_git_hooks)?;
        auto_commit = wizard_confirm(
            "Enable automatic commits for Backlog operations?",
            auto_commit,
        )?;
        zero_padded_ids = if wizard_confirm(
            "Enable zero-padded IDs for consistent formatting?",
            zero_padded_ids.unwrap_or(0.0) > 0.0,
        )? {
            Some(wizard_number(
                "Number of digits for zero-padding:",
                zero_padded_ids.unwrap_or(3.0),
                1.0,
                10.0,
            )?)
        } else {
            None
        };
        let editor_initial = default_editor.clone().unwrap_or_else(|| {
            if is_re_initialization {
                String::new()
            } else if cfg!(windows) {
                "notepad".into()
            } else if cfg!(unix) {
                "nano".into()
            } else {
                "vi".into()
            }
        });
        let entered_editor = wizard_line(
            "Default editor command (leave blank to use system default):",
            &editor_initial,
        )?;
        let editor_value = if entered_editor.is_empty() {
            editor_initial
        } else {
            entered_editor
        };
        default_editor = if editor_value.is_empty() {
            None
        } else if editor_exists(&editor_value)
            || wizard_confirm("Editor not found. Set it anyway?", false)?
        {
            Some(editor_value)
        } else {
            None
        };
        loop {
            let preview = if definition_of_done.is_empty() {
                "(none)".to_owned()
            } else {
                definition_of_done
                    .iter()
                    .enumerate()
                    .map(|(index, item)| format!("{}. {item}", index + 1))
                    .collect::<Vec<_>>()
                    .join(" | ")
            };
            let actions = if definition_of_done.is_empty() {
                vec![1, 5]
            } else if definition_of_done.len() == 1 {
                vec![1, 2, 4, 5]
            } else {
                vec![1, 2, 3, 4, 5]
            };
            let selected_action = loop {
                let default_action = if definition_of_done.is_empty() { 1 } else { 5 };
                let choices = actions
                    .iter()
                    .map(|action| match action {
                        1 => "1 Add",
                        2 => "2 Remove",
                        3 => "3 Reorder",
                        4 => "4 Clear",
                        _ => "5 Done",
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let value = wizard_line(
                    &format!("Edit Definition of Done defaults: {preview}\nChoose: {choices}"),
                    &default_action.to_string(),
                )?;
                let action = if value.is_empty() {
                    Some(default_action)
                } else {
                    value.parse::<usize>().ok()
                };
                if let Some(action) = action
                    && actions.contains(&action)
                {
                    break action;
                }
                write_stderr("Choose one of the available actions.\n")?;
            };
            match selected_action {
                1 => {
                    let item = wizard_text("New Definition of Done item:", "")?;
                    if !item.is_empty() {
                        definition_of_done.push(item);
                    }
                }
                2 => {
                    let index = wizard_choice(
                        &format!("Remove which item number (1-{})?", definition_of_done.len()),
                        definition_of_done.len(),
                        1,
                        definition_of_done.len(),
                    )? - 1;
                    definition_of_done.remove(index);
                }
                3 => {
                    let from = wizard_choice(
                        &format!("Move which item number (1-{})?", definition_of_done.len()),
                        definition_of_done.len(),
                        1,
                        definition_of_done.len(),
                    )? - 1;
                    let to = wizard_choice(
                        &format!("Move to which position (1-{})?", definition_of_done.len()),
                        1,
                        1,
                        definition_of_done.len(),
                    )? - 1;
                    if from != to {
                        let item = definition_of_done.remove(from);
                        definition_of_done.insert(to, item);
                    }
                }
                4 => {
                    if wizard_confirm("Clear all Definition of Done defaults?", false)? {
                        definition_of_done.clear();
                    }
                }
                _ => break,
            }
        }
        if wizard_confirm("Configure web UI settings now?", false)? {
            default_port = wizard_number("Default web UI port:", default_port, 1.0, 65535.0)?;
            auto_open_browser = wizard_confirm(
                "Automatically open browser when starting web UI?",
                auto_open_browser,
            )?;
        }
        if integration_mode == "cli" {
            should_install_claude_agent =
                wizard_confirm("Install Claude Code Backlog.md agent?", false)?;
        }
    }
    if let Some(value) = options.check_branches.as_deref() {
        check_active_branches = parse_init_bool(Some(value), check_active_branches);
    }
    if check_active_branches {
        if let Some(value) = options.include_remote.as_deref() {
            remote_operations = parse_init_bool(Some(value), remote_operations);
        }
        if let Some(value) = options.branch_days.as_deref() {
            active_branch_days = parse_init_int(Some(value), active_branch_days);
        }
    } else {
        remote_operations = false;
    }
    if let Some(value) = options.bypass_git_hooks.as_deref() {
        bypass_git_hooks = parse_init_bool(Some(value), bypass_git_hooks);
    }
    if let Some(value) = options.zero_padded_ids.as_deref() {
        let width = parse_init_int(Some(value), zero_padded_ids.unwrap_or(0.0));
        zero_padded_ids = (width > 0.0).then_some(width);
    }
    if let Some(value) = options.default_editor.as_ref() {
        default_editor = (!value.is_empty()).then(|| value.clone());
    }
    if let Some(value) = options.web_port.as_deref() {
        default_port = parse_init_int(Some(value), default_port);
    }
    if let Some(value) = options.auto_open_browser.as_deref() {
        auto_open_browser = parse_init_bool(Some(value), auto_open_browser);
    }
    if filesystem_only {
        check_active_branches = false;
        remote_operations = false;
        bypass_git_hooks = false;
        auto_commit = false;
    }
    if existing.is_some_and(|config| config.filesystem_only == Some(true)) {
        filesystem_only = true;
        check_active_branches = false;
        remote_operations = false;
        bypass_git_hooks = false;
        auto_commit = false;
    }

    let task_prefix = existing
        .and_then(|config| config.task_prefix.as_deref())
        .unwrap_or(&task_prefix);
    let mut config = if let Some(existing) = existing {
        existing.clone()
    } else {
        initial_config(&project_name, Some(task_prefix), filesystem_only)?
    };
    config.project_name = project_name.clone();
    config.filesystem_only = Some(filesystem_only);
    config.check_active_branches = Some(check_active_branches);
    config.remote_operations = Some(remote_operations);
    config.active_branch_days = Some(active_branch_days);
    config.bypass_git_hooks = Some(bypass_git_hooks);
    config.auto_commit = Some(auto_commit);
    config.default_port = Some(default_port);
    config.auto_open_browser = Some(auto_open_browser);
    config.task_prefix = Some(task_prefix.to_owned());
    config.default_editor = default_editor;
    config.zero_padded_ids = zero_padded_ids;
    if advanced_configured {
        config.definition_of_done = if definition_of_done.is_empty() {
            None
        } else {
            Some(definition_of_done.clone())
        };
    }
    config.backlog_directory =
        (config_location == ConfigLocation::Root).then(|| backlog_directory.clone());

    let backlog_path = root.join(&backlog_directory);
    if !is_re_initialization {
        for directory in [
            "tasks",
            "drafts",
            "archive/tasks",
            "archive/drafts",
            "milestones",
            "archive/milestones",
            "docs",
            "decisions",
        ] {
            fs::create_dir_all(backlog_path.join(directory)).map_err(|error| error.to_string())?;
        }
    }
    let config_path = existing_config_path.cloned().unwrap_or_else(|| {
        if config_location == ConfigLocation::Root {
            root.join("backlog.config.yml")
        } else {
            backlog_path.join("config.yml")
        }
    });
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(
        &config_path,
        serialize_config(&config, config_location, &backlog_directory),
    )
    .map_err(|error| error.to_string())?;

    let agent_results = write_init_agent_instructions(&root, &agent_files)?;
    if auto_commit && !filesystem_only && !agent_results.1.is_empty() {
        let context = ProjectContext {
            root: root.clone(),
            backlog_directory: PathBuf::from(&backlog_directory),
            config_path: Some(config_path.clone()),
            config: config.clone(),
            store: TaskStore::new(&root, &backlog_path, task_prefix),
            prefix: task_prefix.to_owned(),
        };
        commit_record_change(&context, "Add AI agent instructions", &agent_results.1)?;
    }
    if should_install_claude_agent {
        install_claude_agent(&root)?;
    }

    let ai_summary = match integration_mode.as_str() {
        "cli" => "CLI instructions",
        "mcp" => "MCP connector",
        _ => "skipped (configure later via `backlog init`)",
    };
    let completion_summary = if install_completions {
        match install_shell_completion() {
            Ok((path, instructions)) => {
                write_stdout(&format!(
                    "Shell completions installed to {}\n{}",
                    path.display(),
                    instructions
                ))?;
                format!("installed to {}", path.display())
            }
            Err(error) => {
                write_stderr(&format!("Warning: {error}\n"))?;
                "installation failed (see warning below)".into()
            }
        }
    } else if advanced_configured {
        "skipped".into()
    } else {
        "not configured".into()
    };
    write_stdout("Initialization Summary\n")?;
    write_stdout(&format!("Project Name: {project_name}\n"))?;
    write_stdout(&format!("Backlog directory: {backlog_directory}\n"))?;
    write_stdout(&format!(
        "Config location: {}\n",
        if config_location == ConfigLocation::Root {
            "backlog.config.yml"
        } else {
            "folder config.yml"
        }
    ))?;
    write_stdout(&format!(
        "Git integration: {}\n",
        if filesystem_only {
            "disabled (filesystem-only)"
        } else if git_enabled {
            "enabled"
        } else {
            "disabled"
        }
    ))?;
    write_stdout(&format!("AI Integration: {ai_summary}\n"))?;
    if integration_mode == "cli" {
        if agent_files.is_empty() {
            write_stdout(&format!(
                "Agent instructions: {}\n",
                if agent_skipped { "skipped" } else { "none" }
            ))?;
        } else {
            write_stdout(&format!("Agent instructions: {}\n", agent_files.join(", ")))?;
        }
    } else if integration_mode == "mcp" {
        write_stdout(&format!(
            "Agent instruction files: guidance is provided through the MCP connector.\nMCP server name: backlog\nMCP client setup: {}\n",
            mcp_client_setup_summary.as_deref().unwrap_or("skipped")
        ))?;
    }
    write_stdout(&format!("Shell completions: {completion_summary}\n"))?;
    if advanced_configured || filesystem_only {
        write_stdout("Advanced settings:\n")?;
        write_stdout(&format!(
            "  Check active branches: {check_active_branches}\n"
        ))?;
        write_stdout(&format!("  Remote operations: {remote_operations}\n"))?;
        write_stdout(&format!(
            "  Active branch days: {}\n",
            num_string(active_branch_days)
        ))?;
        write_stdout(&format!("  Bypass git hooks: {bypass_git_hooks}\n"))?;
        write_stdout(&format!("  Auto commit: {auto_commit}\n"))?;
        write_stdout(&format!(
            "  Zero-padded IDs: {}\n",
            zero_padded_ids.map_or_else(
                || "disabled".into(),
                |width| format!("{} digits", num_string(width))
            )
        ))?;
        write_stdout(&format!("  Web UI port: {}\n", num_string(default_port)))?;
        write_stdout(&format!("  Auto open browser: {auto_open_browser}\n"))?;
        if let Some(editor) = &config.default_editor {
            write_stdout(&format!("  Default editor: {editor}\n"))?;
        }
        write_stdout(&format!(
            "  Definition of Done defaults: {}\n",
            if definition_of_done.is_empty() {
                "none".into()
            } else {
                definition_of_done.join(" | ")
            }
        ))?;
    } else {
        write_stdout("Advanced settings: unchanged (run `backlog config` to customize)\n")?;
    }
    if is_re_initialization {
        write_stdout(&format!(
            "Updated backlog project configuration: {project_name}\n"
        ))?;
    } else {
        write_stdout(&format!("Initialized backlog project: {project_name}\n"))?;
    }
    if integration_mode == "cli" {
        if let Some(message) = agent_results.0 {
            write_stdout(&format!("{message}\n"))?;
        } else if agent_skipped {
            write_stdout("Skipping agent instruction files per selection.\n")?;
        }
        if should_install_claude_agent {
            write_stdout("Claude Code Backlog.md agent Installed to .claude/agents/\n")?;
        }
    }
    Ok(())
}

fn normalize_init_integration_mode(value: &str) -> Result<String, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "cli" | "instructions" | "agent-instructions" | "local" => Ok("cli".into()),
        "mcp" | "connector" | "server" | "remote" => Ok("mcp".into()),
        "none" | "skip" | "manual" | "later" | "no" | "off" => Ok("none".into()),
        _ => Err(format!(
            "Invalid integration mode: {value}. Valid options are: mcp, cli, none"
        )),
    }
}

fn task_prefix_error(prefix: &str) -> Option<String> {
    if prefix.is_empty() {
        return None;
    }
    if !prefix.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return Some("Task prefix must contain only letters (a-z, A-Z).".into());
    }
    if ["draft", "doc", "decision"]
        .iter()
        .any(|reserved| prefix.eq_ignore_ascii_case(reserved))
    {
        return Some(format!(
            "Task prefix \"{prefix}\" is reserved for drafts, docs, or decisions. Choose a different prefix."
        ));
    }
    None
}

fn parse_init_agent_selection(value: &str) -> Result<(Vec<String>, bool), String> {
    let mut selected = Vec::new();
    let mut skip = false;
    for instruction in value
        .split(',')
        .map(|item| item.trim().to_ascii_lowercase())
    {
        let file = match instruction.as_str() {
            "cursor" | "agents" | "agents.md" => Some("AGENTS.md"),
            "claude" | "claude.md" => Some("CLAUDE.md"),
            "gemini" | "gemini.md" => Some("GEMINI.md"),
            "copilot" | ".github/copilot-instructions.md" => {
                Some(".github/copilot-instructions.md")
            }
            "none" => {
                skip = true;
                None
            }
            "" => None,
            _ => {
                return Err(format!(
                    "Invalid agent instruction: {instruction}\nValid options are: cursor, claude, agents, gemini, copilot, none"
                ));
            }
        };
        if let Some(file) = file
            && !selected.iter().any(|current| current == file)
        {
            selected.push(file.to_owned());
        }
    }
    let skipped = selected.is_empty() && skip;
    if selected.is_empty() && !skipped {
        return Err("Please select at least one agent instruction file before continuing.".into());
    }
    Ok((selected, skipped))
}

fn parse_init_bool(value: Option<&str>, default: bool) -> bool {
    value.map_or(default, |value| {
        value.eq_ignore_ascii_case("true") || value == "1"
    })
}

fn parse_init_int(value: Option<&str>, default: f64) -> f64 {
    value
        .and_then(parse_int_prefix)
        .map(|value| value as f64)
        .unwrap_or(default)
}

fn write_init_agent_instructions(
    root: &std::path::Path,
    selected: &[String],
) -> Result<(Option<String>, Vec<PathBuf>), String> {
    const NUDGE: &str = include_str!("../../src/guidelines/cli-agent-nudge.md");
    let version = env!("CARGO_PKG_VERSION");
    let mut actions: Vec<(&str, Vec<String>)> = vec![
        ("Created", Vec::new()),
        ("Updated", Vec::new()),
        ("Unchanged", Vec::new()),
    ];
    let mut changed_paths = Vec::new();
    for name in selected {
        let path = root.join(name);
        let existed = path.exists();
        let original = if existed {
            fs::read_to_string(&path).map_err(|error| error.to_string())?
        } else {
            String::new()
        };
        let (without_mcp, _, _) = strip_managed(&original, "BACKLOG.MD MCP GUIDELINES");
        let (content, removed, insertion) = strip_managed(&without_mcp, "BACKLOG.MD GUIDELINES");
        let block = format!(
            "\n<!-- BACKLOG.MD GUIDELINES START -->\n<!-- backlog.md-instructions-version: {version} -->\n{}\n<!-- BACKLOG.MD GUIDELINES END -->\n",
            NUDGE.trim()
        );
        let updated = if removed {
            let index = insertion.unwrap_or(content.len()).min(content.len());
            let mut updated = content;
            updated.insert_str(index, &block);
            updated
        } else if content.contains("<!-- BACKLOG.MD GUIDELINES START -->") {
            content
        } else {
            let mut updated = content;
            if !updated.is_empty() && !updated.ends_with('\n') {
                updated.push('\n');
            }
            updated.push_str(&block);
            updated
        };
        let action_index = if updated == original {
            2
        } else if existed {
            1
        } else {
            0
        };
        actions[action_index].1.push(name.clone());
        if updated != original {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            fs::write(&path, updated).map_err(|error| error.to_string())?;
            changed_paths.push(path);
        }
    }
    let summary = actions
        .into_iter()
        .filter(|(_, names)| !names.is_empty())
        .map(|(action, names)| format!("{action}: {}", names.join(", ")))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(((!summary.is_empty()).then_some(summary), changed_paths))
}

fn config_display(c: &Config, key: &str) -> String {
    match key {
        "projectName" => c.project_name.clone(),
        "defaultEditor" => c
            .default_editor
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or("(not set)".into()),
        "defaultAssignee" => c.default_assignee.clone().unwrap_or_default().join(", "),
        "defaultStatus" => c
            .default_status
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or("(not set)".into()),
        "statuses" => c.statuses.join(", "),
        "labels" => c.labels.join(", "),
        "priorities" => c
            .priorities
            .clone()
            .unwrap_or_else(|| vec!["High".into(), "Medium".into(), "Low".into()])
            .join(", "),
        "types" => c
            .types
            .clone()
            .unwrap_or_else(|| {
                vec![
                    "bug".into(),
                    "feature".into(),
                    "enhancement".into(),
                    "task".into(),
                    "chore".into(),
                    "docs".into(),
                    "spike".into(),
                ]
            })
            .join(", "),
        "projects" => c.projects.clone().unwrap_or_default().join(", "),
        "milestones" => "[]".into(),
        "definitionOfDone" => c.definition_of_done.clone().unwrap_or_default().join(" | "),
        "dateFormat" => c.date_format.clone(),
        "maxColumnWidth" => c
            .max_column_width
            .map(num_string)
            .unwrap_or("(not set)".into()),
        "autoOpenBrowser" => c
            .auto_open_browser
            .map(|v| v.to_string())
            .unwrap_or("(not set)".into()),
        "hideEmptyColumns" => c
            .hide_empty_columns
            .map(|v| v.to_string())
            .unwrap_or("(not set)".into()),
        "defaultPort" => c.default_port.map(num_string).unwrap_or("(not set)".into()),
        "remoteOperations" => c
            .remote_operations
            .map(|v| v.to_string())
            .unwrap_or("(not set)".into()),
        "autoCommit" => c
            .auto_commit
            .map(|v| v.to_string())
            .unwrap_or("(not set)".into()),
        "filesystemOnly" => c.filesystem_only.unwrap_or(false).to_string(),
        "bypassGitHooks" => c
            .bypass_git_hooks
            .map(|v| v.to_string())
            .unwrap_or("(not set)".into()),
        "zeroPaddedIds" => c
            .zero_padded_ids
            .filter(|v| *v != 0.0)
            .map(|v| format!("{} digits", num_string(v)))
            .unwrap_or("(disabled)".into()),
        "checkActiveBranches" => c.check_active_branches.unwrap_or(true).to_string(),
        "activeBranchDays" => c.active_branch_days.map(num_string).unwrap_or("30".into()),
        _ => String::new(),
    }
}

fn run_agents_update() -> Result<(), String> {
    let context = project_context()?;
    if context.config_path.is_none() {
        return Err("No backlog project found. Initialize one first with: backlog init".into());
    }
    write_stdout(
        "Select agent instruction files to update (enter comma-separated names): CLAUDE.md, AGENTS.md, GEMINI.md, .github/copilot-instructions.md\n",
    )?;
    let mut selection = String::new();
    io::stdin()
        .read_line(&mut selection)
        .map_err(|e| e.to_string())?;
    let selected: Vec<&str> = selection
        .trim()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if selected.is_empty() {
        return write_stdout("No files selected for update.\n");
    }
    const NUDGE: &str = include_str!("../../src/guidelines/cli-agent-nudge.md");
    let nudge = NUDGE.trim();
    let version = env!("CARGO_PKG_VERSION");
    let names = [
        "CLAUDE.md",
        "AGENTS.md",
        "GEMINI.md",
        ".github/copilot-instructions.md",
    ];
    let mut changed = Vec::new();
    for name in &selected {
        if !names.contains(name) {
            return Err(format!("Unknown instruction file: {name}"));
        }
        let path = context.root.join(name);
        let exists = path.exists();
        let original = if exists {
            fs::read_to_string(&path).map_err(|e| e.to_string())?
        } else {
            String::new()
        };
        let mut content = original.clone();
        let (mcp_stripped, _, _) = strip_managed(&content, "BACKLOG.MD MCP GUIDELINES");
        content = mcp_stripped;
        let (stripped, removed, insertion) = strip_managed(&content, "BACKLOG.MD GUIDELINES");
        let mut content = stripped;
        if !removed && content.contains("<!-- BACKLOG.MD GUIDELINES START -->") {
            continue;
        }
        let block = format!(
            "\n<!-- BACKLOG.MD GUIDELINES START -->\n<!-- backlog.md-instructions-version: {version} -->\n{nudge}\n<!-- BACKLOG.MD GUIDELINES END -->\n"
        );
        let updated = if removed {
            let at = insertion.unwrap_or(content.len()).min(content.len());
            content.insert_str(at, &block);
            content
        } else {
            if !content.is_empty() && !content.ends_with('\n') {
                content.push('\n');
            }
            content.push_str(&block);
            content
        };
        if updated == original {
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&path, updated).map_err(|e| e.to_string())?;
        changed.push((name.to_string(), path));
    }
    if context.config.auto_commit == Some(true)
        && context.config.filesystem_only != Some(true)
        && !changed.is_empty()
    {
        let paths: Vec<PathBuf> = changed.iter().map(|(_, path)| path.clone()).collect();
        commit_record_change(&context, "Add AI agent instructions", &paths)?;
    }
    if changed.is_empty() {
        return write_stdout(&format!(
            "Updated {} agent instruction file(s): {}\n",
            selected.len(),
            selected.join(", ")
        ));
    }
    write_stdout(&format!(
        "Updated {} agent instruction file(s): {}\n",
        selected.len(),
        selected.join(", ")
    ))
}

fn strip_managed(content: &str, label: &str) -> (String, bool, Option<usize>) {
    let start = format!("<!-- {label} START -->");
    let end = format!("<!-- {label} END -->");
    let mut result = content.to_owned();
    let mut first = None;
    let mut removed = false;
    while let Some(si) = result.find(&start) {
        let Some(ei0) = result[si..].find(&end) else {
            break;
        };
        let ei = si + ei0 + end.len();
        let mut rs = si;
        while rs > 0 && matches!(result.as_bytes()[rs - 1], b' ' | b'\t') {
            rs -= 1;
        }
        if rs > 0 && result.as_bytes()[rs - 1] == b'\n' {
            rs -= 1;
            if rs > 0 && result.as_bytes()[rs - 1] == b'\r' {
                rs -= 1;
            }
        }
        let mut re = ei;
        if result.as_bytes().get(re) == Some(&b'\r') {
            re += 1;
        }
        if result.as_bytes().get(re) == Some(&b'\n') {
            re += 1;
        }
        first.get_or_insert(rs);
        result.replace_range(rs..re, "");
        removed = true;
    }
    (result, removed, first)
}

fn run_task_view(task_id: &str, options: TaskViewArgs) -> Result<(), String> {
    let context = project_context()?;
    let archived = context.store.list_archived_tasks();
    let (entry, is_archived) = match context
        .store
        .load_task(task_id)
        .map_err(|error| error.to_string())?
    {
        Some(entry) => (entry, false),
        None => {
            let entry = archived
                .iter()
                .find(|entry| task_ids_equal(task_id, &entry.task.id, &context.prefix))
                .cloned()
                .ok_or_else(|| {
                    format!(
                        "Task {task_id} not found. Task lookups read only the local working copy; use 'backlog browser' to see tasks from other branches."
                    )
                })?;
            (entry, true)
        }
    };
    let active_entries = context.store.list_tasks();
    let active = active_entries
        .iter()
        .map(|entry| entry.task.clone())
        .collect::<Vec<_>>();
    let archived = archived
        .into_iter()
        .map(|entry| entry.task)
        .collect::<Vec<_>>();
    let detail = build_task_detail(
        &entry.task,
        &active,
        &archived,
        &context.config.statuses,
        &[],
        &context.prefix,
    );
    if options.json {
        write_json(&task_view_json(
            &detail,
            &entry.path,
            &context.root,
            is_archived,
            &active,
            &context.prefix,
        ))
    } else {
        write_stdout(&format!(
            "{}\n",
            format_task_detail(
                &detail,
                &entry.path,
                is_archived,
                &active,
                &context.config,
                &context.prefix,
                true,
            )
        ))
    }
}

fn run_task_create(options: TaskCreateArgs) -> Result<(), String> {
    let title = options.title.as_deref().unwrap_or_default().trim();
    if title.is_empty() {
        return Err("error: missing required argument 'title'".into());
    }
    let context = project_context()?;
    if options
        .ordinal
        .is_some_and(|ordinal| !ordinal.is_finite() || ordinal < 0.0)
    {
        return Err(format!(
            "Invalid ordinal: {}. Must be a non-negative number.",
            options.ordinal.unwrap()
        ));
    }
    let status = match options
        .status
        .as_deref()
        .filter(|status| !status.trim().is_empty())
    {
        Some(requested) => Some(
            resolve_config_value(requested, &context.config.statuses).ok_or_else(|| {
                format!(
                    "Invalid status: {requested}. Valid statuses are: {}",
                    context.config.statuses.join(", ")
                )
            })?,
        ),
        None => context
            .config
            .default_status
            .clone()
            .or_else(|| Some("To Do".into())),
    };
    let default_priorities = ["High".into(), "Medium".into(), "Low".into()];
    let priority_values = context
        .config
        .priorities
        .as_deref()
        .unwrap_or(&default_priorities);
    let priority = options
        .priority
        .as_deref()
        .map(|value| {
            resolve_config_value(value, priority_values)
                .ok_or_else(|| {
                    format!(
                        "Invalid priority: {value}. Valid values are: {}",
                        priority_values.join(", ")
                    )
                })
                .map(|priority| priority.to_lowercase())
        })
        .transpose()?;
    let task_type = options
        .task_type
        .as_deref()
        .map(|value| {
            let default_types = [
                "bug".into(),
                "feature".into(),
                "enhancement".into(),
                "task".into(),
                "chore".into(),
                "docs".into(),
                "spike".into(),
            ];
            let values = context.config.types.as_deref().unwrap_or(&default_types);
            resolve_config_value(value, values).ok_or_else(|| {
                format!(
                    "Invalid type: {value}. Valid types are: {}",
                    values.join(", ")
                )
            })
        })
        .transpose()?;
    let project = options.project.as_deref().map(|value| {
        let values = context.config.projects.as_deref().unwrap_or_default();
        if values.is_empty() {
            return Err("No projects are configured. Add a 'projects:' list to the project config file.".to_owned());
        }
        resolve_config_value(value, values).ok_or_else(|| {
            format!("Invalid project: {value}. Valid projects are: {}", values.join(", "))
        })
    }).transpose()?;
    let milestone = options
        .milestone
        .as_deref()
        .map(|value| resolve_milestone_for_create(value, &context.root, &context.backlog_directory))
        .transpose()?;

    let acceptance_criteria_items = options
        .acceptance_criteria
        .iter()
        .enumerate()
        .map(|(index, text)| Criterion {
            index: index + 1,
            text: text.trim().to_owned(),
            checked: false,
        })
        .filter(|criterion| !criterion.text.is_empty())
        .collect::<Vec<_>>();
    let mut definition_of_done_items = Vec::new();
    if !options.no_dod_defaults {
        definition_of_done_items.extend(
            context
                .config
                .definition_of_done
                .as_deref()
                .unwrap_or_default()
                .iter()
                .cloned(),
        );
    }
    definition_of_done_items.extend(options.definition_of_done);
    let definition_of_done_items = definition_of_done_items
        .iter()
        .enumerate()
        .map(|(index, text)| Criterion {
            index: index + 1,
            text: text.trim().to_owned(),
            checked: false,
        })
        .filter(|criterion| !criterion.text.is_empty())
        .collect::<Vec<_>>();
    let mut dependencies = options.depends_on;
    dependencies.extend(options.dep);
    if dependencies
        .iter()
        .any(|value| is_blank_create_list_value(value))
    {
        return Err("Cannot use an empty value with --depends-on or --dep. Omit the flag to leave task dependencies unset.".into());
    }
    if options
        .references
        .iter()
        .any(|value| is_blank_create_list_value(value))
    {
        return Err(
            "Cannot use an empty value with --ref. Omit the flag to leave references unset.".into(),
        );
    }
    if options
        .documentation
        .iter()
        .any(|value| is_blank_create_list_value(value))
    {
        return Err(
            "Cannot use an empty value with --doc. Omit the flag to leave documentation unset."
                .into(),
        );
    }
    let parent_task_id = options
        .parent
        .as_deref()
        .map(|parent| {
            context
                .store
                .load_task(parent)
                .map_err(|error| error.to_string())?
                .map(|entry| canonical_task_id(&entry.task.id, &context.prefix))
                .ok_or_else(|| {
                    let parent_id = canonical_task_id(parent, &context.prefix);
                    format!("Parent task {parent_id} not found. Task lookups read only the local working copy; use 'backlog browser' to see tasks from other branches. Use an existing task ID with --parent; use --milestone to assign a task to a milestone.")
                })
        })
        .transpose()?;

    let task = TaskRecord {
        id: String::new(),
        title: title.to_owned(),
        status: status.unwrap_or_else(|| "To Do".into()),
        assignee: if options.assignee.is_empty() {
            context.config.default_assignee.clone().unwrap_or_default()
        } else {
            normalized_create_values(options.assignee)
        },
        reporter: None,
        created_date: crate::task_store::utc_timestamp(),
        updated_date: None,
        due_date: options.due_date,
        labels: normalized_create_values(options.labels),
        milestone,
        dependencies: normalized_delimited_create_values(dependencies),
        references: normalized_delimited_create_values(options.references),
        documentation: normalized_delimited_create_values(options.documentation),
        modified_files: normalized_delimited_create_values(options.modified_files),
        raw_content: String::new(),
        acceptance_criteria_items,
        definition_of_done_items,
        description: options.description.unwrap_or_default(),
        implementation_plan: options.plan,
        implementation_notes: options.notes,
        comments: Vec::new(),
        final_summary: options.final_summary,
        parent_task_id,
        subtasks: None,
        priority,
        task_type,
        project,
        ordinal: options.ordinal,
        on_status_change: None,
    };
    let entry = context
        .store
        .create_task_with_default_ordinal(task, &context.config)
        .map_err(|error| error.to_string())?;
    commit_created_record(
        &context,
        "Task",
        &format!("Create task {}", entry.task.id),
        &entry.path,
    )?;
    if options.plain {
        let active = context
            .store
            .list_tasks()
            .into_iter()
            .map(|entry| entry.task)
            .collect::<Vec<_>>();
        let archived = context
            .store
            .list_archived_tasks()
            .into_iter()
            .map(|entry| entry.task)
            .collect::<Vec<_>>();
        let detail = build_task_detail(
            &entry.task,
            &active,
            &archived,
            &context.config.statuses,
            &[],
            &context.prefix,
        );
        return write_stdout(&format!(
            "{}\n",
            format_task_detail(
                &detail,
                &entry.path,
                false,
                &active,
                &context.config,
                &context.prefix,
                false,
            )
        ));
    }
    write_stdout(&format!(
        "Created task {}\nFile: {}\n",
        entry.task.id,
        entry.path.display()
    ))
}

fn run_task_edit(options: TaskEditArgs) -> Result<(), String> {
    run_record_edit(options, EditTarget::Task)
}

fn run_draft_edit(options: TaskEditArgs) -> Result<(), String> {
    run_record_edit(options, EditTarget::Draft)
}

fn run_record_edit(options: TaskEditArgs, target: EditTarget) -> Result<(), String> {
    let checklist_options = options.clone();
    let context = project_context()?;
    let draft_store = DraftStore::new(&context.root, &context.backlog_directory);
    validate_task_edit_list_options(&options)?;
    if options.clear_due_date && options.due_date.is_some() {
        return Err(
            "Cannot use --due-date and --clear-due-date together.\nNo changes were applied.".into(),
        );
    }
    if options.clear_milestone && options.milestone.is_some() {
        return Err(
            "Cannot use --milestone and --clear-milestone together.\nNo changes were applied."
                .into(),
        );
    }
    if options.clear_labels
        && (options.labels.is_some()
            || !options.add_labels.is_empty()
            || !options.remove_labels.is_empty())
    {
        return Err("Cannot combine --clear-labels with --label, --add-label, or --remove-label. Use --clear-labels by itself, or --label a,b for the final full label set.\nNo changes were applied.".into());
    }
    if options.labels.is_some()
        && (!options.add_labels.is_empty() || !options.remove_labels.is_empty())
    {
        return Err("Cannot combine --label with --add-label or --remove-label. Use --label a,b for the final full label set, or use add/remove flags without --label.\nNo changes were applied.".into());
    }
    if options
        .ordinal
        .is_some_and(|ordinal| !ordinal.is_finite() || ordinal < 0.0)
    {
        return Err(format!(
            "Invalid ordinal: {}. Must be a non-negative number.\nNo changes were applied.",
            options.ordinal.unwrap()
        ));
    }

    let entry = match target {
        EditTarget::Task => context
            .store
            .load_task(&options.task_id)
            .map_err(|error| format!("{}\nNo changes were applied.", error))?
            .map(|entry| EditableRecord {
                task: entry.task,
                path: entry.path,
            }),
        EditTarget::Draft => {
            let path = draft_store
                .resolve_path(&options.task_id)
                .map_err(|error| format!("{error}\nNo changes were applied."))?;
            match path {
                None => None,
                Some(path) => match draft_store.read_entry(&path) {
                    Ok(entry) => Some(EditableRecord {
                        task: entry.task,
                        path: entry.path,
                    }),
                    Err(crate::draft_store::DraftStoreError::Serialization(_)) => {
                        return Err(format!(
                            "Draft file {} could not be parsed. Repair or remove the file, then rerun the edit.\nNo changes were applied.",
                            path.file_name().unwrap_or_default().to_string_lossy()
                        ));
                    }
                    Err(error) => {
                        return Err(format!("{error}\nNo changes were applied."));
                    }
                },
            }
        }
    };
    let Some(entry) = entry else {
        if target == EditTarget::Task {
            let archived = context.store.list_archived_tasks();
            if let Some(candidate) = archived.iter().find(|candidate| {
                task_ids_equal(&options.task_id, &candidate.task.id, &context.prefix)
            }) {
                return Err(format!(
                    "{}\nNo changes were applied.",
                    crate::task_store::TaskStoreError::Archived(candidate.task.id.clone())
                ));
            }
            return Err(format!(
                "Task {} not found. Task lookups read only the local working copy; use 'backlog browser' to see tasks from other branches.\nNo changes were applied.",
                options.task_id
            ));
        }
        return Err(format!(
            "Draft {} not found.\nNo changes were applied.",
            options.task_id
        ));
    };
    if target == EditTarget::Draft {
        let filename_id = draft_store
            .filename_id(&entry.path)
            .map_err(|error| format!("{error}\nNo changes were applied."))?;
        if !task_ids_equal(&entry.task.id, &filename_id, "draft") {
            return Err(format!(
                "Draft file {} declares frontmatter id {}, which does not match its filename id {}. Fix the frontmatter id or rename the file so they agree, then rerun the edit.\nNo changes were applied.",
                entry.path.file_name().unwrap_or_default().to_string_lossy(),
                canonical_task_id(&entry.task.id, "draft"),
                canonical_task_id(&filename_id, "draft")
            ));
        }
    }

    let mut updated = entry.task.clone();
    if let Some(title) = options.title.filter(|title| !title.is_empty()) {
        let title = title.trim();
        if title.is_empty() {
            return Err("Title cannot be empty.\nNo changes were applied.".into());
        }
        updated.title = title.to_owned();
    }
    if let Some(description) = options.description {
        updated.description = description;
    }
    if let Some(status) = options.status.filter(|status| !status.trim().is_empty()) {
        if target == EditTarget::Draft {
            if !status.trim().eq_ignore_ascii_case("Draft") {
                return Err(format!(
                    "Invalid status: {status}. Valid statuses are: Draft\nNo changes were applied."
                ));
            }
            updated.status = "Draft".into();
        } else {
            updated.status = resolve_config_value(&status, &context.config.statuses).ok_or_else(|| {
                format!(
                    "Invalid status: {status}. Valid statuses are: {}\nNo changes were applied.",
                    context.config.statuses.join(", ")
                )
            })?;
        }
    }
    if let Some(assignee) = options.assignee {
        updated.assignee = normalized_create_values(assignee);
    }

    let label_values = options.labels.map(normalized_delimited_create_values);
    if options.clear_labels {
        updated.labels.clear();
    } else if let Some(labels) = label_values
        && !labels.is_empty()
    {
        updated.labels = labels;
    }
    for label in normalized_delimited_create_values(options.add_labels) {
        if !updated
            .labels
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&label))
        {
            updated.labels.push(label);
        }
    }
    let removals = normalized_delimited_create_values(options.remove_labels);
    if !removals.is_empty() {
        updated.labels.retain(|label| {
            !removals
                .iter()
                .any(|removed| removed.eq_ignore_ascii_case(label))
        });
    }

    if let Some(priority) = options
        .priority
        .filter(|priority| !priority.trim().is_empty())
    {
        let defaults = ["High".into(), "Medium".into(), "Low".into()];
        let values = context.config.priorities.as_deref().unwrap_or(&defaults);
        updated.priority = Some(
            resolve_config_value(&priority, values)
                .ok_or_else(|| {
                    format!(
                        "Invalid priority: {priority}. Valid values are: {}\nNo changes were applied.",
                        values.join(", ")
                    )
                })?
                .to_lowercase(),
        );
    }
    if let Some(task_type) = options.task_type {
        if task_type.is_empty() {
            updated.task_type = None;
        } else {
            let defaults = [
                "bug".into(),
                "feature".into(),
                "enhancement".into(),
                "task".into(),
                "chore".into(),
                "docs".into(),
                "spike".into(),
            ];
            let values = context.config.types.as_deref().unwrap_or(&defaults);
            updated.task_type =
                Some(resolve_config_value(&task_type, values).ok_or_else(|| {
                    format!(
                        "Invalid type: {task_type}. Valid types are: {}\nNo changes were applied.",
                        values.join(", ")
                    )
                })?);
        }
    }
    if let Some(project) = options.project {
        if project.is_empty() {
            updated.project = None;
        } else {
            let values = context.config.projects.as_deref().unwrap_or_default();
            if values.is_empty() {
                return Err("No projects are configured. Add a 'projects:' list to the project config file.\nNo changes were applied.".into());
            }
            updated.project = Some(resolve_config_value(&project, values).ok_or_else(|| {
                format!(
                    "Invalid project: {project}. Valid projects are: {}\nNo changes were applied.",
                    values.join(", ")
                )
            })?);
        }
    }
    if let Some(due_date) = options.due_date {
        updated.due_date = Some(due_date);
    } else if options.clear_due_date {
        updated.due_date = None;
    }
    if let Some(ordinal) = options.ordinal {
        updated.ordinal = Some(ordinal);
    }
    if let Some(milestone) = options.milestone {
        updated.milestone = Some(resolve_milestone_for_create(
            &milestone,
            &context.root,
            &context.backlog_directory,
        )?);
    } else if options.clear_milestone {
        updated.milestone = None;
    }
    if let Some(dependencies) = options.dependencies {
        updated.dependencies = normalized_delimited_create_values(dependencies);
    }
    if options.clear_deps {
        updated.dependencies.clear();
    }
    if let Some(references) = options.references {
        updated.references = normalized_delimited_create_values(references);
    }
    if options.clear_refs {
        updated.references.clear();
    }
    for reference in normalized_delimited_create_values(options.add_references) {
        if !updated.references.contains(&reference) {
            updated.references.push(reference);
        }
    }
    let removals = normalized_delimited_create_values(options.remove_references);
    if !removals.is_empty() {
        updated
            .references
            .retain(|reference| !removals.contains(reference));
    }
    if let Some(documentation) = options.documentation {
        updated.documentation = normalized_delimited_create_values(documentation);
    }
    if options.clear_docs {
        updated.documentation.clear();
    }
    for document in normalized_delimited_create_values(options.add_documentation) {
        if !updated.documentation.contains(&document) {
            updated.documentation.push(document);
        }
    }
    let removals = normalized_delimited_create_values(options.remove_documentation);
    if !removals.is_empty() {
        updated
            .documentation
            .retain(|document| !removals.contains(document));
    }

    for (value, key) in [
        (options.plan.as_deref(), SectionKey::ImplementationPlan),
        (options.notes.as_deref(), SectionKey::ImplementationNotes),
        (options.final_summary.as_deref(), SectionKey::FinalSummary),
    ] {
        if let Some(value) = value {
            crate::sections::assert_section_input_safe(value, key)
                .map_err(|error| format!("{error}\nNo changes were applied."))?;
        }
    }
    for (values, key) in [
        (&options.append_plan, SectionKey::ImplementationPlan),
        (&options.append_notes, SectionKey::ImplementationNotes),
        (&options.append_final_summary, SectionKey::FinalSummary),
    ] {
        for value in values {
            crate::sections::assert_section_input_safe(value, key)
                .map_err(|error| format!("{error}\nNo changes were applied."))?;
        }
    }
    if let Some(plan) = options.plan {
        set_optional_text(&mut updated.implementation_plan, plan);
    }
    append_text_blocks(&mut updated.implementation_plan, &options.append_plan);
    if let Some(notes) = options.notes {
        set_optional_text(&mut updated.implementation_notes, notes);
    }
    append_text_blocks(&mut updated.implementation_notes, &options.append_notes);
    if !options.comment.is_empty() {
        let author = options
            .comment_author
            .as_deref()
            .map(|value| value.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|value| !value.is_empty());
        if let Some(author) = author.as_deref() {
            validate_comment_field(author, "Comment author")?;
        }
        let mut comments = updated.comments.clone();
        let mut next_index = comments
            .iter()
            .map(|comment| comment.index)
            .max()
            .unwrap_or(0)
            + 1;
        let created_date = crate::task_store::utc_timestamp();
        for body in &options.comment {
            let body = body.replace("\r\n", "\n").trim().to_owned();
            if body.is_empty() {
                continue;
            }
            validate_comment_field(&body, "Comment body")?;
            comments.push(Comment {
                index: next_index,
                body,
                created_date: created_date.clone(),
                author: author.clone(),
            });
            next_index += 1;
        }
        updated.comments = comments;
    }
    if options.clear_final_summary && updated.final_summary.is_some() {
        updated.final_summary = Some(String::new());
    }
    if let Some(summary) = options.final_summary {
        set_optional_text(&mut updated.final_summary, summary);
    }
    append_text_blocks(&mut updated.final_summary, &options.append_final_summary);
    if let Some(modified_files) = options.modified_files {
        let modified_files = normalized_delimited_create_values(modified_files);
        if !modified_files.is_empty() {
            updated.modified_files = modified_files;
        }
    }
    apply_checklist_options(
        &mut updated,
        &entry.task.raw_content,
        &checklist_options,
        if target == EditTarget::Task {
            "task"
        } else {
            "draft"
        },
        &canonical_task_id(
            &entry.task.id,
            if target == EditTarget::Task {
                &context.prefix
            } else {
                "draft"
            },
        ),
    )?;

    let updated_entry = if updated != entry.task {
        updated.updated_date = Some(crate::task_store::utc_timestamp());
        let updated_entry = match target {
            EditTarget::Task => context
                .store
                .update_task(updated)
                .map_err(|error| {
                    format!(
                        "{}\nNo changes were applied.",
                        task_edit_store_error(error, &entry.task.id, &context.prefix)
                    )
                })?
                .map(|entry| EditableRecord {
                    task: entry.task,
                    path: entry.path,
                }),
            EditTarget::Draft => draft_store
                .update_draft(updated)
                .map_err(|error| format!("{error}\nNo changes were applied."))?
                .map(|entry| EditableRecord {
                    task: entry.task,
                    path: entry.path,
                }),
        }
        .ok_or_else(|| match target {
            EditTarget::Task => format!(
                "Task {} not found. Task lookups read only the local working copy; use 'backlog browser' to see tasks from other branches.\nNo changes were applied.",
                options.task_id
            ),
            EditTarget::Draft => format!(
                "Draft {} not found.\nNo changes were applied.",
                options.task_id
            ),
        })?;
        let selected_paths = if entry.path == updated_entry.path {
            vec![updated_entry.path.clone()]
        } else {
            vec![entry.path.clone(), updated_entry.path.clone()]
        };
        commit_record_change(
            &context,
            &format!(
                "Update {} {}",
                if target == EditTarget::Task {
                    "task"
                } else {
                    "draft"
                },
                canonical_task_id(
                    &updated_entry.task.id,
                    if target == EditTarget::Task {
                        &context.prefix
                    } else {
                        "draft"
                    }
                )
            ),
            &selected_paths,
        )?;
        updated_entry
    } else {
        entry
    };
    let mut updated_task = updated_entry.task;
    let id_prefix = if target == EditTarget::Task {
        &context.prefix
    } else {
        "draft"
    };
    updated_task.id = canonical_task_id(&updated_task.id, id_prefix);
    if options.plain {
        let active = context
            .store
            .list_tasks()
            .into_iter()
            .map(|entry| entry.task)
            .collect::<Vec<_>>();
        let archived = context
            .store
            .list_archived_tasks()
            .into_iter()
            .map(|entry| entry.task)
            .collect::<Vec<_>>();
        let detail = build_task_detail(
            &updated_task,
            &active,
            &archived,
            &context.config.statuses,
            &[],
            &context.prefix,
        );
        return write_stdout(&format!(
            "{}\n",
            format_task_detail(
                &detail,
                &updated_entry.path,
                false,
                &active,
                &context.config,
                &context.prefix,
                true,
            )
        ));
    }
    write_stdout(&format!(
        "Updated {} {}\n",
        if target == EditTarget::Task {
            "task"
        } else {
            "draft"
        },
        updated_task.id
    ))
}

fn validate_task_edit_list_options(options: &TaskEditArgs) -> Result<(), String> {
    let values_are_blank = |values: &[String]| {
        values
            .iter()
            .all(|value| value.split(',').all(|item| item.trim().is_empty()))
    };
    let has_nonblank = |values: &[String]| !values.is_empty() && !values_are_blank(values);
    if options.clear_ac
        && (options.acceptance_criteria.is_some()
            || !options.add_ac.is_empty()
            || !options.remove_ac.is_empty()
            || !options.check_ac.is_empty()
            || !options.uncheck_ac.is_empty())
    {
        return Err("Cannot combine --clear-ac with --acceptance-criteria, --ac, --remove-ac, --check-ac, or --uncheck-ac. Use --clear-ac by itself.\nNo changes were applied.".into());
    }
    if options.acceptance_criteria.is_some()
        && (!options.add_ac.is_empty()
            || !options.remove_ac.is_empty()
            || !options.check_ac.is_empty()
            || !options.uncheck_ac.is_empty())
    {
        return Err("Cannot combine --acceptance-criteria with --ac, --remove-ac, --check-ac, or --uncheck-ac. Use replacement by itself, or use only incremental operations.\nNo changes were applied.".into());
    }
    if options.clear_deps && options.dependencies.as_deref().is_some_and(has_nonblank) {
        return Err("Cannot combine --clear-deps with --depends-on or --dep. Use --clear-deps by itself.\nNo changes were applied.".into());
    }
    if options.clear_refs && options.references.as_deref().is_some_and(has_nonblank) {
        return Err("Cannot combine --clear-refs with --ref. Use --clear-refs by itself.\nNo changes were applied.".into());
    }
    if options.clear_refs && has_nonblank(&options.add_references) {
        return Err("Cannot combine --clear-refs with --add-ref. Use --clear-refs by itself.\nNo changes were applied.".into());
    }
    if options.clear_refs && has_nonblank(&options.remove_references) {
        return Err("Cannot combine --clear-refs with --remove-ref. Use --clear-refs by itself.\nNo changes were applied.".into());
    }
    if options.references.is_some()
        && (!options.add_references.is_empty() || !options.remove_references.is_empty())
    {
        return Err("Cannot combine --ref with --add-ref or --remove-ref. Use --ref a,b for the final full reference set, or use add/remove flags without --ref.\nNo changes were applied.".into());
    }
    if options.clear_docs && options.documentation.as_deref().is_some_and(has_nonblank) {
        return Err("Cannot combine --clear-docs with --doc. Use --clear-docs by itself.\nNo changes were applied.".into());
    }
    if options.clear_docs && has_nonblank(&options.add_documentation) {
        return Err("Cannot combine --clear-docs with --add-doc. Use --clear-docs by itself.\nNo changes were applied.".into());
    }
    if options.clear_docs && has_nonblank(&options.remove_documentation) {
        return Err("Cannot combine --clear-docs with --remove-doc. Use --clear-docs by itself.\nNo changes were applied.".into());
    }
    if options.documentation.is_some()
        && (!options.add_documentation.is_empty() || !options.remove_documentation.is_empty())
    {
        return Err("Cannot combine --doc with --add-doc or --remove-doc. Use --doc a,b for the final full documentation set, or use add/remove flags without --doc.\nNo changes were applied.".into());
    }
    for (flag, values, clear_flag, subject) in [
        (
            "--add-ref",
            &options.add_references,
            "--clear-refs",
            "references",
        ),
        (
            "--remove-ref",
            &options.remove_references,
            "--clear-refs",
            "references",
        ),
        (
            "--add-doc",
            &options.add_documentation,
            "--clear-docs",
            "documentation",
        ),
        (
            "--remove-doc",
            &options.remove_documentation,
            "--clear-docs",
            "documentation",
        ),
    ] {
        if values
            .iter()
            .any(|value| values_are_blank(std::slice::from_ref(value)))
        {
            return Err(format!(
                "Cannot use an empty value with {flag}. Use {clear_flag} to remove all {subject}.\nNo changes were applied."
            ));
        }
    }
    Ok(())
}

fn checklist_indices(
    values: &[String],
    command_kind: &str,
    task_id: &str,
) -> Result<Vec<usize>, String> {
    values
        .iter()
        .map(|value| {
            let index = value.parse::<usize>().ok().filter(|index| *index > 0);
            index.ok_or_else(|| {
                format!(
                    "Invalid index: {value}. Index must be a positive number (1-based). Try 'backlog {command_kind} edit {task_id} --help' for index options.\nNo changes were applied."
                )
            })
        })
        .collect()
}

fn apply_checklist_options(
    task: &mut TaskRecord,
    original_body: &str,
    options: &TaskEditArgs,
    command_kind: &str,
    task_id: &str,
) -> Result<(), String> {
    let ac_touched = options.acceptance_criteria.is_some()
        || options.clear_ac
        || !options.add_ac.is_empty()
        || !options.remove_ac.is_empty()
        || !options.check_ac.is_empty()
        || !options.uncheck_ac.is_empty();
    let dod_touched = !options.add_dod.is_empty()
        || !options.remove_dod.is_empty()
        || !options.check_dod.is_empty()
        || !options.uncheck_dod.is_empty();
    if !options.remove_ac.is_empty()
        || !options.check_ac.is_empty()
        || !options.uncheck_ac.is_empty()
    {
        crate::sections::assert_valid_checklist_marks(
            original_body,
            ChecklistKind::AcceptanceCriteria,
        )
        .map_err(|error| {
            format_malformed_checklist_error(&error.to_string(), command_kind, task_id)
        })?;
    }
    if !options.remove_dod.is_empty()
        || !options.check_dod.is_empty()
        || !options.uncheck_dod.is_empty()
    {
        crate::sections::assert_valid_checklist_marks(
            original_body,
            ChecklistKind::DefinitionOfDone,
        )
        .map_err(|error| {
            format_malformed_checklist_error(&error.to_string(), command_kind, task_id)
        })?;
    }
    if ac_touched {
        let mut criteria = task.acceptance_criteria_items.clone();
        if options.clear_ac || options.acceptance_criteria.is_some() {
            let replacement = if options.clear_ac {
                Vec::new()
            } else {
                normalized_create_values(options.acceptance_criteria.clone().unwrap_or_default())
            };
            let mut old = criteria.clone();
            criteria = replacement
                .into_iter()
                .enumerate()
                .map(|(i, text)| {
                    let checked = old
                        .iter()
                        .position(|item| item.text.trim() == text.trim())
                        .map(|position| old.remove(position).checked)
                        .unwrap_or(false);
                    Criterion {
                        index: i + 1,
                        text,
                        checked,
                    }
                })
                .collect();
        }
        criteria.extend(
            normalized_create_values(options.add_ac.clone())
                .into_iter()
                .map(|text| Criterion {
                    index: 0,
                    text,
                    checked: false,
                }),
        );
        reindex_criteria(&mut criteria);
        let removals = checklist_indices(&options.remove_ac, command_kind, task_id)?;
        if !removals.is_empty() {
            let before = criteria.len();
            criteria.retain(|item| !removals.contains(&item.index));
            if before == criteria.len() {
                return Err(missing_checklist_index_error(
                    "Acceptance criterion",
                    &removals,
                    &criteria,
                    "No acceptance criteria are defined.",
                    command_kind,
                    task_id,
                ));
            }
            reindex_criteria(&mut criteria);
        }
        apply_check_state(
            &mut criteria,
            &options.check_ac,
            true,
            "Acceptance criterion",
            command_kind,
            task_id,
            "No acceptance criteria are defined.",
        )?;
        apply_check_state(
            &mut criteria,
            &options.uncheck_ac,
            false,
            "Acceptance criterion",
            command_kind,
            task_id,
            "No acceptance criteria are defined.",
        )?;
        task.acceptance_criteria_items = criteria;
    }
    if dod_touched {
        let mut criteria = task.definition_of_done_items.clone();
        criteria.extend(
            normalized_create_values(options.add_dod.clone())
                .into_iter()
                .map(|text| Criterion {
                    index: 0,
                    text,
                    checked: false,
                }),
        );
        reindex_criteria(&mut criteria);
        apply_check_state(
            &mut criteria,
            &options.check_dod,
            true,
            "Definition of Done item",
            command_kind,
            task_id,
            "No Definition of Done items are defined.",
        )?;
        apply_check_state(
            &mut criteria,
            &options.uncheck_dod,
            false,
            "Definition of Done item",
            command_kind,
            task_id,
            "No Definition of Done items are defined.",
        )?;
        let removals = checklist_indices(&options.remove_dod, command_kind, task_id)?;
        if !removals.is_empty() {
            let before = criteria.len();
            criteria.retain(|item| !removals.contains(&item.index));
            if before == criteria.len() {
                return Err(missing_checklist_index_error(
                    "Definition of Done item",
                    &removals,
                    &criteria,
                    "No Definition of Done items are defined.",
                    command_kind,
                    task_id,
                ));
            }
            reindex_criteria(&mut criteria);
        }
        task.definition_of_done_items = criteria;
    }
    Ok(())
}

fn reindex_criteria(criteria: &mut [Criterion]) {
    for (index, item) in criteria.iter_mut().enumerate() {
        item.index = index + 1;
    }
}

fn apply_check_state(
    criteria: &mut [Criterion],
    values: &[String],
    checked: bool,
    name: &str,
    command_kind: &str,
    task_id: &str,
    empty_hint: &str,
) -> Result<(), String> {
    let indices = checklist_indices(values, command_kind, task_id)?;
    let mut missing = Vec::new();
    for index in indices {
        let Some(item) = criteria.iter_mut().find(|item| item.index == index) else {
            missing.push(index);
            continue;
        };
        item.checked = checked;
    }
    if !missing.is_empty() {
        return Err(missing_checklist_index_error(
            name,
            &missing,
            criteria,
            empty_hint,
            command_kind,
            task_id,
        ));
    }
    Ok(())
}

fn missing_checklist_index_error(
    name: &str,
    missing: &[usize],
    available: &[Criterion],
    empty_hint: &str,
    command_kind: &str,
    task_id: &str,
) -> String {
    let missing = missing
        .iter()
        .map(|index| format!("#{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    if command_kind == "task" {
        return format!("{name} {missing} not found.\nNo changes were applied.");
    }
    let hint = if available.is_empty() {
        empty_hint.to_owned()
    } else {
        let first = available.iter().map(|item| item.index).min().unwrap_or(1);
        let last = available
            .iter()
            .map(|item| item.index)
            .max()
            .unwrap_or(first);
        let range = if first == last {
            format!("#{first}")
        } else {
            format!("#{first}-#{last}")
        };
        format!("Available indexes: {range}.")
    };
    format!(
        "{name} {missing} not found. {hint}\nRun 'backlog {command_kind} view {task_id} --plain' to inspect indexes, or 'backlog {command_kind} edit {task_id} --help' for edit options.\nNo changes were applied."
    )
}

fn format_malformed_checklist_error(error: &str, command_kind: &str, task_id: &str) -> String {
    if command_kind == "task" {
        return format!("{error}\nNo changes were applied.");
    }
    format!(
        "{error}\nRun 'backlog draft view {task_id} --plain' to locate the draft file, repair or remove the malformed marker block in that Markdown file, then rerun the edit.\nNo changes were applied."
    )
}

fn task_edit_store_error(
    error: crate::task_store::TaskStoreError,
    task_id: &str,
    prefix: &str,
) -> String {
    let message = error
        .to_string()
        .replace("due_date must be a date in", "Due date must be a date in");
    if let Some((_, details)) = message
        .strip_prefix("Task ")
        .and_then(|message| message.split_once(" cannot depend on itself"))
    {
        return format!(
            "Task {} cannot depend on itself{details}",
            canonical_task_id(task_id, prefix)
        );
    }
    if let Some(cycle) = message.strip_prefix("These dependencies would create a cycle: ") {
        let cycle = cycle
            .split(" -> ")
            .map(|task_id| canonical_task_id(task_id, prefix))
            .collect::<Vec<_>>()
            .join(" -> ");
        return format!("These dependencies would create a cycle: {cycle}");
    }
    message
}

fn run_task_archive(task_id: &str) -> Result<(), String> {
    let context = project_context()?;
    let Some(task) = context
        .store
        .load_task(task_id)
        .map_err(|error| error.to_string())?
    else {
        if let Some(archived) = context
            .store
            .list_archived_tasks()
            .into_iter()
            .find(|task| task_ids_equal(task_id, &task.task.id, &context.prefix))
        {
            return Err(crate::task_store::TaskStoreError::Archived(archived.task.id).to_string());
        }
        return Err(task_not_found(task_id));
    };
    let id = canonical_task_id(&task.task.id, &context.prefix);
    if !context
        .store
        .archive_task(&id)
        .map_err(|error| lifecycle_error(error, &context.prefix))?
    {
        return Err(task_not_found(task_id));
    }
    let archived_path = context
        .backlog_directory
        .join("archive/tasks")
        .join(task.path.file_name().unwrap_or_default());
    commit_record_change(
        &context,
        &format!("backlog: Archive task {}", task.task.id),
        &[task.path.clone(), archived_path],
    )
    .map_err(|error| {
        format!(
            "Task {} was archived, but the change was not committed: {error}",
            task.task.id
        )
    })?;
    write_stdout(&format!("Archived task {id}\n"))
}

fn run_task_restore(task_id: &str) -> Result<(), String> {
    let context = project_context()?;
    let id = canonical_task_id(task_id, &context.prefix);
    if context
        .store
        .load_task(&id)
        .map_err(|error| error.to_string())?
        .is_some()
    {
        return Err(format!(
            "Task {id} is not archived. It is already on the board."
        ));
    }
    let archived = context.store.list_archived_tasks();
    let Some(task) = archived
        .iter()
        .find(|task| task_ids_equal(&id, &task.task.id, &context.prefix))
    else {
        return Err(task_not_found(task_id));
    };
    let archived_id = task.task.id.clone();
    let archived_path = task.path.clone();
    let active_path = context
        .backlog_directory
        .join("tasks")
        .join(archived_path.file_name().unwrap_or_default());
    if !context
        .store
        .restore_task(&archived_id)
        .map_err(|error| lifecycle_error(error, &context.prefix))?
    {
        return Err(task_not_found(task_id));
    }
    commit_record_change(
        &context,
        &format!("backlog: Restore task {archived_id}"),
        &[archived_path, active_path],
    )
    .map_err(|error| {
        format!("Task {archived_id} was restored, but the change was not committed: {error}")
    })?;
    write_stdout(&format!("Restored task {archived_id}\n"))
}

fn run_task_delete(task_id: &str) -> Result<(), String> {
    let context = project_context()?;
    let id = canonical_task_id(task_id, &context.prefix);
    if context
        .store
        .load_task(&id)
        .map_err(|error| error.to_string())?
        .is_some()
    {
        return Err(format!(
            "Task {id} is not archived. A task must be archived before it can be deleted."
        ));
    }
    let archived = context.store.list_archived_tasks();
    let Some(task) = archived
        .iter()
        .find(|task| task_ids_equal(&id, &task.task.id, &context.prefix))
    else {
        return Err(task_not_found(task_id));
    };
    let archived_id = task.task.id.clone();
    let board_path = context
        .backlog_directory
        .join("tasks")
        .join(task.path.file_name().unwrap_or_default());
    let deletion = context
        .store
        .delete_task(&archived_id)
        .map_err(|error| lifecycle_error(error, &context.prefix))?;
    if !deletion.success {
        return Err(task_not_found(task_id));
    }
    let mut changed_paths = deletion.changed_paths;
    changed_paths.push(board_path);
    commit_record_change(
        &context,
        &format!("backlog: Delete task {archived_id}"),
        &changed_paths,
    )
    .map_err(|error| {
        format!("Task {archived_id} was deleted, but the change was not committed: {error}")
    })?;
    write_stdout(&format!("Deleted task {archived_id}\n"))?;
    if !deletion.cleaned_task_ids.is_empty() {
        write_stdout(&format!(
            "Removed references to {archived_id} from {}\n",
            deletion.cleaned_task_ids.join(", ")
        ))?;
    }
    Ok(())
}

fn run_task_demote(task_id: &str) -> Result<(), String> {
    let context = project_context()?;
    let Some(task) = context
        .store
        .load_task(task_id)
        .map_err(|error| error.to_string())?
    else {
        if let Some(archived) = context
            .store
            .load_archived_task(task_id)
            .map_err(|error| lifecycle_error(error, &context.prefix))?
        {
            return Err(lifecycle_error(
                crate::task_store::TaskStoreError::Archived(archived.task.id),
                &context.prefix,
            ));
        }
        return Err(task_not_found(task_id));
    };
    let id = canonical_task_id(&task.task.id, &context.prefix);
    let demotion = context
        .store
        .demote_task(&id, &context.config)
        .map_err(|error| lifecycle_error(error, &context.prefix))?;
    if !demotion.success {
        return Err(task_not_found(task_id));
    }
    commit_record_change(
        &context,
        &format!("backlog: Demote task {}", task.task.id),
        &demotion.changed_paths,
    )
    .map_err(|error| {
        format!(
            "Task {} was demoted, but the change was not committed: {error}",
            task.task.id
        )
    })?;
    write_stdout(&format!("Demoted task {}\n", task.task.id))?;
    if !demotion.cleaned_task_ids.is_empty() {
        write_stdout(&format!(
            "Removed references to {} from {}\n",
            task.task.id,
            demotion.cleaned_task_ids.join(", ")
        ))?;
    }
    Ok(())
}

fn task_not_found(task_id: &str) -> String {
    format!(
        "Task {task_id} not found. Task lookups read only the local working copy; use 'backlog browser' to see tasks from other branches."
    )
}

fn lifecycle_error(error: crate::task_store::TaskStoreError, prefix: &str) -> String {
    use crate::task_store::TaskStoreError;
    match error {
        TaskStoreError::HasSubtasks {
            id: stored_id,
            child_ids,
        } => format!(
            "Task {stored_id} cannot be deleted while its subtasks exist: {}.",
            child_ids
                .iter()
                .map(|child| canonical_task_id(child, prefix))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        error => error.to_string(),
    }
}

fn normalized_create_values(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .collect()
}

fn set_optional_text(field: &mut Option<String>, value: String) {
    if field.as_deref().unwrap_or_default() != value {
        *field = Some(value);
    }
}

fn append_text_blocks(field: &mut Option<String>, values: &[String]) {
    let additions = values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if additions.is_empty() {
        return;
    }
    let current = field.as_deref().unwrap_or_default().trim();
    let addition = additions.join("\n\n");
    *field = Some(if current.is_empty() {
        addition
    } else {
        format!("{current}\n\n{addition}")
    });
}

fn validate_comment_field(value: &str, field: &str) -> Result<(), String> {
    if value.to_ascii_lowercase().contains("<!-- comment:")
        || value.to_ascii_lowercase().contains("<!-- comments:")
    {
        return Err(format!(
            "{field} cannot contain Backlog comment markers.\nNo changes were applied."
        ));
    }
    if value
        .replace("\r\n", "\n")
        .lines()
        .any(|line| line.trim() == "---")
    {
        return Err(format!(
            "{field} cannot contain standalone '---' delimiter lines.\nNo changes were applied."
        ));
    }
    Ok(())
}

fn normalized_delimited_create_values(values: Vec<String>) -> Vec<String> {
    normalized_create_values(
        values
            .into_iter()
            .flat_map(|value| value.split(',').map(str::to_owned).collect::<Vec<_>>())
            .collect(),
    )
}

fn is_blank_create_list_value(value: &str) -> bool {
    value.split(',').all(|item| item.trim().is_empty())
}

fn resolve_milestone_for_create(
    input: &str,
    root: &std::path::Path,
    backlog_directory: &std::path::Path,
) -> Result<String, String> {
    let normalized = input.trim();
    if normalized.is_empty() {
        return Ok(normalized.to_owned());
    }
    let store = MilestoneStore::new(root, backlog_directory);
    let active = store.list_active();
    let archived = store.list_archived();
    let id_like = normalized
        .strip_prefix("m-")
        .or_else(|| normalized.strip_prefix("M-"))
        .unwrap_or(normalized);
    let looks_like_id = id_like.bytes().all(|byte| byte.is_ascii_digit());
    let all = active.iter().chain(archived.iter()).collect::<Vec<_>>();
    if looks_like_id {
        let requested_number = id_like.trim_start_matches('0');
        let requested_number = if requested_number.is_empty() {
            "0"
        } else {
            requested_number
        };
        if let Some(milestone) = all.iter().find(|milestone| {
            let number = milestone
                .id
                .strip_prefix("m-")
                .or_else(|| milestone.id.strip_prefix("M-"));
            number.is_some_and(|number| {
                let number = number.trim_start_matches('0');
                let number = if number.is_empty() { "0" } else { number };
                number == requested_number
            })
        }) {
            return Ok(milestone.id.clone());
        }
    }
    let active_matches = active
        .iter()
        .filter(|milestone| milestone.title.trim().eq_ignore_ascii_case(normalized))
        .collect::<Vec<_>>();
    if active_matches.len() == 1 {
        return Ok(active_matches[0].id.clone());
    }
    if active_matches.len() > 1 {
        return Ok(normalized.to_owned());
    }
    let archived_matches = archived
        .iter()
        .filter(|milestone| milestone.title.trim().eq_ignore_ascii_case(normalized))
        .collect::<Vec<_>>();
    if archived_matches.len() == 1 {
        return Ok(archived_matches[0].id.clone());
    }
    Ok(normalized.to_owned())
}

fn resolve_config_value(value: &str, configured: &[String]) -> Option<String> {
    configured
        .iter()
        .find(|candidate| candidate.eq_ignore_ascii_case(value.trim()))
        .cloned()
}

fn run_task_list(options: TaskListArgs) -> Result<(), String> {
    let context = project_context()?;
    let active_entries = context.store.list_tasks();
    let tasks = active_entries
        .iter()
        .map(|entry| entry.task.clone())
        .collect::<Vec<_>>();
    let archived = context
        .store
        .list_archived_tasks()
        .into_iter()
        .map(|entry| entry.task)
        .collect::<Vec<_>>();
    let filter = task_filter(&options, &context.config.statuses)?;
    let index = TaskSearchIndex::new(&tasks, &[]);
    let query = options.search.as_deref().unwrap_or_default();
    let mut results = index
        .search(query, &filter, &context.prefix)
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let sort_field = options.sort.map(sort_name);
    let priorities = context.config.priorities.as_deref().unwrap_or_default();
    results = sort_tasks(&results, sort_field, priorities);
    if let Some(limit) = options.limit {
        results.truncate(limit);
    }

    if options.json {
        let list = with_readiness(
            &results,
            &tasks,
            &archived,
            &context.config.statuses,
            &[],
            &context.prefix,
        );
        write_json(&task_list_json(&list))
    } else {
        print_task_sections(
            &results,
            sort_field,
            &context.config.statuses,
            &context.config.date_format,
        )
    }
}

fn run_draft_list(options: DraftListArgs) -> Result<(), String> {
    if options.window.count && options.json {
        return Err(
            "--count cannot be combined with --json. Try 'backlog draft list --help' for options."
                .into(),
        );
    }
    if options.plain && options.json {
        return Err("--json cannot be combined with --plain.".into());
    }
    let sort_field = options
        .sort
        .as_deref()
        .unwrap_or("priority")
        .to_ascii_lowercase();
    if !["priority", "id", "ordinal"].contains(&sort_field.as_str()) {
        return Err(format!(
            "Invalid sort field: {}. Valid values are: priority, id, ordinal",
            options.sort.as_deref().unwrap()
        ));
    }
    let window = parse_list_window(&options.window, "backlog draft list")?;
    let context = project_context()?;
    let mut drafts = DraftStore::new(&context.root, &context.backlog_directory)
        .list_drafts()
        .into_iter()
        .map(|entry| entry.task)
        .collect::<Vec<_>>();
    drafts = sort_tasks(
        &drafts,
        Some(&sort_field),
        context.config.priorities.as_deref().unwrap_or_default(),
    );
    let total = drafts.len();
    let start = window.skip.min(total);
    let end = window
        .max_count
        .map_or(total, |count| start.saturating_add(count).min(total));
    let page = &drafts[start..end];
    let next_skip = (end < total).then_some(end);
    let cut = page.len() < total;

    if options.json {
        let draft_values = page
            .iter()
            .map(|task| {
                let mut value = task_compact_json(task, false);
                value["id"] = json!(normalize_task_id(&task.id, "draft"));
                value
            })
            .collect::<Vec<_>>();
        let mut result = json!({
            "schemaVersion": 1,
            "kind": "draft-list",
            "drafts": draft_values,
        });
        if cut {
            result["total"] = json!(total);
            result["nextSkip"] = json!(next_skip);
        }
        return write_json(&result);
    }
    if window.count {
        return write_stdout(&format!("{}\n", page.len()));
    }
    if page.is_empty() {
        if total == 0 {
            write_stdout("No drafts found.\n")?;
        }
    } else {
        let mut output = String::from("Drafts:\n");
        for draft in page {
            let priority = draft
                .priority
                .as_deref()
                .filter(|priority| !priority.is_empty())
                .map(|priority| format!("[{}] ", priority.to_uppercase()))
                .unwrap_or_default();
            output.push_str(&format!(
                "  {priority}{} - {}\n",
                normalize_task_id(&draft.id, "draft"),
                draft.title
            ));
        }
        write_stdout(&output)?;
    }
    if cut {
        write_stdout(&format_list_window_footer(
            window.skip,
            page.len(),
            total,
            next_skip,
            "\n",
        ))?;
    }
    Ok(())
}

fn run_document_list(options: DocListArgs) -> Result<(), String> {
    let _ = options.plain;
    let window = parse_list_window(&options.window, "backlog doc list")?;
    let context = project_context()?;
    let documents = DocumentStore::new(&context.root, &context.backlog_directory, &context.prefix)
        .list_documents();
    let total = documents.len();
    let start = window.skip.min(total);
    let end = window
        .max_count
        .map_or(total, |count| start.saturating_add(count).min(total));
    let page = &documents[start..end];
    let next_skip = (end < total).then_some(end);
    let cut = page.len() < total;

    if window.count {
        return write_stdout(&format!("{}\n", page.len()));
    }
    if page.is_empty() {
        if total == 0 {
            write_stdout("No docs found.\n")?;
        }
    } else {
        let mut output = String::new();
        for entry in page {
            output.push_str(&format!(
                "{} - {}\n",
                entry.document.id, entry.document.title
            ));
        }
        write_stdout(&output)?;
    }
    if cut {
        write_stdout(&format_list_window_footer(
            window.skip,
            page.len(),
            total,
            next_skip,
            "\n",
        ))?;
    }
    Ok(())
}

fn run_document_view(document_id: &str, options: DocumentViewArgs) -> Result<(), String> {
    const ARCHIVED_DOCUMENT_NOTICE: &str = "This document is archived and read-only.";
    let _ = options.plain;
    let context = project_context()?;
    let store = DocumentStore::new(&context.root, &context.backlog_directory, &context.prefix);
    let Some(entry) = store
        .load_document_for_view(document_id)
        .map_err(|error| error.to_string())?
    else {
        return Err(format!("Document {document_id} not found."));
    };
    let content = fs::read_to_string(&entry.path)
        .map_err(|_| format!("Document {document_id} not found."))?;
    if entry.archived {
        write_stdout(&format!("{ARCHIVED_DOCUMENT_NOTICE}\n\n{content}\n"))
    } else {
        write_stdout(&format!("{content}\n"))
    }
}

fn run_decision_create(options: DecisionCreateArgs) -> Result<(), String> {
    let context = project_context()?;
    let _ = options.plain;
    let store = DecisionStore::new(&context.root, &context.backlog_directory, &context.prefix);
    let entry = store
        .create_decision(
            crate::decision_store::DecisionCreateInput {
                title: options.title,
                status: options.status,
                context: options.context.unwrap_or_default(),
                decision: options.decision.unwrap_or_default(),
                consequences: options.consequences.unwrap_or_default(),
                alternatives: options.alternatives,
            },
            &context.config,
        )
        .map_err(|error| error.to_string())?;
    commit_created_record(
        &context,
        "Decision",
        &format!("backlog: Add decision {}", entry.decision.id),
        &entry.path,
    )?;
    write_stdout(&format!("Created decision {}\n", entry.decision.id))
}

fn run_decision_edit(options: DecisionEditArgs) -> Result<(), String> {
    let has_field = options.title.is_some()
        || options.status.is_some()
        || options.context.is_some()
        || options.decision.is_some()
        || options.consequences.is_some()
        || options.alternatives.is_some()
        || options.superseded_by.is_some();
    if !has_field {
        return Err("Provide at least one edit field. For example: backlog decision edit decision-1 --context \"New context\"".into());
    }

    let context = project_context()?;
    let _ = options.plain;
    let store = DecisionStore::new(&context.root, &context.backlog_directory, &context.prefix);
    let existing = store
        .get_decision(&options.decision_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("Decision {} not found", options.decision_id))?;
    let updated = store
        .update_decision(
            &options.decision_id,
            crate::decision_store::DecisionUpdateInput {
                title: options.title,
                status: options.status,
                context: options.context,
                decision: options.decision,
                consequences: options.consequences,
                alternatives: options.alternatives,
                superseded_by: options.superseded_by,
            },
        )
        .map_err(|error| match error {
            crate::decision_store::DecisionStoreError::NotFound(id) => {
                format!("Decision {id} not found")
            }
            other => other.to_string(),
        })?;
    let selected_paths = if existing.path == updated.path {
        vec![updated.path.clone()]
    } else {
        vec![existing.path, updated.path.clone()]
    };
    commit_record_change(
        &context,
        &format!("backlog: Update decision {}", updated.decision.id),
        &selected_paths,
    )?;
    write_stdout(&format!("Updated decision {}\n", updated.decision.id))
}

fn run_decision_list(options: DecisionListArgs) -> Result<(), String> {
    if options.plain && options.json {
        return Err("--json cannot be combined with --plain.".into());
    }
    if options.window.count && options.json {
        return Err(
            "--count cannot be combined with --json. Try 'backlog decision list --help' for options."
                .into(),
        );
    }
    let window = parse_list_window(&options.window, "backlog decision list")?;
    let context = project_context()?;
    let decisions = DecisionStore::new(&context.root, &context.backlog_directory, &context.prefix)
        .list_decisions();
    let total = decisions.len();
    let start = window.skip.min(total);
    let end = window
        .max_count
        .map_or(total, |count| start.saturating_add(count).min(total));
    let page = &decisions[start..end];
    let next_skip = (end < total).then_some(end);
    let cut = page.len() < total;

    if options.json {
        return write_stdout(&decision_list_json(page, cut, total, next_skip));
    }
    if window.count {
        return write_stdout(&format!("{}\n", page.len()));
    }
    if page.is_empty() {
        if total == 0 {
            write_stdout("No decisions found.\n")?;
        }
    } else {
        let mut output = String::new();
        for entry in page {
            let decision = &entry.decision;
            let status = if decision.status.is_empty() {
                String::new()
            } else {
                format!(" ({})", decision.status)
            };
            let replacement = decision
                .superseded_by
                .as_deref()
                .filter(|value| !value.is_empty())
                .map(|value| format!(" [superseded by {value}]"))
                .unwrap_or_default();
            output.push_str(&format!(
                "{} - {}{status}{replacement}\n",
                decision.id, decision.title
            ));
        }
        write_stdout(&output)?;
    }
    if cut {
        write_stdout(&format_list_window_footer(
            window.skip,
            page.len(),
            total,
            next_skip,
            "\n",
        ))?;
    }
    Ok(())
}

fn run_decision_view(decision_id: &str, options: DecisionViewArgs) -> Result<(), String> {
    if options.plain && options.json {
        return Err("--json cannot be combined with --plain.".into());
    }
    let context = project_context()?;
    let store = DecisionStore::new(&context.root, &context.backlog_directory, &context.prefix);
    let entry = store
        .get_decision(decision_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("Decision {decision_id} not found."))?;
    if options.json {
        let decision = &entry.decision;
        return write_stdout(&decision_view_json(decision, entry.archived));
    }
    let decision = &entry.decision;
    let mut lines = vec![format!("{} - {}", decision.id, decision.title)];
    if entry.archived {
        lines.push("Archived — read-only. Restore before editing.".into());
    }
    lines.push(format!("Date: {}", decision.date));
    lines.push(format!("Status: {}", decision.status));
    if let Some(replacement) = decision.superseded_by.as_deref() {
        lines.push(format!("Superseded by: {replacement}"));
    }
    if let Some(updated) = decision.updated_date.as_deref() {
        lines.push(format!("Updated: {updated}"));
    }
    lines.push(String::new());
    lines.push(strip_decision_section_markers(&decision.raw_content));
    write_stdout(&format!("{}\n", lines.join("\n")))
}

fn decision_list_json(
    page: &[crate::decision_store::DecisionFileEntry],
    cut: bool,
    total: usize,
    next_skip: Option<usize>,
) -> String {
    let mut output = String::from(
        "{\n  \"schemaVersion\": 1,\n  \"kind\": \"decision-list\",\n  \"decisions\": [",
    );
    if !page.is_empty() {
        output.push('\n');
    }
    for (index, entry) in page.iter().enumerate() {
        let decision = &entry.decision;
        let comma = if index + 1 == page.len() { "" } else { "," };
        output.push_str(&format!(
            "    {{\n      \"id\": {},\n      \"title\": {},\n      \"status\": {},\n      \"date\": {},\n      \"supersededBy\": {}\n    }}{comma}\n",
            json_string(&decision.id),
            json_string(&decision.title),
            json_string(&decision.status),
            json_optional_string(public_optional_date(&decision.date).as_deref()),
            json_optional_string(decision.superseded_by.as_deref()),
        ));
    }
    output.push_str("  ]");
    if cut {
        output.push_str(&format!(
            ",\n  \"total\": {total},\n  \"nextSkip\": {}",
            next_skip.map_or_else(|| "null".into(), |value| value.to_string())
        ));
    }
    output.push_str("\n}\n");
    output
}

fn decision_view_json(decision: &crate::records::DecisionRecord, archived: bool) -> String {
    format!(
        "{{\n  \"schemaVersion\": 1,\n  \"kind\": \"decision-view\",\n  \"decision\": {{\n    \"id\": {},\n    \"title\": {},\n    \"status\": {},\n    \"date\": {},\n    \"supersededBy\": {},\n    \"updatedAt\": {},\n    \"archived\": {archived},\n    \"content\": {}\n  }}\n}}\n",
        json_string(&decision.id),
        json_string(&decision.title),
        json_string(&decision.status),
        json_optional_string(public_optional_date(&decision.date).as_deref()),
        json_optional_string(decision.superseded_by.as_deref()),
        json_optional_string(
            decision
                .updated_date
                .as_deref()
                .and_then(public_optional_date)
                .as_deref()
        ),
        json_string(&decision.raw_content),
    )
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).expect("serializing a string cannot fail")
}

fn json_optional_string(value: Option<&str>) -> String {
    value.map_or_else(|| "null".into(), json_string)
}

fn public_optional_date(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| normalize_public_date(value))
}

fn strip_decision_section_markers(content: &str) -> String {
    let Ok(tree) = to_mdast(content, &ParseOptions::default()) else {
        return content.to_owned();
    };
    let lines = content
        .split_inclusive('\n')
        .scan(0, |offset, line| {
            let start = *offset;
            *offset += line.len();
            Some((
                start,
                *offset,
                line.trim_end_matches('\n').trim_end_matches('\r'),
            ))
        })
        .collect::<Vec<_>>();
    let decision_markers = ["CONTEXT", "DECISION", "CONSEQUENCES", "ALTERNATIVES"];
    let mut removals = Vec::new();
    for node in tree.children().into_iter().flatten() {
        let Node::Heading(heading) = node else {
            continue;
        };
        if heading.depth != 2 {
            continue;
        }
        let Some(position) = &heading.position else {
            continue;
        };
        let Some(source_heading) = content.get(position.start.offset..position.end.offset) else {
            continue;
        };
        let Some(marker_key) = decision_markers.iter().find(|key| {
            source_heading
                .trim_end()
                .eq_ignore_ascii_case(&format!("## {key}"))
        }) else {
            continue;
        };
        let begin_marker = format!("<!-- SECTION:{marker_key}:BEGIN -->");
        let end_marker = format!("<!-- SECTION:{marker_key}:END -->");
        let Some(heading_line) = lines.iter().position(|(start, end, _)| {
            *start <= position.start.offset && position.end.offset <= *end
        }) else {
            continue;
        };
        let mut begin_line = heading_line + 1;
        while begin_line < lines.len() && lines[begin_line].2.trim().is_empty() {
            begin_line += 1;
        }
        if begin_line == lines.len() || lines[begin_line].2.trim_end() != begin_marker {
            continue;
        }
        let mut depth = 1usize;
        let mut end_line = begin_line + 1;
        while end_line < lines.len() {
            let line = lines[end_line].2.trim_end();
            if line == begin_marker {
                depth += 1;
            } else if line == end_marker {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            end_line += 1;
        }
        if depth == 0 {
            removals.push((lines[begin_line].0, lines[begin_line].1));
            removals.push((lines[end_line].0, lines[end_line].1));
        }
    }
    let mut stripped = content.to_owned();
    removals.sort_unstable_by_key(|range| std::cmp::Reverse(range.0));
    for (start, end) in removals {
        stripped.replace_range(start..end, "");
    }
    stripped
}

fn run_document_create(options: DocCreateArgs) -> Result<(), String> {
    let context = project_context()?;
    let title = options.title.trim();
    if title.is_empty() {
        return Err("Title is required to create a document.".into());
    }
    let _ = options.plain;
    let document = crate::records::DocumentRecord {
        id: String::new(),
        title: title.to_owned(),
        document_type: options
            .document_type
            .filter(|document_type| !document_type.is_empty())
            .unwrap_or_else(|| "other".into()),
        created_date: crate::task_store::utc_timestamp(),
        updated_date: None,
        raw_content: String::new(),
        tags: None,
    };
    let store = DocumentStore::new(&context.root, &context.backlog_directory, &context.prefix);
    let entry = store
        .create_document(
            document,
            &context.config,
            options.path.as_deref().unwrap_or_default(),
        )
        .map_err(|error| error.to_string())?;
    commit_created_record(
        &context,
        "Document",
        &format!("backlog: Add document {}", entry.document.id),
        &entry.path,
    )?;
    let docs_directory = context.root.join(&context.backlog_directory).join("docs");
    let relative_path = entry
        .path
        .strip_prefix(docs_directory)
        .map_err(|error| error.to_string())?
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    let backlog_directory = context
        .backlog_directory
        .to_string_lossy()
        .replace('\\', "/");
    write_stdout(&format!(
        "Created document {}\nPath: {backlog_directory}/docs/{relative_path}\n",
        entry.document.id
    ))
}

fn run_document_update(options: DocUpdateArgs) -> Result<(), String> {
    let context = project_context()?;
    let store = DocumentStore::new(&context.root, &context.backlog_directory, &context.prefix);
    let entry = store
        .load_document_for_view(&options.document_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("Document not found: {}", options.document_id))?;
    if entry.archived {
        return Err(
            crate::document_store::DocumentStoreError::Archived(entry.document.id).to_string(),
        );
    }

    let mut document = entry.document;
    if let Some(title) = options.title {
        let title = title.trim();
        if title.is_empty() {
            return Err("Document title cannot be empty.".into());
        }
        document.title = title.to_owned();
    }
    if let Some(document_type) = options.document_type {
        if !["readme", "guide", "specification", "other"].contains(&document_type.as_str()) {
            return Err(
                "Document type must be one of: readme, guide, specification, other.".into(),
            );
        }
        document.document_type = document_type;
    }
    if let Some(tags) = options.tags {
        let mut tags = normalized_delimited_create_values(tags);
        let mut seen = HashSet::new();
        tags.retain(|tag| seen.insert(tag.clone()));
        document.tags = (!tags.is_empty()).then_some(tags);
    }
    if let Some(content) = options.content {
        document.raw_content = content;
    }
    document.updated_date = Some(crate::task_store::utc_timestamp());

    let old_path = entry.path;
    let updated = store
        .update_document_at_path(document, options.path.as_deref())
        .map_err(|error| error.to_string())?;
    let message = format!("backlog: Add document {}", updated.document.id);
    if context.config.auto_commit == Some(true) && context.config.filesystem_only != Some(true) {
        let changed_paths = if old_path == updated.path {
            vec![updated.path.clone()]
        } else {
            vec![old_path, updated.path.clone()]
        };
        if let Err(error) = commit_record_change(&context, &message, &changed_paths) {
            return Err(document_update_commit_error(
                error,
                &context.root,
                &updated.path,
            ));
        }
    }

    let docs_directory = context.root.join(&context.backlog_directory).join("docs");
    let relative_path = updated
        .path
        .strip_prefix(docs_directory)
        .map_err(|error| error.to_string())?
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    let backlog_directory = context
        .backlog_directory
        .to_string_lossy()
        .replace('\\', "/");
    write_stdout(&format!(
        "Updated document {}\nPath: {backlog_directory}/docs/{relative_path}\n",
        updated.document.id
    ))
}

fn document_update_commit_error(
    error: String,
    project_root: &std::path::Path,
    path: &std::path::Path,
) -> String {
    let Some((_, stage_error)) = error.split_once("Git stage selected files failed (exit code ")
    else {
        return error;
    };
    let Some((exit_code, command_and_stderr)) =
        stage_error.split_once("). Command: git --literal-pathspecs add -A -- ")
    else {
        return error;
    };
    let Some((_, stderr)) = command_and_stderr.split_once('\n') else {
        return error;
    };
    let Some(relative_path) = path.strip_prefix(project_root).ok() else {
        return error;
    };
    let relative_path = relative_path
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    format!("Git command failed (exit code {exit_code}): git add {relative_path}\n{stderr}\n")
}

fn content_record_lifecycle_stage_error(
    error: String,
    project_root: &std::path::Path,
    path: &std::path::Path,
) -> String {
    let Some((_, stage_error)) = error.split_once("Git stage selected files failed (exit code ")
    else {
        return error;
    };
    let Some((exit_code, command_and_stderr)) =
        stage_error.split_once("). Command: git --literal-pathspecs add -A -- ")
    else {
        return error;
    };
    let Some((_, stderr)) = command_and_stderr.split_once('\n') else {
        return error;
    };
    let Some(relative_path) = path.strip_prefix(project_root).ok() else {
        return error;
    };
    let relative_path = relative_path
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    format!("Git command failed (exit code {exit_code}): git add {relative_path}\n{stderr}\n")
}

fn run_document_lifecycle(
    document_id: &str,
    operation: DocumentLifecycleOperation,
) -> Result<(), String> {
    let context = project_context()?;
    let store = DocumentStore::new(&context.root, &context.backlog_directory, &context.prefix);
    let before = store
        .load_document_for_view(document_id)
        .map_err(|error| error.to_string())?;
    let change = match operation {
        DocumentLifecycleOperation::Archive => store.archive_document(document_id),
        DocumentLifecycleOperation::Restore => store.restore_document(document_id),
        DocumentLifecycleOperation::Delete => store.delete_document(document_id),
    }
    .map_err(|error| error.to_string())?
    .ok_or_else(|| format!("Document not found: {document_id}"))?;

    if context.config.auto_commit == Some(true) && context.config.filesystem_only != Some(true) {
        let before = before.ok_or_else(|| format!("Document not found: {document_id}"))?;
        let mut paths = vec![before.path.clone()];
        match operation {
            DocumentLifecycleOperation::Archive | DocumentLifecycleOperation::Restore => {
                let after = store
                    .load_document_for_view(&change.id)
                    .map_err(|error| error.to_string())?
                    .ok_or_else(|| format!("Document not found: {}", change.id))?;
                paths.push(after.path);
            }
            DocumentLifecycleOperation::Delete => {
                let archive_directory = context
                    .root
                    .join(&context.backlog_directory)
                    .join("archive/docs");
                let docs_directory = context.root.join(&context.backlog_directory).join("docs");
                let relative = before
                    .path
                    .strip_prefix(&archive_directory)
                    .map_err(|error| error.to_string())?;
                paths.push(docs_directory.join(relative));
            }
        }
        let (verb, commit_verb, stage_path) = match operation {
            DocumentLifecycleOperation::Archive => ("archived", "Archive", paths.last().unwrap()),
            DocumentLifecycleOperation::Restore => ("restored", "Restore", paths.last().unwrap()),
            DocumentLifecycleOperation::Delete => ("deleted", "Delete", paths.first().unwrap()),
        };
        commit_record_change(
            &context,
            &format!("backlog: {commit_verb} document {}", change.id),
            &paths,
        )
        .map_err(|error| {
            let error = content_record_lifecycle_stage_error(error, &context.root, stage_path);
            format!(
                "Document {} was {verb}, but committing the change failed: {error}",
                change.id
            )
        })?;
    }

    let confirmation = match operation {
        DocumentLifecycleOperation::Archive => "Archived",
        DocumentLifecycleOperation::Restore => "Restored",
        DocumentLifecycleOperation::Delete => "Deleted",
    };
    write_stdout(&format!("{confirmation} document {}\n", change.id))?;
    if !change.listed_as_documentation_by.is_empty() {
        write_stdout(&format!(
            "Still listed as documentation by: {}\n",
            change.listed_as_documentation_by.join(", ")
        ))?;
    }
    Ok(())
}

fn run_decision_lifecycle(
    decision_id: &str,
    operation: DecisionLifecycleOperation,
) -> Result<(), String> {
    let context = project_context()?;
    let store = DecisionStore::new(&context.root, &context.backlog_directory, &context.prefix);
    let before = store
        .get_decision(decision_id)
        .map_err(|error| error.to_string())?;
    let change = match operation {
        DecisionLifecycleOperation::Archive => store.archive_decision(decision_id),
        DecisionLifecycleOperation::Restore => store.restore_decision(decision_id),
        DecisionLifecycleOperation::Delete => store.delete_decision(decision_id),
    }
    .map_err(|error| error.to_string())?
    .ok_or_else(|| format!("Decision not found: {decision_id}"))?;

    if context.config.auto_commit == Some(true) && context.config.filesystem_only != Some(true) {
        let before = before.ok_or_else(|| format!("Decision not found: {decision_id}"))?;
        let mut paths = vec![before.path.clone()];
        match operation {
            DecisionLifecycleOperation::Archive | DecisionLifecycleOperation::Restore => {
                let after = store
                    .get_decision(&change.id)
                    .map_err(|error| error.to_string())?
                    .ok_or_else(|| format!("Decision not found: {}", change.id))?;
                paths.push(after.path);
            }
            DecisionLifecycleOperation::Delete => {
                let archive_directory = context
                    .root
                    .join(&context.backlog_directory)
                    .join("archive/decisions");
                let decisions_directory = context
                    .root
                    .join(&context.backlog_directory)
                    .join("decisions");
                let relative = before
                    .path
                    .strip_prefix(&archive_directory)
                    .map_err(|error| error.to_string())?;
                paths.push(decisions_directory.join(relative));
            }
        }
        let (verb, commit_verb, stage_path) = match operation {
            DecisionLifecycleOperation::Archive => ("archived", "Archive", paths.last().unwrap()),
            DecisionLifecycleOperation::Restore => ("restored", "Restore", paths.last().unwrap()),
            DecisionLifecycleOperation::Delete => ("deleted", "Delete", paths.first().unwrap()),
        };
        commit_record_change(
            &context,
            &format!("backlog: {commit_verb} decision {}", change.id),
            &paths,
        )
        .map_err(|error| {
            let error = content_record_lifecycle_stage_error(error, &context.root, stage_path);
            format!(
                "Decision {} was {verb}, but committing the change failed: {error}",
                change.id
            )
        })?;
    }

    let confirmation = match operation {
        DecisionLifecycleOperation::Archive => "Archived",
        DecisionLifecycleOperation::Restore => "Restored",
        DecisionLifecycleOperation::Delete => "Deleted",
    };
    write_stdout(&format!("{confirmation} decision {}\n", change.id))?;
    if !change.listed_as_documentation_by.is_empty() {
        write_stdout(&format!(
            "Still listed as documentation by: {}\n",
            change.listed_as_documentation_by.join(", ")
        ))?;
    }
    Ok(())
}

#[derive(Clone)]
struct MilestoneListRow {
    id: String,
    label: String,
    total: usize,
    done: usize,
    due_date: Option<String>,
    is_completed: bool,
}

fn run_milestone_list(options: MilestoneListArgs) -> Result<(), String> {
    let window = parse_list_window(&options.window, "backlog milestone list")?;
    let context = project_context()?;
    let milestones = MilestoneStore::new(&context.root, &context.backlog_directory);
    let active_milestones = milestones.list_active();
    let archived_milestones = milestones.list_archived();
    let tasks = context
        .store
        .list_tasks()
        .into_iter()
        .map(|entry| entry.task)
        .collect::<Vec<_>>();
    let rows = milestone_list_rows(&tasks, &active_milestones, &archived_milestones);
    let active = rows
        .iter()
        .filter(|row| !row.is_completed)
        .collect::<Vec<_>>();
    let completed = rows
        .iter()
        .filter(|row| row.is_completed)
        .collect::<Vec<_>>();
    let show_completed = options.show_completed;
    let active_count = active.len();
    let completed_count = completed.len();
    let listed = if show_completed {
        rows
    } else {
        rows.into_iter().filter(|row| !row.is_completed).collect()
    };
    let total = listed.len();
    let start = window.skip.min(total);
    let end = window
        .max_count
        .map_or(total, |count| start.saturating_add(count).min(total));
    let page = &listed[start..end];
    if window.count {
        return write_stdout(&format!("{}\n", page.len()));
    }

    let cut = page.len() < total;
    if !page.is_empty() || total == 0 {
        let next_skip = (end < total).then_some(end);
        let first_window = window.skip == 0 || total == 0;
        let lists_completed = show_completed && completed_count > 0;
        let print_active =
            page.iter().any(|row| !row.is_completed) || (active_count == 0 && first_window);
        let print_completed =
            page.iter().any(|row| row.is_completed) || (!lists_completed && next_skip.is_none());
        let mut sections = Vec::new();
        if print_active {
            let lines = if active_count == 0 {
                vec!["  (none)".to_owned()]
            } else {
                page.iter()
                    .filter(|row| !row.is_completed)
                    .map(format_milestone_list_row)
                    .collect()
            };
            sections.push(format!(
                "Active milestones ({}):\n{}",
                active_count,
                lines.join("\n")
            ));
        }
        if print_completed {
            let lines = if completed_count == 0 {
                vec!["  (none)".to_owned()]
            } else if show_completed {
                page.iter()
                    .filter(|row| row.is_completed)
                    .map(format_milestone_list_row)
                    .collect()
            } else {
                vec!["  (collapsed, use --show-completed to list)".to_owned()]
            };
            sections.push(format!(
                "Completed milestones ({}):\n{}",
                completed_count,
                lines.join("\n")
            ));
        }
        if !sections.is_empty() {
            write_stdout(&format!("{}\n", sections.join("\n\n")))?;
        }
    }
    if cut {
        write_stdout(&format_list_window_footer(
            window.skip,
            page.len(),
            total,
            (end < total).then_some(end),
            "\n",
        ))?;
    }
    let _ = options.plain;
    Ok(())
}

fn format_milestone_list_row(row: &MilestoneListRow) -> String {
    let due_date = row
        .due_date
        .as_deref()
        .map(|date| format!(", due {}", format_display_date(date, "")))
        .unwrap_or_default();
    format!(
        "  {}: {} ({}/{} done{due_date})",
        row.id, row.label, row.done, row.total
    )
}

fn milestone_list_rows(
    tasks: &[TaskRecord],
    active_milestones: &[crate::records::MilestoneRecord],
    archived_milestones: &[crate::records::MilestoneRecord],
) -> Vec<MilestoneListRow> {
    let mut values = active_milestones
        .iter()
        .map(|milestone| milestone.id.clone())
        .collect::<Vec<_>>();
    let mut assignments = Vec::with_capacity(tasks.len());
    for task in tasks {
        let Some(value) = task
            .milestone
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            assignments.push(None);
            continue;
        };
        let resolved =
            resolve_milestone_list_assignment(value, active_milestones, archived_milestones);
        if let Some(value) = resolved.as_ref()
            && !values
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(value))
        {
            values.push(value.clone());
        }
        assignments.push(resolved);
    }

    values
        .into_iter()
        .map(|id| {
            let milestone = active_milestones
                .iter()
                .find(|milestone| milestone.id.eq_ignore_ascii_case(&id));
            let assigned = assignments
                .iter()
                .zip(tasks)
                .filter_map(|(value, task)| {
                    value
                        .as_deref()
                        .filter(|value| value.eq_ignore_ascii_case(&id))
                        .map(|_| task)
                })
                .collect::<Vec<_>>();
            let total = assigned.len();
            let done = assigned
                .iter()
                .filter(|task| is_milestone_done_status(&task.status))
                .count();
            MilestoneListRow {
                id: id.clone(),
                label: milestone.map_or_else(|| id.clone(), |milestone| milestone.title.clone()),
                total,
                done,
                due_date: milestone.and_then(|milestone| milestone.due_date.clone()),
                is_completed: total > 0 && done == total,
            }
        })
        .collect()
}

fn is_milestone_done_status(status: &str) -> bool {
    let status = status.to_lowercase();
    status.contains("done") || status.contains("complete")
}

fn resolve_milestone_list_assignment(
    value: &str,
    active_milestones: &[crate::records::MilestoneRecord],
    archived_milestones: &[crate::records::MilestoneRecord],
) -> Option<String> {
    if let Some(milestone) = resolve_milestone_record(value, active_milestones) {
        return Some(milestone.id.clone());
    }
    if resolve_milestone_record(value, archived_milestones).is_some() {
        return None;
    }
    Some(value.trim().to_owned())
}

fn resolve_milestone_record<'a>(
    value: &str,
    milestones: &'a [crate::records::MilestoneRecord],
) -> Option<&'a crate::records::MilestoneRecord> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return None;
    }
    let raw_key = normalized.to_lowercase();
    if let Some(milestone) = milestones
        .iter()
        .find(|milestone| milestone.id.trim().eq_ignore_ascii_case(&raw_key))
    {
        return Some(milestone);
    }
    if let Some(number) = milestone_numeric_id(normalized)
        && let Some(milestone) = milestones
            .iter()
            .find(|milestone| milestone_numeric_id(&milestone.id) == Some(number))
    {
        return Some(milestone);
    }
    let mut title_matches = milestones
        .iter()
        .filter(|milestone| milestone.title.trim().eq_ignore_ascii_case(normalized));
    let first = title_matches.next()?;
    title_matches.next().is_none().then_some(first)
}

fn milestone_numeric_id(value: &str) -> Option<u64> {
    let normalized = value.trim();
    let numeric = normalized
        .strip_prefix("m-")
        .or_else(|| normalized.strip_prefix("M-"))
        .unwrap_or(normalized);
    (!numeric.is_empty() && numeric.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| numeric.parse().ok())
        .flatten()
}

fn run_milestone_add(options: MilestoneAddArgs) -> Result<(), String> {
    let title = options.name.trim();
    if title.is_empty() {
        return Err("Milestone name cannot be empty.".into());
    }
    let context = project_context()?;
    let store = MilestoneStore::new(&context.root, &context.backlog_directory);
    let active = store.list_active();
    let active = active.iter().collect::<Vec<_>>();
    let requested = milestone_create_keys(title, &active);
    if let Some(duplicate) = active.iter().find(|milestone| {
        requested
            .iter()
            .any(|key| milestone_create_record_keys(milestone).contains(key))
    }) {
        return Err(format!(
            "Milestone alias conflict: \"{title}\" matches existing milestone \"{}\" ({}).",
            duplicate.title, duplicate.id
        ));
    }
    let milestone = store
        .create(
            title,
            options.description.as_deref(),
            options.due_date.as_deref(),
        )
        .map_err(|error| error.to_string())?;
    let path = find_milestone_record_path(&context, &milestone.id)?;
    commit_created_record(
        &context,
        "Milestone",
        &format!("backlog: Add milestone {}", milestone.id),
        &path,
    )?;
    let due_date = milestone
        .due_date
        .as_deref()
        .map(|date| format!("\nDue: {}", format_display_date(date, "")))
        .unwrap_or_default();
    write_stdout(&format!(
        "Created milestone \"{}\" ({}).{due_date}\n",
        milestone.title, milestone.id
    ))
}

fn run_milestone_rename(options: MilestoneRenameArgs) -> Result<(), String> {
    if options.due_date.is_some() && options.clear_due_date {
        return Err("Cannot use --due-date and --clear-due-date together.".into());
    }
    let from = options.from.trim();
    let to = options.to.trim();
    if from.is_empty() || to.is_empty() {
        return Err("Both 'from' and 'to' milestone names are required.".into());
    }

    let context = project_context()?;
    let milestones = MilestoneStore::new(&context.root, &context.backlog_directory);
    let source = milestones
        .load_active(from)
        .ok_or_else(|| format!("Milestone not found: \"{from}\""))?;
    let requested_due_date = if options.clear_due_date {
        None
    } else if let Some(date) = options.due_date.as_deref() {
        crate::milestone_store::normalize_due_date(Some(date)).map_err(|error| error.to_string())?
    } else {
        source.due_date.clone()
    };
    let title_changed = to != source.title.trim();
    let due_date_changed = requested_due_date != source.due_date;
    if !title_changed && !due_date_changed {
        return write_stdout(&format!(
            "Milestone \"{}\" ({}) is already named \"{}\". No changes made.\n",
            source.title, source.id, source.title
        ));
    }

    let active = milestones.list_active();
    let active_refs = active.iter().collect::<Vec<_>>();
    let requested_keys = milestone_create_keys(to, &active_refs);
    let duplicate = active.iter().find(|candidate| {
        !milestone_ids_equal(&candidate.id, &source.id)
            && requested_keys
                .iter()
                .any(|key| milestone_create_record_keys(candidate).contains(key))
    });
    if let Some(duplicate) = duplicate {
        return Err(format!(
            "Milestone alias conflict: \"{to}\" matches existing milestone \"{}\" ({}).",
            duplicate.title, duplicate.id
        ));
    }

    let source_path = find_milestone_record_path(&context, &source.id)?;
    let original_milestone = fs::read(&source_path).map_err(|error| error.to_string())?;
    let task_store = TaskStore::new(&context.root, &context.backlog_directory, &context.prefix);
    let archived = milestones.list_archived();
    let all_milestones = active.iter().chain(&archived).collect::<Vec<_>>();
    let title_collision = all_milestones.iter().any(|candidate| {
        !milestone_ids_equal(&candidate.id, &source.id)
            && milestone_create_record_keys(candidate).contains(&source.title.trim().to_lowercase())
    });
    let match_keys = milestone_task_match_keys(from, &source, !title_collision);
    let matching_tasks = if title_changed && options.update_tasks {
        task_store
            .list_tasks()
            .into_iter()
            .filter(|entry| {
                entry
                    .task
                    .milestone
                    .as_deref()
                    .is_some_and(|value| match_keys.contains(&value.trim().to_lowercase()))
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let due_date = if options.clear_due_date {
        Some(None)
    } else {
        options.due_date.as_deref().map(Some)
    };
    let change = milestones
        .rename(&source.id, to, due_date)
        .map_err(|error| error.to_string())?;
    if !change.success {
        return Err(format!("Failed to rename milestone \"{}\".", source.title));
    }
    let renamed = change
        .milestone
        .ok_or_else(|| format!("Failed to rename milestone \"{}\".", source.title))?;
    let renamed_path = change
        .target_path
        .clone()
        .ok_or_else(|| format!("Failed to rename milestone \"{}\".", source.title))?;

    let mut changed_tasks = Vec::new();
    let mut updated_task_ids = Vec::new();
    let mut updated_task_paths = Vec::new();
    if title_changed && options.update_tasks {
        for entry in matching_tasks {
            let mut updated = entry.task.clone();
            updated.milestone = Some(source.id.clone());
            updated.updated_date = Some(crate::task_store::utc_timestamp());
            match task_store.update_task(updated) {
                Ok(Some(updated)) => {
                    updated_task_ids.push(updated.task.id.to_uppercase());
                    updated_task_paths.push(updated.path);
                    changed_tasks.push(entry);
                }
                Ok(None) | Err(_) => {
                    let rollback_failures = rollback_milestone_tasks(&task_store, &changed_tasks);
                    let mut details = Vec::new();
                    if !rollback_milestone_file(&source_path, &renamed_path, &original_milestone) {
                        details.push("failed to rollback milestone file rename".to_owned());
                    }
                    if !rollback_failures.is_empty() {
                        details.push(format!(
                            "failed to rollback task milestones for: {}",
                            rollback_failures.join(", ")
                        ));
                    }
                    let suffix = if details.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", details.join("; "))
                    };
                    return Err(format!(
                        "Failed to update task milestones after renaming \"{}\"{suffix}.",
                        source.title
                    ));
                }
            }
        }
    }
    updated_task_ids.sort_by_key(|id| id.to_lowercase());
    updated_task_paths.sort();
    updated_task_paths.dedup();

    if context.config.auto_commit == Some(true) && context.config.filesystem_only != Some(true) {
        let mut selected_paths = vec![source_path.clone(), renamed_path.clone()];
        selected_paths.extend(updated_task_paths.iter().cloned());
        selected_paths.sort();
        selected_paths.dedup();
        let action = if title_changed { "Rename" } else { "Update" };
        if let Err(error) = commit_record_change(
            &context,
            &format!("backlog: {action} milestone {}", source.id),
            &selected_paths,
        ) {
            let rollback_failures = rollback_milestone_tasks(&task_store, &changed_tasks);
            let mut details = Vec::new();
            if !rollback_milestone_file(&source_path, &renamed_path, &original_milestone) {
                details.push("failed to rollback milestone file rename".to_owned());
            }
            if !rollback_failures.is_empty() {
                details.push(format!(
                    "failed to rollback task milestones for: {}",
                    rollback_failures.join(", ")
                ));
            }
            let suffix = if details.is_empty() {
                String::new()
            } else {
                format!(" ({})", details.join("; "))
            };
            return Err(format!(
                "Failed while finalizing milestone rename \"{}\"{suffix}: {error}",
                source.title
            ));
        }
    }

    let mut lines = Vec::new();
    if title_changed {
        lines.push(format!(
            "Renamed milestone \"{}\" ({}) → \"{}\" ({}).",
            source.title, source.id, renamed.title, renamed.id
        ));
    }
    if due_date_changed {
        lines.push(renamed.due_date.as_deref().map_or_else(
            || "Cleared milestone due date.".to_owned(),
            |date| format!("Due: {}", format_display_date(date, "")),
        ));
    }
    if title_changed && options.update_tasks {
        lines.push(format!(
            "Updated {} local task{}: {}",
            updated_task_ids.len(),
            if updated_task_ids.len() == 1 { "" } else { "s" },
            format_milestone_task_ids(&updated_task_ids)
        ));
    } else if title_changed {
        lines.push("Skipped updating tasks (updateTasks=false).".into());
    }
    if source_path != renamed_path {
        lines.push(format!(
            "Renamed milestone file: {} -> {}",
            source_path.display(),
            renamed_path.display()
        ));
    }
    write_stdout(&format!("{}\n", lines.join("\n")))
}

fn run_milestone_archive(name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Milestone name cannot be empty.".into());
    }
    let context = project_context()?;
    let store = MilestoneStore::new(&context.root, &context.backlog_directory);
    let change = store.archive(name).map_err(|error| error.to_string())?;
    if !change.success {
        return Err(format!("Milestone not found: \"{name}\""));
    }
    if context.config.auto_commit == Some(true) && context.config.filesystem_only != Some(true) {
        let source_path = change
            .source_path
            .as_ref()
            .ok_or_else(|| format!("Milestone not found: \"{name}\""))?;
        let target_path = change
            .target_path
            .as_ref()
            .ok_or_else(|| format!("Milestone not found: \"{name}\""))?;
        if let Err(error) = commit_record_change(
            &context,
            &format!(
                "backlog: Archive milestone {}",
                change.milestone.as_ref().unwrap().id
            ),
            &[source_path.clone(), target_path.clone()],
        ) {
            let _ = fs::rename(target_path, source_path);
            return Err(error);
        }
    }
    let milestone = change.milestone.unwrap();
    write_stdout(&format!(
        "Archived milestone \"{}\" ({}).\n",
        milestone.title, milestone.id
    ))
}

fn run_milestone_remove(options: MilestoneRemoveArgs) -> Result<(), String> {
    let handling = match options.task_handling.trim().to_lowercase().as_str() {
        "clear" => crate::milestone_service::TaskMilestoneHandling::Clear,
        "keep" => crate::milestone_service::TaskMilestoneHandling::Keep,
        "reassign" => crate::milestone_service::TaskMilestoneHandling::Reassign,
        _ => {
            return Err(format!(
                "Invalid task handling: {}. Valid values are: clear, keep, reassign",
                options.task_handling
            ));
        }
    };
    let context = project_context()?;
    let milestones = MilestoneStore::new(&context.root, &context.backlog_directory);
    let tasks = TaskStore::new(&context.root, &context.backlog_directory, &context.prefix);
    let source = milestones
        .load_active(&options.name)
        .ok_or_else(|| format!("Milestone not found: \"{}\"", options.name.trim()))?;
    let source_path = find_milestone_record_path(&context, &source.id)?;
    let active_directory = context
        .root
        .join(&context.backlog_directory)
        .join("milestones");
    let relative_path = source_path
        .strip_prefix(&active_directory)
        .map_err(|error| error.to_string())?;
    let archived_path = context
        .root
        .join(&context.backlog_directory)
        .join("archive/milestones")
        .join(relative_path);
    let before_tasks = tasks.list_tasks();
    let result = crate::milestone_service::remove_milestone(
        &milestones,
        &tasks,
        &options.name,
        handling,
        options.reassign_to.as_deref(),
    )?;

    if context.config.auto_commit == Some(true) && context.config.filesystem_only != Some(true) {
        let affected_ids: HashSet<_> = result
            .affected_task_ids
            .iter()
            .map(|id| id.to_lowercase())
            .collect();
        let mut selected_paths = vec![source_path.clone(), archived_path.clone()];
        selected_paths.extend(
            before_tasks
                .iter()
                .filter(|entry| affected_ids.contains(&entry.task.id.to_lowercase()))
                .map(|entry| entry.path.clone()),
        );
        selected_paths.sort();
        selected_paths.dedup();
        if let Err(error) = commit_record_change(
            &context,
            &format!("backlog: Remove milestone {}", result.milestone_id),
            &selected_paths,
        ) {
            let mut rollback_details = Vec::new();
            if fs::rename(&archived_path, &source_path).is_err() {
                rollback_details.push("failed to rollback milestone archive".to_owned());
            }
            let affected: HashSet<_> = result
                .affected_task_ids
                .iter()
                .map(|id| id.to_lowercase())
                .collect();
            let failed = rollback_milestone_tasks(
                &tasks,
                &before_tasks
                    .iter()
                    .filter(|entry| affected.contains(&entry.task.id.to_lowercase()))
                    .cloned()
                    .collect::<Vec<_>>(),
            );
            if !failed.is_empty() {
                rollback_details.push(format!("failed rollback for: {}", failed.join(", ")));
            }
            let detail_suffix = if rollback_details.is_empty() {
                String::new()
            } else {
                format!(" ({})", rollback_details.join("; "))
            };
            return Err(format!(
                "Failed while finalizing milestone removal \"{}\"{detail_suffix}: {error}",
                result.milestone_title
            ));
        }
    }
    write_stdout(&format!("{}\n", result.summary()))
}

fn run_search(options: SearchArgs) -> Result<(), String> {
    if options.json && options.plain {
        return Err("--json cannot be combined with --plain.".into());
    }
    if options.window.count && options.json {
        return Err(
            "--count cannot be combined with --json. Try 'backlog search --help' for options."
                .into(),
        );
    }
    let window = parse_list_window(&options.window, "backlog search")?;
    let limit = options
        .limit
        .as_deref()
        .map(|value| {
            let value = value.trim();
            if value.is_empty()
                || !value.bytes().all(|byte| byte.is_ascii_digit())
                || value.as_bytes()[0] == b'0'
            {
                return Err(String::from(
                    "--limit must be a positive integer (1 or greater). Try 'backlog search --help' for options.",
                ));
            }
            Ok(value.parse::<usize>().unwrap_or(usize::MAX))
        })
        .transpose()?;

    let context = project_context()?;
    let task_types = clean_search_values(options.task_type.unwrap_or_default());
    let projects = clean_search_values(options.project.unwrap_or_default());
    let modified_files = clean_search_values(options.modified_file.unwrap_or_default());
    let raw_types = options.result_types;
    let mut allowed_types = if let Some(raw_types) = raw_types.as_ref() {
        let mut types = Vec::new();
        for value in raw_types.iter().map(|value| value.trim().to_lowercase()) {
            if ["task", "document", "decision"].contains(&value.as_str()) {
                if !types.contains(&value) {
                    types.push(value);
                }
            } else if !value.is_empty() {
                write_stderr(&format!(
                    "Ignoring unsupported type '{value}'. Supported: task, document, decision\n"
                ))?;
            }
        }
        types
    } else if !task_types.is_empty() || !projects.is_empty() || !modified_files.is_empty() {
        vec!["task".to_owned()]
    } else {
        vec![
            "task".to_owned(),
            "document".to_owned(),
            "decision".to_owned(),
        ]
    };
    if allowed_types.is_empty() {
        allowed_types.extend([
            "task".to_owned(),
            "document".to_owned(),
            "decision".to_owned(),
        ]);
    }
    if !task_types.is_empty()
        && raw_types.is_some()
        && !allowed_types.iter().any(|kind| kind == "task")
    {
        return Err("--task-type filters task results. Include --type task or omit --type.".into());
    }
    if !projects.is_empty()
        && raw_types.is_some()
        && !allowed_types.iter().any(|kind| kind == "task")
    {
        return Err("--project filters task results. Include --type task or omit --type.".into());
    }

    let default_types = [
        "bug".into(),
        "feature".into(),
        "enhancement".into(),
        "task".into(),
        "chore".into(),
        "docs".into(),
        "spike".into(),
    ];
    let task_types = canonical_search_config_values(
        &task_types,
        context.config.types.as_deref().unwrap_or(&default_types),
        "task-type",
        "types",
    )?;
    let projects = if projects.is_empty() {
        Vec::new()
    } else {
        let configured_projects = context.config.projects.as_deref().unwrap_or_default();
        if configured_projects.is_empty() {
            let config_path = context
                .config_path
                .as_deref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| {
                    context
                        .root
                        .join("backlog.config.yml")
                        .display()
                        .to_string()
                });
            return Err(format!(
                "No projects are configured. Add a 'projects:' list to {config_path}."
            ));
        }
        canonical_search_config_values(&projects, configured_projects, "project", "projects")?
    };
    let priorities = context
        .config
        .priorities
        .clone()
        .unwrap_or_else(|| vec!["High".into(), "Medium".into(), "Low".into()]);
    let priority = options
        .priority
        .as_deref()
        .map(|value| {
            resolve_config_value(value, &priorities)
                .map(|priority| priority.to_lowercase())
                .ok_or_else(|| {
                    format!(
                        "Invalid priority: {value}. Valid values are: {}",
                        priorities.join(", ")
                    )
                })
        })
        .transpose()?;
    let excluded_statuses = canonical_statuses(
        &clean_search_values(options.exclude_status.unwrap_or_default()),
        &context.config.statuses,
        "exclude-status",
    )?;
    let filter = TaskFilter {
        statuses: clean_search_values(options.status.unwrap_or_default()),
        exclude_statuses: excluded_statuses,
        types: task_types,
        projects,
        priorities: priority.into_iter().collect(),
        modified_files,
        ..TaskFilter::default()
    };

    let task_entries = context.store.list_tasks();
    let tasks = task_entries
        .iter()
        .map(|entry| &entry.task)
        .collect::<Vec<_>>();
    let task_corpus = tasks.iter().map(|task| (*task).clone()).collect::<Vec<_>>();
    let archived_tasks = context
        .store
        .list_archived_tasks()
        .into_iter()
        .map(|entry| entry.task)
        .collect::<Vec<_>>();
    let documents = DocumentStore::new(&context.root, &context.backlog_directory, &context.prefix)
        .list_documents();
    let decisions = DecisionStore::new(&context.root, &context.backlog_directory, &context.prefix)
        .list_decisions();
    let milestone_store = MilestoneStore::new(&context.root, &context.backlog_directory);
    let all_milestones = milestone_store
        .list_active()
        .into_iter()
        .chain(milestone_store.list_archived())
        .collect::<Vec<_>>();
    let matcher = TaskFilterMatcher::new(&filter, &context.prefix, &all_milestones, &task_corpus);

    let mut corpus = Vec::new();
    for task in &tasks {
        corpus.push(SearchIndexedResult {
            result: SearchResultRef::Task(task),
            fields: build_task_search_fields(task),
        });
    }
    for document in &documents {
        corpus.push(SearchIndexedResult {
            result: SearchResultRef::Document(document),
            fields: content_search_fields(
                &document.document.id,
                &document.document.title,
                &document.document.raw_content,
            ),
        });
    }
    for decision in &decisions {
        corpus.push(SearchIndexedResult {
            result: SearchResultRef::Decision(decision),
            fields: content_search_fields(
                &decision.decision.id,
                &decision.decision.title,
                &decision.decision.raw_content,
            ),
        });
    }

    let query = options.query.as_deref().unwrap_or_default().trim();
    let mut ranked = corpus
        .iter()
        .filter_map(|entry| {
            let kind = entry.result.kind();
            if !allowed_types.iter().any(|allowed| allowed == kind) {
                return None;
            }
            if let SearchResultRef::Task(task) = entry.result
                && !matcher.matches(task)
            {
                return None;
            }
            let score = if query.is_empty() {
                None
            } else {
                search_fields_score(query, &entry.fields)
            };
            (query.is_empty() || score.is_some()).then_some(RankedSearchResult {
                result: entry.result,
                score,
            })
        })
        .collect::<Vec<_>>();
    if !query.is_empty() {
        ranked.sort_by(|left, right| {
            left.score
                .unwrap_or_default()
                .total_cmp(&right.score.unwrap_or_default())
        });
    }
    if let Some(limit) = limit {
        ranked.truncate(limit);
    }

    let plain = ["task", "document", "decision"]
        .into_iter()
        .flat_map(|kind| {
            ranked
                .iter()
                .filter(move |result| result.result.kind() == kind)
        })
        .collect::<Vec<_>>();
    let printed = if options.json {
        ranked.iter().collect::<Vec<_>>()
    } else {
        plain
    };
    let total = printed.len();
    let start = window.skip.min(total);
    let end = window
        .max_count
        .map_or(total, |count| start.saturating_add(count).min(total));
    let page = &printed[start..end];
    let next_skip = (end < total).then_some(end);
    let cut = page.len() < total;

    if window.count {
        return write_stdout(&format!("{}\n", page.len()));
    }
    if options.json {
        return write_pretty_json(&search_results_json(
            page,
            &context,
            &task_corpus,
            &archived_tasks,
            cut.then_some((total, next_skip)),
        ));
    }
    write_search_results(page, &context.prefix)?;
    if cut {
        write_stdout(&format_list_window_footer(
            window.skip,
            page.len(),
            total,
            next_skip,
            "\n",
        ))?;
    }
    Ok(())
}

fn clean_search_values(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .collect()
}

fn canonical_search_config_values(
    values: &[String],
    configured: &[String],
    option: &str,
    valid_name: &str,
) -> Result<Vec<String>, String> {
    let mut canonical = Vec::new();
    let mut invalid = Vec::new();
    for value in values {
        if let Some(resolved) = resolve_config_value(value, configured) {
            if !canonical
                .iter()
                .any(|existing: &String| existing.eq_ignore_ascii_case(&resolved))
            {
                canonical.push(resolved);
            }
        } else {
            invalid.push(value.clone());
        }
    }
    if invalid.is_empty() {
        return Ok(canonical);
    }
    Err(format!(
        "Invalid {option}: {}. Valid {valid_name} are: {}",
        invalid.join(", "),
        configured.join(", ")
    ))
}

fn content_search_fields(id: &str, title: &str, body_text: &str) -> TaskSearchFields {
    TaskSearchFields {
        title: title.to_owned(),
        body_text: body_text.to_owned(),
        id: id.to_owned(),
        id_variants: Vec::new(),
        dependency_ids: Vec::new(),
        modified_files: Vec::new(),
    }
}

fn search_score_text(score: Option<f64>) -> String {
    score
        .map(|score| format!(" [score {:.3}]", 1.0 - score))
        .unwrap_or_default()
}

fn write_search_results(results: &[&RankedSearchResult<'_>], prefix: &str) -> Result<(), String> {
    let mut sections = Vec::new();
    for kind in ["task", "document", "decision"] {
        let rows = results
            .iter()
            .filter(|result| result.result.kind() == kind)
            .map(|result| match result.result {
                SearchResultRef::Task(task) => {
                    let status = if task.status.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", task.status)
                    };
                    let priority = task
                        .priority
                        .as_deref()
                        .filter(|priority| !priority.is_empty())
                        .map(|priority| format!(" [{}]", priority.to_uppercase()))
                        .unwrap_or_default();
                    format!(
                        "  {} - {}{}{}{}",
                        normalize_task_id(&task.id, prefix),
                        task.title,
                        status,
                        priority,
                        search_score_text(result.score)
                    )
                }
                SearchResultRef::Document(document) => format!(
                    "  {} - {}{}",
                    document.document.id,
                    document.document.title,
                    search_score_text(result.score)
                ),
                SearchResultRef::Decision(decision) => format!(
                    "  {} - {}{}",
                    decision.decision.id,
                    decision.decision.title,
                    search_score_text(result.score)
                ),
            })
            .collect::<Vec<_>>();
        if !rows.is_empty() {
            let heading = match kind {
                "task" => "Tasks:",
                "document" => "Documents:",
                _ => "Decisions:",
            };
            sections.push(
                std::iter::once(heading.to_owned())
                    .chain(rows)
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
    }
    if sections.is_empty() {
        write_stdout("No results found.\n")
    } else {
        write_stdout(&format!("{}\n", sections.join("\n\n")))
    }
}

fn search_results_json(
    results: &[&RankedSearchResult<'_>],
    context: &ProjectContext,
    task_corpus: &[TaskRecord],
    archived_tasks: &[TaskRecord],
    cut: Option<(usize, Option<usize>)>,
) -> Value {
    let result_tasks = results
        .iter()
        .filter_map(|result| match result.result {
            SearchResultRef::Task(task) => Some(task.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let readiness = with_readiness(
        &result_tasks,
        task_corpus,
        archived_tasks,
        &context.config.statuses,
        &[],
        &context.prefix,
    );
    let mut next_task = readiness.iter();
    let mut json_results = Vec::new();
    for result in results {
        let entry = match result.result {
            SearchResultRef::Task(task) => {
                let is_ready = next_task.next().is_some_and(|item| item.is_ready);
                json!({"type": "task", "data": task_summary_json(task, false, is_ready)})
            }
            SearchResultRef::Document(document) => {
                let relative = document
                    .path
                    .strip_prefix(&context.root)
                    .map(|path| path.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_else(|_| document.path.to_string_lossy().replace('\\', "/"));
                json!({
                    "type": "document",
                    "data": {
                        "id": document.document.id,
                        "title": document.document.title,
                        "type": document.document.document_type,
                        "path": relative,
                        "tags": document.document.tags.clone().unwrap_or_default(),
                        "createdAt": normalize_public_date(&document.document.created_date),
                        "updatedAt": document.document.updated_date.as_deref().map(normalize_public_date),
                    }
                })
            }
            SearchResultRef::Decision(decision) => json!({
                "type": "decision",
                "data": {
                    "id": decision.decision.id,
                    "title": decision.decision.title,
                    "status": decision.decision.status,
                    "date": normalize_public_date(&decision.decision.date),
                    "supersededBy": decision.decision.superseded_by,
                }
            }),
        };
        json_results.push(entry);
    }
    let mut output = json!({
        "schemaVersion": 1,
        "kind": "search",
        "results": json_results,
    });
    if let Some((total, next_skip)) = cut {
        output["total"] = json!(total);
        output["nextSkip"] = json!(next_skip);
    }
    output
}

fn milestone_task_match_keys(
    name: &str,
    milestone: &crate::records::MilestoneRecord,
    include_title: bool,
) -> HashSet<String> {
    if include_title {
        let records = [milestone];
        let mut keys = milestone_create_keys(name, &records);
        keys.extend(milestone_create_id_keys(&milestone.id));
        keys
    } else {
        milestone_create_id_keys(&milestone.id)
    }
}

fn milestone_ids_equal(left: &str, right: &str) -> bool {
    !milestone_create_id_keys(left).is_disjoint(&milestone_create_id_keys(right))
}

fn format_milestone_task_ids(ids: &[String]) -> String {
    let shown = ids.iter().take(20).cloned().collect::<Vec<_>>().join(", ");
    if ids.len() > 20 {
        format!("{shown} (and {} more)", ids.len() - 20)
    } else {
        shown
    }
}

fn rollback_milestone_tasks(
    tasks: &TaskStore,
    changed: &[crate::task_store::TaskFileEntry],
) -> Vec<String> {
    let mut failed = Vec::new();
    for entry in changed.iter().rev() {
        if !matches!(tasks.update_task(entry.task.clone()), Ok(Some(_))) {
            failed.push(entry.task.id.clone());
        }
    }
    failed.sort();
    failed
}

fn rollback_milestone_file(
    source: &std::path::Path,
    target: &std::path::Path,
    original: &[u8],
) -> bool {
    if source == target {
        return fs::write(source, original).is_ok();
    }
    if fs::rename(target, source).is_err() {
        return false;
    }
    fs::write(source, original).is_ok()
}

fn milestone_create_keys(
    name: &str,
    milestones: &[&crate::records::MilestoneRecord],
) -> HashSet<String> {
    let normalized = name.trim().to_lowercase();
    let mut keys = milestone_create_id_keys(&normalized);
    keys.insert(normalized.clone());
    for milestone in milestones
        .iter()
        .filter(|milestone| milestone.title.trim().eq_ignore_ascii_case(&normalized))
    {
        keys.extend(milestone_create_id_keys(&milestone.id));
    }
    keys
}

fn milestone_create_record_keys(milestone: &crate::records::MilestoneRecord) -> HashSet<String> {
    let mut keys = milestone_create_id_keys(&milestone.id);
    keys.insert(milestone.title.trim().to_lowercase());
    keys
}

fn milestone_create_id_keys(value: &str) -> HashSet<String> {
    let value = value.trim().to_lowercase();
    let mut keys = HashSet::from([value.clone()]);
    if let Some(number) = milestone_numeric_id(&value) {
        keys.insert(number.to_string());
        keys.insert(format!("m-{number}"));
    }
    keys
}

fn find_milestone_record_path(context: &ProjectContext, id: &str) -> Result<PathBuf, String> {
    let directory = context
        .root
        .join(&context.backlog_directory)
        .join("milestones");
    for entry in fs::read_dir(&directory).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path.extension().is_some_and(|extension| extension == "md")
            && let Ok(source) = fs::read_to_string(&path)
            && let Ok(milestone) = crate::records::parse_milestone(&source)
            && milestone.id == id
        {
            return Ok(path);
        }
    }
    Err(format!(
        "Created milestone {id} but could not find its file."
    ))
}

fn run_draft_view(task_id: &str, options: DraftViewArgs) -> Result<(), String> {
    let _ = options.plain;
    let context = project_context()?;
    let store = DraftStore::new(&context.root, &context.backlog_directory);
    let entry = store
        .load_draft(task_id)
        .map_err(|error| error.to_string())?;
    let Some(entry) = entry else {
        return write_stderr(&format!("Draft {task_id} not found.\n"));
    };
    let mut draft = entry.task.clone();
    draft.id = canonical_task_id(&draft.id, "draft");
    let active_entries = context.store.list_tasks();
    let active = active_entries
        .iter()
        .map(|entry| entry.task.clone())
        .collect::<Vec<_>>();
    let archived = context
        .store
        .list_archived_tasks()
        .into_iter()
        .map(|entry| entry.task)
        .collect::<Vec<_>>();
    let detail = build_task_detail(
        &draft,
        &active,
        &archived,
        &context.config.statuses,
        &[],
        &context.prefix,
    );
    write_stdout(&format!(
        "{}\n",
        format_task_detail(
            &detail,
            &entry.path,
            false,
            &active,
            &context.config,
            &context.prefix,
            true,
        )
    ))
}

fn run_draft_archive(task_id: &str) -> Result<(), String> {
    let context = project_context()?;
    let store = DraftStore::new(&context.root, &context.backlog_directory);
    let source = store
        .resolve_path(task_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("Draft {task_id} not found."))?;
    let archived = context
        .backlog_directory
        .join("archive/drafts")
        .join(source.file_name().unwrap_or_default());
    if !store
        .archive_draft(task_id)
        .map_err(|error| error.to_string())?
    {
        return Err(format!("Draft {task_id} not found."));
    }
    let id = canonical_task_id(task_id, "draft");
    commit_record_change(
        &context,
        &format!("backlog: Archive draft {id}"),
        &[source, archived],
    )
    .map_err(|error| {
        format!("Draft {id} was archived, but the change was not committed: {error}")
    })?;
    write_stdout(&format!("Archived draft {id}\n"))
}

fn run_draft_promote(task_id: &str) -> Result<(), String> {
    let context = project_context()?;
    let id = canonical_task_id(task_id, "draft");
    let Some(changed_paths) = context
        .store
        .promote_draft_with_changes(&id, &context.config)
        .map_err(|error| lifecycle_error(error, &context.prefix))?
    else {
        return Err(format!("Draft {task_id} not found."));
    };
    commit_record_change(
        &context,
        &format!("backlog: Promote draft {id}"),
        &changed_paths,
    )
    .map_err(|error| {
        format!("Draft {id} was promoted, but the change was not committed: {error}")
    })?;
    write_stdout(&format!("Promoted draft {id}\n"))
}

fn run_draft_create(options: DraftCreateArgs) -> Result<(), String> {
    let title = options.title.trim();
    if title.is_empty() {
        return Err("Title is required to create a task.".into());
    }
    let _ = options.status;
    let context = project_context()?;
    let assignee = options
        .assignee
        .map(normalized_delimited_create_values)
        .unwrap_or_else(|| context.config.default_assignee.clone().unwrap_or_default());
    let definition_of_done_items = normalized_create_values(
        context
            .config
            .definition_of_done
            .clone()
            .unwrap_or_default(),
    )
    .into_iter()
    .enumerate()
    .map(|(index, text)| Criterion {
        index: index + 1,
        text,
        checked: false,
    })
    .collect();
    let task = TaskRecord {
        id: String::new(),
        title: title.to_owned(),
        status: "Draft".into(),
        assignee,
        reporter: None,
        created_date: crate::task_store::utc_timestamp(),
        updated_date: None,
        due_date: None,
        labels: options
            .labels
            .map(normalized_delimited_create_values)
            .unwrap_or_default(),
        milestone: None,
        dependencies: Vec::new(),
        references: Vec::new(),
        documentation: Vec::new(),
        modified_files: Vec::new(),
        raw_content: String::new(),
        acceptance_criteria_items: Vec::new(),
        definition_of_done_items,
        description: options
            .description
            .filter(|description| !description.is_empty())
            .or(options.desc)
            .unwrap_or_default(),
        implementation_plan: None,
        implementation_notes: None,
        comments: Vec::new(),
        final_summary: None,
        parent_task_id: None,
        subtasks: None,
        priority: None,
        task_type: None,
        project: None,
        ordinal: None,
        on_status_change: None,
    };
    let entry = DraftStore::new(&context.root, &context.backlog_directory)
        .create_draft(task, &context.config)
        .map_err(|error| error.to_string())?;
    commit_created_record(
        &context,
        "Draft",
        &format!("{} - Create draft {}", entry.task.id, entry.task.id),
        &entry.path,
    )?;
    write_stdout(&format!(
        "Created draft {}\nFile: {}\n",
        entry.task.id,
        entry.path.display()
    ))
}

fn commit_record_change(
    context: &ProjectContext,
    message: &str,
    paths: &[PathBuf],
) -> Result<(), String> {
    crate::git::GitRepository::new(&context.root).commit_selected_paths(
        message,
        paths,
        &context.config,
    )
}

fn commit_created_record(
    context: &ProjectContext,
    record_kind: &str,
    message: &str,
    path: &PathBuf,
) -> Result<(), String> {
    if context.config.auto_commit != Some(true) || context.config.filesystem_only == Some(true) {
        return Ok(());
    }
    let written = fs::read(path).map_err(|error| error.to_string())?;
    match commit_record_change(context, message, std::slice::from_ref(path)) {
        Ok(()) => Ok(()),
        Err(error) => match fs::read(path) {
            Ok(current) if current == written => {
                fs::remove_file(path).map_err(|cleanup| {
                    format!("{record_kind} creation failed and cleanup also failed: {cleanup}; Git reported: {error}")
                })?;
                let error = error.replace(
                    "The files remain in the working tree; their previous index entries were restored.",
                    "The previous index entries were restored.",
                );
                Err(format!(
                    "{record_kind} creation failed; the uncommitted record at {} was removed after the Git commit failed. {error}",
                    path.display()
                ))
            }
            Ok(_) => Err(format!(
                "{record_kind} creation failed, and cleanup could not safely remove the changed file at {}. Your changes were preserved. Review or remove the preserved file before retrying. Git reported: {error}",
                path.display()
            )),
            Err(read_error) if read_error.kind() == std::io::ErrorKind::NotFound => Err(error),
            Err(read_error) => Err(format!(
                "{record_kind} creation failed and cleanup could not inspect {}: {read_error}. Git reported: {error}",
                path.display()
            )),
        },
    }
}

fn parse_list_window(options: &ListWindowArgs, command: &str) -> Result<ListWindow, String> {
    let help = format!("Try '{command} --help' for options.");
    let max_count = options
        .max_count
        .as_deref()
        .map(|value| {
            let value = value.trim();
            if value.is_empty()
                || !value.as_bytes()[0].is_ascii_digit()
                || value.as_bytes()[0] == b'0'
                || !value.bytes().all(|byte| byte.is_ascii_digit())
            {
                return Err(format!(
                    "--max-count must be a positive integer (1 or greater). {help}"
                ));
            }
            Ok(value.parse::<usize>().unwrap_or(usize::MAX))
        })
        .transpose()?;
    let skip = options
        .skip
        .as_deref()
        .map(|value| {
            let value = value.trim();
            if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(format!(
                    "--skip must be a non-negative integer (0 or greater). {help}"
                ));
            }
            Ok(value.parse::<usize>().unwrap_or(usize::MAX))
        })
        .transpose()?
        .unwrap_or(0);
    Ok(ListWindow {
        max_count,
        skip,
        count: options.count,
    })
}

fn format_list_window_footer(
    skip: usize,
    shown: usize,
    total: usize,
    next_skip: Option<usize>,
    newline: &str,
) -> String {
    let range = if shown == 0 {
        "0".into()
    } else {
        format!("{}-{}", skip + 1, skip + shown)
    };
    let mut footer = format!("Showing {range} of {total} items.");
    if let Some(next) = next_skip {
        footer.push_str(&format!(" Next: {}", list_next_command(next)));
    }
    footer.push_str(newline);
    footer
}

fn list_next_command(next_skip: usize) -> String {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut kept = Vec::new();
    let mut index = 0;
    let separator = args
        .iter()
        .position(|arg| arg == "--")
        .unwrap_or(args.len());
    while index < separator {
        let arg = &args[index];
        if arg == "--skip" {
            index += 2;
            continue;
        }
        if arg.starts_with("--skip=") {
            index += 1;
            continue;
        }
        kept.push(arg.clone());
        index += 1;
    }
    kept.extend(["--skip".into(), next_skip.to_string()]);
    kept.extend(args[separator..].iter().cloned());
    std::iter::once("backlog".to_owned())
        .chain(kept)
        .map(|arg| quote_shell_argument(&arg))
        .collect::<Vec<_>>()
        .join(" ")
}

fn quote_shell_argument(argument: &str) -> String {
    if !argument.is_empty()
        && argument
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"@+:,./-".contains(&byte))
    {
        argument.to_owned()
    } else {
        format!("'{}'", argument.replace('\'', "'\\''"))
    }
}

fn task_filter(
    options: &TaskListArgs,
    configured_statuses: &[String],
) -> Result<TaskFilter, String> {
    let exclude_status = canonical_statuses(
        &options.exclude_status,
        configured_statuses,
        "exclude-status",
    )?;
    let mut labels = options.labels.clone();
    labels.retain(|label| !label.trim().is_empty());
    Ok(TaskFilter {
        statuses: options.status.clone(),
        exclude_statuses: exclude_status,
        labels,
        label_match: LabelMatch::All,
        ..TaskFilter::default()
    })
}

fn canonical_statuses(
    values: &[String],
    configured: &[String],
    option: &str,
) -> Result<Vec<String>, String> {
    let mut normalized = Vec::new();
    for value in values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        let Some(status) = configured
            .iter()
            .find(|status| status.eq_ignore_ascii_case(value))
        else {
            return Err(format!(
                "Invalid {option}: {value}. Valid statuses are: {}",
                configured.join(", ")
            ));
        };
        if !normalized.contains(status) {
            normalized.push(status.clone());
        }
    }
    Ok(normalized)
}

fn sort_name(field: SortField) -> &'static str {
    match field {
        SortField::Priority => "priority",
        SortField::Id => "id",
        SortField::Ordinal => "ordinal",
    }
}

fn print_task_sections(
    tasks: &[TaskRecord],
    sort_field: Option<&str>,
    statuses: &[String],
    date_format: &str,
) -> Result<(), String> {
    if tasks.is_empty() {
        return write_stdout("No tasks found.\n");
    }
    for section in section_tasks(tasks, sort_field, statuses) {
        match section {
            TaskSection {
                status: None,
                tasks,
            } => {
                write_stdout("Tasks (sorted by priority):\n")?;
                for task in tasks {
                    write_stdout(&format!("{}\n", plain_task_row(&task, true, date_format)))?;
                }
            }
            TaskSection {
                status: Some(status),
                tasks,
            } => {
                write_stdout(&format!(
                    "{}:\n",
                    if status.is_empty() {
                        "No Status"
                    } else {
                        &status
                    }
                ))?;
                for task in tasks {
                    write_stdout(&format!("{}\n", plain_task_row(&task, false, date_format)))?;
                }
                write_stdout("\n")?;
            }
        }
    }
    Ok(())
}

fn format_task_detail(
    detail: &TaskDetail,
    path: &std::path::Path,
    archived: bool,
    active_tasks: &[TaskRecord],
    config: &Config,
    prefix: &str,
    show_parent_title: bool,
) -> String {
    let task = &detail.task;
    let mut lines = vec![format!("File: {}", path.display()), String::new()];
    lines.extend([
        format!("Task {} - {}", task.id, task.title),
        "=".repeat(50),
        String::new(),
        format!(
            "Status: {}{}{}",
            status_icon(&task.status),
            task.status,
            if archived { " (archived)" } else { "" }
        ),
    ]);
    if let Some(priority) = task.priority.as_deref().filter(|value| !value.is_empty()) {
        lines.push(format!(
            "Priority: {}",
            priority_label(priority, config.priorities.as_deref())
        ));
    }
    if let Some(task_type) = task.task_type.as_deref() {
        lines.push(format!("Type: {task_type}"));
    }
    if let Some(project) = task.project.as_deref() {
        lines.push(format!("Project: {project}"));
    }
    if let Some(ordinal) = task.ordinal {
        lines.push(format!("Ordinal: {ordinal}"));
    }
    if !task.assignee.is_empty() {
        lines.push(format!(
            "Assignee: {}",
            task.assignee
                .iter()
                .map(|value| if value.starts_with('@') {
                    value.clone()
                } else {
                    format!("@{value}")
                })
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if let Some(reporter) = task.reporter.as_deref() {
        lines.push(format!(
            "Reporter: {}",
            if reporter.starts_with('@') {
                reporter.to_owned()
            } else {
                format!("@{reporter}")
            }
        ));
    }
    lines.push(format!(
        "Created: {} (UTC)",
        format_display_date(&task.created_date, "")
    ));
    if let Some(updated) = task.updated_date.as_deref() {
        lines.push(format!(
            "Updated: {} (UTC)",
            format_display_date(updated, "")
        ));
    }
    if let Some(due) = task.due_date.as_deref() {
        lines.push(format!("Due: {}", format_display_date(due, "")));
    }
    if !task.labels.is_empty() {
        lines.push(format!("Labels: {}", task.labels.join(", ")));
    }
    if let Some(milestone) = task.milestone.as_deref() {
        lines.push(format!("Milestone: {milestone}"));
    }
    if let Some(parent_id) = task.parent_task_id.as_deref() {
        let parent_title = show_parent_title
            .then(|| {
                active_tasks
                    .iter()
                    .find(|candidate| task_ids_equal(parent_id, &candidate.id, prefix))
                    .map(|candidate| format!("{} - {}", parent_id, candidate.title))
            })
            .flatten()
            .unwrap_or_else(|| parent_id.to_owned());
        lines.push(format!("Parent: {parent_title}"));
    }
    let mut subtasks = active_tasks
        .iter()
        .filter(|candidate| {
            candidate
                .parent_task_id
                .as_deref()
                .is_some_and(|parent_id| task_ids_equal(parent_id, &task.id, prefix))
        })
        .collect::<Vec<_>>();
    subtasks.sort_by(|left, right| crate::task_store::compare_task_ids(&left.id, &right.id));
    if !subtasks.is_empty() {
        lines.push(format!("Subtasks ({}):", subtasks.len()));
        lines.extend(
            subtasks
                .iter()
                .map(|subtask| format!("- {} - {}", subtask.id, subtask.title)),
        );
    }
    if !task.references.is_empty() {
        lines.push(format!("References: {}", task.references.join(", ")));
    }
    if !task.documentation.is_empty() {
        lines.push(format!("Documentation: {}", task.documentation.join(", ")));
    }
    let graph_lines = format_dependency_graph_lines(&detail.dependency_graph);
    if !graph_lines.is_empty() {
        lines.extend([
            String::new(),
            "Dependency Graph:".to_owned(),
            "-".repeat(50),
        ]);
        lines.extend(graph_lines);
    }
    lines.extend([String::new(), "Description:".to_owned(), "-".repeat(50)]);
    let description = task.description.trim();
    lines.push(if description.is_empty() {
        "No description provided".to_owned()
    } else {
        description.to_owned()
    });
    lines.extend([
        String::new(),
        "Acceptance Criteria:".to_owned(),
        "-".repeat(50),
    ]);
    lines.extend(format_checklist(
        &task.acceptance_criteria_items,
        "No acceptance criteria defined",
    ));
    lines.extend([
        String::new(),
        "Definition of Done:".to_owned(),
        "-".repeat(50),
    ]);
    lines.extend(format_checklist(
        &task.definition_of_done_items,
        "No Definition of Done items defined",
    ));
    if let Some(plan) = task
        .implementation_plan
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        lines.extend([
            String::new(),
            "Implementation Plan:".to_owned(),
            "-".repeat(50),
            plan.to_owned(),
        ]);
    }
    if let Some(notes) = task
        .implementation_notes
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        lines.extend([
            String::new(),
            "Implementation Notes:".to_owned(),
            "-".repeat(50),
            notes.to_owned(),
        ]);
    }
    if !task.modified_files.is_empty() {
        lines.extend([
            String::new(),
            format!("Modified files: {}", task.modified_files.join(", ")),
        ]);
    }
    let mut comments = task
        .comments
        .iter()
        .filter(|comment| !comment.body.trim().is_empty())
        .collect::<Vec<_>>();
    comments.sort_by_key(|comment| comment.index);
    if !comments.is_empty() {
        lines.extend([String::new(), "Comments:".to_owned(), "-".repeat(50)]);
        for comment in &comments {
            let mut header = vec![format!("#{}", comment.index)];
            if let Some(author) = comment.author.as_deref().filter(|value| !value.is_empty()) {
                header.push(author.to_owned());
            }
            if !comment.created_date.is_empty() {
                header.push(format!(
                    "{} (UTC)",
                    format_display_date(&comment.created_date, "")
                ));
            }
            lines.push(header.join(" - "));
            lines.push(comment.body.trim().to_owned());
            lines.push(String::new());
        }
    }
    if let Some(summary) = task
        .final_summary
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if comments.is_empty() {
            lines.push(String::new());
        }
        lines.extend([
            "Final Summary:".to_owned(),
            "-".repeat(50),
            summary.to_owned(),
            String::new(),
        ]);
    }
    if !lines.last().is_some_and(String::is_empty) {
        lines.push(String::new());
    }
    lines.join("\n")
}

fn status_icon(status: &str) -> &'static str {
    match status.trim().to_lowercase().as_str() {
        "done" => "✔ ",
        "in progress" => "◒ ",
        "blocked" => "● ",
        "review" => "◆ ",
        "testing" => "▣ ",
        _ => "○ ",
    }
}

fn priority_label(priority: &str, configured: Option<&[String]>) -> String {
    let normalize = |value: &str| value.split_whitespace().collect::<Vec<_>>().join(" ");
    let normalized_priority = normalize(priority);
    let options = configured
        .filter(|values| !values.is_empty())
        .map(|values| values.to_vec())
        .unwrap_or_else(|| vec!["High".to_owned(), "Medium".to_owned(), "Low".to_owned()]);
    if let Some(label) = options
        .iter()
        .find(|label| normalize(label).eq_ignore_ascii_case(&normalized_priority))
    {
        return normalize(label);
    }
    normalized_priority
        .split_whitespace()
        .map(|part| {
            let mut chars = part.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().collect::<String>() + chars.as_str()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_checklist(items: &[crate::sections::Criterion], empty_label: &str) -> Vec<String> {
    if items.is_empty() {
        return vec![empty_label.to_owned()];
    }
    let mut sorted = items.iter().collect::<Vec<_>>();
    sorted.sort_by_key(|item| item.index);
    sorted
        .iter()
        .enumerate()
        .map(|(index, item)| {
            format!(
                "- [{}] #{} {}",
                if item.checked { "x" } else { " " },
                index + 1,
                item.text
            )
        })
        .collect()
}

fn format_dependency_graph_lines(graph: &DependencyGraph) -> Vec<String> {
    let mut output = Vec::new();
    for (direction, heading, separate) in [(true, "Depends on", false), (false, "Dependents", true)]
    {
        let mut reached = graph
            .nodes
            .iter()
            .filter(|node| {
                let depth = if direction {
                    node.dependency_depth
                } else {
                    node.dependent_depth
                };
                depth.is_some_and(|depth| depth > 0)
            })
            .collect::<Vec<_>>();
        if reached.is_empty() {
            continue;
        }
        if separate && !output.is_empty() {
            output.push(String::new());
        }
        reached.sort_by(|left, right| crate::task_store::compare_task_ids(&left.id, &right.id));
        let direct = reached
            .iter()
            .filter(|node| {
                let depth = if direction {
                    node.dependency_depth
                } else {
                    node.dependent_depth
                };
                depth == Some(1)
            })
            .count();
        output.push(format!(
            "{heading} ({direct} direct, {} total):",
            reached.len()
        ));
        let mut children: HashMap<String, Vec<String>> = HashMap::new();
        for edge in &graph.edges {
            let (parent, child) = if direction {
                (&edge.from, &edge.to)
            } else {
                (&edge.to, &edge.from)
            };
            children
                .entry(parent.clone())
                .or_default()
                .push(child.clone());
        }
        for values in children.values_mut() {
            values.sort_by(|left, right| crate::task_store::compare_task_ids(left, right));
        }
        let nodes = graph
            .nodes
            .iter()
            .map(|node| (node.id.clone(), node))
            .collect::<HashMap<_, _>>();
        let mut expanded = HashSet::from([graph.root_id.clone()]);
        let mut branch = HashSet::from([graph.root_id.clone()]);
        let mut stack = vec![GraphFrame {
            id: graph.root_id.clone(),
            children: children.get(&graph.root_id).cloned().unwrap_or_default(),
            position: 0,
            prefix: String::new(),
        }];
        while let Some(frame) = stack.last_mut() {
            let Some(child_id) = frame.children.get(frame.position).cloned() else {
                let frame = stack.pop().expect("graph frame exists");
                branch.remove(&frame.id);
                continue;
            };
            frame.position += 1;
            let last = frame.position == frame.children.len();
            let line_prefix = frame.prefix.clone();
            let Some(node) = nodes.get(&child_id).copied() else {
                continue;
            };
            let repeat = if branch.contains(&child_id) {
                Some(" (cycle)")
            } else if expanded.contains(&child_id) {
                Some(" (shown above)")
            } else {
                None
            };
            output.push(format!(
                "{}{connector}{}{suffix}",
                line_prefix,
                dependency_node_label(node),
                suffix = repeat.unwrap_or(""),
                connector = if last { "└─ " } else { "├─ " },
            ));
            if repeat.is_some() || node.state.as_str() != "resolved" {
                continue;
            }
            expanded.insert(child_id.clone());
            branch.insert(child_id.clone());
            stack.push(GraphFrame {
                id: child_id.clone(),
                children: children.get(&child_id).cloned().unwrap_or_default(),
                position: 0,
                prefix: format!("{line_prefix}{}", if last { "   " } else { "│  " }),
            });
        }
    }
    output
}

struct GraphFrame {
    id: String,
    children: Vec<String>,
    position: usize,
    prefix: String,
}

fn dependency_node_label(node: &DependencyGraphNode) -> String {
    if node.state.as_str() != "resolved" {
        let label = if node.state.as_str() == "ambiguous" {
            "ambiguous task ID"
        } else {
            "unknown task ID"
        };
        return format!("{} - {label}", node.id);
    }
    let state = if node.completed {
        Some("completed")
    } else {
        node.status.as_deref()
    };
    let suffix = state.map(|value| format!(" [{value}]")).unwrap_or_default();
    let title = node.title.as_deref().unwrap_or_default().trim();
    if title.is_empty() {
        format!("{}{suffix}", node.id)
    } else {
        format!("{} - {}{suffix}", node.id, title)
    }
}

fn plain_task_row(task: &TaskRecord, include_status: bool, date_format: &str) -> String {
    let priority = task
        .priority
        .as_deref()
        .filter(|priority| !priority.is_empty())
        .map(|priority| format!("[{}] ", priority.to_uppercase()))
        .unwrap_or_default();
    let task_type = task
        .task_type
        .as_deref()
        .filter(|task_type| !task_type.is_empty())
        .map(|task_type| format!("[{task_type}] "))
        .unwrap_or_default();
    let status = if include_status && !task.status.is_empty() {
        format!(" ({})", task.status)
    } else {
        String::new()
    };
    let criteria_count = task.acceptance_criteria_items.len();
    let criteria_suffix = if criteria_count == 0 {
        String::new()
    } else {
        let checked = task
            .acceptance_criteria_items
            .iter()
            .filter(|criterion| criterion.checked)
            .count();
        format!(" (ac: {checked}/{criteria_count})")
    };
    let due_date = task
        .due_date
        .as_deref()
        .map(|date| format!(" (due {})", format_display_date(date, date_format)))
        .unwrap_or_default();
    format!(
        "  {priority}{task_type}{} - {}{status}{criteria_suffix}{due_date}",
        task.id, task.title
    )
}

fn format_display_date(value: &str, date_format: &str) -> String {
    let value = value.trim().trim_end_matches(" (UTC)").trim();
    let canonical = if value.len() == 10 {
        value.to_owned()
    } else if let Some(millis) = crate::statistics::parse_stored_utc_millis(value) {
        let days = millis.div_euclid(86_400_000);
        let day_millis = millis.rem_euclid(86_400_000);
        let (year, month, day) = civil_from_days(days);
        format!(
            "{year:04}-{month:02}-{day:02} {:02}:{:02}",
            day_millis / 3_600_000,
            day_millis % 3_600_000 / 60_000
        )
    } else {
        return value.to_owned();
    };
    apply_date_format(&canonical, date_format).unwrap_or(canonical)
}

fn apply_date_format(value: &str, format: &str) -> Option<String> {
    let mut parts = value.splitn(2, ' ');
    let date = parts.next()?;
    let time = parts.next();
    let mut date_parts = date.split('-');
    let (year, month, day) = (date_parts.next()?, date_parts.next()?, date_parts.next()?);
    let fmt = format.trim();
    let date_fmt = fmt.split_whitespace().next()?;
    if date_fmt.to_lowercase().matches("yyyy").count() != 1
        || date_fmt.to_lowercase().matches("mm").count() != 1
        || date_fmt.to_lowercase().matches("dd").count() != 1
    {
        return None;
    }
    let output = replace_date_tokens(date_fmt, year, month, day);
    match (
        time,
        fmt.split_once(char::is_whitespace)
            .map(|(_, time)| time.trim()),
    ) {
        (Some(time), Some(time_format)) if !time_format.is_empty() => {
            let mut time_parts = time.split(':');
            let (hour, minute) = (time_parts.next()?, time_parts.next()?);
            let time_fmt = time_format.to_lowercase();
            if time_fmt.matches("hh").count() != 1 || time_fmt.matches("mm").count() != 1 {
                return None;
            }
            Some(format!(
                "{output} {}",
                replace_time_tokens(time_format, hour, minute)
            ))
        }
        (Some(time), _) => Some(format!("{output} {time}")),
        _ => Some(output),
    }
}

fn replace_date_tokens(format: &str, year: &str, month: &str, day: &str) -> String {
    let chars = format.char_indices().collect::<Vec<_>>();
    let mut output = String::new();
    let mut index = 0;
    while index < chars.len() {
        let remaining = &format[chars[index].0..];
        let token = if remaining.len() >= 4 && remaining[..4].eq_ignore_ascii_case("yyyy") {
            Some((4, year))
        } else if remaining.len() >= 2 && remaining[..2].eq_ignore_ascii_case("mm") {
            Some((2, month))
        } else if remaining.len() >= 2 && remaining[..2].eq_ignore_ascii_case("dd") {
            Some((2, day))
        } else {
            None
        };
        if let Some((length, replacement)) = token {
            output.push_str(replacement);
            index += length;
        } else {
            output.push(chars[index].1);
            index += 1;
        }
    }
    output
}

fn replace_time_tokens(format: &str, hour: &str, minute: &str) -> String {
    format
        .replace("hh", hour)
        .replace("HH", hour)
        .replace("mm", minute)
        .replace("MM", minute)
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
}

fn task_list_json(items: &[TaskListItem]) -> Value {
    let tasks = items
        .iter()
        .map(|item| task_summary_json(&item.task, false, item.is_ready))
        .collect::<Vec<_>>();
    json!({"schemaVersion": 1, "kind": "task-list", "tasks": tasks})
}

fn task_summary_json(task: &TaskRecord, archived: bool, is_ready: bool) -> Value {
    let mut summary = task_compact_json(task, archived);
    summary["isReady"] = json!(is_ready);
    summary
}

fn task_compact_json(task: &TaskRecord, archived: bool) -> Value {
    let criteria = &task.acceptance_criteria_items;
    json!({
        "id": task.id,
        "title": task.title,
        "status": task.status,
        "archived": archived,
        "type": task.task_type,
        "priority": task.priority,
        "project": task.project,
        "assignees": task.assignee,
        "reporter": task.reporter,
        "labels": task.labels,
        "milestone": task.milestone,
        "parentTaskId": task.parent_task_id,
        "acceptanceCriteriaCompleted": criteria.iter().filter(|item| item.checked).count(),
        "acceptanceCriteriaCount": criteria.len(),
        "dependencies": task.dependencies,
        "references": task.references,
        "modifiedFiles": task.modified_files,
        "ordinal": public_ordinal(task.ordinal),
        "createdAt": normalize_public_date(task.created_date.as_str()),
        "updatedAt": task.updated_date.as_deref().map(normalize_public_date),
        "dueDate": task.due_date.as_deref().map(normalize_public_date),
    })
}

fn public_ordinal(ordinal: Option<f64>) -> Value {
    let Some(ordinal) = ordinal.filter(|value| value.is_finite()) else {
        return Value::Null;
    };
    if ordinal.fract() == 0.0 && ordinal >= i64::MIN as f64 && ordinal < i64::MAX as f64 {
        json!(ordinal as i64)
    } else {
        json!(ordinal)
    }
}

fn task_view_json(
    detail: &TaskDetail,
    path: &std::path::Path,
    project_root: &std::path::Path,
    archived: bool,
    active_tasks: &[TaskRecord],
    prefix: &str,
) -> Value {
    let task = &detail.task;
    let mut task_json = task_summary_json(task, archived, detail.readiness.is_ready);
    let relative_path = path
        .strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    let mut criteria = task.acceptance_criteria_items.clone();
    criteria.sort_by_key(|item| item.index);
    let mut definition_of_done = task.definition_of_done_items.clone();
    definition_of_done.sort_by_key(|item| item.index);
    let mut comments = task.comments.clone();
    comments.sort_by_key(|comment| comment.index);
    let mut subtasks = active_tasks
        .iter()
        .filter(|candidate| {
            candidate
                .parent_task_id
                .as_deref()
                .is_some_and(|parent_id| task_ids_equal(parent_id, &task.id, prefix))
        })
        .collect::<Vec<_>>();
    subtasks.sort_by(|left, right| crate::task_store::compare_task_ids(&left.id, &right.id));
    let criteria_json = criteria
        .iter()
        .map(|item| json!({"index": item.index, "text": item.text, "checked": item.checked}))
        .collect::<Vec<_>>();
    let definition_json = definition_of_done
        .iter()
        .map(|item| json!({"index": item.index, "text": item.text, "checked": item.checked}))
        .collect::<Vec<_>>();
    let comments_json = comments
        .iter()
        .map(|comment| {
            json!({
                "index": comment.index,
                "body": comment.body,
                "createdAt": (!comment.created_date.is_empty())
                    .then(|| normalize_public_date(&comment.created_date)),
                "author": comment.author,
            })
        })
        .collect::<Vec<_>>();
    let subtasks_json = subtasks
        .iter()
        .map(|subtask| json!({"id": subtask.id, "title": subtask.title}))
        .collect::<Vec<_>>();
    let nodes = detail
        .dependency_graph
        .nodes
        .iter()
        .map(dependency_node_json)
        .collect::<Vec<_>>();
    let edges = detail
        .dependency_graph
        .edges
        .iter()
        .map(|edge| json!({"from": edge.from, "to": edge.to}))
        .collect::<Vec<_>>();
    let object = task_json
        .as_object_mut()
        .expect("task summary is a JSON object");
    object.insert("path".to_owned(), json!(relative_path));
    object.insert(
        "description".to_owned(),
        json!((!task.description.is_empty()).then_some(&task.description)),
    );
    object.insert(
        "dependencyGraph".to_owned(),
        json!({"root": detail.dependency_graph.root_id, "nodes": nodes, "edges": edges}),
    );
    object.insert("readiness".to_owned(), readiness_json(&detail.readiness));
    object.insert("documentation".to_owned(), json!(task.documentation));
    object.insert("subtasks".to_owned(), json!(subtasks_json));
    object.insert("acceptanceCriteria".to_owned(), json!(criteria_json));
    object.insert("definitionOfDone".to_owned(), json!(definition_json));
    object.insert(
        "implementationPlan".to_owned(),
        json!(task.implementation_plan),
    );
    object.insert(
        "implementationNotes".to_owned(),
        json!(task.implementation_notes),
    );
    object.insert("comments".to_owned(), json!(comments_json));
    object.insert("finalSummary".to_owned(), json!(task.final_summary));
    json!({"schemaVersion": 1, "kind": "task-view", "task": task_json})
}

fn dependency_node_json(node: &DependencyGraphNode) -> Value {
    json!({
        "id": node.id,
        "title": node.title,
        "status": node.status,
        "state": node.state.as_str(),
        "completed": node.completed,
        "dependencyDepth": node.dependency_depth,
        "dependentDepth": node.dependent_depth,
    })
}

fn readiness_json(readiness: &crate::readiness::TaskReadiness) -> Value {
    json!({
        "isReady": readiness.is_ready,
        "isBlocked": readiness.is_blocked,
        "blockingDependencies": readiness.blocking_dependencies,
        "missingDependencies": readiness.missing_dependencies,
    })
}

fn normalize_public_date(value: &str) -> String {
    if value.len() == 10 && crate::statistics::parse_stored_utc_millis(value).is_some() {
        return value.to_owned();
    }
    let Some(millis) = crate::statistics::parse_stored_utc_millis(value) else {
        return value.to_owned();
    };
    let days = millis.div_euclid(86_400_000);
    let day_millis = millis.rem_euclid(86_400_000);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        day_millis / 3_600_000,
        day_millis % 3_600_000 / 60_000,
        day_millis % 60_000 / 1_000
    )
}

fn write_json(value: &Value) -> Result<(), String> {
    let mut stdout = io::stdout().lock();
    serde_json::to_writer(&mut stdout, value).map_err(|error| error.to_string())?;
    stdout.write_all(b"\n").map_err(|error| error.to_string())
}

fn write_pretty_json(value: &Value) -> Result<(), String> {
    let mut stdout = io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, value).map_err(|error| error.to_string())?;
    stdout.write_all(b"\n").map_err(|error| error.to_string())
}

fn write_stdout(value: &str) -> Result<(), String> {
    io::stdout()
        .lock()
        .write_all(value.as_bytes())
        .map_err(|error| error.to_string())
}

fn write_stderr(value: &str) -> Result<(), String> {
    io::stderr()
        .lock()
        .write_all(value.as_bytes())
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod editor_lookup_tests {
    use super::editor_candidate_names;

    #[test]
    fn windows_editor_lookup_uses_pathext_order_and_respects_explicit_extensions() {
        assert_eq!(
            editor_candidate_names("notepad", ".EXE;.CMD;.BAT", true),
            ["notepad.EXE", "notepad.CMD", "notepad.BAT"]
        );
        assert_eq!(
            editor_candidate_names("notepad.exe", ".CMD;.EXE", true),
            ["notepad.exe"]
        );
        assert_eq!(
            editor_candidate_names("notepad", ".EXE;.CMD", false),
            ["notepad"]
        );
    }
}
