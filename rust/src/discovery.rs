//! Read-only runtime and project selection, independent of stored config parsing.
use crate::config::ConfigLocation;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BacklogDirectorySource {
    Backlog,
    Hidden,
    Custom,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BacklogDirectoryResolution {
    pub project_root: PathBuf,
    pub backlog_directory: Option<String>,
    pub backlog_path: Option<PathBuf>,
    pub source: Option<BacklogDirectorySource>,
    pub config_path: Option<PathBuf>,
    pub config_location: Option<ConfigLocation>,
    pub root_config_path: PathBuf,
    pub root_config_exists: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeCwdSource {
    Option,
    Environment,
    Process,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeCwd {
    pub path: PathBuf,
    pub source: RuntimeCwdSource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeCwdError {
    pub path: PathBuf,
    pub source: RuntimeCwdSource,
}

impl fmt::Display for RuntimeCwdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self.source {
            RuntimeCwdSource::Option => "--cwd",
            RuntimeCwdSource::Environment => "BACKLOG_CWD",
            RuntimeCwdSource::Process => "process.cwd()",
        };
        write!(
            formatter,
            "Invalid directory from {label}: {}",
            self.path.display()
        )
    }
}
impl std::error::Error for RuntimeCwdError {}

fn lexical_path(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if result.file_name().is_some_and(|name| name != "..") {
                    result.pop();
                } else if !result.has_root() {
                    result.push("..");
                }
            }
            _ => result.push(component.as_os_str()),
        }
    }
    result
}

pub fn resolve_runtime_cwd(
    option: Option<&str>,
    environment: Option<&str>,
    process_cwd: &Path,
) -> Result<RuntimeCwd, RuntimeCwdError> {
    let candidate = option
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| (value, RuntimeCwdSource::Option))
        .or_else(|| {
            environment
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| (value, RuntimeCwdSource::Environment))
        });
    let Some((value, source)) = candidate else {
        return Ok(RuntimeCwd {
            path: process_cwd.to_owned(),
            source: RuntimeCwdSource::Process,
        });
    };
    let path = lexical_path(&process_cwd.join(value));
    if !path.is_dir() {
        return Err(RuntimeCwdError { path, source });
    }
    Ok(RuntimeCwd { path, source })
}

/// Host normalization precedes backslash replacement, matching the existing path rule.
/// This is lexical validation: descendant symlinks are deliberately not resolved.
pub fn normalize_backlog_directory(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let bytes = trimmed.as_bytes();
    if trimmed.is_empty()
        || trimmed.starts_with(['/', '\\'])
        || (bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'/' | b'\\'))
    {
        return None;
    }
    let mut parts = Vec::new();
    for part in trimmed.split(|character| character == '/' || (cfg!(windows) && character == '\\'))
    {
        match part {
            "" | "." => {}
            ".." if parts.last().is_some_and(|last| *last != "..") => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    let normalized = parts.join("/").replace('\\', "/");
    let normalized = normalized.trim_end_matches('/');
    if normalized.is_empty()
        || normalized == "."
        || normalized == ".."
        || normalized.starts_with("../")
    {
        None
    } else {
        Some(normalized.to_owned())
    }
}

struct ConfigMetadata {
    project_name: Option<String>,
    backlog_directory: Option<String>,
}

// Discovery only recognizes metadata; malformed unrelated YAML must not hide a root.
fn root_config_metadata(path: &Path) -> Option<ConfigMetadata> {
    if !path.is_file() {
        return None;
    }
    let content = fs::read_to_string(path).ok()?;
    let mut metadata = ConfigMetadata {
        project_name: None,
        backlog_directory: None,
    };
    for line in content.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        let value = value.strip_prefix(['\'', '"']).unwrap_or(value);
        let value = value.strip_suffix(['\'', '"']).unwrap_or(value);
        match key.trim() {
            "project_name" | "projectName" if !value.is_empty() => {
                metadata.project_name = Some(value.to_owned())
            }
            "backlog_directory" | "backlogDirectory" => {
                metadata.backlog_directory = normalize_backlog_directory(value)
            }
            _ => {}
        }
    }
    metadata.project_name.as_ref()?;
    Some(metadata)
}

