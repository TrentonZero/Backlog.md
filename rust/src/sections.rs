//! Managed task and decision Markdown, checklists, and comments.
//!
//! Source is supplied and returned by callers; persistence and indexed application
//! operations belong outside this component. Family-specific parsing deliberately
//! follows the shipped formats, rather than imposing one Markdown grammar.
use std::{collections::BTreeMap, error::Error, fmt, ops::Range};

use markdown::{ParseOptions, mdast::Node, to_mdast};

/// Task sections precede decision sections in their canonical insertion order.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SectionKey {
    Description,
    ImplementationPlan,
    ImplementationNotes,
    FinalSummary,
    Context,
    Decision,
    Consequences,
    Alternatives,
}
const TASK_KEYS: [SectionKey; 4] = [
    SectionKey::Description,
    SectionKey::ImplementationPlan,
    SectionKey::ImplementationNotes,
    SectionKey::FinalSummary,
];
const DECISION_KEYS: [SectionKey; 4] = [
    SectionKey::Context,
    SectionKey::Decision,
    SectionKey::Consequences,
    SectionKey::Alternatives,
];
const KNOWN_TITLES: [&str; 12] = [
    "Description",
    "Acceptance Criteria",
    "Definition of Done",
    "Implementation Plan",
    "Implementation Notes",
    "Comments",
    "Final Summary",
    "Acceptance Criteria (Optional)",
    "Implementation Plan (Optional)",
    "Implementation Notes (Optional)",
    "Notes",
    "Notes & Comments (Optional)",
];

impl SectionKey {
    fn title(self) -> &'static str {
        match self {
            Self::Description => "Description",
            Self::ImplementationPlan => "Implementation Plan",
            Self::ImplementationNotes => "Implementation Notes",
            Self::FinalSummary => "Final Summary",
            Self::Context => "Context",
            Self::Decision => "Decision",
            Self::Consequences => "Consequences",
            Self::Alternatives => "Alternatives",
        }
    }
    fn marker_id(self) -> &'static str {
        match self {
            Self::Description => "DESCRIPTION",
            Self::ImplementationPlan => "PLAN",
            Self::ImplementationNotes => "NOTES",
            Self::FinalSummary => "FINAL_SUMMARY",
            Self::Context => "CONTEXT",
            Self::Decision => "DECISION",
            Self::Consequences => "CONSEQUENCES",
            Self::Alternatives => "ALTERNATIVES",
        }
    }
    fn is_decision(self) -> bool {
        DECISION_KEYS.contains(&self)
    }
    fn begin(self) -> String {
        format!("<!-- SECTION:{}:BEGIN -->", self.marker_id())
    }
    fn end(self) -> String {
        format!("<!-- SECTION:{}:END -->", self.marker_id())
    }
}

/// An unsafe or ambiguous managed-body operation, with repair guidance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SectionError(String);
impl fmt::Display for SectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for SectionError {}

#[derive(Clone, Debug)]
struct Block {
    span: Range<usize>,
    body: Range<usize>,
}

fn lines(content: &str) -> Vec<(usize, &str)> {
    let mut offset = 0;
    content
        .split('\n')
        .map(|line| {
            let start = offset;
            offset += line.len() + 1;
            (start, line)
        })
        .collect()
}
fn within(index: usize, ranges: &[Range<usize>]) -> bool {
    ranges.iter().any(|range| range.contains(&index))
}
fn overlaps(a: &Range<usize>, b: &Range<usize>) -> bool {
    a.start < b.end && b.start < a.end
}
fn normalize(content: &str) -> String {
    content.replace("\r\n", "\n")
}
fn restore(content: String, crlf: bool) -> String {
    if crlf {
        content.replace('\n', "\r\n")
    } else {
        content
    }
}

