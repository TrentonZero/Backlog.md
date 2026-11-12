//! Native Rust CLI for noninteractive local backlog commands.

use crate::{
    config::{Config, parse_config},
    dependency_graph::{DependencyGraph, DependencyGraphNode},
    discovery::{find_project_root, resolve_backlog_directory},
    draft_store::DraftStore,
    identity::{canonical_task_id, normalize_task_id, task_ids_equal},
    milestone_store::MilestoneStore,
    records::TaskRecord,
    sections::{ChecklistKind, Criterion},
    task_detail::{TaskDetail, build_task_detail},
    task_filtering::{LabelMatch, TaskFilter},
    task_list::{TaskListItem, with_readiness},
    task_search::TaskSearchIndex,
    task_sorting::{TaskSection, section_tasks, sort_tasks},
    task_store::TaskStore,
};
use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{self, Write},
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
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
    /// Draft commands.
    Draft {
        #[command(subcommand)]
        command: DraftCommand,
    },
}

#[derive(Debug, Subcommand)]
enum DraftCommand {
    /// List local drafts.
    List(DraftListArgs),
    /// Create a local draft.
    Create(DraftCreateArgs),
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
    /// Print at most this many drafts after sorting.
    #[arg(long, allow_hyphen_values = true)]
    max_count: Option<String>,
    /// Leave out this many drafts after sorting.
    #[arg(long, allow_hyphen_values = true)]
    skip: Option<String>,
    /// Print only the number of drafts in this window.
    #[arg(long)]
    count: bool,
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
    /// Replacement status.
    #[arg(short, long)]
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
    config: Config,
    store: TaskStore,
    prefix: String,
}

pub fn run() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.command {
        Command::Task {
            command: TaskCommand::List(options),
        } => run_task_list(options),
        Command::Task {
            command: TaskCommand::View { task_id, options },
        } => run_task_view(&task_id, options),
        Command::Task {
            command: TaskCommand::Create(options),
        } => run_task_create(*options),
        Command::Task {
            command: TaskCommand::Edit(options),
        } => run_task_edit(*options),
        Command::Task {
            command: TaskCommand::Archive { task_id },
        } => run_task_archive(&task_id),
        Command::Task {
            command: TaskCommand::Restore { task_id },
        } => run_task_restore(&task_id),
        Command::Task {
            command: TaskCommand::Delete { task_id },
        } => run_task_delete(&task_id),
        Command::Draft {
            command: DraftCommand::List(options),
        } => run_draft_list(options),
        Command::Draft {
            command: DraftCommand::Create(options),
        } => run_draft_create(options),
    }
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
        config,
        store,
        prefix,
    })
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
    let checklist_options = options.clone();
    let context = project_context()?;
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

    let entry = context
        .store
        .load_task(&options.task_id)
        .map_err(|error| format!("{}\nNo changes were applied.", error))?;
    let Some(entry) = entry else {
        let archived = context.store.list_archived_tasks();
        if let Some(candidate) = archived
            .iter()
            .find(|candidate| task_ids_equal(&options.task_id, &candidate.task.id, &context.prefix))
        {
            return Err(format!(
                "{}\nNo changes were applied.",
                crate::task_store::TaskStoreError::Archived(candidate.task.id.clone())
            ));
        }
        return Err(format!(
            "Task {} not found. Task lookups read only the local working copy; use 'backlog browser' to see tasks from other branches.\nNo changes were applied.",
            options.task_id
        ));
    };

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
        updated.status =
            resolve_config_value(&status, &context.config.statuses).ok_or_else(|| {
                format!(
                    "Invalid status: {status}. Valid statuses are: {}\nNo changes were applied.",
                    context.config.statuses.join(", ")
                )
            })?;
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
    apply_checklist_options(&mut updated, &entry.task.raw_content, &checklist_options)?;

    let updated_entry = if updated != entry.task {
        updated.updated_date = Some(crate::task_store::utc_timestamp());
        let updated_entry = context
            .store
            .update_task(updated)
            .map_err(|error| {
                format!(
                    "{}\nNo changes were applied.",
                    task_edit_store_error(error, &entry.task.id, &context.prefix)
                )
            })?
            .ok_or_else(|| {
                format!(
                    "Task {} not found. Task lookups read only the local working copy; use 'backlog browser' to see tasks from other branches.\nNo changes were applied.",
                    options.task_id
                )
            })?;
        commit_record_change(
            &context,
            &format!(
                "Update task {}",
                canonical_task_id(&updated_entry.task.id, &context.prefix)
            ),
            std::slice::from_ref(&updated_entry.path),
        )?;
        updated_entry
    } else {
        entry
    };
    let mut updated_task = updated_entry.task;
    updated_task.id = canonical_task_id(&updated_task.id, &context.prefix);
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
    write_stdout(&format!("Updated task {}\n", updated_task.id))
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

