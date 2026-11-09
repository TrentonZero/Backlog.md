//! Source-preserving record I/O and locks shared with the current executable.
use filetime::{FileTime, set_file_mtime};
use fs2::FileExt;
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime},
};

const DEFAULT_LOCK_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_RETRY_DELAY: Duration = Duration::from_millis(100);
const DEFAULT_LOCK_STALE: Duration = Duration::from_secs(10);

/// File operations use explicit paths so application code remains responsible for identity and layout.
#[derive(Clone, Debug)]
pub struct Storage {
    backlog_dir: PathBuf,
    common_git_dir: Option<PathBuf>,
    lock_timeout: Duration,
    retry_delay: Duration,
    stale_after: Duration,
}

impl Storage {
    pub fn new(backlog_dir: impl Into<PathBuf>, common_git_dir: Option<PathBuf>) -> Self {
        Self {
            backlog_dir: backlog_dir.into(),
            common_git_dir,
            lock_timeout: DEFAULT_LOCK_TIMEOUT,
            retry_delay: DEFAULT_RETRY_DELAY,
            stale_after: DEFAULT_LOCK_STALE,
        }
    }

    #[must_use]
    pub fn with_lock_settings(
        mut self,
        timeout: Duration,
        retry_delay: Duration,
        stale_after: Duration,
    ) -> Self {
        self.lock_timeout = timeout;
        self.retry_delay = retry_delay;
        self.stale_after = stale_after.max(Duration::from_secs(2));
        self
    }

    pub fn read(&self, path: &Path) -> io::Result<String> {
        fs::read_to_string(path)
    }

    /// Create or update a record. New destinations are claimed without replacement.
    /// Moving a record first claims the destination, then removes its source.
    pub fn write_record(
        &self,
        target: &Path,
        contents: &str,
        source: Option<&Path>,
    ) -> io::Result<()> {
        let parent = target.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)?;
        let temporary = unique_sibling(target);
        let result = (|| {
            write_complete_file(&temporary, contents)?;
            match source {
                None => fs::hard_link(&temporary, target),
                Some(source) if source == target => fs::rename(&temporary, target),
                Some(source) => {
                    fs::hard_link(&temporary, target)?;
                    if let Err(error) = fs::remove_file(source) {
                        if error.kind() == io::ErrorKind::NotFound {
                            return Ok(());
                        }
                        let _ = fs::remove_file(target);
                        return Err(error);
                    }
                    Ok(())
                }
            }
        })();
        let _ = fs::remove_file(&temporary);
        result
    }

    /// Move a record to an unoccupied path without changing its bytes or metadata.
    pub fn move_record(&self, source: &Path, target: &Path) -> io::Result<()> {
        if source == target {
            return Ok(());
        }
        let parent = target.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)?;
        if fs::symlink_metadata(source)?.file_type().is_symlink() {
            let link_target = fs::read_link(source)?;
            let link_target = if link_target.is_absolute() {
                link_target
            } else {
                let target_directory = fs::canonicalize(parent)?;
                relative_path(&target_directory, &fs::canonicalize(source)?)
            };
            create_file_symlink(&link_target, target)?;
        } else {
            fs::hard_link(source, target)?;
        }
        if let Err(error) = fs::remove_file(source) {
            if error.kind() == io::ErrorKind::NotFound {
                return Ok(());
            }
            let _ = fs::remove_file(target);
            return Err(error);
        }
        Ok(())
    }

    /// Acquire the shared task-ID allocation lock used by create/promote/demote operations.
    pub fn acquire_create_lock(&self) -> io::Result<FileLock> {
        let lock_dir = self.create_lock_path();
        acquire_lock(
            &lock_dir,
            self.lock_timeout,
            self.retry_delay,
            self.stale_after,
        )
    }

    /// Acquire a record lock. These locks fail immediately on contention.
    pub fn acquire_entity_lock(
        &self,
        scope: &str,
        entity_id: &str,
        target: &Path,
    ) -> io::Result<FileLock> {
        let key = lock_key(entity_id);
        let key = if key.is_empty() { "unknown" } else { &key };
        let lock_dir = self
            .backlog_dir
            .join(".locks")
            .join(format!("{scope}-{key}"));
        let _ = fs::metadata(target)?;
        acquire_lock(&lock_dir, Duration::ZERO, Duration::ZERO, self.stale_after)
    }

    fn create_lock_path(&self) -> PathBuf {
        match &self.common_git_dir {
            Some(common_git_dir) => common_git_dir.join("backlog.md/locks/create"),
            None => self.backlog_dir.join(".locks/create"),
        }
    }
}