fn folder_config_path(path: &Path) -> Option<PathBuf> {
    ["config.yml", "config.yaml"]
        .into_iter()
        .map(|name| path.join(name))
        .find(|path| path.is_file())
}

fn built_in_directory(root: &Path) -> Option<(&'static str, BacklogDirectorySource)> {
    let candidates = [
        ("backlog", BacklogDirectorySource::Backlog),
        (".backlog", BacklogDirectorySource::Hidden),
    ];
    candidates
        .iter()
        .copied()
        .find(|(name, _)| {
            let path = root.join(name);
            path.is_dir() && folder_config_path(&path).is_some()
        })
        .or_else(|| {
            candidates
                .into_iter()
                .find(|(name, _)| root.join(name).is_dir())
        })
}

pub fn resolve_backlog_directory(project_root: &Path) -> BacklogDirectoryResolution {
    let root_config_path = project_root.join("backlog.config.yml");
    let root_config_exists = root_config_path.is_file();
    let metadata = root_config_metadata(&root_config_path);
    let selected = metadata
        .as_ref()
        .and_then(|metadata| metadata.backlog_directory.clone())
        .map(|directory| {
            let source = match directory.as_str() {
                "backlog" => BacklogDirectorySource::Backlog,
                ".backlog" => BacklogDirectorySource::Hidden,
                _ => BacklogDirectorySource::Custom,
            };
            (directory, source)
        })
        .or_else(|| {
            built_in_directory(project_root).map(|(name, source)| (name.to_owned(), source))
        });
    let mut result = BacklogDirectoryResolution {
        project_root: project_root.to_owned(),
        backlog_directory: None,
        backlog_path: None,
        source: None,
        config_path: None,
        config_location: None,
        root_config_path,
        root_config_exists,
    };
    if let Some((directory, source)) = selected {
        let backlog_path = project_root.join(&directory);
        if metadata.is_some() {
            result.config_path = Some(result.root_config_path.clone());
            result.config_location = Some(ConfigLocation::Root);
        } else if let Some(path) = folder_config_path(&backlog_path) {
            result.config_path = Some(path);
            result.config_location = Some(ConfigLocation::Folder);
        }
        result.backlog_directory = Some(directory);
        result.backlog_path = Some(backlog_path);
        result.source = Some(source);
    }
    result
}

fn has_project_marker(path: &Path) -> bool {
    resolve_backlog_directory(path).config_path.is_some() || path.join("backlog.json").is_file()
}

fn find_project_root_with_git(
    start: &Path,
    git_root: impl FnOnce(&Path) -> Option<PathBuf>,
) -> Option<PathBuf> {
    let mut current = start;
    // Retain the existing ancestor loop's boundary: the filesystem root is not scanned.
    while let Some(parent) = current.parent().filter(|parent| *parent != current) {
        if has_project_marker(current) {
            return Some(current.to_owned());
        }
        current = parent;
    }
    git_root(start).filter(|root| has_project_marker(root))
}

pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    find_project_root_with_git(start, |start| {
        let output = Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .current_dir(start)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let path = String::from_utf8(output.stdout).ok()?;
        let path = path.trim();
        (!path.is_empty()).then(|| PathBuf::from(path))
    })
}

#[cfg(test)]
mod tests {
    use super::find_project_root_with_git;
    use std::fs;
    use std::path::PathBuf;

    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn git_is_only_a_fallback_and_its_root_requires_a_marker() {
        let root =
            std::env::temp_dir().join(format!("backlog-git-discovery-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let _fixture = Fixture(root.clone());
        let start = root.join("start");
        let git = root.join("git");
        fs::create_dir(&start).unwrap();
        fs::create_dir(&git).unwrap();
        assert_eq!(find_project_root_with_git(&start, |_| None), None);
        assert_eq!(
            find_project_root_with_git(&start, |given| {
                assert_eq!(given, start);
                Some(git.clone())
            }),
            None
        );
        fs::write(git.join("backlog.json"), "marker only").unwrap();
        assert_eq!(
            find_project_root_with_git(&start, |_| Some(git.clone())),
            Some(git)
        );
        fs::write(start.join("backlog.json"), "marker only").unwrap();
        assert_eq!(
            find_project_root_with_git(&start, |_| panic!("ancestor marker must avoid Git")),
            Some(start)
        );
    }
}