// Whole source lines, including stored examples. Count depth to recover the
// complete interior of pre-fix same-family nested marker blocks.
fn sentinel_blocks(content: &str, key: SectionKey) -> Vec<Block> {
    let heading = format!("## {}", key.title());
    let begin = key.begin();
    let end = key.end();
    let source = lines(content);
    let mut result = Vec::new();
    let mut i = 0;
    while i < source.len() {
        if !source[i].1.trim_end().eq_ignore_ascii_case(&heading) {
            i += 1;
            continue;
        }
        let mut b = i + 1;
        while b < source.len() && source[b].1.trim().is_empty() {
            b += 1;
        }
        if b == source.len() || source[b].1.trim_end() != begin {
            i += 1;
            continue;
        }
        let mut depth = 1;
        let mut e = b + 1;
        while e < source.len() {
            let line = source[e].1.trim_end();
            if line == begin {
                depth += 1;
            } else if line == end {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            e += 1;
        }
        if depth != 0 {
            i += 1;
            continue;
        }
        result.push(Block {
            span: source[i].0..source[e].0 + source[e].1.len(),
            body: (source[b].0 + source[b].1.len() + 1).min(content.len())..source[e].0,
        });
        i = e + 1;
    }
    result
}

#[derive(Clone)]
struct Legacy {
    span: Range<usize>,
    body: Range<usize>,
    heading_start: usize,
}
fn task_boundary(line: &str, current: &str) -> bool {
    let Some(title) = line.strip_prefix("## ") else {
        return false;
    };
    KNOWN_TITLES.iter().any(|other| {
        !other.eq_ignore_ascii_case(current)
            && title
                .get(..other.len())
                .is_some_and(|part| part.eq_ignore_ascii_case(other))
            && title[other.len()..]
                .chars()
                .next()
                .is_none_or(char::is_whitespace)
    })
}
fn legacy_sections(content: &str, title: &str, unanchored: bool, comments: bool) -> Vec<Legacy> {
    let prefix = format!("## {title}");
    let mut result = Vec::new();
    let mut search = 0;
    while search < content.len() {
        let Some(start) = content[search..].find("## ").map(|n| search + n) else {
            break;
        };
        search = start + 3;
        if !unanchored && start > 0 && content.as_bytes()[start - 1] != b'\n' {
            continue;
        }
        if !content
            .get(start..start + prefix.len())
            .is_some_and(|s| s.eq_ignore_ascii_case(&prefix))
        {
            continue;
        }
        let after = start + prefix.len();
        // Preserve the legacy regex's greedy whitespace: an empty section can
        // consume the next heading as body text, so that heading is then guarded
        // by the legacy replacement refusal rather than treated as a boundary.
        let mut body_start = after;
        for (i, ch) in content[after..].char_indices() {
            if !ch.is_whitespace() {
                break;
            }
            if ch == '\n' {
                body_start = after + i + 1;
            }
        }
        if body_start == after {
            continue;
        }
        if comments && content[body_start..].starts_with("<!-- COMMENTS:BEGIN -->") {
            continue;
        }
        let mut body_end = content.len();
        for (relative, line) in lines(&content[body_start..]) {
            let heading = if comments {
                line.strip_prefix("##")
                    .is_some_and(|s| s.chars().next().is_some_and(char::is_whitespace))
            } else {
                task_boundary(line, title)
            };
            let at = body_start + relative;
            if heading && at > body_start && content.as_bytes()[at - 1] == b'\n' {
                body_end = at;
                while body_end > body_start && content.as_bytes()[body_end - 1] == b'\n' {
                    body_end -= 1;
                }
                break;
            }
        }
        if body_end == content.len() {
            while body_end > body_start && content.as_bytes()[body_end - 1] == b'\n' {
                body_end -= 1;
            }
        }
        let span_start = if !unanchored && start > 0 {
            start - 1
        } else {
            start
        };
        result.push(Legacy {
            span: span_start..body_end,
            body: body_start..body_end,
            heading_start: start,
        });
        search = body_end.max(search);
    }
    result
}
fn task_ranges(content: &str) -> Vec<(SectionKey, Range<usize>)> {
    let mut ranges = Vec::new();
    for key in TASK_KEYS {
        for block in sentinel_blocks(content, key) {
            ranges.push((key, block.span));
        }
        for legacy in legacy_sections(content, key.title(), false, false) {
            if !ranges
                .iter()
                .any(|(_, range)| overlaps(range, &legacy.span))
            {
                ranges.push((key, legacy.span));
            }
        }
    }
    ranges
}

/// Decision empty fields remain Some(""); task empty fields are absent.
pub fn extract_section(content: &str, key: SectionKey) -> Option<String> {
    if key.is_decision() {
        return decision_ranges(content)
            .into_iter()
            .find(|range| range.key == key)
            .map(|range| normalize(&content[range.body]).trim().to_owned());
    }
    let src = normalize(content);
    let other: Vec<_> = task_ranges(&src)
        .into_iter()
        .filter_map(|(k, span)| (k != key).then_some(span))
        .collect();
    let body = sentinel_blocks(&src, key)
        .into_iter()
        .find(|block| !within(block.span.start, &other))
        .map(|block| block.body)
        .or_else(|| {
            legacy_sections(&src, key.title(), true, false)
                .into_iter()
                .find(|legacy| !within(legacy.heading_start, &other))
                .map(|legacy| legacy.body)
        })?;
    let value = src[body].trim();
    (!value.is_empty()).then(|| value.to_owned())
}
// markdown-rs 1.0 reports leading indentation in heading spans and counts list
// tabs differently from the current micromark reference. Expand visual tab columns
// only in the parser input, then map its heading span back to original byte offsets.
fn first_markdown_heading(content: &str) -> Option<Range<usize>> {
    let mut projected = String::new();
    let mut offsets = vec![0];
    let mut column = 0;
    for (start, ch) in content.char_indices() {
        if ch == '\t' {
            let width = 4 - column % 4;
            for i in 0..width {
                projected.push(' ');
                offsets.push(if i + 1 == width { start + 1 } else { start });
            }
            column += width;
        } else {
            projected.push(ch);
            for byte in 1..=ch.len_utf8() {
                offsets.push(start + byte);
            }
            column = if ch == '\n' || ch == '\r' {
                0
            } else {
                column + 1
            };
        }
    }
    let tree =
        to_mdast(&projected, &ParseOptions::default()).expect("CommonMark has no syntax errors");
    tree.children()?.iter().find_map(|node| match node {
        Node::Heading(heading) if heading.depth == 2 => heading.position.as_ref().map(|p| {
            let indent = leading_spaces(&projected[p.start.offset..]);
            offsets[p.start.offset + indent]..offsets[p.end.offset]
        }),
        _ => None,
    })
}
fn keep_legacy(key: SectionKey, body: &str, next: &str) -> Result<bool, SectionError> {
    // CommonMark parsers accept directly self-closing raw-text tags. The reference
    // masks those openers so a real following heading remains visible.
    let markdown = body
        .split('\n')
        .map(|line| {
            let trimmed = line.trim_start_matches(' ');
            let raw = ["pre", "script", "style", "textarea"].iter().any(|tag| {
                trimmed
                    .trim_end_matches(['\t', ' '])
                    .eq_ignore_ascii_case(&format!("<{tag}/>"))
            });
            if line.len() - trimmed.len() <= 3 && raw {
                line.replace('<', "x")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let Some(heading) = first_markdown_heading(&markdown) else {
        return Ok(false);
    };
    if collapse_blank_lines(next).starts_with(&collapse_blank_lines(body.trim())) {
        return Ok(true);
    }
    let text = body[heading].split('\n').next().unwrap_or("").trim_end();
    Err(SectionError(format!(
        "Cannot replace {} without section markers: \"{}\" may be a separate section. Ask the user to put {} after \"## {}\" and {} after the intended body in the Markdown file, then retry.",
        key.title(),
        text,
        key.begin(),
        key.title(),
        key.end()
    )))
}
fn task_block(key: SectionKey, body: &str, legacy: bool) -> String {
    let body = collapse_blank_lines(normalize(body).trim_end());
    if legacy {
        format!("## {}\n\n{body}", key.title())
    } else {
        format!(
            "## {}\n\n{}\n{}{}",
            key.title(),
            key.begin(),
            if body.is_empty() {
                String::new()
            } else {
                format!("{body}\n")
            },
            key.end()
        )
    }
}
fn append(content: &str, block: &str, at_start: bool) -> String {
    let content = content.trim();
    let block = block.trim();
    if block.is_empty() {
        return content.to_owned();
    }
    if content.is_empty() {
        return block.to_owned();
    }
    if at_start {
        format!("{block}\n\n{content}")
    } else {
        format!("{content}\n\n{block}")
    }
}
fn insert(content: &str, title: &str, block: &str, after: bool) -> Option<String> {
    let span = section_span(content, title)?;
    let at = if after { span.end } else { span.start };
    let before = content[..at].trim_end();
    let rest = content[at..].trim_start();
    Some(format!(
        "{}{}{}{}{}",
        before,
        if before.is_empty() { "" } else { "\n\n" },
        block,
        if rest.is_empty() { "" } else { "\n\n" },
        rest
    ))
}
fn place(content: &str, block: &str, anchors: &[(&str, bool)], at_start: bool) -> String {
    anchors
        .iter()
        .find_map(|(title, after)| insert(content, title, block, *after))
        .unwrap_or_else(|| append(content, block, at_start))
}
fn update_task_sections(
    content: &str,
    changes: &BTreeMap<SectionKey, Option<String>>,
) -> Result<String, SectionError> {
    let src = normalize(content);
    let values: BTreeMap<_, _> = TASK_KEYS
        .into_iter()
        .map(|key| {
            (
                key,
                changes
                    .get(&key)
                    .cloned()
                    .unwrap_or_else(|| extract_section(&src, key))
                    .unwrap_or_default()
                    .trim()
                    .to_owned(),
            )
        })
        .collect();
    let mut working = src.clone();
    let mut legacy = Vec::new();
    for key in TASK_KEYS {
        let mut blocks = sentinel_blocks(&working, key);
        blocks.sort_by_key(|block| std::cmp::Reverse(block.span.start));
        for block in blocks {
            working.replace_range(block.span, "");
        }
        let protected = known_ranges(&tokens(&working));
        let ranges = legacy_sections(&working, key.title(), false, false);
        for range in &ranges {
            if !within(range.heading_start, &protected)
                && keep_legacy(key, &working[range.body.clone()], &values[&key])?
            {
                legacy.push(key);
            }
        }
        for range in ranges.into_iter().rev() {
            working.replace_range(range.span, "\n");
        }
        working = collapse_blank_lines(&working).trim_end().to_owned();
    }
    working = working.trim().to_owned();
    for key in [
        SectionKey::ImplementationPlan,
        SectionKey::ImplementationNotes,
        SectionKey::FinalSummary,
    ] {
        let body = &values[&key];
        if body.is_empty() {
            continue;
        }
        let block = task_block(key, body, legacy.contains(&key));
        let anchors: &[(&str, bool)] = match key {
            SectionKey::ImplementationPlan => &[
                ("Definition of Done", true),
                ("Acceptance Criteria", true),
                ("Description", true),
            ],
            SectionKey::ImplementationNotes => &[
                ("Implementation Plan", true),
                ("Definition of Done", true),
                ("Acceptance Criteria", true),
                ("Comments", false),
                ("Final Summary", false),
            ],
            _ => &[
                ("Comments", true),
                ("Implementation Notes", true),
                ("Implementation Plan", true),
                ("Definition of Done", true),
                ("Acceptance Criteria", true),
            ],
        };
        working = place(
            &working,
            &block,
            anchors,
            key == SectionKey::ImplementationPlan,
        );
    }
    if !values[&SectionKey::Description].is_empty() {
        working = append(
            &working,
            &task_block(
                SectionKey::Description,
                &values[&SectionKey::Description],
                legacy.contains(&SectionKey::Description),
            ),
            true,
        );
    }
    Ok(restore(
        collapse_blank_lines(&working).trim().to_owned(),
        content.contains("\r\n"),
    ))
}

#[derive(Clone)]
struct DecisionRange {
    key: SectionKey,
    body: Range<usize>,
    marked: bool,
}
fn decision_ranges(content: &str) -> Vec<DecisionRange> {
    let blocks: Vec<_> = DECISION_KEYS
        .into_iter()
        .flat_map(|key| {
            sentinel_blocks(content, key)
                .into_iter()
                .map(move |block| (key, block))
        })
        .collect();
    let mut headings = Vec::new();
    let mut offset = 0;
    while offset < content.len() {
        let Some(range) = first_markdown_heading(&content[offset..]) else {
            break;
        };
        let heading = offset + range.start..offset + range.end;
        offset = blocks
            .iter()
            .find(|(_, block)| block.span.start == heading.start)
            .map_or(heading.end, |(_, block)| block.span.end);
        headings.push(heading);
    }
    headings
        .iter()
        .enumerate()
        .filter_map(|(index, heading)| {
            let key = DECISION_KEYS.into_iter().find(|key| {
                content[heading.clone()]
                    .trim_end()
                    .eq_ignore_ascii_case(&format!("## {}", key.title()))
            })?;
            let block = blocks
                .iter()
                .find(|(k, block)| *k == key && block.span.start == heading.start)
                .map(|(_, block)| block);
            let start = content[heading.end..]
                .find('\n')
                .map_or(content.len(), |n| heading.end + n + 1);
            Some(DecisionRange {
                key,
                body: block.map_or(
                    start..headings.get(index + 1).map_or(content.len(), |h| h.start),
                    |b| b.body.clone(),
                ),
                marked: block.is_some(),
            })
        })
        .collect()
}
fn unique_decisions(ranges: &[DecisionRange]) -> Result<(), SectionError> {
    let mut seen = Vec::new();
    for range in ranges {
        if seen.contains(&range.key) {
            return Err(SectionError(format!(
                "Cannot save decision: more than one {} section exists. Keep one managed section before saving.",
                range.key.title()
            )));
        }
        seen.push(range.key);
    }
    Ok(())
}
fn terminated(value: &str, newline: &str) -> String {
    if !value.is_empty() && !value.ends_with('\n') {
        format!("{value}{newline}")
    } else {
        value.to_owned()
    }
}
fn update_decision_sections(
    content: &str,
    changes: &BTreeMap<SectionKey, Option<String>>,
) -> Result<String, SectionError> {
    let newline = if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    update_decision_sections_with_newline(content, changes, newline)
}

/// Update decision sections using the source record's line ending, which may
/// come from its frontmatter header when its body is empty.
pub(crate) fn update_decision_sections_with_newline(
    content: &str,
    changes: &BTreeMap<SectionKey, Option<String>>,
    newline: &str,
) -> Result<String, SectionError> {
    let ranges = decision_ranges(content);
    unique_decisions(&ranges)?;
    let mut edits = Vec::new();
    let mut appended = String::new();
    for key in DECISION_KEYS {
        let value = changes
            .get(&key)
            .map(|v| restore(normalize(v.as_deref().unwrap_or("")), newline == "\r\n"));
        if let Some(value) = &value {
            assert_section_input_safe(value, key)?;
        }
        let Some(range) = ranges.iter().find(|range| range.key == key) else {
            if let Some(body) = value {
                appended.push_str(&format!(
                    "## {}{newline}{newline}{}{newline}{}{}{newline}{newline}",
                    key.title(),
                    key.begin(),
                    terminated(&body, newline),
                    key.end()
                ));
            }
            continue;
        };
        if range.marked {
            if let Some(body) = value {
                edits.push((range.body.clone(), terminated(&body, newline)));
            }
        } else {
            let existing = &content[range.body.clone()];
            assert_section_input_safe(existing, key)?;
            let prefix =
                if range.body.start > 0 && content.as_bytes()[range.body.start - 1] != b'\n' {
                    newline
                } else {
                    ""
                };
            let suffix = if range.body.end == content.len() && !content.ends_with('\n') {
                ""
            } else {
                newline
            };
            edits.push((
                range.body.clone(),
                format!(
                    "{prefix}{}{newline}{}{}{suffix}",
                    key.begin(),
                    terminated(value.as_deref().unwrap_or(existing), newline),
                    key.end()
                ),
            ));
        }
    }
    let mut result = content.to_owned();
    edits.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    for (range, text) in edits {
        result.replace_range(range, &text);
    }
    if !appended.is_empty() {
        if !result.is_empty() && !result.ends_with(&format!("{newline}{newline}")) {
            if !result.ends_with(newline) {
                result.push_str(newline);
            }
            result.push_str(newline);
        }
        result.push_str(&appended);
    }
    Ok(result)
}

/// Omitted keys retain their values. None and an empty string explicitly clear a
/// requested body. Task fields use canonical serialization; decision fields edit
/// in place and migrate existing legacy decision fields to markers.
pub fn update_sections(
    content: &str,
    changes: &BTreeMap<SectionKey, Option<String>>,
) -> Result<String, SectionError> {
    if changes.is_empty() {
        return Ok(content.to_owned());
    }
    let mut result = content.to_owned();
    if changes.keys().any(|key| !key.is_decision()) {
        result = update_task_sections(&result, changes)?;
    }
    if changes.keys().any(|key| key.is_decision()) {
        result = update_decision_sections(&result, changes)?;
    }
    Ok(result)
}

/// Task fields reserve their own whole marker lines; decision fields reserve all
/// SECTION lines. Inline mentions and indented lines remain legal.
pub fn assert_section_input_safe(value: &str, key: SectionKey) -> Result<(), SectionError> {
    for line in normalize(value).split('\n') {
        let line = line.trim_end();
        let reserved = if key.is_decision() {
            section_sentinel_line(line)
        } else {
            line == key.begin() || line == key.end()
        };
        if reserved {
            return Err(SectionError(format!(
                "{} content cannot contain the reserved marker line \"{}\". Indent it with a space to include it as literal text.",
                key.title(),
                line
            )));
        }
    }
    Ok(())
}

/// Validate raw editor structure before extracting editable decision fields.
pub fn validate_decision_editor_structure(
    original: &str,
    edited: &str,
) -> Result<(), SectionError> {
    fn marker_lines(content: &str) -> Vec<&str> {
        content
            .split('\n')
            .map(str::trim_end)
            .filter(|line| section_sentinel_line(line))
            .collect()
    }
    if marker_lines(original) != marker_lines(edited) {
        return Err(SectionError(
            "Keep decision section marker lines unchanged and edit only the section text."
                .to_owned(),
        ));
    }
    unique_decisions(&decision_ranges(edited))
}

#[derive(Clone)]
struct Token {
    family: String,
    begin: bool,
    span: Range<usize>,
}
fn marker_line(line: &str) -> Option<(&str, bool)> {
    let inside = line
        .trim_end_matches(['\t', ' '])
        .strip_prefix("<!-- ")?
        .strip_suffix(" -->")?;
    let (family, kind) = inside.rsplit_once(':')?;
    let known = matches!(family, "COMMENTS" | "COMMENT" | "AC" | "DOD")
        || family.strip_prefix("SECTION:").is_some_and(|id| {
            !id.is_empty()
                && id.as_bytes()[0].is_ascii_uppercase()
                && id
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
        });
    (known && matches!(kind, "BEGIN" | "END")).then_some((family, kind == "BEGIN"))
}
fn section_sentinel_line(line: &str) -> bool {
    line == line.trim_end_matches(['\t', ' '])
        && marker_line(line).is_some_and(|(family, _)| family.starts_with("SECTION:"))
}
fn tokens(content: &str) -> Vec<Token> {
    lines(content)
        .into_iter()
        .filter_map(|(start, line)| {
            marker_line(line).map(|(family, begin)| Token {
                family: family.to_owned(),
                begin,
                span: start..start + line.len(),
            })
        })
        .collect()
}
fn known_ranges(tokens: &[Token]) -> Vec<Range<usize>> {
    let mut pending: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    let mut ranges = Vec::new();
    for token in tokens {
        let stack = pending.entry(&token.family).or_default();
        if token.begin {
            stack.push(token.span.start);
        } else if let Some(start) = stack.pop() {
            ranges.push(start..token.span.end);
        }
    }
    ranges.sort_by_key(|range| (range.start, range.end));
    let mut merged: Vec<Range<usize>> = Vec::new();
    for range in ranges {
        if let Some(last) = merged.last_mut()
            && range.start <= last.end
        {
            last.end = last.end.max(range.end);
        } else {
            merged.push(range);
        }
    }
    merged
}

// Checklist and comments add their family-specific anchors below.
fn section_span(content: &str, title: &str) -> Option<Range<usize>> {
    if let Some(kind) = [
        ChecklistKind::AcceptanceCriteria,
        ChecklistKind::DefinitionOfDone,
    ]
    .into_iter()
    .find(|kind| kind.title().eq_ignore_ascii_case(title))
    {
        return checklist_ranges(content, kind, &checklist_resolution(content, kind), false)
            .into_iter()
            .next()
            .map(|range| range.span);
    }
    let protected = known_ranges(&tokens(content));
    if title.eq_ignore_ascii_case("Comments")
        && let Some(block) = comments_blocks(content)
            .into_iter()
            .find(|block| !within(block.span.start, &protected))
    {
        return Some(block.span);
    }
    if let Some(key) = TASK_KEYS
        .into_iter()
        .find(|key| key.title().eq_ignore_ascii_case(title))
        && let Some(block) = sentinel_blocks(content, key)
            .into_iter()
            .find(|block| !within(block.span.start, &protected))
    {
        return Some(block.span);
    }
    legacy_sections(content, title, false, title == "Comments")
        .into_iter()
        .find(|legacy| !within(legacy.span.start, &protected))
        .map(|legacy| legacy.span)
}

#[derive(Clone, Copy)]
struct Fence {
    character: u8,
    min_length: usize,
}
#[derive(Clone)]
struct HtmlBlock {
    end: Option<String>,
    blank: bool,
    comment: bool,
}
fn leading_spaces(line: &str) -> usize {
    line.bytes().take_while(|b| *b == b' ').count()
}
fn list_column(line: &str) -> Option<usize> {
    let indent = leading_spaces(line);
    let rest = &line[indent..];
    let marker = if rest.starts_with(['-', '+', '*']) {
        1
    } else {
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if !(1..=9).contains(&digits)
            || !rest
                .get(digits..)
                .is_some_and(|s| s.starts_with(['.', ')']))
        {
            return None;
        }
        digits + 1
    };
    let tail = &rest[marker..];
    if !tail.is_empty() && !tail.starts_with(['\t', ' ']) {
        return None;
    }
    Some(
        indent
            + marker
            + tail
                .bytes()
                .take_while(|b| matches!(b, b' ' | b'\t'))
                .count(),
    )
}
fn update_containers(line: &str, columns: &mut Vec<usize>) {
    while columns
        .last()
        .is_some_and(|column| *column > leading_spaces(line))
    {
        columns.pop();
    }
    if let Some(column) = list_column(line) {
        columns.push(column);
    }
}
fn within_container(indent: usize, columns: &[usize]) -> bool {
    indent <= 3
        || columns
            .iter()
            .any(|base| indent >= *base && indent - base <= 3)
}
fn fence_opener(line: &str, columns: &[usize]) -> Option<Fence> {
    let indent = leading_spaces(line);
    if !within_container(indent, columns) {
        return None;
    }
    let rest = &line[indent..];
    let character = *rest.as_bytes().first()?;
    if character != b'\x60' && character != b'~' {
        return None;
    }
    let run = rest.bytes().take_while(|b| *b == character).count();
    if run < 3 || character == b'\x60' && rest[run..].contains('\x60') {
        return None;
    }
    Some(Fence {
        character,
        min_length: run,
    })
}
fn closes_fence(line: &str, fence: Fence, columns: &[usize]) -> bool {
    let indent = leading_spaces(line);
    if !within_container(indent, columns) {
        return false;
    }
    let rest = &line[indent..];
    let run = rest.bytes().take_while(|b| *b == fence.character).count();
    run >= fence.min_length && rest[run..].bytes().all(|b| matches!(b, b' ' | b'\t'))
}
fn html_ends(line: &str, html: &HtmlBlock) -> bool {
    if html.comment {
        line.contains("-->") || line.contains("--!>")
    } else {
        html.end
            .as_ref()
            .is_some_and(|end| line.to_ascii_lowercase().contains(end))
    }
}
fn html_start(line: &str, columns: &[usize]) -> Option<HtmlBlock> {
    let base = columns.last().copied().unwrap_or(0);
    let relative = if base > 0 && leading_spaces(line) >= base {
        &line[base..]
    } else {
        line
    };
    if leading_spaces(relative) > 3 {
        return None;
    }
    let value = relative.trim_start_matches(' ');
    let lower = value.to_ascii_lowercase();
    for tag in ["script", "pre", "style", "textarea"] {
        let prefix = format!("<{tag}");
        if lower
            .strip_prefix(&prefix)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(['\t', ' ', '/', '>']))
        {
            let block = HtmlBlock {
                end: Some(format!("</{tag}>")),
                blank: false,
                comment: false,
            };
            return (!html_ends(value, &block)).then_some(block);
        }
    }
    let ending = if value.starts_with("<!--") {
        Some(("-->", true))
    } else if value.starts_with("<?") {
        Some(("?>", false))
    } else if value.starts_with("<![CDATA[") {
        Some(("]]>", false))
    } else if value
        .strip_prefix("<!")
        .is_some_and(|rest| rest.starts_with(|ch: char| ch.is_ascii_alphabetic()))
    {
        Some((">", false))
    } else {
        None
    };
    if let Some((end, comment)) = ending {
        let block = HtmlBlock {
            end: Some(end.to_owned()),
            blank: false,
            comment,
        };
        return (!html_ends(value, &block)).then_some(block);
    }
    let rest = lower
        .strip_prefix('<')?
        .strip_prefix('/')
        .unwrap_or_else(|| &lower[1..]);
    let name_len = rest
        .bytes()
        .take_while(|b| b.is_ascii_alphanumeric() || *b == b'-')
        .count();
    let name = &rest[..name_len];
    let tail = &rest[name_len..];
    const TAGS: &str = "address article aside base basefont blockquote body caption center col colgroup dd details dialog dir div dl dt fieldset figcaption figure footer form frame frameset h1 h2 h3 h4 h5 h6 head header hr html iframe legend li link main menu menuitem nav noframes ol optgroup option p param search section summary table tbody td tfoot th thead title tr track ul";
    let block_tag = TAGS.split(' ').any(|tag| tag == name)
        && (tail.is_empty() || tail.starts_with(['\t', ' ', '/', '>']));
    let tag_line = !name.is_empty() && name.as_bytes()[0].is_ascii_alphabetic() && {
        let tail = tail.trim_end_matches(['\t', ' ']);
        let inner = tail
            .strip_suffix('>')
            .map(|s| s.strip_suffix('/').unwrap_or(s));
        inner.is_some_and(|s| {
            (s.is_empty() || s.starts_with(['\t', ' '])) && !s.contains(['<', '>'])
        })
    };
    (block_tag || tag_line).then_some(HtmlBlock {
        end: None,
        blank: true,
        comment: false,
    })
}
fn collapse_blank_lines(content: &str) -> String {
    let mut result = Vec::new();
    let mut blanks = 0;
    let mut fence = None;
    let mut html: Option<HtmlBlock> = None;
    let mut columns = Vec::new();
    for line in content.split('\n') {
        if let Some(open) = fence {
            result.push(line);
            if closes_fence(line, open, &columns) {
                fence = None;
            }
            continue;
        }
        if section_sentinel_line(line) {
            html = None;
            columns.clear();
        }
        if line.is_empty() {
            if html.as_ref().is_some_and(|block| block.blank) {
                html = None;
            }
            blanks += 1;
            continue;
        }
        if blanks > 0 {
            result.push("");
            blanks = 0;
        }
        result.push(line);
        if let Some(block) = &html {
            if html_ends(line, block) {
                html = None;
            }
            continue;
        }
        update_containers(line, &mut columns);
        fence = fence_opener(line, &columns);
        if fence.is_none() {
            html = html_start(line, &columns);
        }
    }
    if blanks > 0 {
        result.push("");
    }
    result.join("\n")
}

/// Acceptance Criteria and Definition of Done share one checklist format.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChecklistKind {
    AcceptanceCriteria,
    DefinitionOfDone,
}
impl ChecklistKind {
    fn title(self) -> &'static str {
        match self {
            Self::AcceptanceCriteria => "Acceptance Criteria",
            Self::DefinitionOfDone => "Definition of Done",
        }
    }
    fn marker(self) -> &'static str {
        match self {
            Self::AcceptanceCriteria => "AC",
            Self::DefinitionOfDone => "DOD",
        }
    }
    fn begin(self) -> String {
        format!("<!-- {}:BEGIN -->", self.marker())
    }
    fn end(self) -> String {
        format!("<!-- {}:END -->", self.marker())
    }
}

