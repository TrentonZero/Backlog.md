//! Local decision reads and mutations.

use crate::{
    config::Config,
    git::GitRepository,
    identity::{
        AmbiguousIdError, HasEntityId, decision_id_key, entity_ids_equal, extract_id_from_filename,
        find_unique_entity_by_id, generate_next_id, normalize_entity_id,
    },
    records::{self, DecisionRecord},
    sections::{self, SectionKey},
    serialization,
    storage::Storage,
    task_file_ids::list_task_file_paths,
};
use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, PartialEq)]
pub struct DecisionFileEntry {
    pub decision: DecisionRecord,
    pub path: PathBuf,
    pub archived: bool,
}

impl HasEntityId for DecisionFileEntry {
    fn entity_id(&self) -> &str {
        &self.decision.id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionCreateInput {
    pub title: String,
    pub status: Option<String>,
    pub context: String,
    pub decision: String,
    pub consequences: String,
    pub alternatives: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DecisionUpdateInput {
    pub title: Option<String>,
    pub status: Option<String>,
    pub context: Option<String>,
    pub decision: Option<String>,
    pub consequences: Option<String>,
    pub alternatives: Option<String>,
    /// `None` keeps the current link; an empty string clears it.
    pub superseded_by: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecisionStoreError {
    Ambiguous(AmbiguousIdError),
    Archived(String),
    NotFound(String),
    Refused(String),
    Lock(String),
    Storage(String),
    Serialization(String),
}

impl fmt::Display for DecisionStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ambiguous(error) => error.fmt(formatter),
            Self::Archived(id) => write!(
                formatter,
                "Decision {id} is archived. Archived decisions are read-only. Restore it first: backlog decision restore {id}"
            ),
            Self::NotFound(id) => write!(formatter, "Decision not found: {id}"),
            Self::Refused(message)
            | Self::Lock(message)
            | Self::Storage(message)
            | Self::Serialization(message) => formatter.write_str(message),
        }
    }
}

impl Error for DecisionStoreError {}

#[derive(Clone, Debug)]
pub struct DecisionStore {
    project_root: PathBuf,
    backlog_directory: PathBuf,
    decisions_directory: PathBuf,
    archive_directory: PathBuf,
    storage: Storage,
}

impl DecisionStore {
    pub fn new(
        project_root: impl Into<PathBuf>,
        backlog_relative_directory: impl Into<PathBuf>,
    ) -> Self {
        let project_root = project_root.into();
        let backlog_directory = project_root.join(backlog_relative_directory.into());
        let repository = GitRepository::new(&project_root);
        Self {
            storage: Storage::new(&backlog_directory, repository.common_directory()),
            decisions_directory: backlog_directory.join("decisions"),
            archive_directory: backlog_directory.join("archive/decisions"),
            project_root,
            backlog_directory,
        }
    }

    pub fn list_decisions(&self) -> Vec<DecisionFileEntry> {
        self.read_directory(&self.decisions_directory, false)
    }

    pub fn list_archived_decisions(&self) -> Vec<DecisionFileEntry> {
        self.read_directory(&self.archive_directory, true)
    }

    pub fn get_decision(&self, id: &str) -> Result<Option<DecisionFileEntry>, DecisionStoreError> {
        self.resolve_decision(id)
    }

    pub fn create_decision(
        &self,
        input: DecisionCreateInput,
        config: &Config,
    ) -> Result<DecisionFileEntry, DecisionStoreError> {
        validate_input(
            &input.title,
            input.status.as_deref(),
            [
                Some(input.context.as_str()),
                Some(input.decision.as_str()),
                Some(input.consequences.as_str()),
                input.alternatives.as_deref(),
            ],
        )?;
        let lock = self
            .storage
            .acquire_create_lock()
            .map_err(|error| DecisionStoreError::Lock(error.to_string()))?;
        let id = self.next_id(config);
        let record = DecisionRecord {
            id: id.clone(),
            title: input.title,
            date: utc_timestamp(),
            updated_date: None,
            superseded_by: None,
            status: input.status.unwrap_or_else(|| "proposed".into()),
            context: input.context,
            decision: input.decision,
            consequences: input.consequences,
            alternatives: input.alternatives,
            raw_content: String::new(),
        };
        let source = serialization::serialize_decision(&record, "")
            .map_err(|error| DecisionStoreError::Serialization(error.to_string()))?;
        let target = self.decisions_directory.join(decision_filename(&record));
        lock.check()
            .map_err(|error| DecisionStoreError::Lock(error.to_string()))?;
        self.storage
            .write_record(&target, &source, None)
            .map_err(|error| DecisionStoreError::Storage(error.to_string()))?;
        self.read_entry(target, false)
    }

