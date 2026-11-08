//! Pure task-ID normalization, comparison, filename extraction, and allocation rules.

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
