use backlog_md::{records::parse_document, serialization::serialize_document, storage::Storage};
use std::{
    fs,
    io::{self, BufRead, BufReader, Read},
    ops::Deref,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        Arc, Barrier,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::Duration,
};

struct TempDir(PathBuf);

impl Deref for TempDir {
    type Target = PathBuf;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn temp_dir() -> TempDir {
    let path = std::env::temp_dir().join(format!(
        "backlog-storage-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&path).unwrap();
    TempDir(path)
}

#[test]
fn storage_reads_and_saves_records_through_explicit_paths() {
    let root = temp_dir();
    let path = root.join("docs/guide.md");
    let storage = Storage::new(root.0.clone(), None);
    let source = "---\nid: DOC-1\ntitle: Guide\ntype: note\nunknown: retained\n---\nBody.\n";
    let mut record = parse_document(source).unwrap();
    record.title = "Renamed".into();
    let serialized = serialize_document(&record, source).unwrap();

    storage.write_record(&path, &serialized, None).unwrap();
    assert_eq!(storage.read(&path).unwrap(), serialized);
    assert!(storage.read(&path).unwrap().contains("unknown: retained"));
}

#[test]
fn record_creation_never_overwrites_an_existing_target() {
    let root = temp_dir();
    let path = root.join("task.md");
    fs::write(&path, "original").unwrap();
    let storage = Storage::new(root.0.clone(), None);
    let error = storage
        .write_record(&path, "replacement", None)
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(fs::read_to_string(&path).unwrap(), "original");
}

#[test]
fn storage_updates_in_place_and_moves_to_an_unoccupied_path() {
    let root = temp_dir();
    let source = root.join("old.md");
    let target = root.join("new.md");
    fs::write(&source, "old").unwrap();
    let storage = Storage::new(root.0.clone(), None);

    storage
        .write_record(&source, "updated", Some(&source))
        .unwrap();
    assert_eq!(fs::read_to_string(&source).unwrap(), "updated");
    storage
        .write_record(&target, "moved", Some(&source))
        .unwrap();
    assert!(!source.exists());
    assert_eq!(fs::read_to_string(&target).unwrap(), "moved");
}

#[test]
fn missing_move_source_still_writes_the_free_target() {
    let root = temp_dir();
    let source = root.join("removed.md");
    let target = root.join("new.md");
    let storage = Storage::new(root.0.clone(), None);

    storage
        .write_record(&target, "saved", Some(&source))
        .unwrap();
    assert_eq!(fs::read_to_string(target).unwrap(), "saved");
}

#[test]
fn moved_record_claims_target_without_overwriting_and_keeps_source_on_collision() {
    let root = temp_dir();
    let source = root.join("old.md");
    let target = root.join("new.md");
    fs::write(&source, "old").unwrap();
    fs::write(&target, "occupied").unwrap();
    let storage = Storage::new(root.0.clone(), None);
    let error = storage
        .write_record(&target, "new content", Some(&source))
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(fs::read_to_string(&source).unwrap(), "old");
    assert_eq!(fs::read_to_string(&target).unwrap(), "occupied");
}

#[test]
fn create_lock_waits_and_entity_lock_fails_fast_on_contention() {
    let root = temp_dir();
    let target = root.join("task.md");
    fs::write(&target, "task").unwrap();
    let storage = Storage::new(root.0.clone(), None).with_lock_settings(
        Duration::from_millis(80),
        Duration::from_millis(5),
        Duration::from_secs(2),
    );
    let held = storage
        .acquire_entity_lock("task", "TASK-1", &target)
        .unwrap();
    assert_eq!(
        storage
            .acquire_entity_lock("task", "TASK-1", &target)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    drop(held);
    let lock = storage.acquire_create_lock().unwrap();
    drop(lock);
}

#[test]
fn rust_and_typescript_share_proper_lockfile_directories() {
    const HOLD_LOCK: &str = r#"
import lockfile from "proper-lockfile";
const release = await lockfile.lock(process.env.LOCK_TARGET, {
  lockfilePath: process.env.LOCK_PATH,
  stale: 2000,
  retries: { retries: 0, factor: 1, minTimeout: 0, maxTimeout: 0, randomize: false }
});
process.stdout.write("ready\n");
await new Promise(resolve => setTimeout(resolve, 400));
await release();
"#;
    const TRY_LOCK: &str = r#"
import lockfile from "proper-lockfile";
try {
  await lockfile.lock(process.env.LOCK_TARGET, {
    lockfilePath: process.env.LOCK_PATH,
    stale: 2000,
    retries: { retries: 0, factor: 1, minTimeout: 0, maxTimeout: 0, randomize: false }
  });
  process.stdout.write("acquired\n");
} catch (error) {
  process.stdout.write(`${error.code}\n`);
}
"#;
    let root = temp_dir();
    let target = root.join("tasks");
    fs::create_dir(&target).unwrap();
    let common_git_dir = root.join(".git");
    fs::create_dir(&common_git_dir).unwrap();
    let lock_path = common_git_dir.join("backlog.md/locks/create");
    fs::create_dir_all(lock_path.parent().unwrap()).unwrap();
    let bun = std::env::var_os("BACKLOG_REFERENCE_BUN").unwrap_or_else(|| "bun".into());
    let mut typescript = Command::new(&bun)
        .args(["--eval", HOLD_LOCK])
        .env("LOCK_TARGET", &target)
        .env("LOCK_PATH", &lock_path)
        .current_dir(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap(),
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Bun is required for the TypeScript lock comparison");
    let mut ready = String::new();
    BufReader::new(typescript.stdout.take().unwrap())
        .read_line(&mut ready)
        .unwrap();
    if ready != "ready\n" {
        let mut stderr = String::new();
        typescript
            .stderr
            .take()
            .unwrap()
            .read_to_string(&mut stderr)
            .unwrap();
        panic!("TypeScript lock failed before acquisition: {stderr}");
    }
    let storage = Storage::new(root.0.clone(), Some(common_git_dir)).with_lock_settings(
        Duration::from_millis(80),
        Duration::from_millis(5),
        Duration::from_secs(2),
    );
    let started = std::time::Instant::now();
    assert_eq!(
        storage.acquire_create_lock().unwrap_err().kind(),
        io::ErrorKind::TimedOut
    );
    assert!(started.elapsed() >= Duration::from_millis(70));
    assert!(typescript.wait().unwrap().success());

    let rust_lock = storage.acquire_create_lock().unwrap();
    let attempt = Command::new(&bun)
        .args(["--eval", TRY_LOCK])
        .env("LOCK_TARGET", &target)
        .env("LOCK_PATH", &lock_path)
        .current_dir(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap(),
        )
        .output()
        .unwrap();
    assert!(attempt.status.success());
    assert_eq!(String::from_utf8(attempt.stdout).unwrap(), "ELOCKED\n");
    drop(rust_lock);

    let entity_target = root.join("task.md");
    fs::write(&entity_target, "task").unwrap();
    let entity_lock_path = root.join(".locks/task-task-1");
    fs::create_dir_all(entity_lock_path.parent().unwrap()).unwrap();
    let mut typescript_entity = Command::new(&bun)
        .args(["--eval", HOLD_LOCK])
        .env("LOCK_TARGET", &entity_target)
        .env("LOCK_PATH", &entity_lock_path)
        .current_dir(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap(),
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut ready = String::new();
    BufReader::new(typescript_entity.stdout.take().unwrap())
        .read_line(&mut ready)
        .unwrap();
    assert_eq!(ready, "ready\n");
    assert_eq!(
        storage
            .acquire_entity_lock("task", "TASK-1", &entity_target)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    assert!(typescript_entity.wait().unwrap().success());

    let rust_entity = storage
        .acquire_entity_lock("task", "TASK-1", &entity_target)
        .unwrap();
    let attempt = Command::new(&bun)
        .args(["--eval", TRY_LOCK])
        .env("LOCK_TARGET", &entity_target)
        .env("LOCK_PATH", &entity_lock_path)
        .current_dir(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap(),
        )
        .output()
        .unwrap();
    assert!(attempt.status.success());
    assert_eq!(String::from_utf8(attempt.stdout).unwrap(), "ELOCKED\n");
    drop(rust_entity);
}

#[test]
fn stale_entity_lock_is_recovered_and_held_locks_refresh_their_age() {
    let root = temp_dir();
    let target = root.join("task.md");
    fs::write(&target, "task").unwrap();
    let lock_path = root.join(".locks/task-task-1");
    fs::create_dir_all(&lock_path).unwrap();
    filetime::set_file_mtime(
        &lock_path,
        filetime::FileTime::from_system_time(std::time::SystemTime::now() - Duration::from_secs(4)),
    )
    .unwrap();

    let storage = Storage::new(root.0.clone(), None).with_lock_settings(
        Duration::ZERO,
        Duration::ZERO,
        Duration::from_secs(2),
    );
    let held = storage
        .acquire_entity_lock("task", "TASK-1", &target)
        .unwrap();
    thread::sleep(Duration::from_millis(2200));
    assert_eq!(
        storage
            .acquire_entity_lock("task", "TASK-1", &target)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    drop(held);
}

#[test]
fn competing_stale_lock_recovery_never_admits_two_holders() {
    let root = temp_dir();
    let target = root.join("task.md");
    fs::write(&target, "task").unwrap();
    let lock_path = root.join(".locks/task-task-1");
    fs::create_dir_all(&lock_path).unwrap();
    filetime::set_file_mtime(
        &lock_path,
        filetime::FileTime::from_system_time(std::time::SystemTime::now() - Duration::from_secs(4)),
    )
    .unwrap();

    let workers = 32;
    let barrier = Arc::new(Barrier::new(workers));
    let active = Arc::new(AtomicUsize::new(0));
    let maximum_active = Arc::new(AtomicUsize::new(0));
    let mut threads = Vec::new();
    for _ in 0..workers {
        let root = root.0.clone();
        let target = target.clone();
        let barrier = Arc::clone(&barrier);
        let active = Arc::clone(&active);
        let maximum_active = Arc::clone(&maximum_active);
        threads.push(std::thread::spawn(move || {
            let storage = Storage::new(root, None).with_lock_settings(
                Duration::ZERO,
                Duration::ZERO,
                Duration::from_secs(2),
            );
            barrier.wait();
            let Ok(guard) = storage.acquire_entity_lock("task", "TASK-1", &target) else {
                return false;
            };
            let current = active.fetch_add(1, Ordering::SeqCst) + 1;
            maximum_active.fetch_max(current, Ordering::SeqCst);
            thread::sleep(Duration::from_millis(10));
            active.fetch_sub(1, Ordering::SeqCst);
            drop(guard);
            true
        }));
    }
    let successes = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .filter(|success| *success)
        .count();
    assert_eq!(successes, 1);
    assert_eq!(maximum_active.load(Ordering::SeqCst), 1);
}
