//! Read-time dependency relationships shared by task detail surfaces.

use crate::{
    identity::canonical_task_id,
    records::TaskRecord,
    task_record_index::{TaskRecordIndex, TaskRecordLookup},
    task_store::compare_task_ids,
};
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyNodeState {
    Resolved,
    Ambiguous,
    Missing,
}

impl DependencyNodeState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Resolved => "resolved",
            Self::Ambiguous => "ambiguous",
            Self::Missing => "missing",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyGraphNode {
    pub id: String,
    pub title: Option<String>,
    pub status: Option<String>,
    pub state: DependencyNodeState,
    pub completed: bool,
    pub dependency_depth: Option<usize>,
    pub dependent_depth: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyGraphEdge {
    pub from: String,
    pub to: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyGraph {
    pub root_id: String,
    pub nodes: Vec<DependencyGraphNode>,
    pub edges: Vec<DependencyGraphEdge>,
}

/// Build the dependency context around one task. Drafts belong in `tasks`; archived records retain
/// their identities and completion status but are listed separately to callers.
pub fn build_dependency_graph(
    root: &TaskRecord,
    tasks: &[TaskRecord],
    archived_tasks: &[TaskRecord],
    statuses: &[String],
    ambiguous_ids: &[String],
    prefix: &str,
) -> DependencyGraph {
    let index = TaskRecordIndex::new(tasks, archived_tasks, statuses, ambiguous_ids, prefix);
    let root_key = canonical_task_id(&root.id, prefix);
    let root_record = match index.lookup(&root.id) {
        TaskRecordLookup::Unique(record) => record.task,
        TaskRecordLookup::Missing | TaskRecordLookup::Ambiguous => root,
    };
    let mut nodes = HashMap::new();
    nodes.insert(
        root_key.clone(),
        DependencyGraphNode {
            id: root.id.clone(),
            title: Some(root.title.clone()),
            status: Some(root.status.clone()),
            state: DependencyNodeState::Resolved,
            completed: index.is_finished(&root_record.status),
            dependency_depth: Some(0),
            dependent_depth: Some(0),
        },
    );

    let mut declarers: HashMap<String, BTreeSet<String>> = HashMap::new();
    for record in index.records() {
        let from = canonical_task_id(&record.task.id, prefix);
        for dependency in &record.task.dependencies {
            declarers
                .entry(canonical_task_id(dependency, prefix))
                .or_default()
                .insert(from.clone());
        }
    }

    let mut edges = HashSet::new();
    let mut dependency_queue = VecDeque::from([(root_key.clone(), 0usize)]);
    while let Some((current, depth)) = dependency_queue.pop_front() {
        if nodes
            .get(&current)
            .is_none_or(|node: &DependencyGraphNode| node.state != DependencyNodeState::Resolved)
        {
            continue;
        }
        let dependencies = if current == root_key {
            root.dependencies.as_slice()
        } else if let TaskRecordLookup::Unique(record) = index.lookup(&current) {
            record.task.dependencies.as_slice()
        } else {
            &[]
        };
        for reference in dependencies {
            let next = canonical_task_id(reference, prefix);
            ensure_node(&next, reference, &index, &mut nodes);
            edges.insert((current.clone(), next.clone()));
            let node = nodes.get_mut(&next).expect("dependency node was inserted");
            if node.dependency_depth.is_none() {
                node.dependency_depth = Some(depth + 1);
                dependency_queue.push_back((next, depth + 1));
            }
        }
    }

    let mut dependent_queue = VecDeque::from([(root_key.clone(), 0usize)]);
    while let Some((current, depth)) = dependent_queue.pop_front() {
        if nodes
            .get(&current)
            .is_none_or(|node: &DependencyGraphNode| node.state != DependencyNodeState::Resolved)
        {
            continue;
        }
        let Some(dependents) = declarers.get(&current) else {
            continue;
        };
        let mut dependents: Vec<_> = dependents.iter().cloned().collect();
        dependents.sort_by(|left, right| compare_task_ids(left, right));
        for dependent in dependents {
            ensure_node(&dependent, &dependent, &index, &mut nodes);
            edges.insert((dependent.clone(), current.clone()));
            let node = nodes
                .get_mut(&dependent)
                .expect("dependent node was inserted");
            if node.dependent_depth.is_none() {
                node.dependent_depth = Some(depth + 1);
                dependent_queue.push_back((dependent, depth + 1));
            }
        }
    }

    let mut ordered_nodes: Vec<_> = nodes.into_iter().collect();
    ordered_nodes.sort_by(|(left_key, left), (right_key, right)| {
        if left_key == &root_key {
            return std::cmp::Ordering::Less;
        }
        if right_key == &root_key {
            return std::cmp::Ordering::Greater;
        }
        compare_task_ids(&left.id, &right.id)
    });
    let display_ids: HashMap<_, _> = ordered_nodes
        .iter()
        .map(|(key, node)| (key.clone(), node.id.clone()))
        .collect();
    let mut ordered_edges: Vec<_> = edges
        .into_iter()
        .map(|(from, to)| DependencyGraphEdge {
            from: display_ids.get(&from).cloned().unwrap_or(from),
            to: display_ids.get(&to).cloned().unwrap_or(to),
        })
        .collect();
    ordered_edges.sort_by(|left, right| {
        compare_task_ids(&left.from, &right.from)
            .then_with(|| compare_task_ids(&left.to, &right.to))
    });

    DependencyGraph {
        root_id: root.id.clone(),
        nodes: ordered_nodes.into_iter().map(|(_, node)| node).collect(),
        edges: ordered_edges,
    }
}

fn ensure_node(
    key: &str,
    reference: &str,
    index: &TaskRecordIndex<'_>,
    nodes: &mut HashMap<String, DependencyGraphNode>,
) {
    if nodes.contains_key(key) {
        return;
    }
    let unresolved = || DependencyGraphNode {
        id: if reference.trim().is_empty() {
            key.to_owned()
        } else {
            reference.trim().to_owned()
        },
        title: None,
        status: None,
        state: DependencyNodeState::Missing,
        completed: false,
        dependency_depth: None,
        dependent_depth: None,
    };
    let node = match index.lookup(key) {
        TaskRecordLookup::Missing => unresolved(),
        TaskRecordLookup::Unique(record) => DependencyGraphNode {
            id: record.task.id.clone(),
            title: Some(record.task.title.clone()),
            status: Some(record.task.status.clone()),
            state: DependencyNodeState::Resolved,
            completed: index.is_finished(&record.task.status),
            dependency_depth: None,
            dependent_depth: None,
        },
        TaskRecordLookup::Ambiguous => DependencyGraphNode {
            id: key.to_owned(),
            title: None,
            status: None,
            state: DependencyNodeState::Ambiguous,
            completed: false,
            dependency_depth: None,
            dependent_depth: None,
        },
    };
    nodes.insert(key.to_owned(), node);
}