/// The current application workflow numbers parsed items sequentially, then sorts
/// supplied items by index and writes fresh sequential numbers during replacement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Criterion {
    pub index: usize,
    pub text: String,
    pub checked: bool,
}

struct ChecklistResolution {
    foreign: Vec<Range<usize>>,
    pairs: Vec<(Token, Token)>,
    issue: Option<&'static str>,
}
fn checklist_resolution(content: &str, kind: ChecklistKind) -> ChecklistResolution {
    let all = tokens(content);
    let foreign = known_ranges(
        &all.iter()
            .filter(|t| t.family != kind.marker())
            .cloned()
            .collect::<Vec<_>>(),
    );
    let mut resolution = ChecklistResolution {
        foreign,
        pairs: Vec::new(),
        issue: None,
    };
    let mut begin = None;
    for token in all
        .into_iter()
        .filter(|token| token.family == kind.marker())
    {
        if within(token.span.start, &resolution.foreign) {
            continue;
        }
        if token.begin {
            if begin.is_some() {
                resolution.issue = Some("repeated-begin");
                break;
            }
            begin = Some(token);
        } else {
            let Some(open) = begin.take() else {
                resolution.issue = Some("unexpected-end");
                break;
            };
            resolution.pairs.push((open, token));
        }
    }
    if resolution.issue.is_none() && begin.is_some() {
        resolution.issue = Some("unclosed-begin");
    }
    if resolution.issue.is_some() {
        resolution.pairs.clear();
    }
    resolution
}
fn assert_checklist_resolution(
    resolution: &ChecklistResolution,
    kind: ChecklistKind,
) -> Result<(), SectionError> {
    let Some(issue) = resolution.issue else {
        return Ok(());
    };
    let detail = match issue {
        "unexpected-end" => format!("found {} without a preceding {}", kind.end(), kind.begin()),
        "repeated-begin" => format!(
            "found a second {} before the matching {}",
            kind.begin(),
            kind.end()
        ),
        _ => format!("found {} without a following {}", kind.begin(), kind.end()),
    };
    Err(SectionError(format!(
        "Malformed {} markers: {detail}.",
        kind.title()
    )))
}

