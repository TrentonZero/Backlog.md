//! Pure serializers for typed Markdown records.
use serde_json::{Value, json};
use std::{collections::BTreeMap, error::Error, fmt};

use crate::{
    frontmatter::{self, FrontmatterError},
    records::{DecisionRecord, DocumentRecord, TaskRecord},
    sections::{self, ChecklistKind, SectionKey},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SerializeError(String);

impl fmt::Display for SerializeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for SerializeError {}

impl From<FrontmatterError> for SerializeError {
    fn from(error: FrontmatterError) -> Self {
        Self(error.to_string())
    }
}

impl From<sections::SectionError> for SerializeError {
    fn from(error: sections::SectionError) -> Self {
        Self(error.to_string())
    }
}

type Changes = BTreeMap<String, Option<Value>>;

fn optional_text(value: Option<&String>) -> Option<Value> {
    value
        .filter(|text| !text.is_empty())
        .map(|text| json!(text))
}

fn fields_with_source(
    fields: Vec<(&str, Option<Value>)>,
    source: &str,
    body: &str,
) -> Result<String, SerializeError> {
    let changes: Changes = fields
        .iter()
        .map(|(key, value)| ((*key).to_owned(), value.clone()))
        .collect();
    let updated = if source.is_empty() {
        new_frontmatter(&fields, body)?
    } else {
        let updated = frontmatter::update_frontmatter(source, &changes)?;
        replace_body(&updated, body)?
    };
    Ok(updated)
}

fn new_frontmatter(fields: &[(&str, Option<Value>)], body: &str) -> Result<String, SerializeError> {
    let mut header = String::from("---\n");
    for (key, value) in fields {
        if let Some(value) = value {
            let line = serde_yaml_ng::to_string(&BTreeMap::from([(*key, value)]))
                .map_err(|error| SerializeError(error.to_string()))?;
            header.push_str(line.trim_end());
            header.push('\n');
        }
    }
    header.push_str("---\n");
    header.push_str(body);
    Ok(header)
}

fn replace_body(source: &str, body: &str) -> Result<String, SerializeError> {
    let parsed = frontmatter::parse_frontmatter(source)?;
    let header_length = source.len() - parsed.content.len();
    Ok(format!("{}{body}", &source[..header_length]))
}

fn update_field(body: &str, key: SectionKey, value: &str) -> Result<String, SerializeError> {
    Ok(sections::update_sections(
        body,
        &BTreeMap::from([(key, Some(value.to_owned()))]),
    )?)
}

fn section_changed(body: &str, key: SectionKey, value: &str) -> bool {
    let previous = sections::extract_section(body, key);
    let previous = previous.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let value = value.trim();
    previous != (!value.is_empty()).then_some(value)
}

fn task_fields(task: &TaskRecord) -> Vec<(&'static str, Option<Value>)> {
    vec![
        ("id", Some(json!(task.id))),
        ("title", Some(json!(task.title))),
        ("status", Some(json!(task.status))),
        ("assignee", Some(json!(task.assignee))),
        ("reporter", optional_text(task.reporter.as_ref())),
        ("created_date", Some(json!(task.created_date))),
        ("updated_date", optional_text(task.updated_date.as_ref())),
        ("due_date", optional_text(task.due_date.as_ref())),
        ("labels", Some(json!(task.labels))),
        ("milestone", optional_text(task.milestone.as_ref())),
        ("dependencies", Some(json!(task.dependencies))),
        (
            "references",
            (!task.references.is_empty()).then(|| json!(task.references)),
        ),
        (
            "documentation",
            (!task.documentation.is_empty()).then(|| json!(task.documentation)),
        ),
        (
            "modified_files",
            (!task.modified_files.is_empty()).then(|| json!(task.modified_files)),
        ),
        (
            "parent_task_id",
            optional_text(task.parent_task_id.as_ref()),
        ),
        (
            "subtasks",
            task.subtasks
                .as_ref()
                .filter(|v| !v.is_empty())
                .map(|v| json!(v)),
        ),
        ("priority", optional_text(task.priority.as_ref())),
        ("type", optional_text(task.task_type.as_ref())),
        ("project", optional_text(task.project.as_ref())),
        (
            "ordinal",
            task.ordinal
                .filter(|value| value.is_finite())
                .map(|value| json!(value)),
        ),
        (
            "onStatusChange",
            optional_text(task.on_status_change.as_ref()),
        ),
    ]
}

/// Serialize task metadata and managed body edits, retaining unknown source fields.
pub fn serialize_task(task: &TaskRecord, original_source: &str) -> Result<String, SerializeError> {
    let original_body = task.raw_content.as_str();
    let mut body = original_body.to_owned();
    if section_changed(original_body, SectionKey::Description, &task.description) {
        body = update_field(&body, SectionKey::Description, &task.description)?;
    }
    for (kind, items) in [
        (
            ChecklistKind::AcceptanceCriteria,
            task.acceptance_criteria_items.as_slice(),
        ),
        (
            ChecklistKind::DefinitionOfDone,
            task.definition_of_done_items.as_slice(),
        ),
    ] {
        let existing = sections::parse_criteria(original_body, kind);
        if items.is_empty() || existing != items {
            sections::assert_valid_checklist_marks(&body, kind)?;
            body = sections::update_criteria(&body, kind, items)?;
        }
    }
    for (key, value) in [
        (
            SectionKey::ImplementationPlan,
            task.implementation_plan.as_deref(),
        ),
        (
            SectionKey::ImplementationNotes,
            task.implementation_notes.as_deref(),
        ),
        (SectionKey::FinalSummary, task.final_summary.as_deref()),
    ] {
        if let Some(value) = value
            && section_changed(original_body, key, value)
        {
            body = update_field(&body, key, value)?;
        }
    }
    let existing_comments = sections::parse_comments(original_body);
    if (!task.comments.is_empty() || !existing_comments.is_empty())
        && existing_comments != task.comments
    {
        body = sections::update_comments(&body, &task.comments)?;
    }
    let mut output = fields_with_source(task_fields(task), original_source, &body)?;
    if let Some(ordinal) = task.ordinal.filter(|value| !value.is_finite()) {
        let scalar = if ordinal.is_nan() {
            ".nan"
        } else if ordinal.is_sign_positive() {
            ".inf"
        } else {
            "-.inf"
        };
        output = frontmatter::update_frontmatter_scalar(&output, "ordinal", scalar)?;
    }
    if !body.is_empty() {
        let parsed = frontmatter::parse_frontmatter(&output)?;
        let body_start = output.len() - parsed.content.len();
        if !output[body_start..].starts_with(['\n', '\r']) {
            output.insert(
                body_start,
                if output[..body_start].ends_with("\r\n") {
                    '\r'
                } else {
                    '\n'
                },
            );
            if output[body_start..].starts_with('\r') {
                output.insert(body_start + 1, '\n');
            }
        }
    }
    Ok(output)
}

fn decision_fields(decision: &DecisionRecord) -> Vec<(&'static str, Option<Value>)> {
    vec![
        ("id", Some(json!(decision.id))),
        ("title", Some(json!(decision.title))),
        ("date", Some(json!(decision.date))),
        ("status", Some(json!(decision.status))),
        (
            "updated_date",
            decision.updated_date.as_ref().map(|v| json!(v)),
        ),
        (
            "superseded_by",
            decision.superseded_by.as_ref().map(|v| json!(v)),
        ),
    ]
}

/// Serialize a decision, changing only metadata and managed sections that differ.
pub fn serialize_decision(
    decision: &DecisionRecord,
    original_source: &str,
) -> Result<String, SerializeError> {
    let parsed_source = if original_source.is_empty() {
        None
    } else {
        Some(frontmatter::parse_frontmatter(original_source)?)
    };
    let original_body = parsed_source
        .as_ref()
        .map_or(decision.raw_content.as_str(), |parsed| {
            parsed.content.as_str()
        });
    let fields = [
        (SectionKey::Context, Some(decision.context.as_str())),
        (SectionKey::Decision, Some(decision.decision.as_str())),
        (
            SectionKey::Consequences,
            Some(decision.consequences.as_str()),
        ),
        (SectionKey::Alternatives, decision.alternatives.as_deref()),
    ];
    let mut section_changes = BTreeMap::new();
    if original_source.is_empty() {
        for (key, value) in fields {
            if let Some(value) = value {
                section_changes.insert(key, Some(value.to_owned()));
            }
        }
    } else {
        for (key, value) in fields {
            if let Some(value) = value
                && sections::extract_section(original_body, key).unwrap_or_default() != value
            {
                section_changes.insert(key, Some(value.to_owned()));
            }
        }
    }
    let source_without_bom = original_source
        .strip_prefix('\u{feff}')
        .unwrap_or(original_source);
    let has_frontmatter =
        source_without_bom.starts_with("---\n") || source_without_bom.starts_with("---\r\n");
    let starts_crlf_header = source_without_bom.starts_with("---\r\n");
    let newline = if (has_frontmatter && starts_crlf_header)
        || (!has_frontmatter && original_body.contains("\r\n"))
    {
        "\r\n"
    } else {
        "\n"
    };
    let body =
        sections::update_decision_sections_with_newline(original_body, &section_changes, newline)?;
    let fields = decision_fields(decision);
    if !original_source.is_empty() {
        let old = crate::records::parse_decision(original_source)
            .map_err(|error| SerializeError(error.to_string()))?;
        let old_fields = decision_fields(&old);
        let changes: Changes = fields
            .into_iter()
            .zip(old_fields)
            .filter(|((_, next), (_, previous))| next != previous)
            .map(|((key, next), _)| (key.to_owned(), next))
            .collect();
        let updated = frontmatter::update_frontmatter(original_source, &changes)?;
        replace_body(&updated, &body)
    } else {
        fields_with_source(fields, "", &body)
    }
}

/// Serialize document metadata and body while preserving unknown frontmatter.
pub fn serialize_document(
    document: &DocumentRecord,
    original_source: &str,
) -> Result<String, SerializeError> {
    let fields = vec![
        ("id", Some(json!(document.id))),
        ("title", Some(json!(document.title))),
        ("type", Some(json!(document.document_type))),
        ("created_date", Some(json!(document.created_date))),
        (
            "updated_date",
            optional_text(document.updated_date.as_ref()),
        ),
        (
            "tags",
            document
                .tags
                .as_ref()
                .filter(|v| !v.is_empty())
                .map(|v| json!(v)),
        ),
    ];
    fields_with_source(fields, original_source, &document.raw_content)
}
