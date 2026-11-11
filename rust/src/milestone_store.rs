//! Local milestone listing, lookup, and creation.

use crate::{
    git::GitRepository,
    records::{self, MilestoneRecord},
    storage::Storage,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq)]
pub struct MilestoneChangeResult {
    pub success: bool,
    pub source_path: Option<PathBuf>,
    pub target_path: Option<PathBuf>,
    pub milestone: Option<MilestoneRecord>,
    pub previous_title: Option<String>,
    pub previous_due_date: Option<String>,
}

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
        let project_root = project_root.into();
        let backlog = project_root.join(backlog_relative_directory.into());
        let common_git_dir = GitRepository::new(&project_root).common_directory();
        Self {
            active: backlog.join("milestones"),
            archived: backlog.join("archive/milestones"),
            storage: Storage::new(&backlog, common_git_dir),
        }
    }

    pub fn list_active(&self) -> Vec<MilestoneRecord> {
        self.list(&self.active)
    }
    pub fn list_archived(&self) -> Vec<MilestoneRecord> {
        self.list(&self.archived)
    }

    fn list(&self, directory: &Path) -> Vec<MilestoneRecord> {
        let mut records = self.entries(directory);
        records.sort_by(|a, b| milestone_id_order(&a.1.id, &b.1.id));
        records.into_iter().map(|(_, record)| record).collect()
    }

    fn entries(&self, directory: &Path) -> Vec<(PathBuf, MilestoneRecord)> {
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
                records.push((path.to_owned(), record));
            }
        });
        records
    }

    pub fn load_active(&self, identifier: &str) -> Option<MilestoneRecord> {
        self.resolve_active(identifier).map(|(_, record)| record)
    }

    pub fn rename(
        &self,
        identifier: &str,
        title: &str,
        due_date: Option<Option<&str>>,
    ) -> Result<MilestoneChangeResult, String> {
        let normalized_title = title.trim();
        if normalized_title.is_empty() {
            return Ok(MilestoneChangeResult::failed());
        }
        let normalized_due_date = match due_date {
            Some(Some(value)) => normalize_due_date(Some(value))?,
            _ => None,
        };
        let Some((source_path, milestone)) = self.resolve_active(identifier) else {
            return Ok(MilestoneChangeResult::failed());
        };
        let next_due_date = match due_date {
            None => milestone.due_date.clone(),
            Some(_) => normalized_due_date,
        };
        let target_path = self.active.join(format!(
            "{} - {}.md",
            milestone.id,
            milestone_slug(normalized_title)
        ));
        if source_path != target_path && fs::symlink_metadata(&target_path).is_ok() {
            return Ok(MilestoneChangeResult::failed());
        }
        let raw_content =
            rewrite_default_description(&milestone.raw_content, &milestone.title, normalized_title);
        let source = serialize_milestone(
            &milestone.id,
            normalized_title,
            &raw_content,
            next_due_date.as_deref(),
        );
        if self
            .storage
            .write_record(&target_path, &source, Some(&source_path))
            .is_err()
        {
            return Ok(MilestoneChangeResult::failed());
        }
        let updated = records::parse_milestone(&source).map_err(|error| error.to_string())?;
        Ok(MilestoneChangeResult {
            success: true,
            source_path: Some(source_path),
            target_path: Some(target_path),
            milestone: Some(updated),
            previous_title: Some(milestone.title),
            previous_due_date: milestone.due_date,
        })
    }

    pub fn archive(&self, identifier: &str) -> Result<MilestoneChangeResult, String> {
        let Some((source_path, milestone)) = self.resolve_active(identifier) else {
            return Ok(MilestoneChangeResult::failed());
        };
        let Ok(relative) = source_path.strip_prefix(&self.active) else {
            return Ok(MilestoneChangeResult::failed());
        };
        let target_path = self.archived.join(relative);
        if self
            .storage
            .move_record(&source_path, &target_path)
            .is_err()
        {
            return Ok(MilestoneChangeResult::failed());
        }
        Ok(MilestoneChangeResult {
            success: true,
            source_path: Some(source_path),
            target_path: Some(target_path),
            milestone: Some(milestone),
            previous_title: None,
            previous_due_date: None,
        })
    }

    fn resolve_active(&self, identifier: &str) -> Option<(PathBuf, MilestoneRecord)> {
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
        for entry in self.entries(&self.active) {
            let id_key = entry.1.id.trim().to_lowercase();
            let id_keys = identifier_keys(&entry.1.id);
            let title_key = entry.1.title.trim().to_lowercase();
            if id_key == normalized {
                raw.push(entry);
                continue;
            }
            if canonical.as_deref() == Some(id_key.as_str()) {
                canonical_raw.push(entry);
                continue;
            }
            if id_keys.contains(&normalized) {
                aliases.push(entry);
                continue;
            }
            if title_key == normalized {
                titles.push(entry);
                continue;
            }
            if id_keys.iter().any(|key| variants.contains(key)) {
                variant_ids.push(entry);
                continue;
            }
            if variants.contains(&title_key) {
                variant_titles.push(entry);
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
        let slug = milestone_slug(title);
        let body = description
            .filter(|s| !s.is_empty())
            .map_or_else(|| format!("Milestone: {title}"), str::to_owned);
        let source = serialize_milestone(
            &id,
            title,
            &format!("## Description\n\n{body}"),
            due_date.as_deref(),
        );
        let path = self.active.join(format!("{id} - {slug}.md"));
        self.storage
            .write_record(&path, &source, None)
            .map_err(|e| e.to_string())?;
        records::parse_milestone(&source).map_err(|e| e.to_string())
    }
}

impl MilestoneChangeResult {
    fn failed() -> Self {
        Self {
            success: false,
            source_path: None,
            target_path: None,
            milestone: None,
            previous_title: None,
            previous_due_date: None,
        }
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

fn milestone_slug(title: &str) -> String {
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
    slug
}

fn serialize_milestone(id: &str, title: &str, raw_content: &str, due_date: Option<&str>) -> String {
    let title = title.replace('\\', "\\\\").replace('"', "\\\"");
    format!(
        "---\nid: {id}\ntitle: \"{title}\"\n{}---\n\n{}\n",
        due_date.map_or_else(String::new, |date| format!("due_date: \"{date}\"\n")),
        raw_content.trim()
    )
}

fn rewrite_default_description(raw_content: &str, old_title: &str, new_title: &str) -> String {
    let default = format!("Milestone: {old_title}");
    let lines: Vec<&str> = raw_content.split_inclusive('\n').collect();
    let Some(section_start) = lines
        .iter()
        .position(|line| line.trim_end().eq_ignore_ascii_case("## Description"))
    else {
        return raw_content.to_owned();
    };
    let end = lines
        .iter()
        .enumerate()
        .skip(section_start + 1)
        .find(|(_, line)| line.starts_with("## "))
        .map_or(lines.len(), |(index, _)| index);
    let body: String = lines[section_start + 1..end].concat();
    if body.trim() != default {
        return raw_content.to_owned();
    }
    let Some(match_start) = body.find(&default) else {
        return raw_content.to_owned();
    };
    let match_end = match_start + default.len();
    let mut output = lines[..section_start + 1].concat();
    output.push_str(&body[..match_start]);
    output.push_str(&format!("Milestone: {new_title}"));
    output.push_str(&body[match_end..]);
    output.push_str(&lines[end..].concat());
    output
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
