//! Shared identity and completion index for task relationship read models.

use crate::{identity::canonical_task_id, records::TaskRecord};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy)]
pub(crate) struct IndexedTaskRecord<'a> {
    pub task: &'a TaskRecord,
    pub archived: bool,
}

#[derive(Clone, Copy)]
pub(crate) enum TaskRecordLookup<'a> {
    Missing,
    Ambiguous,
    Unique(IndexedTaskRecord<'a>),
}

/// One canonical identity index shared by dependency graph and readiness reads.
pub(crate) struct TaskRecordIndex<'a> {
    records: Vec<IndexedTaskRecord<'a>>,
    by_identity: HashMap<String, Vec<IndexedTaskRecord<'a>>>,
    ambiguous_ids: HashSet<String>,
    terminal_status: Option<String>,
    prefix: String,
}

impl<'a> TaskRecordIndex<'a> {
    pub fn new(
        tasks: &'a [TaskRecord],
        archived_tasks: &'a [TaskRecord],
        statuses: &[String],
        ambiguous_ids: &[String],
        prefix: &str,
    ) -> Self {
        let records: Vec<_> = tasks
            .iter()
            .map(|task| IndexedTaskRecord {
                task,
                archived: false,
            })
            .chain(archived_tasks.iter().map(|task| IndexedTaskRecord {
                task,
                archived: true,
            }))
            .collect();
        let mut by_identity: HashMap<String, Vec<IndexedTaskRecord<'a>>> = HashMap::new();
        for record in &records {
            by_identity
                .entry(canonical_task_id(&record.task.id, prefix))
                .or_default()
                .push(*record);
        }
        let terminal_status = if statuses.is_empty() {
            Some("done".to_owned())
        } else {
            statuses
                .last()
                .map(|status| status.trim().to_lowercase())
                .filter(|status| !status.is_empty())
        };
        Self {
            records,
            by_identity,
            ambiguous_ids: ambiguous_ids
                .iter()
                .map(|id| canonical_task_id(id, prefix))
                .collect(),
            terminal_status,
            prefix: prefix.to_owned(),
        }
    }

    pub fn records(&self) -> &[IndexedTaskRecord<'a>] {
        &self.records
    }

    pub fn key(&self, id: &str) -> String {
        canonical_task_id(id, &self.prefix)
    }

    pub fn lookup(&self, id: &str) -> TaskRecordLookup<'a> {
        let key = self.key(id);
        if self.ambiguous_ids.contains(&key) {
            return TaskRecordLookup::Ambiguous;
        }
        match self.by_identity.get(&key).map(Vec::as_slice) {
            None | Some([]) => TaskRecordLookup::Missing,
            Some([record]) => TaskRecordLookup::Unique(*record),
            Some(_) => TaskRecordLookup::Ambiguous,
        }
    }

    pub fn is_finished(&self, status: &str) -> bool {
        self.terminal_status
            .as_deref()
            .is_some_and(|terminal| status.trim().eq_ignore_ascii_case(terminal))
    }
}
