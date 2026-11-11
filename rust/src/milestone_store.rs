//! Local milestone listing, lookup, and creation.

use crate::{
    records::{self, MilestoneRecord},
    storage::Storage,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct MilestoneStore {
    active: PathBuf,
    archived: PathBuf,
    storage: Storage,
}

impl MilestoneStore {
    pub fn new(
        project_root: impl Into<PathBuf>,
        backlog_relative_directory: impl Into<PathBuf>,
    ) -> Self {
        let backlog = project_root.into().join(backlog_relative_directory.into());
        Self {
            active: backlog.join("milestones"),
            archived: backlog.join("archive/milestones"),
            storage: Storage::new(&backlog, None),
        }
    }

    pub fn list_active(&self) -> Vec<MilestoneRecord> {
        self.list(&self.active)
    }
    pub fn list_archived(&self) -> Vec<MilestoneRecord> {
        self.list(&self.archived)
    }

    fn list(&self, directory: &Path) -> Vec<MilestoneRecord> {
        let mut records = Vec::new();
        collect_markdown(directory, &mut |path| {
            if path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("README.md"))
            {
                return;
            }
            if let Ok(source) = fs::read_to_string(path)
                && let Ok(record) = records::parse_milestone(&source)
            {
                records.push(record);
            }
        });
        records.sort_by(|a, b| milestone_id_order(&a.id, &b.id));
        records
    }

    pub fn load_active(&self, identifier: &str) -> Option<MilestoneRecord> {
        let normalized = identifier.trim().to_lowercase();
        if normalized.is_empty() {
            return None;
        }
        let candidate_keys = identifier_keys(identifier);
        let mut variants = candidate_keys.clone();
        variants.remove(&normalized);
        let canonical = milestone_number(identifier).map(|number| format!("m-{number}"));
        let numeric_input = is_numeric_identifier(&normalized);
        let mut raw = Vec::new();
        let mut canonical_raw = Vec::new();
        let mut aliases = Vec::new();
        let mut titles = Vec::new();
        let mut variant_ids = Vec::new();
        let mut variant_titles = Vec::new();
        for item in self.list_active() {
            let id_key = item.id.trim().to_lowercase();
            let id_keys = identifier_keys(&item.id);
            let title_key = item.title.trim().to_lowercase();
            if id_key == normalized {
                raw.push(item);
                continue;
            }
            if canonical.as_deref() == Some(id_key.as_str()) {
                canonical_raw.push(item);
                continue;
            }
            if id_keys.contains(&normalized) {
                aliases.push(item);
                continue;
            }
            if title_key == normalized {
                titles.push(item);
                continue;
            }
            if id_keys.iter().any(|key| variants.contains(key)) {
                variant_ids.push(item);
                continue;
            }
            if variants.contains(&title_key) {
                variant_titles.push(item);
            }
        }
        if numeric_input {
            raw.into_iter()
                .next()
                .or_else(|| canonical_raw.into_iter().next())
                .or_else(|| unique(aliases))
                .or_else(|| unique(variant_ids))
                .or_else(|| unique(titles))
                .or_else(|| unique(variant_titles))
        } else {
            raw.into_iter()
                .next()
                .or_else(|| unique(titles))
                .or_else(|| canonical_raw.into_iter().next())
                .or_else(|| unique(variant_ids))
                .or_else(|| unique(variant_titles))
        }
    }

    pub fn create(
        &self,
        title: &str,
        description: Option<&str>,
        due_date: Option<&str>,
    ) -> Result<MilestoneRecord, String> {
        let due_date = normalize_due_date(due_date)?;
        let _lock = self
            .storage
            .acquire_create_lock()
            .map_err(|e| e.to_string())?;
        fs::create_dir_all(&self.active).map_err(|e| e.to_string())?;
        fs::create_dir_all(&self.archived).map_err(|e| e.to_string())?;
        let mut max_id = None;
        for directory in [&self.active, &self.archived] {
            collect_markdown(directory, &mut |path| {
                let parsed = fs::read_to_string(path)
                    .ok()
                    .and_then(|s| records::parse_milestone(&s).ok())
                    .and_then(|m| milestone_number(&m.id));
                let from_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .and_then(|n| {
                        n.get(..2)
                            .filter(|prefix| prefix.eq_ignore_ascii_case("m-"))
                            .map(|_| &n[2..])
                    })
                    .and_then(|n| n.split(|c: char| !c.is_ascii_digit()).next())
                    .and_then(|n| n.parse::<u64>().ok());
                if let Some(id) = parsed.or(from_name) {
                    max_id = Some(max_id.map_or(id, |old: u64| old.max(id)));
                }
            });
        }
        let id = format!("m-{}", max_id.map_or(0, |n| n + 1));
        let mut slug = String::new();
        let mut previous_whitespace = false;
        for character in title.chars().filter(|c| !"<>:\"/\\|?*".contains(*c)) {
            if character.is_whitespace() {
                if !previous_whitespace {
                    slug.push('-');
                }
                previous_whitespace = true;
            } else {
                slug.extend(character.to_lowercase());
                previous_whitespace = false;
            }
            if slug.chars().count() >= 50 {
                break;
            }
        }
        let escaped_title = serde_json::to_string(title).map_err(|e| e.to_string())?;
        let body = description
            .filter(|s| !s.is_empty())
            .map_or_else(|| format!("Milestone: {title}"), str::to_owned);
        let source = format!(
            "---\nid: {id}\ntitle: {escaped_title}\n{}---\n\n## Description\n\n{}\n",
            due_date
                .as_ref()
                .map_or(String::new(), |d| format!("due_date: \"{d}\"\n")),
            body.trim()
        );
        let path = self.active.join(format!("{id} - {slug}.md"));
        self.storage
            .write_record(&path, &source, None)
            .map_err(|e| e.to_string())?;
        records::parse_milestone(&source).map_err(|e| e.to_string())
    }
}

