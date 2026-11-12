//! Shared exact task field filters, separate from fuzzy text search.

use crate::{
    identity::task_ids_equal,
    records::{MilestoneRecord, TaskRecord},
};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LabelMatch {
    #[default]
    Any,
    All,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TaskFilter {
    pub statuses: Vec<String>,
    pub exclude_statuses: Vec<String>,
    pub types: Vec<String>,
    pub projects: Vec<String>,
    pub priorities: Vec<String>,
    pub assignees: Vec<String>,
    pub unassigned: bool,
    pub labels: Vec<String>,
    pub label_match: LabelMatch,
    pub modified_files: Vec<String>,
    pub milestone: Option<String>,
    pub parent_task_id: Option<String>,
}

/// Return matching task records in their original order.
///
/// Supply active and archived milestones together so aliases resolve across both stores.
/// Supply the full task corpus for stable fuzzy milestone resolution, even when `tasks` is a subset.
pub fn filter_tasks<'a>(
    tasks: &'a [TaskRecord],
    filter: &TaskFilter,
    prefix: &str,
    milestones: &[MilestoneRecord],
    milestone_corpus: &[TaskRecord],
) -> Vec<&'a TaskRecord> {
    let statuses = normalized_set(&filter.statuses, false);
    let excluded_statuses = normalized_set(&filter.exclude_statuses, false);
    let types = normalized_set(&filter.types, false);
    let projects = normalized_set(&filter.projects, false);
    let priorities = normalized_set(&filter.priorities, true);
    let assignees = normalized_set(&filter.assignees, false);
    let labels: Vec<_> = filter
        .labels
        .iter()
        .map(|label| label.trim().to_lowercase())
        .filter(|label| !label.is_empty())
        .collect();
    let modified_files = normalized_set(&filter.modified_files, false);
    let parent_id = filter
        .parent_task_id
        .as_deref()
        .filter(|parent_id| !parent_id.is_empty());
    let milestone_values = milestone_corpus
        .iter()
        .filter_map(|task| task.milestone.as_deref())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let milestone_matcher = filter
        .milestone
        .as_deref()
        .filter(|query| !query.trim().is_empty())
        .map(|query| {
            crate::milestone_filtering::MilestoneMatcher::new(query, milestones, &milestone_values)
        });

    tasks
        .iter()
        .filter(|task| {
            if !statuses.is_empty() && !statuses.contains(&task.status.to_lowercase()) {
                return false;
            }
            if excluded_statuses.contains(&task.status.to_lowercase()) {
                return false;
            }
            if !types.is_empty()
                && !task
                    .task_type
                    .as_deref()
                    .map(|value| value.trim().to_lowercase())
                    .is_some_and(|value| types.contains(&value))
            {
                return false;
            }
            if !projects.is_empty()
                && !task
                    .project
                    .as_deref()
                    .map(|value| value.trim().to_lowercase())
                    .is_some_and(|value| projects.contains(&value))
            {
                return false;
            }
            if !priorities.is_empty()
                && !task
                    .priority
                    .as_deref()
                    .map(normalize_priority)
                    .is_some_and(|value| priorities.contains(&value))
            {
                return false;
            }
            if !assignees.is_empty()
                && !task
                    .assignee
                    .iter()
                    .any(|value| assignees.contains(&value.trim().to_lowercase()))
            {
                return false;
            }
            if filter.unassigned && task.assignee.iter().any(|value| !value.trim().is_empty()) {
                return false;
            }
            if !labels.is_empty() {
                let task_labels: HashSet<_> = task
                    .labels
                    .iter()
                    .map(|label| label.trim().to_lowercase())
                    .filter(|label| !label.is_empty())
                    .collect();
                if task_labels.is_empty() {
                    return false;
                }
                let matches = match filter.label_match {
                    LabelMatch::Any => labels.iter().any(|label| task_labels.contains(label)),
                    LabelMatch::All => labels.iter().all(|label| task_labels.contains(label)),
                };
                if !matches {
                    return false;
                }
            }
            if !modified_files.is_empty()
                && !task.modified_files.iter().any(|path| {
                    let path = path.trim().to_lowercase();
                    !path.is_empty() && modified_files.iter().any(|filter| path.contains(filter))
                })
            {
                return false;
            }
            if let Some(parent_id) = parent_id
                && !task
                    .parent_task_id
                    .as_deref()
                    .is_some_and(|task_parent| task_ids_equal(parent_id, task_parent, prefix))
            {
                return false;
            }
            if milestone_matcher
                .as_ref()
                .is_some_and(|matcher| !matcher.matches(task.milestone.as_deref()))
            {
                return false;
            }
            true
        })
        .collect()
}

fn normalized_set(values: &[String], collapse_whitespace: bool) -> HashSet<String> {
    values
        .iter()
        .map(|value| {
            if collapse_whitespace {
                normalize_priority(value)
            } else {
                value.trim().to_lowercase()
            }
        })
        .filter(|value| !value.is_empty())
        .collect()
}

fn normalize_priority(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
