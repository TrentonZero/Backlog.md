//! Sparse stored configuration. Initialization and migration defaults belong to explicit callers.
use crate::frontmatter::{FrontmatterError, parse_yaml_mapping};
use serde_json::Value;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub project_name: String,
    pub statuses: Vec<String>,
    pub labels: Vec<String>,
    pub date_format: String,
    pub default_assignee: Option<Vec<String>>,
    pub default_reporter: Option<String>,
    pub types: Option<Vec<String>>,
    pub priorities: Option<Vec<String>>,
    pub projects: Option<Vec<String>>,
    pub definition_of_done: Option<Vec<String>>,
    pub default_status: Option<String>,
    pub default_editor: Option<String>,
    pub max_column_width: Option<f64>,
    pub default_port: Option<f64>,
    pub zero_padded_ids: Option<f64>,
    pub active_branch_days: Option<f64>,
    pub auto_open_browser: Option<bool>,
    pub hide_empty_columns: Option<bool>,
    pub remote_operations: Option<bool>,
    pub auto_commit: Option<bool>,
    pub filesystem_only: Option<bool>,
    pub bypass_git_hooks: Option<bool>,
    pub check_active_branches: Option<bool>,
    pub on_status_change: Option<String>,
    pub task_prefix: Option<String>,
    pub backlog_directory: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            project_name: String::new(),
            statuses: vec!["To Do".into(), "In Progress".into(), "Done".into()],
            labels: vec![],
            date_format: "yyyy-mm-dd".into(),
            default_assignee: None,
            default_reporter: None,
            types: None,
            priorities: None,
            projects: None,
            definition_of_done: None,
            default_status: None,
            default_editor: None,
            max_column_width: None,
            default_port: None,
            zero_padded_ids: None,
            active_branch_days: None,
            auto_open_browser: None,
            hide_empty_columns: None,
            remote_operations: None,
            auto_commit: None,
            filesystem_only: None,
            bypass_git_hooks: None,
            check_active_branches: None,
            on_status_change: None,
            task_prefix: None,
            backlog_directory: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigLocation {
    Folder,
    Root,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigValueError {
    pub path: PathBuf,
    pub key: &'static str,
    pub problem: String,
    pub remedy: String,
}

impl fmt::Display for ConfigValueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Backlog could not start because {} has an invalid value for \"{}\"{}{}. {}",
            self.path.display(),
            self.key,
            if self.problem.is_empty() { "" } else { ": " },
            self.problem,
            self.remedy
        )
    }
}

impl std::error::Error for ConfigValueError {}

fn key_indent(line: &str, key: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    let after_key = trimmed.strip_prefix(key)?;
    after_key
        .trim_start()
        .starts_with(':')
        .then_some(line.len() - trimmed.len())
}

fn is_mapping_key(line: &str) -> bool {
    let trimmed = line.trim_start();
    !trimmed.starts_with('#')
        && !trimmed
            .strip_prefix('-')
            .is_some_and(|rest| rest.starts_with(char::is_whitespace))
        && trimmed.contains(':')
}

/// Prefer the last column-zero occurrence, using indented legacy keys only when necessary.
fn extract_key_yaml(source: &str, key: &str) -> Option<String> {
    let lines: Vec<_> = source
        .split('\n')
        .map(|line| line.trim_end_matches('\r'))
        .collect();
    let start = lines
        .iter()
        .rposition(|line| key_indent(line, key) == Some(0))
        .or_else(|| {
            lines
                .iter()
                .rposition(|line| key_indent(line, key).is_some())
        })?;
    let start_indent = key_indent(lines[start], key)?;
    let mut collected = Vec::new();
    for (offset, line) in lines[start..].iter().enumerate() {
        let indent = line.len() - line.trim_start().len();
        if offset > 0 && indent <= start_indent && is_mapping_key(line) {
            break;
        }
        collected.push(&line[start_indent.min(indent)..]);
    }
    Some(collected.join("\n"))
}

