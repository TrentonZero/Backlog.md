use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;
use yaml_edit::{Mapping, YamlFile};

#[derive(Debug)]
pub struct ParsedMarkdown {
    pub data: Map<String, Value>,
    pub content: String,
}

#[derive(Debug)]
pub enum FrontmatterError {
    MissingClosingDelimiter,
    ExpectedMapping,
    InvalidYaml(String),
    ReferenceChange,
}

impl fmt::Display for FrontmatterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingClosingDelimiter => {
                write!(formatter, "Missing closing frontmatter delimiter")
            }
            Self::ExpectedMapping => write!(formatter, "Record frontmatter must be a YAML mapping"),
            Self::InvalidYaml(reason) => write!(formatter, "Invalid frontmatter: {reason}"),
            Self::ReferenceChange => write!(
                formatter,
                "The update would change preserved metadata or a YAML reference would prevent the requested change"
            ),
        }
    }
}

impl std::error::Error for FrontmatterError {}

struct Header<'a> {
    opening: &'a str,
    yaml: &'a str,
    closing: &'a str,
    body: &'a str,
}

fn split_header(source: &str) -> Result<Option<Header<'_>>, FrontmatterError> {
    let without_bom = source.strip_prefix('\u{feff}').unwrap_or(source);
    let opening_length = if without_bom.starts_with("---\r\n") {
        5
    } else if without_bom.starts_with("---\n") {
        4
    } else {
        return Ok(None);
    } + source.len()
        - without_bom.len();
    let mut position = opening_length;
    for line in source[opening_length..].split_inclusive('\n') {
        if line.trim_end() == "---" {
            return Ok(Some(Header {
                opening: &source[..opening_length],
                yaml: &source[opening_length..position],
                closing: line,
                body: &source[position + line.len()..],
            }));
        }
        position += line.len();
    }
    Err(FrontmatterError::MissingClosingDelimiter)
}

