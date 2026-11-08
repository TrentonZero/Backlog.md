//! Source contracts: runtime-cwd.ts, backlog-directory.ts and find-backlog-root.ts
//! under src/utils, plus their matching src/test fixtures and ADVANCED-CONFIG.md.
use backlog_md::config::ConfigLocation;
use backlog_md::discovery::{
    BacklogDirectoryResolution, BacklogDirectorySource as Source, RuntimeCwdSource as CwdSource,
    find_project_root, normalize_backlog_directory, resolve_backlog_directory, resolve_runtime_cwd,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "backlog-discovery-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn dir(&self, path: &str) -> PathBuf {
        let path = self.0.join(path);
        fs::create_dir_all(&path).unwrap();
        path
    }
    fn file(&self, path: &str, content: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

// A wrong precedence branch or fallback after an invalid selected override must fail.
#[test]
fn cwd_uses_first_nonblank_override_relative_to_process_and_fails_without_fallback() {
    let fixture = Fixture::new();
    fixture.dir("option");
    fixture.dir("environment");
    fixture.file("file", "regular file");
    for (option, environment, expected, source) in [
        (
            Some(" option "),
            Some("environment"),
            "option",
            CwdSource::Option,
        ),
        (
            Some(" \t "),
            Some(" environment "),
            "environment",
            CwdSource::Environment,
        ),
        (None, None, "", CwdSource::Process),
        (Some(" "), Some(" "), "", CwdSource::Process),
        (
            Some("./option/../environment"),
            None,
            "environment",
            CwdSource::Option,
        ),
    ] {
        let result = resolve_runtime_cwd(option, environment, &fixture.0).unwrap();
        assert_eq!(
            result.path,
            if expected.is_empty() {
                fixture.0.clone()
            } else {
                fixture.0.join(expected)
            }
        );
        assert_eq!(result.source, source);
    }
    let absolute = fixture.0.join("option");
    assert_eq!(
        resolve_runtime_cwd(absolute.to_str(), None, &fixture.0)
            .unwrap()
            .path,
        absolute
    );
    for (option, environment, selected, source, label) in [
        (
            Some("missing"),
            Some("environment"),
            "missing",
            CwdSource::Option,
            "--cwd",
        ),
        (
            Some("file"),
            Some("environment"),
            "file",
            CwdSource::Option,
            "--cwd",
        ),
        (
            Some(" "),
            Some("missing"),
            "missing",
            CwdSource::Environment,
            "BACKLOG_CWD",
        ),
        (
            None,
            Some("file"),
            "file",
            CwdSource::Environment,
            "BACKLOG_CWD",
        ),
    ] {
        let error = resolve_runtime_cwd(option, environment, &fixture.0).unwrap_err();
        assert_eq!(error.source, source);
        assert_eq!(error.path, fixture.0.join(selected));
        assert!(error.to_string().contains(label));
        assert!(
            error
                .to_string()
                .contains(&error.path.display().to_string())
        );
    }
    // The process cwd is supplied by the caller and is not override-validated.
    let missing_process = fixture.0.join("missing-process");
    assert_eq!(
        resolve_runtime_cwd(None, None, &missing_process)
            .unwrap()
            .path,
        missing_process
    );
}

#[test]
fn custom_directory_is_trimmed_and_normalized_lexically() {
    for (input, expected) in [
        (" planning/./old/../data/ ", Some("planning/data")),
        ("backlog", Some("backlog")),
        ("a/../../outside", None),
        ("", None),
        (" \t", None),
        (".", None),
        ("./", None),
        ("..", None),
        ("../outside", None),
        ("a/..", None),
        ("/absolute", None),
        ("C:/absolute", None),
        ("C:\\absolute", None),
        ("\\\\server\\share", None),
        ("\\rooted", None),
    ] {
        assert_eq!(
            normalize_backlog_directory(input).as_deref(),
            expected,
            "{input:?}"
        );
    }
}

// Unix normalizes '/' before replacing backslashes; Windows normalizes both separators.
#[cfg(unix)]
#[test]
fn unix_backslashes_are_replaced_only_after_host_normalization() {
    assert_eq!(
        normalize_backlog_directory("a\\..\\data").as_deref(),
        Some("a/../data")
    );
    assert_eq!(
        normalize_backlog_directory("a\\../data").as_deref(),
        Some("a/../data")
    );
    assert_eq!(normalize_backlog_directory("..\\outside"), None);

    let fixture = Fixture::new();
    let target = fixture.dir("data");
    fixture.file(
        "backlog.config.yml",
        "project_name: Root\nbacklog_directory: a\\..\\data\n",
    );
    let resolution = resolve_backlog_directory(&fixture.0);
    assert_eq!(resolution.backlog_directory.as_deref(), Some("data"));
    assert_eq!(resolution.backlog_path, Some(target));
    assert!(resolution.backlog_path.unwrap().is_dir());
    assert!(!fixture.0.join("a").exists());
}
#[cfg(windows)]
#[test]
fn windows_backslashes_participate_in_host_normalization() {
    assert_eq!(
        normalize_backlog_directory("a\\..\\data").as_deref(),
        Some("data")
    );
    assert_eq!(normalize_backlog_directory("a\\..\\..\\outside"), None);
}

fn assert_resolution(
    actual: BacklogDirectoryResolution,
    root: &Path,
    directory: Option<&str>,
    source: Option<Source>,
    config: Option<&str>,
    location: Option<ConfigLocation>,
    root_exists: bool,
) {
    assert_eq!(actual.project_root, root);
    assert_eq!(actual.backlog_directory.as_deref(), directory);
    assert_eq!(
        actual.backlog_path,
        directory.map(|directory| root.join(directory))
    );
    assert_eq!(actual.source, source);
    assert_eq!(actual.config_path, config.map(|config| root.join(config)));
    assert_eq!(actual.config_location, location);
    assert_eq!(actual.root_config_path, root.join("backlog.config.yml"));
    assert_eq!(actual.root_config_exists, root_exists);
}

#[test]
fn root_config_owns_custom_selection_even_when_target_is_missing() {
    let fixture = Fixture::new();
    fixture.file("backlog/config.yml", "project_name: Folder\n");
    fixture.file(".backlog/config.yml", "project_name: Hidden\n");
    for metadata in [
        "project_name: 'Root'\nbacklog_directory: 'planning/data'\n",
        "projectName: \"Root\"\nbacklogDirectory: \"planning/data\"\nstatuses: [broken\n",
    ] {
        fixture.file("backlog.config.yml", metadata);
        assert_resolution(
            resolve_backlog_directory(&fixture.0),
            &fixture.0,
            Some("planning/data"),
            Some(Source::Custom),
            Some("backlog.config.yml"),
            Some(ConfigLocation::Root),
            true,
        );
        assert!(!fixture.0.join("planning/data").exists());
    }
    for (directory, source) in [("backlog", Source::Backlog), (".backlog", Source::Hidden)] {
        fixture.file(
            "backlog.config.yml",
            &format!("project_name: Root\nbacklog_directory: {directory}\n"),
        );
        assert_resolution(
            resolve_backlog_directory(&fixture.0),
            &fixture.0,
            Some(directory),
            Some(source),
            Some("backlog.config.yml"),
            Some(ConfigLocation::Root),
            true,
        );
    }
}

#[test]
fn built_in_selection_prefers_marked_folders_and_primary_filename() {
    let fixture = Fixture::new();
    assert_resolution(
        resolve_backlog_directory(&fixture.0),
        &fixture.0,
        None,
        None,
        None,
        None,
        false,
    );
    fixture.dir("backlog");
    assert_resolution(
        resolve_backlog_directory(&fixture.0),
        &fixture.0,
        Some("backlog"),
        Some(Source::Backlog),
        None,
        None,
        false,
    );
    fixture.file(".backlog/config.yaml", "");
    assert_resolution(
        resolve_backlog_directory(&fixture.0),
        &fixture.0,
        Some(".backlog"),
        Some(Source::Hidden),
        Some(".backlog/config.yaml"),
        Some(ConfigLocation::Folder),
        false,
    );
    fixture.file("backlog/config.yaml", "");
    fixture.file("backlog/config.yml", "backlog_directory: elsewhere\n");
    assert_resolution(
        resolve_backlog_directory(&fixture.0),
        &fixture.0,
        Some("backlog"),
        Some(Source::Backlog),
        Some("backlog/config.yml"),
        Some(ConfigLocation::Folder),
        false,
    );
    fixture.file(
        "backlog.config.yml",
        "name: placeholder\nbacklog_directory: elsewhere\n",
    );
    assert_resolution(
        resolve_backlog_directory(&fixture.0),
        &fixture.0,
        Some("backlog"),
        Some(Source::Backlog),
        Some("backlog/config.yml"),
        Some(ConfigLocation::Folder),
        true,
    );
}

#[test]
fn root_without_valid_custom_path_uses_built_in_but_remains_canonical() {
    let fixture = Fixture::new();
    fixture.dir("backlog");
    fixture.file(".backlog/config.yml", "project_name: Hidden\n");
    for metadata in [
        "project_name: Root\n",
        "project_name: Root\nbacklog_directory: ../outside\n",
    ] {
        fixture.file("backlog.config.yml", metadata);
        assert_resolution(
            resolve_backlog_directory(&fixture.0),
            &fixture.0,
            Some(".backlog"),
            Some(Source::Hidden),
            Some("backlog.config.yml"),
            Some(ConfigLocation::Root),
            true,
        );
    }
    fs::remove_dir_all(fixture.0.join("backlog")).unwrap();
    fs::remove_dir_all(fixture.0.join(".backlog")).unwrap();
    assert_resolution(
        resolve_backlog_directory(&fixture.0),
        &fixture.0,
        None,
        None,
        None,
        None,
        true,
    );
}

#[test]
fn folder_only_key_cannot_relocate_and_root_config_filename_is_exact() {
    let fixture = Fixture::new();
    fixture.file(
        "backlog/config.yml",
        "project_name: Folder\nbacklog_directory: elsewhere\n",
    );
    fixture.file(
        "backlog.config.yaml",
        "project_name: Root\nbacklog_directory: elsewhere\n",
    );
    assert_resolution(
        resolve_backlog_directory(&fixture.0),
        &fixture.0,
        Some("backlog"),
        Some(Source::Backlog),
        Some("backlog/config.yml"),
        Some(ConfigLocation::Folder),
        false,
    );
}

#[test]
fn lexical_descendant_symlink_is_accepted_without_realpath_confinement() {
    let fixture = Fixture::new();
    let outside = Fixture::new();
    let link = fixture.0.join("linked");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside.0, &link).unwrap();
    #[cfg(windows)]
    if let Err(error) = std::os::windows::fs::symlink_dir(&outside.0, &link) {
        if error.kind() == std::io::ErrorKind::PermissionDenied {
            return;
        }
        panic!("symlink fixture failed: {error}");
    }
    fixture.file(
        "backlog.config.yml",
        "project_name: Root\nbacklog_directory: linked\n",
    );
    assert_resolution(
        resolve_backlog_directory(&fixture.0),
        &fixture.0,
        Some("linked"),
        Some(Source::Custom),
        Some("backlog.config.yml"),
        Some(ConfigLocation::Root),
        true,
    );
    assert_eq!(
        fs::canonicalize(link).unwrap(),
        fs::canonicalize(&outside.0).unwrap()
    );
}

