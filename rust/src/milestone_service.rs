//! Coordinates milestone removal with the task records that refer to it.

use crate::{milestone_store::MilestoneStore, task_store::TaskStore};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskMilestoneHandling {
    Clear,
    Keep,
    Reassign,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MilestoneRemovalResult {
    pub milestone_id: String,
    pub milestone_title: String,
    pub task_handling: TaskMilestoneHandling,
    pub affected_task_ids: Vec<String>,
    pub reassigned_to: Option<(String, String)>,
}

impl MilestoneRemovalResult {
    pub fn summary(&self) -> String {
        let mut lines = vec![format!(
            "Removed milestone \"{}\" ({}).",
            self.milestone_title, self.milestone_id
        )];
        match self.task_handling {
            TaskMilestoneHandling::Keep => {
                lines.push("Kept task milestone values unchanged (taskHandling=keep).".into())
            }
            TaskMilestoneHandling::Clear => lines.push(format!(
                "Cleared milestone for {} local task{}: {}",
                self.affected_task_ids.len(),
                if self.affected_task_ids.len() == 1 {
                    ""
                } else {
                    "s"
                },
                format_task_ids(&self.affected_task_ids)
            )),
            TaskMilestoneHandling::Reassign => lines.push(format!(
                "Reassigned {} local task{} to \"{}\" ({}): {}",
                self.affected_task_ids.len(),
                if self.affected_task_ids.len() == 1 {
                    ""
                } else {
                    "s"
                },
                self.reassigned_to
                    .as_ref()
                    .map_or("", |target| target.0.as_str()),
                self.reassigned_to
                    .as_ref()
                    .map_or("", |target| target.1.as_str()),
                format_task_ids(&self.affected_task_ids)
            )),
        }
        lines.join("\n")
    }
}

pub fn remove_milestone(
    milestones: &MilestoneStore,
    tasks: &TaskStore,
    name: &str,
    handling: TaskMilestoneHandling,
    reassign_to: Option<&str>,
) -> Result<MilestoneRemovalResult, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Milestone name cannot be empty.".into());
    }
    let source = milestones
        .load_active(name)
        .ok_or_else(|| format!("Milestone not found: \"{name}\""))?;
    let active = milestones.list_active();
    let archived = milestones.list_archived();
    let title_key = key(&source.title);
    let title_collision = active.iter().chain(&archived).any(|candidate| {
        !same_id(&candidate.id, &source.id) && record_keys(candidate).contains(&title_key)
    });
    let mut match_keys = id_keys(&source.id);
    if !title_collision {
        match_keys.insert(key(name));
        match_keys.insert(title_key);
    }

    let target = if handling == TaskMilestoneHandling::Reassign {
        let target_name = reassign_to.unwrap_or_default().trim();
        if target_name.is_empty() {
            return Err("reassignTo is required when taskHandling is reassign.".into());
        }
        let target = milestones
            .load_active(target_name)
            .ok_or_else(|| format!("Target milestone not found: \"{target_name}\""))?;
        if same_id(&target.id, &source.id) {
            return Err("reassignTo must be different from the removed milestone.".into());
        }
        Some(target)
    } else {
        None
    };
    let reassigned_to = target
        .as_ref()
        .map(|milestone| (milestone.title.clone(), milestone.id.clone()));

    let mut affected: Vec<_> = if handling == TaskMilestoneHandling::Keep {
        Vec::new()
    } else {
        tasks
            .list_tasks()
            .into_iter()
            .filter(|entry| {
                entry
                    .task
                    .milestone
                    .as_deref()
                    .is_some_and(|value| match_keys.contains(&key(value)))
            })
            .collect()
    };
    affected.sort_by(|left, right| {
        left.task
            .id
            .to_lowercase()
            .cmp(&right.task.id.to_lowercase())
    });
    let mut changed = Vec::new();
    for entry in &affected {
        let mut updated = entry.task.clone();
        updated.milestone = target.as_ref().map(|milestone| milestone.id.clone());
        updated.updated_date = Some(crate::task_store::utc_timestamp());
        match tasks.update_task(updated) {
            Ok(Some(_)) => changed.push(entry.clone()),
            Ok(None) | Err(_) => {
                let failures = rollback_tasks(tasks, &changed);
                let detail = rollback_detail(&failures);
                return Err(format!(
                    "Failed while updating tasks for milestone removal \"{}\"{detail}.",
                    source.title
                ));
            }
        }
    }

    match milestones.archive(&source.id) {
        Ok(result) if result.success => {}
        _ => {
            let failures = rollback_tasks(tasks, &changed);
            let detail = rollback_detail(&failures);
            return Err(format!(
                "Failed to archive milestone \"{}\" before removal{detail}.",
                source.title
            ));
        }
    }
    Ok(MilestoneRemovalResult {
        milestone_id: source.id,
        milestone_title: source.title,
        task_handling: handling,
        affected_task_ids: affected.into_iter().map(|entry| entry.task.id).collect(),
        reassigned_to,
    })
}

fn format_task_ids(ids: &[String]) -> String {
    let shown = ids.iter().take(20).cloned().collect::<Vec<_>>().join(", ");
    if ids.len() > 20 {
        format!("{shown} (and {} more)", ids.len() - 20)
    } else {
        shown
    }
}

fn rollback_tasks(tasks: &TaskStore, changed: &[crate::task_store::TaskFileEntry]) -> Vec<String> {
    let mut failures = Vec::new();
    for entry in changed.iter().rev() {
        if !matches!(tasks.update_task(entry.task.clone()), Ok(Some(_))) {
            failures.push(entry.task.id.clone());
        }
    }
    failures.sort();
    failures
}

fn rollback_detail(failures: &[String]) -> String {
    if failures.is_empty() {
        String::new()
    } else {
        format!(" (failed rollback for: {})", failures.join(", "))
    }
}

fn key(value: &str) -> String {
    value.trim().to_lowercase()
}

fn same_id(left: &str, right: &str) -> bool {
    id_keys(left)
        .iter()
        .any(|candidate| id_keys(right).contains(candidate))
}

fn id_keys(value: &str) -> HashSet<String> {
    let value = key(value);
    let mut keys = HashSet::new();
    if value.is_empty() {
        return keys;
    }
    keys.insert(value.clone());
    let numeric = value.strip_prefix("m-").unwrap_or(&value);
    if numeric.bytes().all(|byte| byte.is_ascii_digit())
        && let Ok(number) = numeric.parse::<u64>()
    {
        keys.insert(number.to_string());
        keys.insert(format!("m-{number}"));
    }
    keys
}

fn record_keys(record: &crate::records::MilestoneRecord) -> HashSet<String> {
    let mut keys = id_keys(&record.id);
    keys.insert(key(&record.title));
    keys
}
