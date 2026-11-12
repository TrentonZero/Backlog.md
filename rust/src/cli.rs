//! Initial noninteractive Rust CLI surface for listing local tasks.

use crate::{
    config::{Config, parse_config},
    discovery::{find_project_root, resolve_backlog_directory},
    records::TaskRecord,
    task_filtering::{LabelMatch, TaskFilter},
    task_list::{TaskListItem, with_readiness},
    task_search::TaskSearchIndex,
    task_sorting::{TaskSection, section_tasks, sort_tasks},
    task_store::TaskStore,
};
use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};
use serde_json::{Value, json};
use std::{
    fs,
    io::{self, Write},
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
        config,
        store,
        prefix,
    })
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
        .map(|item| {
            let task = &item.task;
            let criteria = &task.acceptance_criteria_items;
            json!({
                "id": task.id,
                "title": task.title,
                "status": task.status,
                "archived": false,
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
                "ordinal": task.ordinal.filter(|ordinal| ordinal.is_finite()),
                "createdAt": normalize_public_date(task.created_date.as_str()),
                "updatedAt": task.updated_date.as_deref().map(normalize_public_date),
                "dueDate": task.due_date.as_deref().map(normalize_public_date),
                "isReady": item.is_ready,
            })
        })
        .collect::<Vec<_>>();
    json!({"schemaVersion": 1, "kind": "task-list", "tasks": tasks})
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
