//! Pure typed parsers for task, decision, document, and milestone Markdown records.
use serde_json::{Map, Value};
use std::{error::Error, fmt};

use crate::{
    frontmatter,
    sections::{self, ChecklistKind, SectionKey},
};

#[derive(Clone, Debug, PartialEq)]
pub struct ParsedMarkdown {
    pub frontmatter: Map<String, Value>,
    pub content: String,
}
pub type Criterion = sections::Criterion;
pub type Comment = sections::Comment;
#[derive(Clone, Debug, PartialEq)]
pub struct TaskRecord {
    pub id: String,
    pub title: String,
    pub status: String,
    pub assignee: Vec<String>,
    pub reporter: Option<String>,
    pub created_date: String,
    pub updated_date: Option<String>,
    pub due_date: Option<String>,
    pub labels: Vec<String>,
    pub milestone: Option<String>,
    pub dependencies: Vec<String>,
    pub references: Vec<String>,
    pub documentation: Vec<String>,
    pub modified_files: Vec<String>,
    pub raw_content: String,
    pub acceptance_criteria_items: Vec<Criterion>,
    pub definition_of_done_items: Vec<Criterion>,
    pub description: String,
    pub implementation_plan: Option<String>,
    pub implementation_notes: Option<String>,
    pub comments: Vec<Comment>,
    pub final_summary: Option<String>,
    pub parent_task_id: Option<String>,
    pub subtasks: Option<Vec<String>>,
    pub priority: Option<String>,
    pub task_type: Option<String>,
    pub project: Option<String>,
    pub ordinal: Option<f64>,
    pub on_status_change: Option<String>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DecisionRecord {
    pub id: String,
    pub title: String,
    pub date: String,
    pub updated_date: Option<String>,
    pub superseded_by: Option<String>,
    pub status: String,
    pub context: String,
    pub decision: String,
    pub consequences: String,
    pub alternatives: Option<String>,
    pub raw_content: String,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DocumentRecord {
    pub id: String,
    pub title: String,
    pub document_type: String,
    pub created_date: String,
    pub updated_date: Option<String>,
    pub raw_content: String,
    pub tags: Option<Vec<String>>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct MilestoneRecord {
    pub id: String,
    pub title: String,
    pub due_date: Option<String>,
    pub description: String,
    pub raw_content: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordError(String);
impl fmt::Display for RecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for RecordError {}
impl From<frontmatter::FrontmatterError> for RecordError {
    fn from(value: frontmatter::FrontmatterError) -> Self {
        Self(value.to_string())
    }
}

fn js_string(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Array(a) => a
            .iter()
            .map(|v| {
                if v.is_null() {
                    String::new()
                } else {
                    js_string(v)
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".into(),
    }
}
fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) | Some(Value::Bool(false)) => false,
        Some(Value::String(s)) if s.is_empty() => false,
        Some(Value::Number(n)) if n.as_f64() == Some(0.0) => false,
        _ => true,
    }
}
fn field(map: &Map<String, Value>, key: &str) -> String {
    map.get(key)
        .filter(|v| truthy(Some(v)))
        .map(js_string)
        .unwrap_or_default()
}
fn optional(map: &Map<String, Value>, key: &str) -> Option<String> {
    map.get(key).filter(|v| truthy(Some(v))).map(js_string)
}
fn strings(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items.iter().map(js_string).collect(),
        _ => Vec::new(),
    }
}
fn date(value: Option<&Value>) -> String {
    if !truthy(value) {
        return String::new();
    }
    let raw = js_string(value.unwrap())
        .trim()
        .trim_matches(['\'', '"'])
        .to_owned();
    for separator in ['-', '/', '.'] {
        let parts: Vec<_> = raw.split(separator).collect();
        if parts.len() == 3
            && parts[0].len() == 2
            && parts[1].len() == 2
            && parts[2].len() == 2
            && parts
                .iter()
                .all(|part| part.bytes().all(|byte| byte.is_ascii_digit()))
        {
            return format!("20{}-{}-{}", parts[2], parts[1], parts[0]);
        }
    }
    raw
}
fn due_date(value: Option<&Value>, name: &str) -> Result<Option<String>, RecordError> {
    match value {
        None | Some(Value::Null) => return Ok(None),
        _ => {}
    }
    let input = js_string(value.unwrap()).trim().to_owned();
    if input.is_empty() {
        return Ok(None);
    }
    let bytes = input.as_bytes();
    let valid = bytes.len() >= 10
        && bytes[0..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(u8::is_ascii_digit);
    let (y, m, d) = if valid {
        (
            input[0..4].parse::<i32>().unwrap_or(0),
            input[5..7].parse::<u32>().unwrap_or(0),
            input[8..10].parse::<u32>().unwrap_or(0),
        )
    } else {
        (0, 0, 0)
    };
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    };
    let rest = input.get(10..).is_some_and(valid_due_time);
    if !valid || d == 0 || d > days || !rest {
        return Err(RecordError(format!(
            "{name} must be a date in YYYY-MM-DD format (for example, 2026-08-10)."
        )));
    }
    Ok(Some(input[..10].to_owned()))
}
fn valid_due_time(suffix: &str) -> bool {
    if suffix.is_empty() {
        return true;
    }
    let Some(time) = suffix.strip_prefix(['T', ' ']) else {
        return false;
    };
    let bytes = time.as_bytes();
    if bytes.len() < 5
        || !bytes[0..2].iter().all(u8::is_ascii_digit)
        || bytes[2] != b':'
        || !bytes[3..5].iter().all(u8::is_ascii_digit)
    {
        return false;
    }
    let number = |part: &[u8]| part.iter().fold(0u32, |n, b| n * 10 + u32::from(b - b'0'));
    if number(&bytes[0..2]) > 23 || number(&bytes[3..5]) > 59 {
        return false;
    }
    let mut i = 5;
    if bytes.get(i) == Some(&b':') {
        if bytes.len() < i + 3
            || !bytes[i + 1..i + 3].iter().all(u8::is_ascii_digit)
            || number(&bytes[i + 1..i + 3]) > 59
        {
            return false;
        }
        i += 3;
        if bytes.get(i) == Some(&b'.') {
            i += 1;
            let start = i;
            while bytes.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
            if i == start || i - start > 9 {
                return false;
            }
        }
    }
    if i == bytes.len() {
        return true;
    }
    if bytes[i] == b'Z' {
        return i + 1 == bytes.len();
    }
    if !matches!(bytes[i], b'+' | b'-') {
        return false;
    }
    let offset = &bytes[i + 1..];
    let (hour, minute) = match offset {
        [h1, h2, b':', m1, m2] if [h1, h2, m1, m2].iter().all(|b| b.is_ascii_digit()) => {
            (number(&offset[0..2]), number(&offset[3..5]))
        }
        [h1, h2, m1, m2] if [h1, h2, m1, m2].iter().all(|b| b.is_ascii_digit()) => {
            (number(&offset[0..2]), number(&offset[2..4]))
        }
        _ => return false,
    };
    minute <= 59 && hour <= 14 && (hour < 14 || minute == 0)
}
pub fn parse_markdown(source: &str) -> Result<ParsedMarkdown, RecordError> {
    let parsed = frontmatter::parse_frontmatter(source)?;
    Ok(ParsedMarkdown {
        frontmatter: parsed.data,
        content: parsed.content.trim().to_owned(),
    })
}
pub fn parse_task(source: &str) -> Result<TaskRecord, RecordError> {
    let parsed = parse_markdown(source)?;
    let f = &parsed.frontmatter;
    let id = field(f, "id");
    let deps = f.get("dependencies");
    let bad = |index: Option<usize>| {
        RecordError(format!(
            "Invalid dependencies in task {}: {} contains a mapping or nested list. Use a list of task IDs in the task Markdown.",
            if id.is_empty() { "(missing id)" } else { &id },
            index.map_or_else(|| "the field".into(), |i| format!("entry {}", i + 1))
        ))
    };
    match deps {
        Some(Value::Object(_)) => return Err(bad(None)),
        Some(Value::Array(a)) => {
            if let Some(i) = a.iter().position(|v| v.is_array() || v.is_object()) {
                return Err(bad(Some(i)));
            }
        }
        _ => {}
    }
    let assignee = match f.get("assignee") {
        Some(Value::Array(a)) => a.iter().map(js_string).collect(),
        Some(v) if truthy(Some(v)) => vec![js_string(v)],
        _ => vec![],
    };
    let priority = optional(f, "priority")
        .map(|s| {
            s.split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase()
        })
        .filter(|s| !s.is_empty());
    let content = &parsed.content;
    Ok(TaskRecord {
        id,
        title: field(f, "title"),
        status: field(f, "status"),
        assignee,
        reporter: optional(f, "reporter"),
        created_date: date(f.get("created_date")),
        updated_date: optional(f, "updated_date").map(|_| date(f.get("updated_date"))),
        due_date: due_date(f.get("due_date"), "due_date")?,
        labels: strings(f.get("labels")),
        milestone: optional(f, "milestone"),
        dependencies: strings(deps),
        references: strings(f.get("references")),
        documentation: strings(f.get("documentation")),
        modified_files: strings(f.get("modified_files")),
        raw_content: content.clone(),
        acceptance_criteria_items: sections::parse_criteria(
            content,
            ChecklistKind::AcceptanceCriteria,
        ),
        definition_of_done_items: sections::parse_criteria(
            content,
            ChecklistKind::DefinitionOfDone,
        ),
        description: sections::extract_section(content, SectionKey::Description)
            .unwrap_or_default(),
        implementation_plan: sections::extract_section(content, SectionKey::ImplementationPlan),
        implementation_notes: sections::extract_section(content, SectionKey::ImplementationNotes),
        comments: sections::parse_comments(content),
        final_summary: sections::extract_section(content, SectionKey::FinalSummary),
        parent_task_id: optional(f, "parent_task_id"),
        subtasks: f
            .get("subtasks")
            .filter(|v| v.is_array())
            .map(|v| strings(Some(v))),
        priority,
        task_type: optional(f, "type"),
        project: optional(f, "project"),
        ordinal: f.get("ordinal").map(js_number),
        on_status_change: optional(f, "onStatusChange"),
    })
}
pub fn parse_decision(source: &str) -> Result<DecisionRecord, RecordError> {
    let p = parse_markdown(source)?;
    let f = &p.frontmatter;
    Ok(DecisionRecord {
        id: field(f, "id"),
        title: field(f, "title"),
        date: date(f.get("date")),
        updated_date: optional(f, "updated_date").map(|_| date(f.get("updated_date"))),
        superseded_by: optional(f, "superseded_by"),
        status: optional(f, "status").unwrap_or_else(|| "proposed".into()),
        context: sections::extract_section(&p.content, SectionKey::Context).unwrap_or_default(),
        decision: sections::extract_section(&p.content, SectionKey::Decision).unwrap_or_default(),
        consequences: sections::extract_section(&p.content, SectionKey::Consequences)
            .unwrap_or_default(),
        alternatives: sections::extract_section(&p.content, SectionKey::Alternatives),
        raw_content: p.content,
    })
}
pub fn parse_document(source: &str) -> Result<DocumentRecord, RecordError> {
    let p = parse_markdown(source)?;
    let f = &p.frontmatter;
    Ok(DocumentRecord {
        id: field(f, "id"),
        title: field(f, "title"),
        document_type: optional(f, "type").unwrap_or_else(|| "other".into()),
        created_date: date(f.get("created_date")),
        updated_date: optional(f, "updated_date").map(|_| date(f.get("updated_date"))),
        raw_content: p.content,
        tags: f
            .get("tags")
            .filter(|v| v.is_array())
            .map(|v| strings(Some(v))),
    })
}
pub fn parse_milestone(source: &str) -> Result<MilestoneRecord, RecordError> {
    let p = parse_markdown(source)?;
    let f = &p.frontmatter;
    let due_date = due_date(f.get("due_date"), "due_date")?;
    let description = extract_milestone_description(&p.content);
    Ok(MilestoneRecord {
        id: field(f, "id"),
        title: field(f, "title"),
        due_date,
        description,
        raw_content: p.content,
    })
}
fn js_number(value: &Value) -> f64 {
    match value {
        Value::Null => 0.0,
        Value::Bool(value) => f64::from(*value as u8),
        Value::Number(value) => value.as_f64().unwrap_or(f64::NAN),
        Value::String(value) if value.trim().is_empty() => 0.0,
        Value::String(value) => parse_js_number(value.trim()),
        Value::Array(_) => parse_js_number(&js_string(value)),
        Value::Object(_) => f64::NAN,
    }
}

fn parse_js_number(value: &str) -> f64 {
    if value.is_empty() {
        return 0.0;
    }
    for (prefix, radix) in [
        ("0x", 16),
        ("0X", 16),
        ("0b", 2),
        ("0B", 2),
        ("0o", 8),
        ("0O", 8),
    ] {
        if let Some(digits) = value.strip_prefix(prefix) {
            return u64::from_str_radix(digits, radix).map_or(f64::NAN, |number| number as f64);
        }
    }
    match value {
        "Infinity" | "+Infinity" => f64::INFINITY,
        "-Infinity" => f64::NEG_INFINITY,
        _ => value.parse().unwrap_or(f64::NAN),
    }
}

fn extract_milestone_description(content: &str) -> String {
    let lines: Vec<_> = content.lines().collect();
    let Some(start) = lines.iter().position(|line| {
        let line = line.trim_end_matches([' ', '\t']);
        line.eq_ignore_ascii_case("## Description")
    }) else {
        return String::new();
    };
    let end = lines[start + 1..]
        .iter()
        .position(|line| line.starts_with("## "))
        .map_or(lines.len(), |offset| start + 1 + offset);
    lines[start + 1..end].join("\n").trim().to_owned()
}