struct ChecklistRange {
    span: Range<usize>,
    body: Range<usize>,
    masks: Vec<Range<usize>>,
    marked: bool,
    has_items: bool,
}
fn relative_ranges(ranges: &[Range<usize>], body: &Range<usize>) -> Vec<Range<usize>> {
    ranges
        .iter()
        .filter(|range| overlaps(range, body))
        .map(|range| range.start.max(body.start) - body.start..range.end.min(body.end) - body.start)
        .collect()
}
fn checkbox(line: &str) -> Option<(char, &str)> {
    let rest = line.strip_prefix("- [")?;
    let mark = rest.chars().next()?;
    if matches!(mark, ']' | '\r' | '\n') {
        return None;
    }
    Some((mark, rest[mark.len_utf8()..].strip_prefix("] ")?))
}
fn criterion_text(line: &str, marked: bool) -> Option<(bool, &str)> {
    let (mark, text) = checkbox(line)?;
    if !matches!(mark, ' ' | 'x') || text.is_empty() || text.contains('\r') {
        return None;
    }
    let text = if marked {
        text.strip_prefix('#')
            .and_then(|rest| {
                let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
                (digits > 0)
                    .then(|| rest[digits..].strip_prefix(' '))
                    .flatten()
            })
            .filter(|s| !s.is_empty())
            .unwrap_or(text)
    } else {
        text
    };
    Some((mark == 'x', text))
}
fn parse_checklist_body(body: &str, marked: bool, masks: &[Range<usize>]) -> Vec<Criterion> {
    let mut result = Vec::new();
    for (start, line) in lines(body) {
        if within(start, masks) {
            continue;
        }
        if let Some((checked, text)) = criterion_text(line, marked) {
            result.push(Criterion {
                index: result.len() + 1,
                text: text.to_owned(),
                checked,
            });
        }
    }
    result
}
fn top_level_source_headings(content: &str, masks: &[Range<usize>]) -> Vec<(Range<usize>, String)> {
    lines(content)
        .into_iter()
        .filter_map(|(start, line)| {
            if within(start, masks) {
                return None;
            }
            let tail = line.strip_prefix("##")?;
            if !tail.starts_with(['\t', ' ']) {
                return None;
            }
            let title = tail.trim_matches(['\t', ' ']);
            if title.is_empty() {
                return None;
            }
            Some((
                start..(start + line.len() + 1).min(content.len()),
                title.to_owned(),
            ))
        })
        .collect()
}
fn checklist_ranges(
    content: &str,
    kind: ChecklistKind,
    resolution: &ChecklistResolution,
    include_legacy: bool,
) -> Vec<ChecklistRange> {
    if resolution.issue.is_some() {
        return Vec::new();
    }
    let mut result = Vec::new();
    for (begin, end) in &resolution.pairs {
        let source = lines(&content[..begin.span.start]);
        // The prefix ends on the marker line's empty prefix. Ignore blank lines.
        let header = source
            .iter()
            .rev()
            .find(|(_, line)| !line.trim_matches(['\t', ' ']).is_empty());
        let Some((start, line)) = header else {
            continue;
        };
        if !line
            .trim_end_matches(['\t', ' '])
            .eq_ignore_ascii_case(&format!("## {}", kind.title()))
            || within(*start, &resolution.foreign)
        {
            continue;
        }
        let raw = &content[begin.span.end..end.span.start];
        let leading = raw
            .find('\n')
            .filter(|n| raw[..*n].bytes().all(|b| matches!(b, b' ' | b'\t')))
            .map_or(0, |n| n + 1);
        let body = begin.span.end + leading..end.span.start;
        let masks = relative_ranges(&resolution.foreign, &body);
        let has_items = !parse_checklist_body(&content[body.clone()], true, &masks).is_empty();
        result.push(ChecklistRange {
            span: *start..end.span.end,
            body,
            masks,
            marked: true,
            has_items,
        });
    }
    if !result.is_empty() && !include_legacy {
        result.sort_by_key(|range| range.span.start);
        return result;
    }
    let headings = top_level_source_headings(content, &resolution.foreign);
    for (index, (heading, title)) in headings.iter().enumerate() {
        if !title.eq_ignore_ascii_case(kind.title()) {
            continue;
        }
        let span = heading.start
            ..headings
                .get(index + 1)
                .map_or(content.len(), |(h, _)| h.start);
        if result.iter().any(|range| overlaps(&range.span, &span)) {
            continue;
        }
        let body = heading.end..span.end;
        let masks = relative_ranges(&resolution.foreign, &body);
        let has_items = !parse_checklist_body(&content[body.clone()], false, &masks).is_empty();
        result.push(ChecklistRange {
            span,
            body,
            masks,
            marked: false,
            has_items,
        });
    }
    result.sort_by_key(|range| range.span.start);
    result
}

