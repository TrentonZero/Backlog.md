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
    let matcher = TaskFilterMatcher::new(filter, prefix, milestones, milestone_corpus);
    tasks.iter().filter(|task| matcher.matches(task)).collect()
}

pub(crate) struct TaskFilterMatcher<'a> {
    statuses: HashSet<String>,
    excluded_statuses: HashSet<String>,
    types: HashSet<String>,
    projects: HashSet<String>,
    priorities: HashSet<String>,
    assignees: HashSet<String>,
    unassigned: bool,
    labels: Vec<String>,
    label_match: LabelMatch,
    modified_files: HashSet<String>,
    parent_id: Option<String>,
    prefix: String,
    milestone: Option<crate::milestone_filtering::MilestoneMatcher<'a>>,
}

impl<'a> TaskFilterMatcher<'a> {
    pub(crate) fn new(
        filter: &TaskFilter,
        prefix: &str,
        milestones: &'a [MilestoneRecord],
        milestone_corpus: &[TaskRecord],
    ) -> Self {
        let milestone_values = milestone_corpus
            .iter()
            .filter_map(|task| task.milestone.as_deref())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let milestone = filter
            .milestone
            .as_deref()
            .filter(|query| !query.trim().is_empty())
            .map(|query| {
                crate::milestone_filtering::MilestoneMatcher::new(
                    query,
                    milestones,
                    &milestone_values,
                )
            });
        Self {
            statuses: normalized_set(&filter.statuses, false),
            excluded_statuses: normalized_set(&filter.exclude_statuses, false),
            types: normalized_set(&filter.types, false),
            projects: normalized_set(&filter.projects, false),
            priorities: normalized_set(&filter.priorities, true),
            assignees: normalized_set(&filter.assignees, false),
            unassigned: filter.unassigned,
            labels: filter
                .labels
                .iter()
                .map(|label| label.trim().to_lowercase())
                .filter(|label| !label.is_empty())
                .collect(),
            label_match: filter.label_match,
            modified_files: normalized_set(&filter.modified_files, false),
            parent_id: filter
                .parent_task_id
                .as_deref()
                .filter(|parent_id| !parent_id.is_empty())
                .map(str::to_owned),
            prefix: prefix.to_owned(),
            milestone,
        }
    }

    pub(crate) fn matches(&self, task: &TaskRecord) -> bool {
        if !self.statuses.is_empty() && !self.statuses.contains(&task.status.to_lowercase()) {
            return false;
        }
        if self.excluded_statuses.contains(&task.status.to_lowercase()) {
            return false;
        }
        if !self.types.is_empty()
            && !task
                .task_type
                .as_deref()
                .map(|value| value.trim().to_lowercase())
                .is_some_and(|value| self.types.contains(&value))
        {
            return false;
        }
        if !self.projects.is_empty()
            && !task
                .project
                .as_deref()
                .map(|value| value.trim().to_lowercase())
                .is_some_and(|value| self.projects.contains(&value))
        {
            return false;
        }
        if !self.priorities.is_empty()
            && !task
                .priority
                .as_deref()
                .map(normalize_priority)
                .is_some_and(|value| self.priorities.contains(&value))
        {
            return false;
        }
        if !self.assignees.is_empty()
            && !task
                .assignee
                .iter()
                .any(|value| self.assignees.contains(&value.trim().to_lowercase()))
        {
            return false;
        }
        if self.unassigned && task.assignee.iter().any(|value| !value.trim().is_empty()) {
            return false;
        }
        if !self.labels.is_empty() {
            let task_labels: HashSet<_> = task
                .labels
                .iter()
                .map(|label| label.trim().to_lowercase())
                .filter(|label| !label.is_empty())
                .collect();
            if task_labels.is_empty() {
                return false;
            }
            let matches = match self.label_match {
                LabelMatch::Any => self.labels.iter().any(|label| task_labels.contains(label)),
                LabelMatch::All => self.labels.iter().all(|label| task_labels.contains(label)),
            };
            if !matches {
                return false;
            }
        }
        if !self.modified_files.is_empty()
            && !task.modified_files.iter().any(|path| {
                let path = path.trim().to_lowercase();
                !path.is_empty()
                    && self
                        .modified_files
                        .iter()
                        .any(|filter| path.contains(filter))
            })
        {
            return false;
        }
        if let Some(parent_id) = &self.parent_id
            && !task
                .parent_task_id
                .as_deref()
                .is_some_and(|task_parent| task_ids_equal(parent_id, task_parent, &self.prefix))
        {
            return false;
        }
        if self
            .milestone
            .as_ref()
            .is_some_and(|matcher| !matcher.matches(task.milestone.as_deref()))
        {
            return false;
        }
        true
    }
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
