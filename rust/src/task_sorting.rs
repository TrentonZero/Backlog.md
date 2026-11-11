//! Canonical task list ordering and status grouping.

use crate::{records::TaskRecord, task_store::compare_task_ids};
use std::cmp::Ordering;

pub const TASK_SORT_FIELDS: [&str; 3] = ["priority", "id", "ordinal"];

#[derive(Clone, Debug, PartialEq)]
pub struct TaskSection {
    pub status: Option<String>,
    pub tasks: Vec<TaskRecord>,
}

/// Sort a task list using the default order or one of the named list orders.
pub fn sort_tasks(
    tasks: &[TaskRecord],
    sort_field: Option<&str>,
    priority_order: &[String],
) -> Vec<TaskRecord> {
    let mut sorted = tasks.to_vec();
    let field = sort_field.unwrap_or_default().to_lowercase();
    let priority_order = normalized_priority_order(priority_order);
    let compare = |left: &TaskRecord, right: &TaskRecord| match field.as_str() {
        "id" => compare_task_ids(&left.id, &right.id),
        "ordinal" => compare_ordinal_or(left, right, || compare_task_ids(&left.id, &right.id)),
        "priority" => compare_priority_rank(left, right, &priority_order)
            .then_with(|| compare_task_ids(&left.id, &right.id)),
        _ => compare_ordinal_or(left, right, || {
            compare_priority_rank(left, right, &priority_order)
                .then_with(|| compare_task_ids(&left.id, &right.id))
        }),
    };
    let contains_nan_ordinal = sorted
        .iter()
        .any(|task| task.ordinal.is_some_and(f64::is_nan));
    if contains_nan_ordinal && matches!(field.as_str(), "" | "ordinal") {
        // JavaScript treats a NaN comparator result as equality. Keep that stable behavior without
        // giving Rust's general-purpose sort a non-transitive comparator.
        stable_insertion_sort(&mut sorted, compare);
    } else {
        sorted.sort_by(compare);
    }
    sorted
}

fn stable_insertion_sort<T>(items: &mut [T], compare: impl Fn(&T, &T) -> Ordering) {
    for index in 1..items.len() {
        let mut low = 0;
        let mut high = index;
        while low < high {
            let middle = low + (high - low) / 2;
            if compare(&items[index], &items[middle]) == Ordering::Less {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        items[low..=index].rotate_right(1);
    }
}

/// Group an already ordered list by status, keeping each group's input order.
/// Priority order is intentionally flat, matching the CLI and other list surfaces.
pub fn section_tasks(
    tasks: &[TaskRecord],
    sort_field: Option<&str>,
    statuses: &[String],
) -> Vec<TaskSection> {
    if sort_field.is_some_and(|field| field.eq_ignore_ascii_case("priority")) {
        return vec![TaskSection {
            status: None,
            tasks: tasks.to_vec(),
        }];
    }

    let mut groups: Vec<(String, Vec<TaskRecord>)> = Vec::new();
    for task in tasks {
        let raw_status = task.status.trim();
        let status = statuses
            .iter()
            .find(|status| status.eq_ignore_ascii_case(raw_status))
            .map_or(raw_status, String::as_str)
            .to_owned();
        if let Some((_, group)) = groups.iter_mut().find(|(name, _)| *name == status) {
            group.push(task.clone());
        } else {
            groups.push((status, vec![task.clone()]));
        }
    }

    let mut ordered = Vec::with_capacity(groups.len());
    for status in statuses {
        if let Some(index) = groups.iter().position(|(name, _)| name == status) {
            ordered.push(groups.remove(index));
        }
    }
    ordered.extend(groups);
    ordered
        .into_iter()
        .map(|(status, tasks)| TaskSection {
            status: Some(status),
            tasks,
        })
        .collect()
}

fn compare_ordinal(left: &TaskRecord, right: &TaskRecord) -> Option<Ordering> {
    match (left.ordinal, right.ordinal) {
        (Some(_), None) => Some(Ordering::Less),
        (None, Some(_)) => Some(Ordering::Greater),
        (Some(left), Some(right)) if left != right => left.partial_cmp(&right),
        _ => Some(Ordering::Equal),
    }
}

fn compare_ordinal_or(
    left: &TaskRecord,
    right: &TaskRecord,
    equal: impl FnOnce() -> Ordering,
) -> Ordering {
    match compare_ordinal(left, right) {
        Some(Ordering::Equal) => equal(),
        Some(order) => order,
        None => Ordering::Equal,
    }
}

fn compare_priority_rank(left: &TaskRecord, right: &TaskRecord, order: &[String]) -> Ordering {
    let left_rank = priority_rank(left.priority.as_deref(), order);
    let right_rank = priority_rank(right.priority.as_deref(), order);
    right_rank.cmp(&left_rank)
}

fn normalized_priority_order(configured_order: &[String]) -> Vec<String> {
    let configured_order = if configured_order.is_empty() {
        ["high", "medium", "low"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    } else {
        configured_order.to_vec()
    };
    let mut order = Vec::new();
    for priority in configured_order {
        let priority = normalize_priority(&priority);
        if !priority.is_empty() && !order.contains(&priority) {
            order.push(priority);
        }
    }
    if order.is_empty() {
        ["high", "medium", "low"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    } else {
        order
    }
}

fn priority_rank(priority: Option<&str>, order: &[String]) -> usize {
    let Some(priority) = priority.map(normalize_priority) else {
        return 0;
    };
    order
        .iter()
        .position(|configured| configured == &priority)
        .map_or(0, |index| order.len() - index)
}

fn normalize_priority(priority: &str) -> String {
    priority
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