/// Read all supported rows in visible marked sections, falling back to legacy
/// sections. Indices match the sequential indexed-edit workflow, not stored # IDs.
pub fn parse_criteria(content: &str, kind: ChecklistKind) -> Vec<Criterion> {
    let src = normalize(content);
    let resolution = checklist_resolution(&src, kind);
    let ranges = checklist_ranges(&src, kind, &resolution, false);
    let mut marked = Vec::new();
    let mut legacy = Vec::new();
    for range in ranges {
        let target = if range.marked {
            &mut marked
        } else {
            &mut legacy
        };
        for mut item in parse_checklist_body(&src[range.body], range.marked, &range.masks) {
            item.index = target.len() + 1;
            target.push(item);
        }
    }
    if marked.is_empty() { legacy } else { marked }
}
fn compose_checklist(criteria: &[Criterion], existing: &str, masks: &[Range<usize>]) -> String {
    let mut sorted: Vec<_> = criteria.iter().collect();
    sorted.sort_by_key(|item| item.index);
    let mut queue = sorted.into_iter();
    let mut result = Vec::new();
    let mut next = 1;
    let row = |item: &Criterion, index| {
        format!(
            "- [{}] #{index} {}",
            if item.checked { 'x' } else { ' ' },
            item.text
        )
    };
    if !existing.is_empty() {
        for (start, line) in lines(existing) {
            let is_checkbox = !within(start, masks)
                && checkbox(line.trim()).is_some_and(|(mark, _)| matches!(mark, ' ' | 'x'));
            if is_checkbox {
                if let Some(item) = queue.next() {
                    result.push(row(item, next));
                    next += 1;
                }
            } else {
                result.push(line.to_owned());
            }
        }
    }
    while result.first().is_some_and(|line| line.trim().is_empty()) {
        result.remove(0);
    }
    while result.last().is_some_and(|line| line.trim().is_empty()) {
        result.pop();
    }
    for item in queue {
        if result
            .last()
            .is_some_and(|line| !line.trim().is_empty() && !line.trim().starts_with("- ["))
        {
            result.push(String::new());
        }
        result.push(row(item, next));
        next += 1;
    }
    result.join("\n")
}

