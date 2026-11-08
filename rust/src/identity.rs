//! Pure task-ID normalization, comparison, filename extraction, and allocation rules.

use std::fmt;

/// A deterministic error raised when several files claim the same entity ID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AmbiguousIdError {
    pub id: String,
    pub candidates: Vec<String>,
    message: String,
}

impl AmbiguousIdError {
    fn new(entity_label: &str, id: &str, mut candidates: Vec<String>) -> Self {
        candidates.sort();
        let message = format!(
            "{} ID {} is ambiguous; {} files match:\n{}\nRun 'backlog doctor' to review the conflicting files.",
            entity_label,
            id,
            candidates.len(),
            candidates
                .iter()
                .map(|path| format!("  - {path}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
        Self {
            id: id.to_owned(),
            candidates,
            message,
        }
    }
}

impl fmt::Display for AmbiguousIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AmbiguousIdError {}

/// Canonical entity key, or `None` when the ID has no addressable body.
pub fn entity_id_key(prefix: &str, id: &str) -> Option<String> {
    let body = entity_id_body(prefix, id).trim();
    if body.is_empty() {
        return None;
    }
    let key_body = if body.bytes().all(|byte| byte.is_ascii_digit()) {
        let stripped = body.trim_start_matches('0');
        if stripped.is_empty() { "0" } else { stripped }
    } else {
        return Some(format!("{}-{}", prefix, body.to_lowercase()));
    };
    Some(format!("{prefix}-{key_body}"))
}

/// Normalize an entity ID while preserving its body spelling and whitespace.
pub fn normalize_entity_id(prefix: &str, id: &str) -> String {
    format!("{prefix}-{}", entity_id_body(prefix, id))
}

/// Compare entity IDs, with empty IDs never matching.
pub fn entity_ids_equal(prefix: &str, left: &str, right: &str) -> bool {
    match entity_id_key(prefix, left) {
        Some(left_key) => Some(left_key) == entity_id_key(prefix, right),
        None => false,
    }
}

fn entity_id_body<'a>(prefix: &str, value: &'a str) -> &'a str {
    let trimmed = value.trim();
    match trimmed.split_once('-') {
        Some((candidate, body)) if candidate.eq_ignore_ascii_case(prefix) => body,
        _ => trimmed,
    }
}

/// Find a unique item matching an entity ID, or return a sorted ambiguity error.
pub fn find_unique_entity_by_id<'a, T, F>(
    entity_label: &str,
    prefix: &str,
    id: &str,
    items: &'a [T],
    describe: F,
) -> Result<Option<&'a T>, AmbiguousIdError>
where
    T: HasEntityId,
    F: Fn(&T) -> String,
{
    let matches: Vec<&T> = items
        .iter()
        .filter(|item| entity_ids_equal(prefix, id, item.entity_id()))
        .collect();
    if matches.len() > 1 {
        return Err(AmbiguousIdError::new(
            entity_label,
            &normalize_entity_id(prefix, id),
            matches.iter().map(|item| describe(item)).collect(),
        ));
    }
    Ok(matches.first().copied())
}

/// Supplies the ID field used by shared entity lookup.
pub trait HasEntityId {
    fn entity_id(&self) -> &str;
}

pub fn document_id_key(id: &str) -> Option<String> {
    entity_id_key("doc", id)
}
pub fn normalize_document_id(id: &str) -> String {
    normalize_entity_id("doc", id)
}
pub fn decision_id_key(id: &str) -> Option<String> {
    entity_id_key("decision", id)
}
pub fn decision_ids_equal(left: &str, right: &str) -> bool {
    entity_ids_equal("decision", left, right)
}

pub fn find_document_by_id<'a, T, F>(
    items: &'a [T],
    id: &str,
    describe: F,
) -> Result<Option<&'a T>, AmbiguousIdError>
where
    T: HasEntityId,
    F: Fn(&T) -> String,
{
    find_unique_entity_by_id("Document", "doc", id, items, describe)
}