// These legacy scalar conventions are accepted by the TypeScript product too.
// Apply them only to semantic parsing; the original YAML remains the edit source.
fn preprocess(yaml: &str) -> String {
    yaml.split('\n')
        .map(|line| {
            let line = line.strip_suffix('\r').unwrap_or(line);
            let Some((key, raw)) = line.split_once(':') else {
                return line.to_owned();
            };
            let prefix = &line[..key.len() + 1];
            let value = raw.trim();
            let quote = |text: &str| Value::String(text.to_owned()).to_string();
            if ["due_date", "\"due_date\"", "'due_date'"].contains(&key.trim()) {
                let comment = raw
                    .as_bytes()
                    .windows(2)
                    .position(|pair| pair[0].is_ascii_whitespace() && pair[1] == b'#');
                let (scalar, suffix) = comment.map_or((raw, ""), |at| (&raw[..at], &raw[at..]));
                let scalar = scalar.trim();
                if scalar.is_empty()
                    || scalar.eq_ignore_ascii_case("null")
                    || scalar == "~"
                    || scalar.starts_with(['\'', '"'])
                {
                    return line.to_owned();
                }
                return format!("{prefix} {}{suffix}", quote(scalar));
            }
            if !["assignee", "reporter"].contains(&key.trim()) {
                return line.to_owned();
            }
            if let Some(list) = value.strip_prefix('[')
                && let Some(end) = list.rfind(']')
            {
                let suffix = list[end + 1..].trim();
                if suffix.is_empty() || suffix.starts_with('#') {
                    let items = list[..end]
                        .split(',')
                        .map(str::trim)
                        .filter(|item| !item.is_empty())
                        .map(|item| {
                            if item.starts_with('@') {
                                quote(item)
                            } else {
                                item.to_owned()
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    return format!(
                        "{prefix} [{items}]{}",
                        if suffix.is_empty() {
                            String::new()
                        } else {
                            format!(" {suffix}")
                        }
                    );
                }
            }
            if !value.is_empty() && !value.starts_with(['[', '\'', '"', '-']) {
                return format!("{prefix} {}", quote(value));
            }
            line.to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_yaml(yaml: &str) -> Result<Map<String, Value>, FrontmatterError> {
    parse_yaml_mapping(&preprocess(yaml))
}

pub(crate) fn parse_yaml_mapping(yaml: &str) -> Result<Map<String, Value>, FrontmatterError> {
    let mut parsed: serde_yaml_ng::Value = serde_yaml_ng::from_str(&canonical_integers(yaml)?)
        .map_err(|error| FrontmatterError::InvalidYaml(error.to_string()))?;
    parsed
        .apply_merge()
        .map_err(|error| FrontmatterError::InvalidYaml(error.to_string()))?;
    let parsed = serde_json::to_value(parsed)
        .map_err(|error| FrontmatterError::InvalidYaml(error.to_string()))?;
    match parsed {
        Value::Object(data) => Ok(data),
        Value::Null => Ok(Map::new()),
        _ => Err(FrontmatterError::ExpectedMapping),
    }
}

// js-yaml reads bare 007 as decimal 7; serde_yaml_ng leaves it a string.
// Canonicalize only actual unquoted integer nodes in the semantic parser input.
// Original source and explicitly tagged/quoted strings remain untouched.
fn canonical_integers(yaml: &str) -> Result<String, FrontmatterError> {
    let syntax = YamlFile::from_str(yaml)
        .map_err(|error| FrontmatterError::InvalidYaml(error.to_string()))?;
    let mut pending = Vec::new();
    for document in syntax.documents() {
        if let Some(mapping) = document.as_mapping() {
            for (key, value) in mapping.iter() {
                pending.extend([key, value]);
            }
        }
    }
    let mut replacements = Vec::new();
    while let Some(node) = pending.pop() {
        if let Some(integer) = node.to_i64() {
            let range = node.byte_range();
            replacements.push((
                range.start as usize,
                range.end as usize,
                integer.to_string(),
            ));
        } else if let Some(mapping) = node.as_mapping() {
            for (key, value) in mapping.iter() {
                pending.extend([key, value]);
            }
        } else if let Some(sequence) = node.as_sequence() {
            pending.extend(sequence.values());
        }
    }
    replacements.sort_by_key(|(start, _, _)| std::cmp::Reverse(*start));
    let mut normalized = yaml.to_owned();
    for (start, end, value) in replacements {
        normalized.replace_range(start..end, &value);
    }
    Ok(normalized)
}

fn preserved_entries(mapping: &Mapping, changes: &BTreeMap<String, Option<Value>>) -> Vec<String> {
    mapping
        .entries()
        .filter(|entry| !changes.keys().any(|key| entry.key_matches(key.as_str())))
        .map(|entry| entry.to_string())
        .collect()
}

pub fn parse_frontmatter(source: &str) -> Result<ParsedMarkdown, FrontmatterError> {
    let Some(header) = split_header(source)? else {
        return Ok(ParsedMarkdown {
            data: Map::new(),
            content: source.to_owned(),
        });
    };
    Ok(ParsedMarkdown {
        data: parse_yaml(header.yaml)?,
        content: header.body.to_owned(),
    })
}

pub fn update_frontmatter(
    source: &str,
    changes: &BTreeMap<String, Option<Value>>,
) -> Result<String, FrontmatterError> {
    let original = parse_frontmatter(source)?.data;
    let mut expected = original.clone();
    for (key, value) in changes {
        match value {
            Some(value) => {
                expected.insert(key.clone(), value.clone());
            }
            None => {
                expected.remove(key);
            }
        }
    }
    if expected == original {
        return Ok(source.to_owned());
    }
    let header = split_header(source)?;
    let yaml = header.as_ref().map_or("{}\n", |header| header.yaml);
    let yaml = if yaml
        .lines()
        .all(|line| line.trim().is_empty() || line.trim_start().starts_with('#'))
    {
        format!(
            "{yaml}{{}}{}",
            if header
                .as_ref()
                .is_some_and(|header| header.opening.ends_with("\r\n"))
            {
                "\r\n"
            } else {
                "\n"
            }
        )
    } else {
        yaml.to_owned()
    };
    let syntax = YamlFile::from_str(&yaml)
        .map_err(|error| FrontmatterError::InvalidYaml(error.to_string()))?;
    let document = syntax
        .documents()
        .next()
        .ok_or(FrontmatterError::ExpectedMapping)?;
    let mapping = document
        .as_mapping()
        .ok_or(FrontmatterError::ExpectedMapping)?;
    let preserved = preserved_entries(&mapping, changes);
    for (key, value) in changes {
        if original.get(key) == value.as_ref() {
            continue;
        }
        match value {
            Some(value) => {
                let replacement = serde_yaml_ng::to_string(&BTreeMap::from([("value", value)]))
                    .map_err(|error| FrontmatterError::InvalidYaml(error.to_string()))?;
                let replacement = YamlFile::from_str(&replacement)
                    .map_err(|error| FrontmatterError::InvalidYaml(error.to_string()))?;
                let node = replacement
                    .documents()
                    .next()
                    .and_then(|document| document.get("value"))
                    .ok_or(FrontmatterError::ExpectedMapping)?;
                mapping.set(key.as_str(), node);
            }
            None => {
                mapping.remove(key.as_str());
            }
        }
    }
    let edited = syntax.to_string();
    if preserved_entries(&mapping, changes) != preserved || parse_yaml(&edited)? != expected {
        return Err(FrontmatterError::ReferenceChange);
    }
    Ok(match header {
        Some(header) => format!(
            "{}{}{}{}",
            header.opening, edited, header.closing, header.body
        ),
        None => format!("---\n{edited}---\n{source}"),
    })
}

#[cfg(test)]
mod tests {
    use super::{FrontmatterError, parse_frontmatter, parse_yaml_mapping};
    use serde_json::json;

    #[test]
    fn raw_yaml_preserves_bare_integer_and_explicit_string_types() {
        for (scalar, expected) in [
            ("007", json!(7)),
            ("'007'", json!("007")),
            ("\"007\"", json!("007")),
            ("!!str 007", json!("007")),
        ] {
            assert_eq!(
                parse_yaml_mapping(&format!("default_assignee: {scalar}\n")).unwrap()["default_assignee"],
                expected
            );
        }
    }

    #[test]
    fn raw_yaml_does_not_accept_record_assignee_conventions() {
        for key in ["default_assignee", "assignee", "reporter"] {
            assert!(matches!(
                parse_yaml_mapping(&format!("{key}: @alex\n")),
                Err(FrontmatterError::InvalidYaml(_))
            ));
        }
        assert_eq!(
            parse_frontmatter("---\nassignee: @alex\n---\n")
                .unwrap()
                .data["assignee"],
            "@alex"
        );
    }

    #[test]
    fn raw_yaml_does_not_apply_record_due_date_coercion() {
        assert_eq!(
            parse_yaml_mapping("due_date: 007\n").unwrap()["due_date"],
            json!(7)
        );
        assert_eq!(
            parse_frontmatter("---\ndue_date: 007\n---\n").unwrap().data["due_date"],
            "007"
        );
    }

    #[test]
    fn raw_yaml_resolves_external_merges_and_requires_mapping_or_null_roots() {
        let parsed = parse_yaml_mapping(
            "defaults: &defaults {default_assignee: ['@alex']}\n<<: *defaults\n",
        )
        .unwrap();
        assert_eq!(parsed["default_assignee"], json!(["@alex"]));
        assert!(parse_yaml_mapping("null\n").unwrap().is_empty());
        assert!(matches!(
            parse_yaml_mapping("text\n"),
            Err(FrontmatterError::ExpectedMapping)
        ));
    }
}
