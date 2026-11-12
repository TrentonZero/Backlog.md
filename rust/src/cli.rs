//! Initial noninteractive Rust CLI surface for listing local tasks.

use crate::{
    config::{Config, parse_config},
    dependency_graph::{DependencyGraph, DependencyGraphNode},
    discovery::{find_project_root, resolve_backlog_directory},
    identity::task_ids_equal,
    records::TaskRecord,
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
    let store = TaskStore::new(&root, backlog_directory, &prefix);
    Ok(ProjectContext {
        root,
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
            )
        ))
    }
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
        let parent_title = active_tasks
            .iter()
            .find(|candidate| task_ids_equal(parent_id, &candidate.id, prefix))
            .map(|candidate| format!("{} - {}", parent_id, candidate.title))
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
        "isReady": is_ready,
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