fn read_yaml_key(source: &str, key: &str) -> Result<Option<Value>, String> {
    parse_yaml_mapping(source)
        .map(|mut mapping| mapping.remove(key))
        .map_err(|error| match error {
            FrontmatterError::InvalidYaml(reason) => reason,
            _ => "expected a YAML mapping".into(),
        })
}

fn parse_list(
    source: &str,
    key: &'static str,
    path: &Path,
) -> Result<Option<Vec<String>>, ConfigValueError> {
    let Some(block) = extract_key_yaml(source, key) else {
        return Ok(None);
    };
    let parsed = match read_yaml_key(&block, key) {
        Ok(value) => value,
        Err(block_error) => read_yaml_key(source, key).map_err(|_| ConfigValueError {
            path: path.to_owned(),
            key,
            problem: block_error.lines().next().unwrap_or("").trim().to_owned(),
            remedy: "Edit that key so its value is valid YAML, then run the command again.".into(),
        })?,
    };
    match parsed {
        Some(Value::Null) => Ok((key == "default_assignee").then(Vec::new)),
        Some(Value::Array(items)) => Ok(Some(
            items
                .iter()
                .map(js_string)
                .map(|item| item.trim().to_owned())
                .filter(|item| !item.is_empty())
                .collect(),
        )),
        Some(Value::String(value)) if key == "default_assignee" => {
            let value = value.trim();
            Ok(Some(if value.is_empty() {
                vec![]
            } else {
                vec![value.to_owned()]
            }))
        }
        value => {
            let expected = if key == "default_assignee" {
                "a list or a single name"
            } else {
                "a list"
            };
            let actual = match value {
                None => "no value Backlog could read",
                Some(Value::String(_)) => "a scalar",
                Some(Value::Number(_)) => "a number",
                Some(Value::Bool(_)) => "a boolean",
                _ => "a mapping",
            };
            Err(ConfigValueError {
                path: path.to_owned(),
                key,
                problem: format!("expected {expected}, got {actual}"),
                remedy: format!(
                    "Edit that key so its value is {expected}, then run the command again."
                ),
            })
        }
    }
}

fn number_string(number: f64) -> String {
    if number.is_nan() {
        return "NaN".into();
    }
    if number == 0.0 {
        return "0".into();
    }
    if number.is_infinite() {
        return if number.is_sign_negative() {
            "-Infinity"
        } else {
            "Infinity"
        }
        .into();
    }
    if number.abs() >= 1e21 || number.abs() < 1e-6 {
        let scientific = format!("{number:e}");
        let (mantissa, exponent) = scientific
            .split_once('e')
            .expect("scientific format has exponent");
        return format!(
            "{mantissa}e{}{exponent}",
            if exponent.starts_with('-') { "" } else { "+" }
        );
    }
    number.to_string()
}