    pub fn update_decision(
        &self,
        id: &str,
        input: DecisionUpdateInput,
    ) -> Result<DecisionFileEntry, DecisionStoreError> {
        validate_input(
            input.title.as_deref().unwrap_or("existing"),
            input.status.as_deref(),
            [
                input.context.as_deref(),
                input.decision.as_deref(),
                input.consequences.as_deref(),
                input.alternatives.as_deref(),
            ],
        )?;
        let target = self
            .resolve_decision(id)?
            .ok_or_else(|| DecisionStoreError::NotFound(id.into()))?;
        if target.archived {
            return Err(DecisionStoreError::Archived(target.decision.id));
        }
        let lock_id = decision_id_key(&target.decision.id)
            .unwrap_or_else(|| normalize_entity_id("decision", &target.decision.id));
        let lock = self
            .storage
            .acquire_entity_lock("decision", &lock_id, &target.path)
            .map_err(|error| DecisionStoreError::Lock(error.to_string()))?;
        let current = self
            .resolve_decision(id)?
            .ok_or_else(|| DecisionStoreError::NotFound(id.into()))?;
        if current.archived {
            return Err(DecisionStoreError::Archived(current.decision.id));
        }
        if current.path != target.path {
            return Err(DecisionStoreError::Lock(format!(
                "Decision {} was moved or removed by another process.",
                target.decision.id
            )));
        }
        let original = self
            .storage
            .read(&current.path)
            .map_err(|error| DecisionStoreError::Storage(error.to_string()))?;
        let mut updated = current.decision.clone();
        if let Some(value) = input.title {
            updated.title = value;
        }
        if let Some(value) = input.status {
            updated.status = value;
        }
        if let Some(value) = input.context {
            updated.context = value;
        }
        if let Some(value) = input.decision {
            updated.decision = value;
        }
        if let Some(value) = input.consequences {
            updated.consequences = value;
        }
        if let Some(value) = input.alternatives {
            updated.alternatives = Some(value);
        }
        if let Some(value) = input.superseded_by {
            if value.is_empty() {
                updated.superseded_by = None;
            } else {
                let replacement = self.resolve_decision(&value)?.ok_or_else(|| {
                    DecisionStoreError::Refused(format!("Replacement decision {value} not found."))
                })?;
                if entity_ids_equal("decision", &replacement.decision.id, &updated.id) {
                    return Err(DecisionStoreError::Refused(
                        "A decision cannot supersede itself.".into(),
                    ));
                }
                updated.superseded_by =
                    decision_id_key(&replacement.decision.id).or(Some(replacement.decision.id));
            }
        }
        let source = serialization::serialize_decision(&updated, &original)
            .map_err(|error| DecisionStoreError::Serialization(error.to_string()))?;
        if source != original {
            updated.updated_date = Some(utc_timestamp());
        }
        let source = serialization::serialize_decision(&updated, &original)
            .map_err(|error| DecisionStoreError::Serialization(error.to_string()))?;
        let target = self.decisions_directory.join(decision_filename(&updated));
        lock.check()
            .map_err(|error| DecisionStoreError::Lock(error.to_string()))?;
        self.storage
            .write_record(&target, &source, Some(&current.path))
            .map_err(|error| DecisionStoreError::Storage(error.to_string()))?;
        self.read_entry(target, false)
    }

    fn read_directory(&self, directory: &Path, archived: bool) -> Vec<DecisionFileEntry> {
        let mut entries: Vec<_> = list_task_file_paths(directory, "decision")
            .into_iter()
            .filter_map(|path| self.read_entry(path, archived).ok())
            .collect();
        entries.sort_by(|left, right| compare_decision_ids(&left.decision.id, &right.decision.id));
        entries
    }

    fn read_entry(
        &self,
        path: PathBuf,
        archived: bool,
    ) -> Result<DecisionFileEntry, DecisionStoreError> {
        let source = self
            .storage
            .read(&path)
            .map_err(|error| DecisionStoreError::Storage(error.to_string()))?;
        let decision = records::parse_decision(&source)
            .map_err(|error| DecisionStoreError::Serialization(error.to_string()))?;
        Ok(DecisionFileEntry {
            decision,
            path,
            archived,
        })
    }

    fn resolve_decision(&self, id: &str) -> Result<Option<DecisionFileEntry>, DecisionStoreError> {
        let entries: Vec<_> = self
            .list_decisions()
            .into_iter()
            .chain(self.list_archived_decisions())
            .collect();
        find_unique_entity_by_id("Decision", "decision", id, &entries, |entry| {
            self.project_path(&entry.path)
        })
        .map(|entry| entry.cloned())
        .map_err(DecisionStoreError::Ambiguous)
    }

