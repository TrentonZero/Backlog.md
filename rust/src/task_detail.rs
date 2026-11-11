//! Shared read model for one task and its derived dependency context.

use crate::{
    dependency_graph::{DependencyGraph, build_dependency_graph},
    readiness::{ReadinessGraph, TaskReadiness},
    records::TaskRecord,
};

#[derive(Clone, Debug, PartialEq)]
pub struct TaskDetail {
    pub task: TaskRecord,
    pub dependency_graph: DependencyGraph,
    pub readiness: TaskReadiness,
}

/// Assemble the stored task and its derived relationships from one active and archived corpus.
pub fn build_task_detail(
    task: &TaskRecord,
    tasks: &[TaskRecord],
    archived_tasks: &[TaskRecord],
    statuses: &[String],
    ambiguous_ids: &[String],
    prefix: &str,
) -> TaskDetail {
    let dependency_graph =
        build_dependency_graph(task, tasks, archived_tasks, statuses, ambiguous_ids, prefix);
    let readiness = ReadinessGraph::new(tasks, archived_tasks, statuses, ambiguous_ids, prefix)
        .get_task_readiness(task);
    TaskDetail {
        task: task.clone(),
        dependency_graph,
        readiness,
    }
}
