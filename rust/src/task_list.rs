//! Read-time task list projection with one readiness index for the full corpus.

use crate::{readiness::ReadinessGraph, records::TaskRecord};

#[derive(Clone, Debug, PartialEq)]
pub struct TaskListItem {
    pub task: TaskRecord,
    pub is_ready: bool,
}

/// Attach readiness to displayed tasks using the unfiltered active and archived corpora.
///
/// `corpus_tasks` must include active tasks hidden by display filters so their dependencies still resolve.
pub fn with_readiness(
    tasks: &[TaskRecord],
    corpus_tasks: &[TaskRecord],
    archived_tasks: &[TaskRecord],
    statuses: &[String],
    ambiguous_ids: &[String],
    prefix: &str,
) -> Vec<TaskListItem> {
    let graph = ReadinessGraph::new(
        corpus_tasks,
        archived_tasks,
        statuses,
        ambiguous_ids,
        prefix,
    );
    tasks
        .iter()
        .map(|task| TaskListItem {
            task: task.clone(),
            is_ready: graph.get_task_readiness(task).is_ready,
        })
        .collect()
}
