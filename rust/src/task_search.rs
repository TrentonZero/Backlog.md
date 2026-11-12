//! Canonical searchable text and fields shared by task search surfaces.

use crate::{
    fuzzy::{bitap_search, field_norm},
    records::{MilestoneRecord, TaskRecord},
    task_filtering::{TaskFilter, TaskFilterMatcher},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskSearchFields {
    pub title: String,
    pub body_text: String,
    pub id: String,
    pub id_variants: Vec<String>,
    pub dependency_ids: Vec<String>,
    pub modified_files: Vec<String>,
}

/// In-memory task search index with Fuse-compatible relevance order.
pub struct TaskSearchIndex<'a> {
    tasks: &'a [TaskRecord],
    milestones: &'a [MilestoneRecord],
    fields: Vec<TaskSearchFields>,
}

impl<'a> TaskSearchIndex<'a> {
    pub fn new(tasks: &'a [TaskRecord], milestones: &'a [MilestoneRecord]) -> Self {
        Self {
            tasks,
            milestones,
            fields: tasks.iter().map(build_task_search_fields).collect(),
        }
    }

    /// Search indexed task text, then apply shared exact filters to the ranked results.
    pub fn search(&self, query: &str, filter: &TaskFilter, prefix: &str) -> Vec<&'a TaskRecord> {
        let matcher = TaskFilterMatcher::new(filter, prefix, self.milestones, self.tasks);
        let query = query.trim();
        if query.is_empty() {
            return self
                .tasks
                .iter()
                .filter(|task| matcher.matches(task))
                .collect();
        }

        let mut ranked = self
            .fields
            .iter()
            .enumerate()
            .filter_map(|(index, fields)| {
                search_fields_score(query, fields).map(|score| (score, index))
            })
            .collect::<Vec<_>>();
        ranked.sort_by(|left, right| left.0.total_cmp(&right.0).then(left.1.cmp(&right.1)));
        ranked
            .into_iter()
            .filter_map(|(_, index)| {
                let task = &self.tasks[index];
                matcher.matches(task).then_some(task)
            })
            .collect()
    }
}

/// Build the body text indexed alongside a task title and identity.
pub fn build_task_search_body_text(task: &TaskRecord) -> String {
    let mut parts = Vec::new();
    if !task.description.is_empty() {
        parts.push(task.description.clone());
    }
    if !task.acceptance_criteria_items.is_empty() {
        let mut criteria = task.acceptance_criteria_items.iter().collect::<Vec<_>>();
        criteria.sort_by_key(|criterion| criterion.index);
        parts.push(
            criteria
                .iter()
                .map(|criterion| {
                    format!(
                        "- [{}] {}",
                        if criterion.checked { "x" } else { " " },
                        criterion.text
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    if let Some(plan) = task
        .implementation_plan
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        parts.push(plan.to_owned());
    }
    if let Some(notes) = task
        .implementation_notes
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        parts.push(notes.to_owned());
    }
    if !task.comments.is_empty() {
        parts.push(
            task.comments
                .iter()
                .map(|comment| comment.body.as_str())
                .collect::<Vec<_>>()
                .join("\n\n"),
        );
    }
    if !task.labels.is_empty() {
        parts.push(task.labels.join("\n"));
    }
    if !task.assignee.is_empty() {
        parts.push(task.assignee.join("\n"));
    }
    if !task.modified_files.is_empty() {
        parts.push(task.modified_files.join("\n"));
    }
    parts.join("\n\n")
}

/// Build each indexed field from one parsed task record.
pub fn build_task_search_fields(task: &TaskRecord) -> TaskSearchFields {
    TaskSearchFields {
        title: task.title.clone(),
        body_text: build_task_search_body_text(task),
        id: task.id.clone(),
        id_variants: task_id_search_variants(&task.id),
        dependency_ids: task
            .dependencies
            .iter()
            .flat_map(|dependency| task_id_search_variants(dependency))
            .collect(),
        modified_files: task.modified_files.clone(),
    }
}

/// Score one set of fields with the shared Fuse-compatible task search weights.
/// Documents and decisions use this same projection with their unused task fields empty.
pub fn search_fields_score(query: &str, fields: &TaskSearchFields) -> Option<f64> {
    let values = [
        (std::slice::from_ref(&fields.title), 0.35),
        (std::slice::from_ref(&fields.body_text), 0.3),
        (std::slice::from_ref(&fields.id), 0.2),
        (&fields.id_variants, 0.1),
        (&fields.dependency_ids, 0.05),
        (&fields.modified_files, 0.15),
    ];
    let total_weight = values.iter().map(|(_, weight)| *weight).sum::<f64>();
    let mut total_score = 1.0;
    let mut matched = false;

    for (texts, weight) in values {
        for text in texts {
            if text.trim().is_empty() {
                continue;
            }
            if let Some(score) = bitap_search(query, text, 0.35, 1) {
                matched = true;
                let score = if score == 0.0 { f64::EPSILON } else { score };
                total_score *= score.powf((weight / total_weight) * field_norm(text));
            }
        }
    }

    matched.then_some(total_score)
}

fn task_id_search_variants(id: &str) -> Vec<String> {
    let lower_id = id.to_lowercase();
    let prefix_end = id
        .find('-')
        .filter(|index| *index > 0 && id[..*index].bytes().all(|byte| byte.is_ascii_alphabetic()));
    let suffix = prefix_end
        .map_or(id, |index| &id[index + 1..])
        .to_lowercase();
    let segments = suffix
        .split('.')
        .map(canonical_numeric_segment)
        .collect::<Option<Vec<_>>>();
    let Some(segments) = segments else {
        if id == lower_id {
            return vec![id.to_owned()];
        }
        return vec![id.to_owned(), lower_id];
    };

    let canonical_suffix = segments.join(".");
    let prefix = prefix_end.map_or("task-", |index| &id[..=index]);
    let mut variants = Vec::new();
    for variant in [
        id.to_owned(),
        lower_id,
        format!("{prefix}{canonical_suffix}"),
        format!("{}{canonical_suffix}", prefix.to_lowercase()),
        canonical_suffix,
    ] {
        push_unique(&mut variants, variant);
    }
    for segment in segments {
        push_unique(&mut variants, segment);
    }
    variants
}

fn canonical_numeric_segment(segment: &str) -> Option<String> {
    if segment.is_empty() || !segment.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let number = segment.parse::<f64>().unwrap_or(f64::INFINITY);
    Some(javascript_number_string(number))
}

fn javascript_number_string(number: f64) -> String {
    if number.is_infinite() {
        return "Infinity".to_owned();
    }

    let decimal = number.to_string();
    if number < 1e21 {
        return decimal;
    }

    let significant = decimal.trim_end_matches('0');
    let exponent = decimal.len() - 1;
    if significant.len() == 1 {
        format!("{}e+{exponent}", significant)
    } else {
        format!("{}.{}e+{exponent}", &significant[..1], &significant[1..])
    }
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
    }
}
