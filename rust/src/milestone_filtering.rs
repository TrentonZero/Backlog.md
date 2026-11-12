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
                if let Some(field_score) = bitap_search(query, text) {
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

fn field_norm(value: &str) -> f64 {
    let mut tokens = 0;
    let mut in_word = false;
    for character in value.chars() {
        let separator = matches!(character, '\t'..='\r' | ' ' | '\u{00a0}');
        if !separator && !in_word {
            tokens += 1;
            in_word = true;
        } else if separator {
            in_word = false;
        }
    }
    let tokens = tokens.max(1) as f64;
    (1000.0 / tokens.sqrt()).round() / 1000.0
}

fn bitap_search(query: &str, text: &str) -> Option<f64> {
    const MAX_PATTERN_LEN: usize = 32;
    const THRESHOLD: f64 = 0.45;
    const MIN_MATCH_LEN: usize = 2;

    let pattern = query.to_lowercase().encode_utf16().collect::<Vec<_>>();
    let text = text.to_lowercase().encode_utf16().collect::<Vec<_>>();
    if pattern.is_empty() {
        return None;
    }

    let chunks = if pattern.len() <= MAX_PATTERN_LEN {
        vec![(pattern.as_slice(), 0)]
    } else {
        let remainder = pattern.len() % MAX_PATTERN_LEN;
        let full_end = pattern.len() - remainder;
        let mut chunks = (0..full_end)
            .step_by(MAX_PATTERN_LEN)
            .map(|start| (&pattern[start..start + MAX_PATTERN_LEN], start))
            .collect::<Vec<_>>();
        if remainder > 0 {
            let start = pattern.len() - MAX_PATTERN_LEN;
            chunks.push((&pattern[start..], start));
        }
        chunks
    };

    let mut total_score = 0.0;
    let mut any_match = false;
    for (chunk, start_index) in chunks.iter().copied() {
        let score = bitap_search_chunk(chunk, &text, start_index, THRESHOLD, MIN_MATCH_LEN);
        if let Some(score) = score {
            any_match = true;
            total_score += score;
        } else {
            total_score += 1.0;
        }
    }
    any_match.then(|| total_score / chunks.len() as f64)
}

fn bitap_search_chunk(
    pattern: &[u16],
    text: &[u16],
    start_index: usize,
    threshold: f64,
    min_match_len: usize,
) -> Option<f64> {
    let pattern_len = pattern.len();
    let text_len = text.len();
    let expected = start_index.min(text_len);
    let mut current_threshold = threshold;
    let mut match_mask = vec![false; text_len];

    if pattern_len == text_len && pattern == text {
        return (pattern_len >= min_match_len).then_some(0.0);
    }

    if pattern_len <= text_len {
        for index in 0..=text_len - pattern_len {
            if text[index..index + pattern_len] == *pattern {
                current_threshold = 0.0;
                match_mask[index..index + pattern_len].fill(true);
            }
        }
    }

    let mut alphabet = HashMap::<u16, u32>::new();
    for (index, character) in pattern.iter().enumerate() {
        *alphabet.entry(*character).or_default() |=
            1_u32.wrapping_shl((pattern_len - index - 1) as u32);
    }

    let mask = 1_u32.wrapping_shl((pattern_len - 1) as u32);
    let mut last_bit_array = Vec::<u32>::new();
    let mut final_score = 1.0;
    let mut best_location = None;
    let mut best_errors = 0;
    let mut bin_max = pattern_len + text_len;

    for errors in 0..pattern_len {
        let mut bin_min = 0;
        let mut bin_mid = bin_max;
        while bin_min < bin_mid {
            if bitap_score(errors, pattern_len) <= current_threshold {
                bin_min = bin_mid;
            } else {
                bin_max = bin_mid;
            }
            bin_mid = (bin_max - bin_min) / 2 + bin_min;
        }
        bin_max = bin_mid;
        let start = expected.saturating_sub(bin_mid).saturating_add(1).max(1);
        let finish = expected.saturating_add(bin_mid).min(text_len) + pattern_len;
        let mut bit_array = vec![0_u32; finish + 2];
        bit_array[finish + 1] = 1_u32.wrapping_shl(errors as u32).wrapping_sub(1);

        for index in (start..=finish).rev() {
            let current_location = index - 1;
            let char_match = text
                .get(current_location)
                .and_then(|character| alphabet.get(character))
                .copied()
                .unwrap_or_default();
            bit_array[index] = (bit_array[index + 1].wrapping_shl(1) | 1) & char_match;
            if errors > 0 {
                bit_array[index] |= (last_bit_array.get(index + 1).copied().unwrap_or_default()
                    | last_bit_array.get(index).copied().unwrap_or_default())
                .wrapping_shl(1)
                    | 1
                    | last_bit_array.get(index + 1).copied().unwrap_or_default();
            }
            if bit_array[index] & mask != 0 {
                final_score = bitap_score(errors, pattern_len);
                if final_score <= current_threshold {
                    current_threshold = final_score;
                    best_location = Some(current_location);
                    best_errors = errors;
                    if current_location <= expected {
                        break;
                    }
                    // ignoreLocation is enabled, so moving this window cannot change the score.
                }
            }
        }
        if bitap_score(errors + 1, pattern_len) > current_threshold {
            break;
        }
        last_bit_array = bit_array;
    }

    if let Some(location) = best_location {
        let match_end = text_len
            .saturating_sub(1)
            .min(location + pattern_len - 1 + best_errors);
        for (index, matched) in match_mask
            .iter_mut()
            .enumerate()
            .take(match_end + 1)
            .skip(location)
        {
            if text
                .get(index)
                .and_then(|character| alphabet.get(character))
                .is_some()
            {
                *matched = true;
            }
        }
    }

    let mut longest_match = 0;
    let mut has_valid_match = false;
    for matched in match_mask {
        if matched {
            longest_match += 1;
        } else {
            has_valid_match |= longest_match >= min_match_len;
            longest_match = 0;
        }
    }
    has_valid_match |= longest_match >= min_match_len;

    if !has_valid_match {
        return None;
    }
    Some(final_score.max(0.001))
}

fn bitap_score(errors: usize, pattern_len: usize) -> f64 {
    // The TypeScript matcher sets ignoreLocation, so score depends only on edit accuracy.
    errors as f64 / pattern_len as f64
}