pub fn find_decision_by_id<'a, T, F>(
    items: &'a [T],
    id: &str,
    describe: F,
) -> Result<Option<&'a T>, AmbiguousIdError>
where
    T: HasEntityId,
    F: Fn(&T) -> String,
{
    find_unique_entity_by_id("Decision", "decision", id, items, describe)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskIdResolution {
    Found(usize),
    Ambiguous(Vec<usize>),
    Invalid,
    NotFound,
}

/// Resolve a task ID against an ordered list; returned indices preserve source ordering.
pub fn resolve_task_by_id(input_id: &str, task_ids: &[String]) -> TaskIdResolution {
    let input = input_id.trim();
    if !is_valid_task_id(input) {
        return TaskIdResolution::Invalid;
    }
    let matches: Vec<usize> = task_ids
        .iter()
        .enumerate()
        .filter_map(|(index, id)| task_ids_equal(input, id, "task").then_some(index))
        .collect();
    match matches.as_slice() {
        [] => TaskIdResolution::NotFound,
        [index] => TaskIdResolution::Found(*index),
        _ => TaskIdResolution::Ambiguous(matches),
    }
}

/// Normalize an ID with the selected prefix, matching `prefix-<body>` case-insensitively.
pub fn normalize_id(id: &str, prefix: &str) -> String {
    let trimmed = id.trim();
    let body = strip_matching_prefix(trimmed, prefix).unwrap_or(trimmed);
    format!("{}-{}", prefix.to_uppercase(), body.to_uppercase())
}

pub fn extract_id_body(id: &str, prefix: &str) -> String {
    let trimmed = id.trim();
    strip_matching_prefix(trimmed, prefix)
        .unwrap_or(trimmed)
        .to_owned()
}

pub fn ids_equal(left: &str, right: &str, prefix: &str) -> bool {
    normalize_id(left, prefix).to_lowercase() == normalize_id(right, prefix).to_lowercase()
}

/// Extract the numeric body claimed by a filename, allowing the title or suffix after it.
pub fn extract_id_from_filename(filename: &str, prefix: &str) -> Option<String> {
    let body = numeric_body_prefix(filename, prefix)?;
    Some(normalize_id(&body, prefix))
}

/// Return the largest free top-level decimal ID plus one, without numeric-width limits.
pub fn generate_next_id(
    existing_ids: &[&str],
    prefix: &str,
    zero_padding: Option<usize>,
) -> String {
    let mut max = "0".to_owned();
    for id in existing_ids {
        let Some(body) = numeric_body_prefix(id, prefix) else {
            continue;
        };
        if body.contains('.') {
            continue;
        }
        let value = strip_zeroes(&body);
        if decimal_greater(&value, &max) {
            max = value;
        }
    }
    let mut next = decimal_increment(&max);
    if let Some(width) = zero_padding.filter(|width| *width > 0)
        && next.len() < width
    {
        next = format!("{}{}", "0".repeat(width - next.len()), next);
    }
    format!("{}-{next}", prefix.to_uppercase())
}

/// Return the next child number under `parent_id`; a positive padding option pads to two digits.
pub fn generate_next_subtask_id(
    existing_ids: &[&str],
    parent_id: &str,
    prefix: &str,
    zero_padding: Option<usize>,
) -> String {
    let normalized_parent = normalize_id(parent_id, prefix);
    let parent_body = extract_id_body(&normalized_parent, prefix);
    let parent_segments: Vec<&str> = parent_body.split('.').collect();
    let canonical_parent = canonical_numeric_segments(&parent_segments);
    let mut max = "0".to_owned();

    for id in existing_ids {
        let body = extract_id_body(id, prefix);
        let segments: Vec<&str> = body.split('.').collect();
        if segments.len() <= parent_segments.len() {
            continue;
        }
        let candidate_parent = &segments[..parent_segments.len()];
        let parent_matches = match (
            &canonical_parent,
            canonical_numeric_segments(candidate_parent),
        ) {
            (Some(expected), Some(actual)) => expected == &actual,
            _ => candidate_parent
                .join(".")
                .eq_ignore_ascii_case(&parent_body),
        };
        if !parent_matches {
            continue;
        }
        let child = segments[parent_segments.len()];
        if !child.bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        let child = strip_zeroes(child);
        if decimal_greater(&child, &max) {
            max = child;
        }
    }

    let mut next = decimal_increment(&max);
    if zero_padding.is_some_and(|width| width > 0) && next.len() < 2 {
        next = format!("0{next}");
    }
    format!("{normalized_parent}.{next}")
}

pub fn normalize_task_id(task_id: &str, prefix: &str) -> String {
    let inferred = extract_any_prefix(task_id);
    let effective_prefix = if prefix == "task" {
        inferred.as_deref().unwrap_or(prefix)
    } else {
        prefix
    };
    normalize_id(task_id, effective_prefix)
}

pub fn canonical_task_id(task_id: &str, prefix: &str) -> String {
    let trimmed = task_id.trim();
    let inferred_prefix = extract_any_prefix(trimmed);
    let effective_prefix = inferred_prefix.as_deref().unwrap_or(prefix);
    let Some(body) = extract_task_body(trimmed, effective_prefix) else {
        return normalize_task_id(trimmed, effective_prefix).to_uppercase();
    };
    if body.is_empty() {
        return normalize_task_id(trimmed, effective_prefix).to_uppercase();
    }
    format!(
        "{}-{}",
        effective_prefix.to_uppercase(),
        body.split('.')
            .map(strip_zeroes)
            .collect::<Vec<_>>()
            .join(".")
    )
}

pub fn task_ids_equal(left: &str, right: &str, prefix: &str) -> bool {
    let effective_prefix = extract_any_prefix(left)
        .or_else(|| extract_any_prefix(right))
        .unwrap_or_else(|| prefix.to_owned());
    let left_body = extract_task_body(left, &effective_prefix);
    let right_body = extract_task_body(right, &effective_prefix);
    if let (Some(left_body), Some(right_body)) = (left_body.as_deref(), right_body.as_deref())
        && !left_body.is_empty()
        && !right_body.is_empty()
    {
        return numeric_bodies_equal(left_body, right_body);
    }
    normalize_task_id(left, &effective_prefix).to_lowercase()
        == normalize_task_id(right, &effective_prefix).to_lowercase()
}

pub fn is_valid_task_id(value: &str) -> bool {
    let trimmed = value.trim();
    is_numeric_task_id(trimmed) || is_legacy_task_id(trimmed)
}

fn is_numeric_task_id(value: &str) -> bool {
    let body = strip_optional_alpha_prefix(value).unwrap_or(value);
    !body.is_empty()
        && body
            .split('.')
            .all(|segment| !segment.is_empty() && segment.bytes().all(|byte| byte.is_ascii_digit()))
}

fn is_legacy_task_id(value: &str) -> bool {
    let Some((prefix, body)) = value.split_once('-') else {
        return false;
    };
    !prefix.is_empty()
        && prefix.bytes().all(|byte| byte.is_ascii_alphabetic())
        && !body.is_empty()
        && body.split(['.', '_', '-']).all(|segment| {
            !segment.is_empty() && segment.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
}

fn numeric_bodies_equal(left: &str, right: &str) -> bool {
    let left: Vec<&str> = left.split('.').collect();
    let right: Vec<&str> = right.split('.').collect();
    left.len() == right.len()
        && left
            .iter()
            .all(|segment| segment.bytes().all(|byte| byte.is_ascii_digit()))
        && right
            .iter()
            .all(|segment| segment.bytes().all(|byte| byte.is_ascii_digit()))
        && left
            .iter()
            .zip(right.iter())
            .all(|(left, right)| strip_zeroes(left) == strip_zeroes(right))
}

fn extract_task_body(value: &str, prefix: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some(String::new());
    }
    let body = strip_matching_prefix(trimmed, prefix).unwrap_or(trimmed);
    let valid = !body.is_empty()
        && body.split('.').all(|segment| {
            !segment.is_empty() && segment.bytes().all(|byte| byte.is_ascii_digit())
        });
    valid.then(|| body.to_owned())
}

fn extract_any_prefix(value: &str) -> Option<String> {
    let (prefix, _) = value.trim().split_once('-')?;
    (!prefix.is_empty() && prefix.bytes().all(|byte| byte.is_ascii_alphabetic()))
        .then(|| prefix.to_lowercase())
}

fn strip_optional_alpha_prefix(value: &str) -> Option<&str> {
    let (prefix, body) = value.split_once('-')?;
    (!prefix.is_empty() && prefix.bytes().all(|byte| byte.is_ascii_alphabetic())).then_some(body)
}

fn strip_matching_prefix<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    let (candidate, body) = value.split_once('-')?;
    (candidate.eq_ignore_ascii_case(prefix) && !body.is_empty()).then_some(body)
}

fn numeric_body_prefix(value: &str, prefix: &str) -> Option<String> {
    let trimmed = value.trim();
    let body = strip_matching_prefix(trimmed, prefix)?;
    let mut end = 0;
    let bytes = body.as_bytes();
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    if end == 0 {
        return None;
    }
    while end + 1 < bytes.len() && bytes[end] == b'.' && bytes[end + 1].is_ascii_digit() {
        end += 2;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
    }
    Some(body[..end].to_owned())
}

fn canonical_numeric_segments(segments: &[&str]) -> Option<Vec<String>> {
    segments
        .iter()
        .all(|segment| !segment.is_empty() && segment.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| {
            segments
                .iter()
                .map(|segment| strip_zeroes(segment))
                .collect()
        })
}

fn strip_zeroes(value: &str) -> String {
    let value = value.trim_start_matches('0');
    if value.is_empty() {
        "0".to_owned()
    } else {
        value.to_owned()
    }
}

fn decimal_greater(left: &str, right: &str) -> bool {
    left.len() > right.len() || (left.len() == right.len() && left > right)
}

fn decimal_increment(value: &str) -> String {
    let mut digits: Vec<u8> = value.bytes().collect();
    let mut carry = 1;
    for digit in digits.iter_mut().rev() {
        let next = (*digit - b'0') + carry;
        *digit = b'0' + next % 10;
        carry = next / 10;
        if carry == 0 {
            break;
        }
    }
    let mut result = String::with_capacity(digits.len() + usize::from(carry > 0));
    if carry > 0 {
        result.push(char::from(b'0' + carry));
    }
    result.extend(digits.into_iter().map(char::from));
    result
}