fn create_file_symlink(source: &Path, target: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(source, target)
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(source, target)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (source, target);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "file symlinks are unsupported on this platform",
        ))
    }
}

fn relative_path(from: &Path, to: &Path) -> PathBuf {
    let from: Vec<_> = from.components().collect();
    let to: Vec<_> = to.components().collect();
    if from
        .first()
        .zip(to.first())
        .is_some_and(|(left, right)| !same_path_component(left, right))
    {
        return to.iter().fold(PathBuf::new(), |mut path, component| {
            path.push(component.as_os_str());
            path
        });
    }
    let common = from
        .iter()
        .zip(&to)
        .take_while(|(left, right)| same_path_component(left, right))
        .count();
    let mut relative = PathBuf::new();
    for _ in &from[common..] {
        relative.push("..");
    }
    for component in &to[common..] {
        relative.push(component.as_os_str());
    }
    if relative.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        relative
    }
}

fn same_path_component(left: &Component<'_>, right: &Component<'_>) -> bool {
    if cfg!(windows) {
        left.as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(&right.as_os_str().to_string_lossy())
    } else {
        left == right
    }
}

fn lock_key(entity_id: &str) -> String {
    let mut key = String::new();
    let mut in_replacement = false;
    for character in entity_id.trim().to_ascii_lowercase().chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
            key.push(character);
            in_replacement = false;
        } else if !in_replacement {
            key.push('-');
            in_replacement = true;
        }
    }
    key.trim_start_matches('.').to_owned()
}

/// A held lock directory. Dropping it stops the heartbeat and releases the lock.
#[derive(Debug)]
pub struct FileLock {
    path: PathBuf,
    expected_mtime: Arc<Mutex<SystemTime>>,
    failure: Arc<Mutex<Option<String>>>,
    wake: Arc<(Mutex<bool>, Condvar)>,
    heartbeat: Option<JoinHandle<()>>,
}

impl FileLock {
    /// Return an error if the heartbeat detects that another process replaced this lock.
    pub fn check(&self) -> io::Result<()> {
        match self
            .failure
            .lock()
            .map_err(|_| io::Error::other("lock state is unavailable"))?
            .as_ref()
        {
            Some(reason) => Err(io::Error::other(format!("lock was compromised: {reason}"))),
            None => Ok(()),
        }
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        if let Ok(mut stopped) = self.wake.0.lock() {
            *stopped = true;
            self.wake.1.notify_all();
        }
        if let Some(heartbeat) = self.heartbeat.take() {
            let _ = heartbeat.join();
        }
        if self.failure.lock().is_ok_and(|failure| failure.is_none())
            && fs::metadata(&self.path)
                .and_then(|metadata| metadata.modified())
                .is_ok_and(|modified| {
                    self.expected_mtime
                        .lock()
                        .is_ok_and(|expected| modified == *expected)
                })
        {
            let _ = fs::remove_dir(&self.path);
        }
    }
}

fn acquire_lock(
    path: &Path,
    timeout: Duration,
    retry_delay: Duration,
    stale_after: Duration,
) -> io::Result<FileLock> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let started = Instant::now();
    loop {
        match fs::create_dir(path) {
            Ok(()) => {
                return match hold_lock(path.to_owned(), stale_after) {
                    Ok(lock) => Ok(lock),
                    Err(error) => {
                        let _ = fs::remove_dir(path);
                        Err(error)
                    }
                };
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                if remove_stale_lock(path, stale_after)? {
                    continue;
                }
                if started.elapsed() >= timeout {
                    return Err(io::Error::new(
                        if timeout.is_zero() {
                            io::ErrorKind::WouldBlock
                        } else {
                            io::ErrorKind::TimedOut
                        },
                        "lock is already held",
                    ));
                }
                thread::sleep(retry_delay.max(Duration::from_millis(1)));
            }
            Err(error) => return Err(error),
        }
    }
}