    fn next_id(&self, config: &Config) -> String {
        let mut ids = Vec::new();
        for directory in [&self.decisions_directory, &self.archive_directory] {
            ids.extend(
                list_task_file_paths(directory, "decision")
                    .into_iter()
                    .filter_map(|path| {
                        path.file_name()
                            .and_then(|name| name.to_str())
                            .and_then(|name| extract_id_from_filename(name, "decision"))
                    }),
            );
        }
        ids.extend(
            self.list_decisions()
                .into_iter()
                .chain(self.list_archived_decisions())
                .map(|entry| entry.decision.id),
        );
        let repository = GitRepository::new(&self.project_root);
        let backlog = self
            .backlog_directory
            .strip_prefix(&self.project_root)
            .unwrap_or(&self.backlog_directory)
            .to_string_lossy()
            .replace('\\', "/");
        let include_origin = config.remote_operations != Some(false);
        if include_origin {
            let _ = repository.fetch_origin();
        }
        for branch in repository.recent_branch_tips(f64::INFINITY, include_origin) {
            for folder in ["decisions", "archive/decisions"] {
                let path = format!("{backlog}/{folder}");
                ids.extend(
                    repository
                        .files_in_tree(&branch.name, &path)
                        .into_iter()
                        .filter_map(|file| {
                            Path::new(&file)
                                .file_name()
                                .and_then(|name| name.to_str())
                                .and_then(|name| extract_id_from_filename(name, "decision"))
                        }),
                );
            }
        }
        let ids: Vec<_> = ids.iter().map(String::as_str).collect();
        generate_next_id(
            &ids,
            "decision",
            config
                .zero_padded_ids
                .filter(|width| width.is_finite() && *width > 0.0)
                .map(|width| width as usize),
        )
        .to_lowercase()
    }

    fn project_path(&self, path: &Path) -> String {
        path.strip_prefix(&self.project_root)
            .unwrap_or(path)
            .components()
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/")
    }
}

fn validate_input(
    title: &str,
    status: Option<&str>,
    sections_to_write: [Option<&str>; 4],
) -> Result<(), DecisionStoreError> {
    if title.trim().is_empty() {
        return Err(DecisionStoreError::Refused(
            "Decision title cannot be empty.".into(),
        ));
    }
    if status.is_some_and(str::is_empty) {
        return Err(DecisionStoreError::Refused(
            "Decision status cannot be empty.".into(),
        ));
    }
    for (key, value) in [
        SectionKey::Context,
        SectionKey::Decision,
        SectionKey::Consequences,
        SectionKey::Alternatives,
    ]
    .into_iter()
    .zip(sections_to_write)
    {
        if let Some(value) = value {
            sections::assert_section_input_safe(value, key)
                .map_err(|error| DecisionStoreError::Refused(error.to_string()))?;
        }
    }
    Ok(())
}

fn decision_filename(decision: &DecisionRecord) -> String {
    format!(
        "{} - {}.md",
        normalize_entity_id("decision", &decision.id),
        sanitize_filename(&decision.title)
    )
}

fn sanitize_filename(title: &str) -> String {
    let mut output = String::new();
    let mut pending_dash = false;
    for character in title.chars() {
        if "<>:\"/\\|?*".contains(character) {
            if !output.ends_with('-') {
                output.push('-');
            }
            pending_dash = true;
        } else if "'(),!@#$%^&+={}[];".contains(character) {
            continue;
        } else if character.is_whitespace() {
            if !output.is_empty() {
                pending_dash = true;
            }
        } else {
            if pending_dash && !output.ends_with('-') {
                output.push('-');
            }
            if character != '-' || !output.ends_with('-') {
                output.push(character);
            }
            pending_dash = false;
        }
    }
    let output = output.trim_matches('-');
    if output.is_empty() {
        "untitled".into()
    } else {
        output.to_owned()
    }
}

fn compare_decision_ids(left: &str, right: &str) -> std::cmp::Ordering {
    fn numeric_parts(id: &str) -> Vec<u128> {
        let body = id
            .trim()
            .split_once('-')
            .filter(|(prefix, _)| prefix.eq_ignore_ascii_case("decision"))
            .map_or(id.trim(), |(_, body)| body);
        let decimal: Option<Vec<_>> = body
            .split('.')
            .map(|part| {
                let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
                (!digits.is_empty())
                    .then(|| digits.parse::<u128>().ok())
                    .flatten()
            })
            .collect();
        if let Some(parts) = decimal {
            return parts;
        }
        let trailing: String = body
            .chars()
            .rev()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        trailing
            .parse()
            .map(|number| vec![number])
            .unwrap_or_else(|_| vec![0])
    }
    let left_parts = numeric_parts(left);
    let right_parts = numeric_parts(right);
    for index in 0..left_parts.len().max(right_parts.len()) {
        let ordering = left_parts
            .get(index)
            .copied()
            .unwrap_or_default()
            .cmp(&right_parts.get(index).copied().unwrap_or_default());
        if ordering != std::cmp::Ordering::Equal {
            return ordering;
        }
    }
    left.to_lowercase().cmp(&right.to_lowercase())
}

fn utc_timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = (seconds / 86_400) as i64;
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_part = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_part + 2) / 5 + 1;
    let month = month_part + if month_part < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:00Z",
        (seconds % 86_400) / 3_600,
        (seconds % 3_600) / 60
    )
}