/// Replace checklist rows while retaining non-row source in the first mutable
/// section and opaque foreign blocks. Call mark validation before indexed edits.
pub fn update_criteria(
    content: &str,
    kind: ChecklistKind,
    criteria: &[Criterion],
) -> Result<String, SectionError> {
    let src = normalize(content);
    let resolution = checklist_resolution(&src, kind);
    assert_checklist_resolution(&resolution, kind)?;
    let ranges: Vec<_> = checklist_ranges(&src, kind, &resolution, true)
        .into_iter()
        .filter(|range| range.marked || range.has_items)
        .collect();
    if criteria.is_empty() && ranges.is_empty() {
        return Ok(content.to_owned());
    }
    let body = ranges.first().map_or_else(
        || compose_checklist(criteria, "", &[]),
        |range| compose_checklist(criteria, &src[range.body.clone()], &range.masks),
    );
    let section = if body.trim().is_empty() {
        String::new()
    } else {
        format!(
            "## {}\n{}\n{body}\n{}",
            kind.title(),
            kind.begin(),
            kind.end()
        )
    };
    let mut stripped = src.clone();
    for range in ranges.into_iter().rev() {
        let before = stripped[..range.span.start].trim_end();
        let after = stripped[range.span.end..].trim_start();
        stripped = if !before.is_empty() && !after.is_empty() {
            format!("{before}\n\n{after}")
        } else {
            format!("{before}{after}")
        };
    }
    stripped = stripped.trim().to_owned();
    if section.is_empty() {
        return Ok(restore(stripped, content.contains("\r\n")));
    }
    let anchors: &[(&str, bool)] = match kind {
        ChecklistKind::AcceptanceCriteria => &[
            ("Description", true),
            ("Definition of Done", false),
            ("Implementation Plan", false),
            ("Implementation Notes", false),
            ("Comments", false),
            ("Final Summary", false),
        ],
        ChecklistKind::DefinitionOfDone => &[
            ("Acceptance Criteria", true),
            ("Description", true),
            ("Implementation Plan", false),
            ("Implementation Notes", false),
            ("Comments", false),
            ("Final Summary", false),
        ],
    };
    Ok(restore(
        place(&stripped, &section, anchors, false).trim().to_owned(),
        content.contains("\r\n"),
    ))
}