#[test]
fn nearest_marked_project_wins_and_unmarked_custom_data_does_not_hide_parent() {
    let fixture = Fixture::new();
    fixture.file(
        "backlog.config.yml",
        "project_name: Parent\nbacklog_directory: planning/backlog\n",
    );
    let data = fixture.dir("planning/backlog/tasks");
    assert_eq!(find_project_root(&data), Some(fixture.0.clone()));
    fixture.file("planning/.backlog/config.yaml", "not parsed as YAML");
    assert_eq!(find_project_root(&data), Some(fixture.0.join("planning")));
    fixture.file("planning/backlog/backlog.json", "not parsed as JSON");
    assert_eq!(
        find_project_root(&data),
        Some(fixture.0.join("planning/backlog"))
    );
}

#[test]
fn bare_folders_placeholders_and_marker_directories_are_not_project_roots() {
    let fixture = Fixture::new();
    let start = fixture.dir("backlog/tasks");
    fixture.file("backlog.config.yml", "project_name: ''\n");
    fixture.dir("backlog.json");
    fixture.dir("backlog/config.yml");
    assert_eq!(find_project_root(&start), None);
    fs::remove_dir(fixture.0.join("backlog/config.yml")).unwrap();
    fixture.file("backlog/config.yaml", "");
    assert_eq!(find_project_root(&start), Some(fixture.0.clone()));
}
