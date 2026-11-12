//! Local document record reads and mutations.

use crate::{
    config::Config,
    content_references::list_documentation_references,
    git::GitRepository,
    identity::{
        AmbiguousIdError, HasEntityId, document_id_key, entity_ids_equal, find_document_by_id,
        generate_next_id, normalize_document_id,
    },
    records::{self, DocumentRecord},
    serialization,
    storage::Storage,
};
use std::{
    collections::{HashSet, VecDeque},
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq)]
pub struct DocumentFileEntry {
    pub document: DocumentRecord,
    /// Absolute path to the file that supplied the record.
    pub path: PathBuf,
    pub archived: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentChangeResult {
    pub id: String,
    pub listed_as_documentation_by: Vec<String>,
}

impl HasEntityId for DocumentFileEntry {
    fn entity_id(&self) -> &str {
        &self.document.id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DocumentStoreError {
    Ambiguous(AmbiguousIdError),
    Archived(String),
    NotFound(String),
    NotArchived {
        id: String,
        operation: &'static str,
    },
    ArchiveFileExists {
        id: String,
        path: String,
    },
    RestoreFileExists {
        id: String,
        path: String,
    },
    RemoveRefused {
        id: String,
        path: String,
        reason: String,
    },
    Lock(String),
    Storage(String),
    Serialization(String),
    InvalidPath(String),
    InvalidType(String),
}

impl fmt::Display for DocumentStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ambiguous(error) => error.fmt(formatter),
            Self::Archived(id) => write!(
                formatter,
                "Document {id} is archived. Archived documents are read-only. Restore it first: backlog doc restore {id}"
            ),
            Self::NotFound(id) => write!(formatter, "Document not found: {id}"),
            Self::NotArchived {
                id,
                operation: "restore",
            } => write!(
                formatter,
                "Document {id} is not archived, so there is nothing to restore."
            ),
            Self::NotArchived {
                id,
                operation: "delete",
            } => write!(
                formatter,
                "Document {id} is not archived. A document must be archived before it can be deleted."
            ),
            Self::NotArchived { id, .. } => write!(formatter, "Document {id} is not archived."),
            Self::ArchiveFileExists { id, path } => write!(
                formatter,
                "Document {id} cannot be archived: the archive already has a file at {path}."
            ),
            Self::RestoreFileExists { id, path } => write!(
                formatter,
                "Document {id} cannot be restored: the docs folder already has a file at {path}."
            ),
            Self::RemoveRefused { id, path, reason } => write!(
                formatter,
                "The file of document {id} was not removed: {path} {reason}"
            ),
            Self::Lock(message)
            | Self::Storage(message)
            | Self::Serialization(message)
            | Self::InvalidPath(message) => formatter.write_str(message),
            Self::InvalidType(message) => formatter.write_str(message),
        }
    }
}

impl Error for DocumentStoreError {}

#[derive(Clone, Debug)]
pub struct DocumentStore {
    project_root: PathBuf,
    task_prefix: String,
    docs_directory: PathBuf,
    archive_directory: PathBuf,
    storage: Storage,
}

impl DocumentStore {
    pub fn new(
        project_root: impl Into<PathBuf>,
        backlog_relative_directory: impl Into<PathBuf>,
        task_prefix: impl Into<String>,
    ) -> Self {
        let project_root = project_root.into();
        let backlog_directory = project_root.join(backlog_relative_directory.into());
        let docs_directory = backlog_directory.join("docs");
        let archive_directory = backlog_directory.join("archive/docs");
        let common_git_dir = GitRepository::new(&project_root).common_directory();
        Self {
            storage: Storage::new(&backlog_directory, common_git_dir),
            project_root,
            task_prefix: task_prefix.into(),
            docs_directory,
            archive_directory,
        }
    }