fn js_string(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => number_string(value.as_f64().expect("JSON number fits f64")),
        Value::String(value) => value.clone(),
        Value::Array(items) => items
            .iter()
            .map(|item| {
                if item.is_null() {
                    String::new()
                } else {
                    js_string(item)
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".into(),
    }
}

fn normalize_strings(items: &[String]) -> Vec<String> {
    items
        .iter()
        .map(|item| item.trim().to_owned())
        .filter(|item| !item.is_empty())
        .collect()
}

fn definition_from_yaml(source: &str) -> Option<Vec<String>> {
    match read_yaml_key(source, "definition_of_done").ok()?? {
        Value::Null => Some(vec![]),
        Value::Array(items) => {
            let strings: Vec<_> = items
                .into_iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect();
            Some(normalize_strings(&strings))
        }
        _ => None,
    }
}

/// Old double-quoted DoD entries used literal Windows backslashes. Keep that compatibility local.
fn escape_legacy_dod_backslashes(source: &str) -> Option<String> {
    let chars: Vec<_> = source.chars().collect();
    let mut escaped = String::new();
    let mut quote = None;
    let mut changed = false;
    let mut index = 0;
    while index < chars.len() {
        let character = chars[index];
        if let Some(active_quote) = quote {
            if active_quote == '"' && character == '\\' {
                let count = chars[index..]
                    .iter()
                    .take_while(|character| **character == '\\')
                    .count();
                let next = chars.get(index + count).copied();
                if next == Some('"') && count % 2 == 1 {
                    escaped.push_str(&"\\".repeat(count));
                    escaped.push('"');
                    index += count + 1;
                    continue;
                }
                let escaped_count = count + count % 2;
                escaped.push_str(&"\\".repeat(escaped_count));
                changed |= escaped_count != count;
                index += count;
                continue;
            }
            if character == active_quote {
                quote = None;
            }
        } else if character == '\'' || character == '"' {
            quote = Some(character);
        }
        escaped.push(character);
        index += 1;
    }
    changed.then_some(escaped)
}

fn parse_definition_of_done(source: &str) -> Option<Vec<String>> {
    let block = extract_key_yaml(source, "definition_of_done")?;
    escape_legacy_dod_backslashes(&block)
        .and_then(|escaped| definition_from_yaml(&escaped))
        .or_else(|| definition_from_yaml(source))
        .or_else(|| definition_from_yaml(&block))
}

fn remove_quotes(value: &str) -> String {
    value.replace(['\'', '"'], "")
}

fn parse_integer_prefix(value: &str) -> f64 {
    let digits_start = usize::from(value.starts_with(['+', '-']));
    let digits_end = digits_start
        + value[digits_start..]
            .bytes()
            .take_while(u8::is_ascii_digit)
            .count();
    if digits_end == digits_start {
        return f64::NAN;
    }
    value[..digits_end].parse().unwrap_or(f64::NAN)
}

pub fn parse_config(source: &str, path: &Path) -> Result<Config, ConfigValueError> {
    let mut config = Config::default();
    if let Some(statuses) = parse_list(source, "statuses", path)? {
        config.statuses = statuses;
    }
    if let Some(labels) = parse_list(source, "labels", path)? {
        config.labels = labels;
    }
    config.types = parse_list(source, "types", path)?;
    config.priorities = parse_list(source, "priorities", path)?;
    config.projects = parse_list(source, "projects", path)?;
    config.default_assignee = parse_list(source, "default_assignee", path)?;
    config.definition_of_done = parse_definition_of_done(source);
    for line in source.split('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "project_name" => config.project_name = remove_quotes(value),
            "default_reporter" => config.default_reporter = Some(remove_quotes(value)),
            "default_status" => config.default_status = Some(remove_quotes(value)),
            "date_format" => config.date_format = remove_quotes(value),
            "default_editor" => config.default_editor = Some(remove_quotes(value)),
            "task_prefix" => config.task_prefix = Some(remove_quotes(value)),
            "backlog_directory" | "backlogDirectory" => {
                config.backlog_directory = Some(remove_quotes(value))
            }
            "onStatusChange" | "on_status_change" => {
                let value = value.strip_prefix(['\'', '"']).unwrap_or(value);
                let value = value.strip_suffix(['\'', '"']).unwrap_or(value);
                config.on_status_change = Some(value.to_owned());
            }
            "max_column_width" => config.max_column_width = Some(parse_integer_prefix(value)),
            "default_port" => config.default_port = Some(parse_integer_prefix(value)),
            "zero_padded_ids" => config.zero_padded_ids = Some(parse_integer_prefix(value)),
            "active_branch_days" => config.active_branch_days = Some(parse_integer_prefix(value)),
            "auto_open_browser" => {
                config.auto_open_browser = Some(value.eq_ignore_ascii_case("true"))
            }
            "hide_empty_columns" => {
                config.hide_empty_columns = Some(value.eq_ignore_ascii_case("true"))
            }
            "remote_operations" => {
                config.remote_operations = Some(value.eq_ignore_ascii_case("true"))
            }
            "auto_commit" => config.auto_commit = Some(value.eq_ignore_ascii_case("true")),
            "filesystem_only" | "filesystemOnly" => {
                config.filesystem_only = Some(value.eq_ignore_ascii_case("true"))
            }
            "bypass_git_hooks" => {
                config.bypass_git_hooks = Some(value.eq_ignore_ascii_case("true"))
            }
            "check_active_branches" => {
                config.check_active_branches = Some(value.eq_ignore_ascii_case("true"))
            }
            _ => {}
        }
    }
    if config.date_format.is_empty() {
        config.date_format = "yyyy-mm-dd".into();
    }
    Ok(config)
}

fn quoted_list(items: &[String], escape: bool) -> String {
    items
        .iter()
        .map(|item| {
            if escape {
                serde_json::to_string(item).expect("strings serialize to JSON")
            } else {
                format!("\"{item}\"")
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn write_string(lines: &mut Vec<String>, key: &str, value: Option<&str>) {
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        lines.push(format!("{key}: \"{value}\""));
    }
}

fn write_boolean(lines: &mut Vec<String>, key: &str, value: Option<bool>) {
    if let Some(value) = value {
        lines.push(format!("{key}: {value}"));
    }
}

fn write_number(lines: &mut Vec<String>, key: &str, value: Option<f64>, truthy_only: bool) {
    if let Some(value) = value.filter(|value| !truthy_only || (*value != 0.0 && !value.is_nan())) {
        lines.push(format!("{key}: {}", number_string(value)));
    }
}

/// Match the stored writer's order and omissions, including its legacy unescaped scalar quoting.
pub fn serialize_config(
    config: &Config,
    location: ConfigLocation,
    selected_backlog_directory: &str,
) -> String {
    let mut lines = vec![format!("project_name: \"{}\"", config.project_name)];
    if let Some(items) = config
        .default_assignee
        .as_ref()
        .filter(|items| !items.is_empty())
    {
        lines.push(format!("default_assignee: [{}]", quoted_list(items, true)));
    }
    write_string(
        &mut lines,
        "default_reporter",
        config.default_reporter.as_deref(),
    );
    write_string(
        &mut lines,
        "default_status",
        config.default_status.as_deref(),
    );
    lines.push(format!(
        "statuses: [{}]",
        quoted_list(&config.statuses, false)
    ));
    lines.push(format!("labels: [{}]", quoted_list(&config.labels, false)));
    for (key, items) in [
        ("types", &config.types),
        ("priorities", &config.priorities),
        ("projects", &config.projects),
    ] {
        if let Some(items) = items.as_ref().filter(|items| !items.is_empty()) {
            lines.push(format!("{key}: [{}]", quoted_list(items, false)));
        }
    }
    if let Some(items) = &config.definition_of_done {
        lines.push(format!(
            "definition_of_done: [{}]",
            quoted_list(&normalize_strings(items), true)
        ));
    }
    lines.push(format!("date_format: {}", config.date_format));
    write_number(
        &mut lines,
        "max_column_width",
        config.max_column_width,
        true,
    );
    write_string(
        &mut lines,
        "default_editor",
        config.default_editor.as_deref(),
    );
    write_boolean(&mut lines, "auto_open_browser", config.auto_open_browser);
    write_boolean(&mut lines, "hide_empty_columns", config.hide_empty_columns);
    write_number(&mut lines, "default_port", config.default_port, true);
    write_boolean(&mut lines, "remote_operations", config.remote_operations);
    write_boolean(&mut lines, "auto_commit", config.auto_commit);
    write_boolean(&mut lines, "filesystem_only", config.filesystem_only);
    write_number(&mut lines, "zero_padded_ids", config.zero_padded_ids, false);
    write_boolean(&mut lines, "bypass_git_hooks", config.bypass_git_hooks);
    write_boolean(
        &mut lines,
        "check_active_branches",
        config.check_active_branches,
    );
    write_number(
        &mut lines,
        "active_branch_days",
        config.active_branch_days,
        false,
    );
    if let Some(callback) = config
        .on_status_change
        .as_ref()
        .filter(|value| !value.is_empty())
    {
        lines.push(format!("onStatusChange: '{callback}'"));
    }
    write_string(&mut lines, "task_prefix", config.task_prefix.as_deref());
    if location == ConfigLocation::Root {
        write_string(
            &mut lines,
            "backlog_directory",
            Some(selected_backlog_directory),
        );
    }
    format!("{}\n", lines.join("\n"))
}