fn collect_markdown(directory: &Path, visit: &mut impl FnMut(&Path)) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_markdown(&path, visit);
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("md"))
            && path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("m-"))
        {
            visit(&path);
        }
    }
}

fn milestone_number(id: &str) -> Option<u64> {
    id.trim()
        .strip_prefix("m-")
        .or_else(|| id.trim().strip_prefix("M-"))
        .unwrap_or(id.trim())
        .parse()
        .ok()
}

fn is_numeric_identifier(value: &str) -> bool {
    value.bytes().all(|byte| byte.is_ascii_digit())
        || value
            .strip_prefix("m-")
            .is_some_and(|body| !body.is_empty() && body.bytes().all(|byte| byte.is_ascii_digit()))
}

fn identifier_keys(identifier: &str) -> std::collections::HashSet<String> {
    let normalized = identifier.trim().to_lowercase();
    let mut keys = std::collections::HashSet::new();
    if normalized.is_empty() {
        return keys;
    }
    keys.insert(normalized.clone());
    if is_numeric_identifier(&normalized)
        && let Some(number) = milestone_number(&normalized)
    {
        keys.insert(number.to_string());
        keys.insert(format!("m-{number}"));
    }
    keys
}

fn unique<T>(items: Vec<T>) -> Option<T> {
    if items.len() == 1 {
        items.into_iter().next()
    } else {
        None
    }
}

fn milestone_id_order(left: &str, right: &str) -> std::cmp::Ordering {
    match (milestone_number(left), milestone_number(right)) {
        (Some(a), Some(b)) => a.cmp(&b),
        _ => left.to_lowercase().cmp(&right.to_lowercase()),
    }
}

fn normalize_due_date(value: Option<&str>) -> Result<Option<String>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let quoted = serde_json::to_string(value).map_err(|error| error.to_string())?;
    let source = format!("---\nid: m-0\ntitle: Date\ndue_date: {quoted}\n---\n");
    records::parse_milestone(&source)
        .map(|record| record.due_date)
        .map_err(|error| error.to_string().replace("due_date must", "Due date must"))
}