/// Refuse unsupported marks in managed rows before indexed changes. Fences, HTML
/// blocks and known foreign sentinel ranges are examples, not invalid live rows.
pub fn assert_valid_checklist_marks(
    content: &str,
    kind: ChecklistKind,
) -> Result<(), SectionError> {
    let src = normalize(content);
    let resolution = checklist_resolution(&src, kind);
    let ranges: Vec<_> = checklist_ranges(&src, kind, &resolution, false)
        .into_iter()
        .map(|range| range.span)
        .collect();
    if ranges.is_empty() {
        return Ok(());
    }
    let mut fence = None;
    let mut html: Option<HtmlBlock> = None;
    let mut columns = Vec::new();
    for (start, line) in lines(&src) {
        if within(start, &resolution.foreign) {
            continue;
        }
        if let Some(open) = fence {
            if closes_fence(line, open, &columns) {
                fence = None;
            }
            continue;
        }
        if let Some(block) = &html {
            if html_ends(line, block) || block.blank && line.trim().is_empty() {
                html = None;
            }
            continue;
        }
        update_containers(line, &mut columns);
        fence = fence_opener(line, &columns);
        if fence.is_some() {
            continue;
        }
        html = html_start(line, &columns);
        if html.is_some() || !within(start, &ranges) {
            continue;
        }
        if let Some((mark, text)) = checkbox(line)
            && !text.is_empty()
            && !matches!(mark, ' ' | 'x')
        {
            return Err(SectionError(format!(
                "Invalid {} checkbox mark in row \"{line}\". Use [ ] or [x].",
                kind.title()
            )));
        }
    }
    Ok(())
}

/// A stored comment with its written UTC date text and optional author.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Comment {
    pub index: usize,
    pub body: String,
    pub created_date: String,
    pub author: Option<String>,
}

