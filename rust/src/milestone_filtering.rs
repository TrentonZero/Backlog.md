use crate::records::MilestoneRecord;
use std::collections::HashMap;

const NO_MILESTONE: &str = "\0no-milestone";

struct MilestoneResolver<'a> {
    records: &'a [MilestoneRecord],
    by_id: HashMap<String, &'a MilestoneRecord>,
    by_title: HashMap<String, Vec<&'a MilestoneRecord>>,
}

impl<'a> MilestoneResolver<'a> {
    fn new(milestones: &'a [MilestoneRecord]) -> Self {
        let mut by_id = HashMap::new();
        let mut by_title: HashMap<String, Vec<&MilestoneRecord>> = HashMap::new();
        for milestone in milestones {
            let id = milestone.id.trim();
            let title = milestone.title.trim();
            if id.is_empty() || title.is_empty() {
                continue;
            }
            by_id.entry(id.to_lowercase()).or_insert(milestone);
            by_title
                .entry(title.to_lowercase())
                .or_default()
                .push(milestone);
        }
        Self {
            records: milestones,
            by_id,
            by_title,
        }
    }

    fn exact_id(&self, value: &str) -> Option<&'a MilestoneRecord> {
        let normalized = value.trim().to_lowercase();
        if normalized.is_empty() {
            return None;
        }
        if let Some(milestone) = self.by_id.get(&normalized) {
            return Some(milestone);
        }
        let numeric = normalized.strip_prefix("m-").unwrap_or(&normalized);
        if numeric.is_empty() || !numeric.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        let canonical = canonical_numeric(numeric);
        self.by_id
            .get(&format!("m-{canonical}"))
            .copied()
            .or_else(|| {
                self.records.iter().find(|milestone| {
                    let candidate = milestone.id.trim().to_lowercase();
                    let Some(candidate) = candidate.strip_prefix("m-") else {
                        return false;
                    };
                    candidate.bytes().all(|byte| byte.is_ascii_digit())
                        && canonical_numeric(candidate) == canonical
                })
            })
    }

    fn exact_title(&self, value: &str) -> Option<&'a MilestoneRecord> {
        self.by_title
            .get(&value.trim().to_lowercase())?
            .first()
            .copied()
    }

    fn resolve_id(&self, value: &str) -> Option<&'a MilestoneRecord> {
        if let Some(milestone) = self.exact_id(value) {
            return Some(milestone);
        }
        let matches = self.by_title.get(&value.trim().to_lowercase())?;
        let id = matches.first()?.id.trim().to_lowercase();
        matches
            .iter()
            .all(|milestone| milestone.id.trim().to_lowercase() == id)
            .then_some(matches[0])
    }

    fn resolve(&self, value: &str) -> String {
        let normalized = value.trim();
        if let Some(milestone) = self
            .exact_id(normalized)
            .or_else(|| self.exact_title(normalized))
        {
            return milestone.title.trim().to_owned();
        }
        value.to_owned()
    }
}

pub(crate) struct MilestoneMatcher<'a> {
    resolver: MilestoneResolver<'a>,
    mode: MatchMode,
}

enum MatchMode {
    None,
    Id(String),
    Title(String),
    Value(String),
}

impl<'a> MilestoneMatcher<'a> {
    pub(crate) fn new(query: &str, milestones: &'a [MilestoneRecord]) -> Self {
        let resolver = MilestoneResolver::new(milestones);
        let query = query.trim().to_lowercase();
        let mode = if query == NO_MILESTONE {
            MatchMode::None
        } else if let Some(milestone) = resolver.exact_id(&query) {
            MatchMode::Id(milestone.id.trim().to_lowercase())
        } else if let Some(milestone) = resolver.exact_title(&query) {
            MatchMode::Title(milestone.title.trim().to_lowercase())
        } else {
            MatchMode::Value(normalize(&resolver.resolve(&query)))
        };
        Self { resolver, mode }
    }

    pub(crate) fn matches(&self, value: Option<&str>) -> bool {
        let value = value.unwrap_or_default();
        match &self.mode {
            MatchMode::None => value.trim().is_empty(),
            MatchMode::Id(target) => self
                .resolver
                .resolve_id(value)
                .is_some_and(|milestone| milestone.id.trim().eq_ignore_ascii_case(target)),
            MatchMode::Title(target) => self
                .resolver
                .resolve(value)
                .trim()
                .eq_ignore_ascii_case(target),
            MatchMode::Value(target) => normalize(&self.resolver.resolve(value)) == *target,
        }
    }
}

fn normalize(value: &str) -> String {
    let normalized = value.trim().to_lowercase();
    let searchable: String = normalized
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect();
    let collapsed = searchable.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        normalized
    } else {
        collapsed
    }
}

fn canonical_numeric(value: &str) -> &str {
    let trimmed = value.trim_start_matches('0');
    if trimmed.is_empty() { "0" } else { trimmed }
}
