use crate::{
    fuzzy::{bitap_search, field_norm},
    records::MilestoneRecord,
};
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
    pub(crate) fn new(
        query: &str,
        milestones: &'a [MilestoneRecord],
        milestone_values: &[String],
    ) -> Self {
        let resolver = MilestoneResolver::new(milestones);
        let query = query.trim().to_lowercase();
        let mode = if query == NO_MILESTONE {
            MatchMode::None
        } else if let Some(milestone) = resolver.exact_id(&query) {
            MatchMode::Id(milestone.id.trim().to_lowercase())
        } else if let Some(milestone) = resolver.exact_title(&query) {
            MatchMode::Title(milestone.title.trim().to_lowercase())
        } else {
            MatchMode::Value(closest_milestone(&query, &resolver, milestone_values))
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

struct Candidate<'a> {
    value: &'a str,
    compact: String,
    value_norm: f64,
}

fn closest_milestone(
    query: &str,
    resolver: &MilestoneResolver<'_>,
    milestone_values: &[String],
) -> String {
    let normalized_query = normalize(query);
    if normalized_query.is_empty() {
        return normalized_query;
    }

    let mut candidates = milestone_values
        .iter()
        .map(|value| normalize(&resolver.resolve(value)))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.dedup();

    if candidates.is_empty() || candidates.iter().any(|value| value == &normalized_query) {
        return normalized_query;
    }

    let indexed = candidates
        .iter()
        .map(|value| Candidate {
            value,
            compact: value
                .chars()
                .filter(|character| !character.is_whitespace())
                .collect(),
            value_norm: field_norm(value),
        })
        .collect::<Vec<_>>();

    fuse_best(&normalized_query, &indexed)
        .or_else(|| {
            let compact_query: String = normalized_query
                .chars()
                .filter(|character| !character.is_whitespace())
                .collect();
            (!compact_query.is_empty())
                .then(|| fuse_best(&compact_query, &indexed))
                .flatten()
        })
        .unwrap_or(normalized_query)
}

fn fuse_best(query: &str, candidates: &[Candidate<'_>]) -> Option<String> {
    let mut results = candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            let mut score = 1.0_f64;
            let mut matched = false;
            for (text, weight, norm) in [
                (candidate.value, 0.7, candidate.value_norm),
                (candidate.compact.as_str(), 0.3, 1.0),
            ] {
                if let Some(field_score) = bitap_search(query, text, 0.45, 2) {
                    matched = true;
                    let field_score = if field_score == 0.0 {
                        f64::EPSILON
                    } else {
                        field_score
                    };
                    score *= field_score.powf(weight * norm);
                }
            }
            matched.then_some((score, index, candidate.value.to_owned()))
        })
        .collect::<Vec<_>>();
    results.sort_by(|left, right| left.0.total_cmp(&right.0).then(left.1.cmp(&right.1)));
    results.into_iter().next().map(|result| result.2)
}