// Comments use an outer managed block and either the older COMMENT block format
// or ordinary Markdown delimiter pairs. Freeform Comments headings are unmanaged.
fn ascii_find(content: &str, needle: &str) -> Option<usize> {
    content
        .to_ascii_lowercase()
        .find(&needle.to_ascii_lowercase())
}
fn comments_blocks(content: &str) -> Vec<Block> {
    let protected: Vec<_> = task_ranges(content)
        .into_iter()
        .map(|(_, span)| span)
        .collect();
    let mut blocks = Vec::new();
    for (start, line) in lines(content) {
        if !line.trim_end().eq_ignore_ascii_case("## Comments") {
            continue;
        }
        let span_start = if start == 0 { 0 } else { start - 1 };
        if within(span_start, &protected) {
            continue;
        }
        let after_heading = start + line.len();
        let tail = &content[after_heading..];
        let ws = tail.len() - tail.trim_start().len();
        if !tail[..ws].contains('\n') {
            continue;
        }
        let begin = after_heading + ws;
        let marker = "<!-- COMMENTS:BEGIN -->";
        if !content
            .get(begin..begin + marker.len())
            .is_some_and(|s| s.eq_ignore_ascii_case(marker))
        {
            continue;
        }
        let after_begin = begin + marker.len();
        let tail = &content[after_begin..];
        let ws = tail.len() - tail.trim_start().len();
        if !tail[..ws].contains('\n') {
            continue;
        }
        let body_start = after_begin + ws;
        let Some(relative_end) = ascii_find(&content[body_start..], "<!-- COMMENTS:END -->") else {
            continue;
        };
        let body_end = body_start + relative_end;
        let span = span_start..body_end + "<!-- COMMENTS:END -->".len();
        if blocks
            .iter()
            .any(|block: &Block| overlaps(&block.span, &span))
        {
            continue;
        }
        blocks.push(Block {
            span,
            body: body_start..body_end,
        });
    }
    blocks
}
fn normalized_metadata(value: &str) -> Option<String> {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    (!normalized.is_empty()).then_some(normalized)
}
fn parse_positive_prefix(value: &str) -> Option<usize> {
    let value = value
        .trim_start()
        .strip_prefix('+')
        .unwrap_or(value.trim_start());
    let digits = value.bytes().take_while(u8::is_ascii_digit).count();
    value
        .get(..digits)?
        .parse::<usize>()
        .ok()
        .filter(|n| *n > 0)
}
fn comment_metadata(lines: &[&str], fallback: usize) -> (usize, Option<String>, String) {
    let mut index = fallback;
    let mut author = None;
    let mut created = String::new();
    for line in lines {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if key.is_empty() || !key.bytes().all(|b| b.is_ascii_alphabetic() || b == b'_') {
            continue;
        }
        match key.to_ascii_lowercase().as_str() {
            "index" => {
                if let Some(value) = parse_positive_prefix(value) {
                    index = value;
                }
            }
            "author" => author = normalized_metadata(value),
            "created" => created = value.trim().to_owned(),
            _ => {}
        }
    }
    (index, author, created)
}
fn legacy_comment(block: &str, fallback: usize) -> Option<Comment> {
    let block = block.trim();
    if block.is_empty() {
        return None;
    }
    let source = lines(block);
    let separator = source.iter().position(|(_, line)| line.trim().is_empty());
    let (metadata, body) = if let Some(separator) = separator {
        let start = source[separator].0.saturating_sub(1);
        (&block[..start], block[start..].trim())
    } else {
        ("", block)
    };
    if body.is_empty() {
        return None;
    }
    let (index, author, created_date) =
        comment_metadata(&metadata.split('\n').collect::<Vec<_>>(), fallback);
    Some(Comment {
        index,
        author,
        created_date,
        body: body.to_owned(),
    })
}
fn delimited_comments(body: &str) -> Vec<Comment> {
    let source: Vec<_> = body.split('\n').collect();
    let mut result = Vec::new();
    let mut i = 0;
    while i < source.len() {
        while i < source.len() && source[i].trim().is_empty() {
            i += 1;
        }
        let metadata_start = i;
        while i < source.len() && source[i].trim() != "---" {
            i += 1;
        }
        if i == source.len() {
            break;
        }
        let metadata = &source[metadata_start..i];
        i += 1;
        let body_start = i;
        while i < source.len() && source[i].trim() != "---" {
            i += 1;
        }
        if i == source.len() {
            break;
        }
        let body = source[body_start..i].join("\n").trim().to_owned();
        i += 1;
        if body.is_empty() {
            continue;
        }
        let (_, author, created_date) = comment_metadata(metadata, result.len() + 1);
        result.push(Comment {
            index: result.len() + 1,
            author,
            created_date,
            body,
        });
    }
    result
}

/// Parse only managed comments, retaining legacy explicit indices and written
/// date text. Canonical delimiter comments get sequential indices in source order.
pub fn parse_comments(content: &str) -> Vec<Comment> {
    let src = normalize(content);
    let Some(block) = comments_blocks(&src).into_iter().next() else {
        return Vec::new();
    };
    let body = &src[block.body];
    if !body.contains("<!-- COMMENT:BEGIN -->") {
        return delimited_comments(body);
    }
    let mut result = Vec::new();
    let mut offset = 0;
    while let Some(start) = ascii_find(&body[offset..], "<!-- COMMENT:BEGIN -->") {
        let after = offset + start + "<!-- COMMENT:BEGIN -->".len();
        let tail = &body[after..];
        let ws = tail.len() - tail.trim_start().len();
        if !tail[..ws].contains('\n') {
            offset = after;
            continue;
        }
        let content_start = after + ws;
        let Some(end) = ascii_find(&body[content_start..], "<!-- COMMENT:END -->") else {
            break;
        };
        if let Some(comment) =
            legacy_comment(&body[content_start..content_start + end], result.len() + 1)
        {
            result.push(comment);
        }
        offset = content_start + end + "<!-- COMMENT:END -->".len();
    }
    result
}
fn assert_comment_field(value: &str, field: &str) -> Result<(), SectionError> {
    let lower = value.to_ascii_lowercase();
    let marker = lower.match_indices("<!--").any(|(start, _)| {
        let tail = lower[start + 4..].trim_start();
        tail.starts_with("comment:") || tail.starts_with("comments:")
    });
    if marker {
        return Err(SectionError(format!(
            "Comment {field} cannot contain Backlog comment markers."
        )));
    }
    if normalize(value)
        .split('\n')
        .any(|line| line.trim() == "---")
    {
        return Err(SectionError(format!(
            "Comment {field} cannot contain standalone '---' delimiter lines."
        )));
    }
    Ok(())
}
fn format_comments(comments: &[Comment]) -> Result<String, SectionError> {
    let mut blocks = Vec::new();
    for comment in comments
        .iter()
        .filter(|comment| !comment.body.trim().is_empty())
    {
        let body = normalize(&comment.body).trim().to_owned();
        assert_comment_field(&body, "body")?;
        let mut metadata = Vec::new();
        if let Some(author) = comment.author.as_deref().and_then(normalized_metadata) {
            assert_comment_field(&author, "author")?;
            metadata.push(format!("author: {author}"));
        }
        let date = comment.created_date.trim();
        assert_comment_field(date, "created date")?;
        if !date.is_empty() {
            metadata.push(format!("created: {date}"));
        }
        metadata.extend([
            "".to_owned(),
            "---".to_owned(),
            "".to_owned(),
            body,
            "".to_owned(),
            "---".to_owned(),
        ]);
        blocks.push(metadata.join("\n"));
    }
    if blocks.is_empty() {
        return Ok(String::new());
    }
    Ok(format!(
        "## Comments\n\n<!-- COMMENTS:BEGIN -->\n{}\n<!-- COMMENTS:END -->",
        blocks.join("\n\n")
    ))
}

/// Rewrite managed comments in canonical delimiter form. Preserve freeform
/// namesake prose and comments embedded in structured task bodies.
pub fn update_comments(content: &str, comments: &[Comment]) -> Result<String, SectionError> {
    let src = normalize(content);
    let mut stripped = src.clone();
    for block in comments_blocks(&src).into_iter().rev() {
        stripped.replace_range(block.span, "\n");
    }
    stripped = collapse_blank_lines(&stripped).trim().to_owned();
    let section = format_comments(comments)?;
    if section.is_empty() {
        return Ok(restore(stripped, content.contains("\r\n")));
    }
    let anchors = [
        ("Implementation Notes", true),
        ("Implementation Plan", true),
        ("Final Summary", false),
        ("Definition of Done", true),
        ("Acceptance Criteria", true),
        ("Description", true),
    ];
    let output = place(&stripped, &section, &anchors, false);
    Ok(restore(
        collapse_blank_lines(&output).trim().to_owned(),
        content.contains("\r\n"),
    ))
}
