//! Read-time task readiness derived from the task and archived-task corpus.

use crate::{
    records::TaskRecord,
    task_record_index::{TaskRecordIndex, TaskRecordLookup},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskReadiness {
    pub is_ready: bool,
    pub is_blocked: bool,
    pub blocking_dependencies: Vec<String>,
    pub missing_dependencies: Vec<String>,
}

/// A reusable identity index for readiness checks over a task list or task detail view.
pub struct ReadinessGraph<'a> {
    index: TaskRecordIndex<'a>,
}

impl<'a> ReadinessGraph<'a> {
    /// Build an index from active tasks and archived tasks. Draft records are intentionally absent
    /// because task detail and readiness use the same task corpus as the TypeScript core.
    pub fn new(
        tasks: &'a [TaskRecord],
        archived_tasks: &'a [TaskRecord],
        statuses: &[String],
        ambiguous_ids: &[String],
        prefix: &str,
    ) -> Self {
        Self {
            index: TaskRecordIndex::new(tasks, archived_tasks, statuses, ambiguous_ids, prefix),
        }
    }

    /// Derive whether a task can start, preserving dependency order and spellings for unresolved IDs.
    pub fn get_task_readiness(&self, task: &TaskRecord) -> TaskReadiness {
        if self.is_archived_record(&task.id) || self.is_terminal_status(&task.status) {
            return TaskReadiness {
                is_ready: false,
                is_blocked: false,
                blocking_dependencies: Vec::new(),
                missing_dependencies: Vec::new(),
            };
        }

        let mut blocking_dependencies = Vec::new();
        let mut missing_dependencies = Vec::new();
        for dependency in &task.dependencies {
            match self.index.lookup(dependency) {
                TaskRecordLookup::Unique(record)
                    if self.is_terminal_status(&record.task.status) => {}
                TaskRecordLookup::Unique(record) => {
                    blocking_dependencies.push(record.task.id.clone())
                }
                TaskRecordLookup::Missing | TaskRecordLookup::Ambiguous => {
                    missing_dependencies.push(dependency.clone())
                }
            }
        }
        let is_blocked = !blocking_dependencies.is_empty() || !missing_dependencies.is_empty();
        TaskReadiness {
            is_ready: !is_blocked,
            is_blocked,
            blocking_dependencies,
            missing_dependencies,
        }
    }

    fn is_archived_record(&self, task_id: &str) -> bool {
        matches!(self.index.lookup(task_id), TaskRecordLookup::Unique(record) if record.archived)
    }

    fn is_terminal_status(&self, status: &str) -> bool {
        self.index.is_finished(status)
    }
}