    /// Lists readable Markdown documents in TypeScript's title-then-relative-path order.
    pub fn list_documents(&self) -> Vec<DocumentFileEntry> {
        let mut entries: Vec<_> = self
            .document_paths(&self.docs_directory, &self.archive_directory)
            .into_iter()
            .filter(|path| {
                !path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("README.md"))
            })
            .filter_map(|path| self.read_entry(path).ok())
            .collect();
        entries.sort_by(|left, right| {
            locale_compare(&left.document.title, &right.document.title).then_with(|| {
                locale_compare(
                    &self.relative_path(&left.path).to_string_lossy(),
                    &self.relative_path(&right.path).to_string_lossy(),
                )
            })
        });
        entries
    }

    /// Resolve one active document by its frontmatter ID; archived files are not included.
    pub fn load_document(&self, id: &str) -> Result<DocumentFileEntry, DocumentStoreError> {
        let entries = self.list_documents();
        find_document_by_id(&entries, id, |entry| self.project_path(&entry.path))
            .map_err(DocumentStoreError::Ambiguous)?
            .cloned()
            .ok_or_else(|| DocumentStoreError::NotFound(id.into()))
    }

    /// Resolve one document for viewing across the docs and archive folders.
    pub fn load_document_for_view(
        &self,
        id: &str,
    ) -> Result<Option<DocumentFileEntry>, DocumentStoreError> {
        self.resolve_document(id)
    }

    /// Create a document using the shared create lock and the first free DOC identity.
    pub fn create_document(
        &self,
        mut document: DocumentRecord,
        config: &Config,
        sub_path: &str,
    ) -> Result<DocumentFileEntry, DocumentStoreError> {
        let sub_path = normalize_document_sub_path(sub_path)?;
        if !["readme", "guide", "specification", "other"].contains(&document.document_type.as_str())
        {
            return Err(DocumentStoreError::InvalidType(
                "Document type must be one of: readme, guide, specification, other.".into(),
            ));
        }
        let lock = self
            .storage
            .acquire_create_lock()
            .map_err(|error| DocumentStoreError::Lock(error.to_string()))?;
        let occupied = self.occupied_ids();
        let occupied: Vec<_> = occupied.iter().map(String::as_str).collect();
        let padding = config
            .zero_padded_ids
            .filter(|width| width.is_finite() && *width > 0.0)
            .map(|width| width as usize);
        document.id = generate_next_id(&occupied, "doc", padding).to_lowercase();
        let relative = self.document_relative_path(&document, &sub_path);
        let target = self.docs_directory.join(relative);
        let source = serialization::serialize_document(&document, "")
            .map_err(|error| DocumentStoreError::Serialization(error.to_string()))?;
        lock.check()
            .map_err(|error| DocumentStoreError::Lock(error.to_string()))?;
        self.storage
            .write_record(&target, &source, None)
            .map_err(|error| DocumentStoreError::Storage(error.to_string()))?;
        self.read_entry(target)
    }

    /// Update one document under its identity lock, preserving its source frontmatter and body.
    pub fn update_document(
        &self,
        document: DocumentRecord,
    ) -> Result<DocumentFileEntry, DocumentStoreError> {
        self.update_document_at_path(document, None)
    }

    /// Update one document and optionally move it to another docs-relative subdirectory.
    pub fn update_document_at_path(
        &self,
        mut document: DocumentRecord,
        requested_sub_path: Option<&str>,
    ) -> Result<DocumentFileEntry, DocumentStoreError> {
        let target = self
            .resolve_document(&document.id)?
            .ok_or_else(|| DocumentStoreError::NotFound(document.id.clone()))?;
        if target.archived {
            return Err(DocumentStoreError::Archived(target.document.id));
        }
        let filename_id = normalize_document_id(&target.document.id);
        let lock_id = document_id_key(&target.document.id).unwrap_or_else(|| filename_id.clone());
        let lock = self
            .storage
            .acquire_entity_lock("document", &lock_id, &target.path)
            .map_err(|error| DocumentStoreError::Lock(error.to_string()))?;
        let current = self
            .resolve_document(&document.id)?
            .ok_or_else(|| DocumentStoreError::NotFound(document.id.clone()))?;
        if current.archived {
            return Err(DocumentStoreError::Archived(current.document.id));
        }
        if current.path != target.path {
            return Err(DocumentStoreError::Lock(format!(
                "Document {filename_id} was moved or removed by another process."
            )));
        }
        if !entity_ids_equal("doc", &current.document.id, &filename_id) {
            return Err(DocumentStoreError::Serialization(
                "Document ID changed while updating".into(),
            ));
        }
        let original = self
            .storage
            .read(&current.path)
            .map_err(|error| DocumentStoreError::Storage(error.to_string()))?;
        document.id = filename_id;
        let source = serialization::serialize_document(&document, &original)
            .map_err(|error| DocumentStoreError::Serialization(error.to_string()))?;
        let sub_path = if let Some(requested_sub_path) = requested_sub_path {
            normalize_document_sub_path(requested_sub_path)?
        } else {
            let current_sub_path = current
                .path
                .strip_prefix(&self.docs_directory)
                .ok()
                .and_then(Path::parent)
                .unwrap_or_else(|| Path::new(""));
            normalize_document_sub_path(&current_sub_path.to_string_lossy())?
        };
        let relative = self.document_relative_path(&document, &sub_path);
        let destination = self.docs_directory.join(relative);
        lock.check()
            .map_err(|error| DocumentStoreError::Lock(error.to_string()))?;
        self.storage
            .write_record(&destination, &source, Some(&current.path))
            .map_err(|error| DocumentStoreError::Storage(error.to_string()))?;
        self.read_entry(destination)
    }

    /// Move one active document into its archive folder without rewriting its Markdown.
    pub fn archive_document(
        &self,
        id: &str,
    ) -> Result<Option<DocumentChangeResult>, DocumentStoreError> {
        self.move_document(id, true)
    }

    /// Restore one archived document to its original relative path without rewriting its Markdown.
    pub fn restore_document(
        &self,
        id: &str,
    ) -> Result<Option<DocumentChangeResult>, DocumentStoreError> {
        self.move_document(id, false)
    }

    /// Permanently remove one archived document and report records that still name either path.
    pub fn delete_document(
        &self,
        id: &str,
    ) -> Result<Option<DocumentChangeResult>, DocumentStoreError> {
        let Some(target) = self.resolve_document(id)? else {
            return Ok(None);
        };
        if !target.archived {
            return Err(DocumentStoreError::NotArchived {
                id: target.document.id,
                operation: "delete",
            });
        }
        let lock_id = document_id_key(&target.document.id)
            .unwrap_or_else(|| normalize_document_id(&target.document.id));
        let lock = self
            .storage
            .acquire_entity_lock("document", &lock_id, &target.path)
            .map_err(|error| DocumentStoreError::Lock(error.to_string()))?;
        let Some(current) = self.resolve_document(id)? else {
            return Ok(None);
        };
        if current.path != target.path {
            return Err(DocumentStoreError::Lock(format!(
                "Document {} was moved or removed by another process.",
                target.document.id
            )));
        }
        if !current.archived {
            return Err(DocumentStoreError::NotArchived {
                id: current.document.id,
                operation: "delete",
            });
        }

        let project_path = self.project_path(&current.path);
        let active_path = self.active_path(&current.path)?;
        let active_project_path = self.project_path(&active_path);
        let listed_as_documentation_by = list_documentation_references(
            &self.project_root,
            self.docs_directory.parent().unwrap_or(&self.docs_directory),
            &self.task_prefix,
            &[project_path.clone(), active_project_path],
        );
        let source =
            self.storage
                .read(&current.path)
                .map_err(|_| DocumentStoreError::RemoveRefused {
                    id: current.document.id.clone(),
                    path: project_path.clone(),
                    reason: "cannot be read as a document.".into(),
                })?;
        let stored =
            records::parse_document(&source).map_err(|_| DocumentStoreError::RemoveRefused {
                id: current.document.id.clone(),
                path: project_path.clone(),
                reason: "cannot be read as a document.".into(),
            })?;
        if !entity_ids_equal("doc", &stored.id, &current.document.id) {
            return Err(DocumentStoreError::RemoveRefused {
                id: current.document.id,
                path: project_path,
                reason: format!("is the file of {}.", stored.id),
            });
        }
        lock.check()
            .map_err(|error| DocumentStoreError::Lock(error.to_string()))?;
        fs::remove_file(&current.path)
            .map_err(|error| DocumentStoreError::Storage(error.to_string()))?;
        Ok(Some(DocumentChangeResult {
            id: current.document.id,
            listed_as_documentation_by,
        }))
    }

    fn move_document(
        &self,
        id: &str,
        archive: bool,
    ) -> Result<Option<DocumentChangeResult>, DocumentStoreError> {
        let Some(target) = self.resolve_document(id)? else {
            return Ok(None);
        };
        if target.archived == archive {
            if archive {
                return Err(DocumentStoreError::Archived(target.document.id));
            }
            return Err(DocumentStoreError::NotArchived {
                id: target.document.id,
                operation: "restore",
            });
        }
        let lock_id = document_id_key(&target.document.id)
            .unwrap_or_else(|| normalize_document_id(&target.document.id));
        let lock = self
            .storage
            .acquire_entity_lock("document", &lock_id, &target.path)
            .map_err(|error| DocumentStoreError::Lock(error.to_string()))?;
        let Some(current) = self.resolve_document(id)? else {
            return Ok(None);
        };
        if current.path != target.path {
            return Err(DocumentStoreError::Lock(format!(
                "Document {} was moved or removed by another process.",
                target.document.id
            )));
        }
        if current.archived == archive {
            if archive {
                return Err(DocumentStoreError::Archived(current.document.id));
            }
            return Err(DocumentStoreError::NotArchived {
                id: current.document.id,
                operation: "restore",
            });
        }
        let relative = current
            .path
            .strip_prefix(if archive {
                &self.docs_directory
            } else {
                &self.archive_directory
            })
            .map_err(|error| DocumentStoreError::Storage(error.to_string()))?;
        let destination = if archive {
            self.archive_directory.join(relative)
        } else {
            self.docs_directory.join(relative)
        };
        let current_project_path = self.project_path(&current.path);
        let destination_project_path = self.project_path(&destination);
        let listed_as_documentation_by = list_documentation_references(
            &self.project_root,
            self.docs_directory.parent().unwrap_or(&self.docs_directory),
            &self.task_prefix,
            &[current_project_path],
        );
        lock.check()
            .map_err(|error| DocumentStoreError::Lock(error.to_string()))?;
        self.storage
            .move_record(&current.path, &destination)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    if archive {
                        DocumentStoreError::ArchiveFileExists {
                            id: current.document.id.clone(),
                            path: destination_project_path,
                        }
                    } else {
                        DocumentStoreError::RestoreFileExists {
                            id: current.document.id.clone(),
                            path: destination_project_path,
                        }
                    }
                } else {
                    DocumentStoreError::Storage(error.to_string())
                }
            })?;
        Ok(Some(DocumentChangeResult {
            id: current.document.id,
            listed_as_documentation_by,
        }))
    }

    fn active_path(&self, archived_path: &Path) -> Result<PathBuf, DocumentStoreError> {
        archived_path
            .strip_prefix(&self.archive_directory)
            .map(|relative| self.docs_directory.join(relative))
            .map_err(|error| DocumentStoreError::Storage(error.to_string()))
    }

    fn read_entry(&self, path: PathBuf) -> Result<DocumentFileEntry, DocumentStoreError> {
        let source = self
            .storage
            .read(&path)
            .map_err(|error| DocumentStoreError::Storage(error.to_string()))?;
        let document = records::parse_document(&source)
            .map_err(|error| DocumentStoreError::Serialization(error.to_string()))?;
        let archived = path.starts_with(&self.archive_directory);
        Ok(DocumentFileEntry {
            document,
            path,
            archived,
        })
    }

    fn resolve_document(&self, id: &str) -> Result<Option<DocumentFileEntry>, DocumentStoreError> {
        let mut entries = self.list_documents();
        entries.extend(
            self.document_paths(&self.archive_directory, &self.docs_directory)
                .into_iter()
                .filter(|path| {
                    !path.file_name().is_some_and(|name| {
                        name.to_string_lossy().eq_ignore_ascii_case("README.md")
                    })
                })
                .filter_map(|path| self.read_entry(path).ok()),
        );
        find_document_by_id(&entries, id, |entry| self.project_path(&entry.path))
            .map(|entry| entry.cloned())
            .map_err(DocumentStoreError::Ambiguous)
    }

    fn document_paths(&self, directory: &Path, other_directory: &Path) -> Vec<PathBuf> {
        let real_directory = fs::canonicalize(directory).ok();
        let real_other_directory = fs::canonicalize(other_directory).ok();
        let foreign_directory = real_directory
            .as_ref()
            .zip(real_other_directory.as_ref())
            .filter(|(current, other)| !other.starts_with(current))
            .map(|(_, other)| other.clone());
        let mut pending = VecDeque::from([directory.to_path_buf()]);
        let mut visited = HashSet::new();
        let mut files = Vec::new();
        while let Some(folder) = pending.pop_front() {
            let Ok(real_folder) = fs::canonicalize(&folder) else {
                continue;
            };
            if foreign_directory
                .as_ref()
                .is_some_and(|foreign| real_folder.starts_with(foreign))
            {
                continue;
            }
            if !visited.insert(real_folder) {
                continue;
            }
            let Ok(entries) = fs::read_dir(&folder) else {
                continue;
            };
            let mut entries: Vec<_> = entries.flatten().collect();
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                let name = entry.file_name();
                if name.to_string_lossy().starts_with('.') {
                    continue;
                }
                let path = entry.path();
                let real_path = fs::canonicalize(&path).ok();
                if foreign_directory
                    .as_ref()
                    .zip(real_path.as_ref())
                    .is_some_and(|(foreign, real_path)| real_path.starts_with(foreign))
                {
                    continue;
                }
                let metadata = match fs::metadata(&path) {
                    Ok(metadata) => metadata,
                    Err(_) => continue,
                };
                if metadata.is_dir() {
                    let points_inside_current =
                        entry.file_type().is_ok_and(|kind| kind.is_symlink())
                            && real_directory
                                .as_ref()
                                .zip(real_path.as_ref())
                                .is_some_and(|(current, real_path)| real_path.starts_with(current));
                    if !points_inside_current {
                        pending.push_back(path);
                    }
                } else if metadata.is_file()
                    && path.extension().is_some_and(|extension| extension == "md")
                {
                    files.push(path);
                }
            }
        }
        files
    }

    fn occupied_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        for path in self
            .document_paths(&self.docs_directory, &self.archive_directory)
            .into_iter()
            .chain(self.document_paths(&self.archive_directory, &self.docs_directory))
        {
            if path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("README.md"))
            {
                continue;
            }
            if let Some(filename) = path.file_name().and_then(|name| name.to_str())
                && let Some(id) = crate::identity::extract_id_from_filename(filename, "doc")
            {
                ids.push(id);
            }
            if let Ok(entry) = self.read_entry(path) {
                ids.push(entry.document.id);
            }
        }
        ids
    }

    fn document_relative_path(&self, document: &DocumentRecord, sub_path: &Path) -> PathBuf {
        let title = sanitize_filename(&document.title);
        sub_path.join(format!(
            "{} - {title}.md",
            normalize_document_id(&document.id)
        ))
    }

    fn relative_path(&self, path: &Path) -> PathBuf {
        path.strip_prefix(&self.docs_directory)
            .unwrap_or(path)
            .to_path_buf()
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

fn normalize_document_sub_path(value: &str) -> Result<PathBuf, DocumentStoreError> {
    let value = value.trim();
    let bytes = value.as_bytes();
    let has_windows_absolute_prefix = bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\');
    if value.starts_with('/') || value.starts_with('\\') || has_windows_absolute_prefix {
        return Err(DocumentStoreError::InvalidPath(
            "Document path must be relative to the docs directory.".into(),
        ));
    }

    let segments: Vec<_> = value
        .split(['/', '\\'])
        .map(str::trim)
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .collect();
    if segments.contains(&"..") {
        return Err(DocumentStoreError::InvalidPath(
            "Document path cannot include traversal segments.".into(),
        ));
    }
    Ok(segments.iter().collect())
}

fn locale_compare(left: &str, right: &str) -> std::cmp::Ordering {
    left.to_lowercase()
        .cmp(&right.to_lowercase())
        .then_with(|| right.cmp(left))
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