fn checklist_indices(values: &[String], task_id: &str) -> Result<Vec<usize>, String> {
    values
        .iter()
        .map(|value| {
            let index = value.parse::<usize>().ok().filter(|index| *index > 0);
            index.ok_or_else(|| {
                format!(
                    "Invalid index: {value}. Index must be a positive number (1-based). Try 'backlog task edit {task_id} --help' for index options.\nNo changes were applied."
                )
            })
        })
        .collect()
}

fn apply_checklist_options(
    task: &mut TaskRecord,
    original_body: &str,
    options: &TaskEditArgs,
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
        .map_err(|error| format!("{error}\nNo changes were applied."))?;
    }
    if !options.remove_dod.is_empty()
        || !options.check_dod.is_empty()
        || !options.uncheck_dod.is_empty()
    {
        crate::sections::assert_valid_checklist_marks(
            original_body,
            ChecklistKind::DefinitionOfDone,
        )
        .map_err(|error| format!("{error}\nNo changes were applied."))?;
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
        let removals = checklist_indices(&options.remove_ac, &options.task_id.to_uppercase())?;
        if !removals.is_empty() {
            let before = criteria.len();
            criteria.retain(|item| !removals.contains(&item.index));
            if before == criteria.len() {
                return Err(format!(
                    "Acceptance criterion #{} not found.\nNo changes were applied.",
                    removals[0]
                ));
            }
            reindex_criteria(&mut criteria);
        }
        apply_check_state(
            &mut criteria,
            &options.check_ac,
            true,
            "Acceptance criterion",
            &options.task_id.to_uppercase(),
        )?;
        apply_check_state(
            &mut criteria,
            &options.uncheck_ac,
            false,
            "Acceptance criterion",
            &options.task_id.to_uppercase(),
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
            &options.task_id.to_uppercase(),
        )?;
        apply_check_state(
            &mut criteria,
            &options.uncheck_dod,
            false,
            "Definition of Done item",
            &options.task_id.to_uppercase(),
        )?;
        let removals = checklist_indices(&options.remove_dod, &options.task_id.to_uppercase())?;
        if !removals.is_empty() {
            let before = criteria.len();
            criteria.retain(|item| !removals.contains(&item.index));
            if before == criteria.len() {
                return Err(format!(
                    "Definition of Done item #{} not found.\nNo changes were applied.",
                    removals[0]
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
    task_id: &str,
) -> Result<(), String> {
    for index in checklist_indices(values, task_id)? {
        let Some(item) = criteria.iter_mut().find(|item| item.index == index) else {
            return Err(format!(
                "{name} #{index} not found.\nNo changes were applied."
            ));
        };
        item.checked = checked;
    }
    Ok(())
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
    if options.count && options.json {
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
    let max_count = options
        .max_count
        .as_deref()
        .map(parse_draft_max_count)
        .transpose()?;
    let skip = options
        .skip
        .as_deref()
        .map(parse_draft_skip)
        .transpose()?
        .unwrap_or(0);
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
    let start = skip.min(total);
    let end = max_count.map_or(total, |count| start.saturating_add(count).min(total));
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
    if options.count {
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
        let shown = page.len();
        let range = if shown == 0 {
            "0".into()
        } else {
            format!("{}-{}", start + 1, start + shown)
        };
        let footer = if let Some(next) = next_skip {
            format!(
                "Showing {range} of {total} items. Next: {}\n",
                draft_next_command(next)
            )
        } else {
            format!("Showing {range} of {total} items.\n")
        };
        write_stdout(&footer)?;
    }
    Ok(())
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

fn parse_draft_max_count(value: &str) -> Result<usize, String> {
    let value = value.trim();
    if value.is_empty()
        || !value.as_bytes()[0].is_ascii_digit()
        || value.as_bytes()[0] == b'0'
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(
            "--max-count must be a positive integer (1 or greater). Try 'backlog draft list --help' for options.".into(),
        );
    }
    Ok(value.parse::<usize>().unwrap_or(usize::MAX))
}

fn parse_draft_skip(value: &str) -> Result<usize, String> {
    let value = value.trim();
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("--skip must be a non-negative integer (0 or greater). Try 'backlog draft list --help' for options.".into());
    }
    Ok(value.parse::<usize>().unwrap_or(usize::MAX))
}

fn draft_next_command(next_skip: usize) -> String {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut kept = Vec::new();
    let mut index = 0;
    while index < args.len() {
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

fn write_stdout(value: &str) -> Result<(), String> {
    io::stdout()
        .lock()
        .write_all(value.as_bytes())
        .map_err(|error| error.to_string())
}
