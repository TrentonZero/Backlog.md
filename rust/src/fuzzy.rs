//! Fuse-compatible Bitap text matching shared by task and milestone search.

use std::collections::HashMap;

pub(crate) fn field_norm(value: &str) -> f64 {
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

/// Return Fuse's field score when a query matches with the supplied options.
pub(crate) fn bitap_search(
    query: &str,
    text: &str,
    threshold: f64,
    min_match_len: usize,
) -> Option<f64> {
    const MAX_PATTERN_LEN: usize = 32;

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
        let score = bitap_search_chunk(chunk, &text, start_index, threshold, min_match_len);
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
                    // Both search configs ignore location, so adjusting the window cannot change score.
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
    // Both task and milestone Fuse configs set ignoreLocation.
    errors as f64 / pattern_len as f64
}
