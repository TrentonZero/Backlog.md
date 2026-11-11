//! Shared identity, missing-reference, and cycle validation for task dependencies.

use crate::{
    draft_store::DraftStore,
    identity::{canonical_task_id, task_ids_equal},
    records::TaskRecord,
    task_store::{TaskStore, TaskStoreError},
};
use std::collections::{HashMap, HashSet, VecDeque};

pub(crate) fn validate_task_dependencies(
    tasks: &TaskStore,
    drafts: &DraftStore,
    dependencies: &[String],
    target: &TaskRecord,
    prefix: &str,
) -> Result<Vec<String>, TaskStoreError> {
    if dependencies.is_empty() {
        return Ok(Vec::new());
    }
    let corpus: Vec<(TaskRecord, String)> = tasks
        .list_tasks()
        .into_iter()
        .chain(tasks.list_archived_tasks())
        .map(|entry| (entry.task, entry.path.display().to_string()))
        .chain(
            drafts
                .list_drafts()
                .into_iter()
                .map(|entry| (entry.task, entry.path.display().to_string())),
        )
        .collect();
    let mut valid = Vec::new();
    let mut invalid = Vec::new();
    for dependency in dependencies {
        let matches: Vec<_> = corpus
            .iter()
            .filter(|(record, _)| task_ids_equal(dependency, &record.id, prefix))
            .collect();
        if matches.len() > 1 {
            let paths = matches.iter().map(|(_, path)| path.clone()).collect();
            let canonical: HashSet<_> = matches
                .iter()
                .map(|(record, _)| canonical_task_id(&record.id, prefix))
                .collect();
            if canonical.len() == 1 {
                return Err(TaskStoreError::Ambiguous(
                    crate::task_store::AmbiguousTaskIdError::new(dependency, paths),
                ));
            }
            let mut paths = paths;
            paths.sort();
            return Err(TaskStoreError::Dependencies(format!(
                "Dependency ID {} is ambiguous; {} files match:\n{}\nUse a full task ID instead of {} to choose one.",
                dependency.trim(),
                paths.len(),
                paths
                    .iter()
                    .map(|path| format!("  - {path}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
                dependency.trim()
            )));
        }
        let Some((record, _)) = matches.first() else {
            if task_ids_equal(dependency, &target.id, prefix) {
                return Err(self_dependency(&target.id, dependency));
            }
            invalid.push(dependency.clone());
            continue;
        };
        if task_ids_equal(&record.id, &target.id, prefix) {
            return Err(self_dependency(&target.id, dependency));
        }
        if !valid
            .iter()
            .any(|existing: &String| task_ids_equal(existing, &record.id, prefix))
        {
            valid.push(record.id.clone());
        }
    }
    if !invalid.is_empty() {
        return Err(TaskStoreError::Dependencies(format!(
            "The following dependencies do not exist: {}. Please create these tasks first or verify the IDs. Task lookups read only the local working copy; use 'backlog browser' to see tasks from other branches.",
            invalid.join(", ")
        )));
    }
    check_cycle(tasks, drafts, target, &valid, prefix)?;
    Ok(valid)
}

fn self_dependency(target_id: &str, dependency: &str) -> TaskStoreError {
    TaskStoreError::Dependencies(format!(
        "Task {target_id} cannot depend on itself (\"{}\" names this task).",
        dependency.trim()
    ))
}

fn check_cycle(
    tasks: &TaskStore,
    drafts: &DraftStore,
    target: &TaskRecord,
    proposed: &[String],
    prefix: &str,
) -> Result<(), TaskStoreError> {
    let root = canonical_task_id(&target.id, prefix);
    let mut corpus: Vec<TaskRecord> = tasks
        .list_tasks()
        .into_iter()
        .chain(tasks.list_archived_tasks())
        .map(|entry| entry.task)
        .chain(drafts.list_drafts().into_iter().map(|entry| entry.task))
        .filter(|record| canonical_task_id(&record.id, prefix) != root)
        .collect();
    let mut root_record = target.clone();
    root_record.dependencies = proposed.to_vec();
    corpus.push(root_record);
    let mut nodes: HashMap<String, Vec<TaskRecord>> = HashMap::new();
    for record in corpus {
        nodes
            .entry(canonical_task_id(&record.id, prefix))
            .or_default()
            .push(record);
    }
    let mut queue = VecDeque::from([vec![root]]);
    let mut visited = HashSet::new();
    visited.insert(canonical_task_id(&target.id, prefix));
    let mut ambiguous_reachable = None;
    while let Some(path) = queue.pop_front() {
        let Some(current) = path.last() else {
            continue;
        };
        let Some(records) = nodes.get(current) else {
            continue;
        };
        if records.len() != 1 {
            if ambiguous_reachable.is_none() {
                ambiguous_reachable = Some(current.clone());
            }
            continue;
        }
        for dependency in &records[0].dependencies {
            let next = canonical_task_id(dependency, prefix);
            if next == canonical_task_id(&target.id, prefix) && current != &next {
                let mut cycle = path
                    .iter()
                    .map(|id| display_id(id, &nodes))
                    .collect::<Vec<_>>();
                cycle.push(target.id.clone());
                return Err(TaskStoreError::Dependencies(format!(
                    "These dependencies would create a cycle: {}",
                    cycle.join(" -> ")
                )));
            }
            if nodes.contains_key(&next) && visited.insert(next.clone()) {
                let mut next_path = path.clone();
                next_path.push(next);
                queue.push_back(next_path);
            }
        }
    }
    if let Some(id) = ambiguous_reachable {
        return Err(TaskStoreError::Dependencies(format!(
            "Cannot verify the dependencies stay acyclic: more than one record claims {id}. Run 'backlog doctor' to repair duplicate IDs first."
        )));
    }
    Ok(())
}

fn display_id(key: &str, nodes: &HashMap<String, Vec<TaskRecord>>) -> String {
    nodes
        .get(key)
        .and_then(|records| records.first())
        .map_or_else(|| key.to_owned(), |record| record.id.clone())
}
