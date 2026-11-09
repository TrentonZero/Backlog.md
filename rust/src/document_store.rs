//! Local document record reads and mutations.

use crate::{
    config::Config,
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
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq)]
pub struct DocumentFileEntry {
    pub document: DocumentRecord,
    /// Absolute path to the file that supplied the record.
    pub path: PathBuf,
    pub archived: bool,
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
    Lock(String),
    Storage(String),
    Serialization(String),
    InvalidPath(String),
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
            Self::Lock(message)
            | Self::Storage(message)
            | Self::Serialization(message)
            | Self::InvalidPath(message) => formatter.write_str(message),
        }
    }
}

impl Error for DocumentStoreError {}

#[derive(Clone, Debug)]
pub struct DocumentStore {
    project_root: PathBuf,
    docs_directory: PathBuf,
    archive_directory: PathBuf,
    storage: Storage,
}

impl DocumentStore {
    pub fn new(
        project_root: impl Into<PathBuf>,
        backlog_relative_directory: impl Into<PathBuf>,
    ) -> Self {
        let project_root = project_root.into();
        let backlog_directory = project_root.join(backlog_relative_directory.into());
        let docs_directory = backlog_directory.join("docs");
        let archive_directory = backlog_directory.join("archive/docs");
        let common_git_dir = GitRepository::new(&project_root).common_directory();
        Self {
            storage: Storage::new(&backlog_directory, common_git_dir),
            project_root,
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

    /// Create a document using the shared create lock and the first free DOC identity.
    pub fn create_document(
        &self,
        mut document: DocumentRecord,
        config: &Config,
        sub_path: &str,
    ) -> Result<DocumentFileEntry, DocumentStoreError> {
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
        let relative = self.document_relative_path(&document, sub_path)?;
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
        mut document: DocumentRecord,
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
        let sub_path = current
            .path
            .strip_prefix(&self.docs_directory)
            .ok()
            .and_then(Path::parent)
            .unwrap_or_else(|| Path::new(""));
        let relative = self.document_relative_path(&document, &sub_path.to_string_lossy())?;
        let destination = self.docs_directory.join(relative);
        lock.check()
            .map_err(|error| DocumentStoreError::Lock(error.to_string()))?;
        self.storage
            .write_record(&destination, &source, Some(&current.path))
            .map_err(|error| DocumentStoreError::Storage(error.to_string()))?;
        self.read_entry(destination)
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

    fn document_relative_path(
        &self,
        document: &DocumentRecord,
        sub_path: &str,
    ) -> Result<PathBuf, DocumentStoreError> {
        let sub_path = Path::new(sub_path.trim());
        if sub_path.is_absolute()
            || sub_path.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            return Err(DocumentStoreError::InvalidPath(
                "Document path must be relative and cannot include traversal segments.".into(),
            ));
        }
        let title = sanitize_filename(&document.title);
        Ok(sub_path.join(format!(
            "{} - {title}.md",
            normalize_document_id(&document.id)
        )))
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