fn remove_stale_lock(path: &Path, stale_after: Duration) -> io::Result<bool> {
    // The directory remains the proper-lockfile contract. Rust contenders serialize stale
    // removal through a sibling advisory lock, which old TypeScript processes ignore.
    let mut claim_name = path.file_name().unwrap_or_default().to_os_string();
    claim_name.push(".stale-claim");
    let claim_path = path.with_file_name(claim_name);
    let claim = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(claim_path)?;
    claim.lock_exclusive()?;
    let stale = match fs::metadata(path).and_then(|metadata| metadata.modified()) {
        Ok(modified) => SystemTime::now()
            .duration_since(modified)
            .is_ok_and(|age| age > stale_after.max(Duration::from_secs(2))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => {
            let _ = FileExt::unlock(&claim);
            return Err(error);
        }
    };
    let removed = if stale {
        match fs::remove_dir(path) {
            Ok(()) => true,
            Err(error) if error.kind() == io::ErrorKind::NotFound => false,
            Err(error) => {
                let _ = FileExt::unlock(&claim);
                return Err(error);
            }
        }
    } else {
        false
    };
    FileExt::unlock(&claim)?;
    Ok(removed)
}

fn hold_lock(path: PathBuf, stale_after: Duration) -> io::Result<FileLock> {
    let mtime = fs::metadata(&path)?.modified()?;
    let expected_mtime = Arc::new(Mutex::new(mtime));
    let failure = Arc::new(Mutex::new(None));
    let wake = Arc::new((Mutex::new(false), Condvar::new()));
    let heartbeat_expected = Arc::clone(&expected_mtime);
    let heartbeat_failure = Arc::clone(&failure);
    let heartbeat_wake = Arc::clone(&wake);
    let heartbeat_path = path.clone();
    let period = (stale_after / 3).max(Duration::from_millis(100));
    let heartbeat = thread::spawn(move || {
        loop {
            let (stopped, changed) = &*heartbeat_wake;
            let Ok(stopped) = stopped.lock() else {
                break;
            };
            let Ok((stopped, timeout)) =
                changed.wait_timeout_while(stopped, period, |stopped| !*stopped)
            else {
                break;
            };
            if *stopped || !timeout.timed_out() {
                break;
            }
            let update = (|| {
                let current = fs::metadata(&heartbeat_path)?.modified()?;
                if current
                    != *heartbeat_expected
                        .lock()
                        .map_err(|_| io::Error::other("lock heartbeat state is unavailable"))?
                {
                    return Err(io::Error::other("lock ownership changed"));
                }
                set_file_mtime(&heartbeat_path, FileTime::now())?;
                let updated = fs::metadata(&heartbeat_path)?.modified()?;
                *heartbeat_expected
                    .lock()
                    .map_err(|_| io::Error::other("lock heartbeat state is unavailable"))? =
                    updated;
                Ok(())
            })();
            if let Err(error) = update {
                if let Ok(mut failure) = heartbeat_failure.lock() {
                    *failure = Some(error.to_string());
                }
                break;
            }
        }
    });
    Ok(FileLock {
        path,
        expected_mtime,
        failure,
        wake,
        heartbeat: Some(heartbeat),
    })
}

fn unique_sibling(target: &Path) -> PathBuf {
    let name = target.file_name().unwrap_or_default().to_string_lossy();
    target.with_file_name(format!(
        ".{name}.{}.{}.tmp",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ))
}

fn write_complete_file(path: &Path, contents: &str) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(contents.as_bytes())?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::{Storage, lock_key};
    use std::{fs, thread, time::Duration};

    #[test]
    fn entity_lock_keys_match_the_typescript_path_convention() {
        assert_eq!(lock_key("TASK /// 4"), "task-4");
        assert_eq!(lock_key("..."), "");
        assert_eq!(lock_key("Task_4"), "task_4");
    }

    #[test]
    fn compromised_lock_does_not_remove_a_replacement_lock_on_drop() {
        let root = std::env::temp_dir().join(format!(
            "backlog-lock-owner-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let target = root.join("task.md");
        fs::write(&target, "task").unwrap();
        let storage = Storage::new(root.clone(), None).with_lock_settings(
            Duration::ZERO,
            Duration::ZERO,
            Duration::from_secs(2),
        );
        let guard = storage
            .acquire_entity_lock("task", "TASK-1", &target)
            .unwrap();
        let replacement = guard.path.clone();
        fs::remove_dir(&replacement).unwrap();
        fs::create_dir(&replacement).unwrap();
        thread::sleep(Duration::from_millis(750));
        assert!(guard.check().is_err());
        drop(guard);
        assert!(replacement.is_dir());
        fs::remove_dir_all(root).unwrap();
    }
}
