use std::{
    env,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use rodio::{Decoder, OutputStreamBuilder, Sink};
use serde::Serialize;

use crate::{
    config::Config,
    error::{Error, Result},
    events::EventKind,
    logging, packs,
    paths::AppPaths,
};

#[derive(Clone, Debug, Default)]
pub struct DispatchDetails {
    pub branch: Option<String>,
    pub commit_count: Option<u64>,
}

#[derive(Serialize)]
struct TestRecord {
    event: EventKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    commit_count: Option<u64>,
}

pub fn dispatch(
    paths: &AppPaths,
    config: &Config,
    requested: EventKind,
    details: DispatchDetails,
) -> Result<()> {
    if !config.enabled {
        return Ok(());
    }

    let pack = packs::find(paths, &config.active_pack)?;
    let Some((resolved_event, _)) = resolve_enabled(config, &pack, requested) else {
        return Ok(());
    };

    if test_mode() {
        let record = TestRecord {
            event: resolved_event,
            branch: details.branch,
            commit_count: details.commit_count,
        };
        return write_test_record(&record);
    }

    let executable = env::current_exe().map_err(|error| {
        Error::Audio(format!("could not locate the GitSama executable: {error}"))
    })?;
    let mut child = Command::new(executable);
    child
        .arg("__play")
        .arg(resolved_event.as_str())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    child
        .spawn()
        .map(|_| ())
        .map_err(|error| Error::Audio(format!("could not start detached playback: {error}")))
}

pub fn play(paths: &AppPaths, config: &Config, event: EventKind) -> Result<()> {
    if !config.enabled {
        return Ok(());
    }
    let pack = packs::find(paths, &config.active_pack)?;
    let Some((_, path)) = pack.resolve(event) else {
        return Ok(());
    };
    let Some(_lock) = PlaybackLock::acquire(paths, config.queue_max_age_ms)? else {
        return Ok(());
    };

    let stream = OutputStreamBuilder::open_default_stream()
        .map_err(|error| Error::Audio(error.to_string()))?;
    let sink = Sink::connect_new(stream.mixer());
    let file = File::open(&path)
        .map_err(|error| Error::Audio(format!("could not open {}: {error}", path.display())))?;
    let decoder = Decoder::try_from(file).map_err(|error| Error::Audio(error.to_string()))?;
    sink.set_volume(f32::from(config.volume) / 100.0);
    sink.append(decoder);
    sink.sleep_until_end();
    Ok(())
}

fn resolve_enabled(
    config: &Config,
    pack: &packs::Pack,
    requested: EventKind,
) -> Option<(EventKind, PathBuf)> {
    if requested == EventKind::MassivePush {
        if config.is_event_enabled(EventKind::MassivePush)
            && !pack.files_for(EventKind::MassivePush).is_empty()
        {
            return pack.resolve(EventKind::MassivePush);
        }
        if config.is_event_enabled(EventKind::Push) {
            return pack.resolve(EventKind::Push);
        }
        return None;
    }
    config
        .is_event_enabled(requested)
        .then(|| pack.resolve(requested))
        .flatten()
}

pub fn probe_output() -> Result<()> {
    let _stream = OutputStreamBuilder::open_default_stream()
        .map_err(|error| Error::Audio(error.to_string()))?;
    Ok(())
}

pub fn test_mode() -> bool {
    test_mode_value(env::var("GITSAMA_TEST_MODE").ok().as_deref())
}

fn test_mode_value(value: Option<&str>) -> bool {
    value == Some("1")
}

fn write_test_record(record: &TestRecord) -> Result<()> {
    let path = env::var_os("GITSAMA_TEST_LOG")
        .map(PathBuf::from)
        .ok_or_else(|| Error::Audio("GITSAMA_TEST_LOG is not set".to_owned()))?;
    write_test_record_to(&path, record)
}

fn write_test_record_to(path: &Path, record: &TestRecord) -> Result<()> {
    let text = serde_json::to_string(record).map_err(|error| Error::Audio(error.to_string()))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| Error::WriteFile {
            path: path.to_path_buf(),
            source: error,
        })?;
    writeln!(file, "{text}").map_err(|source| Error::WriteFile {
        path: path.to_path_buf(),
        source,
    })
}

struct PlaybackLock {
    path: PathBuf,
    stop: Arc<AtomicBool>,
    heartbeat: Option<thread::JoinHandle<()>>,
}

impl PlaybackLock {
    fn acquire(paths: &AppPaths, max_age_ms: u64) -> Result<Option<Self>> {
        paths.ensure_layout()?;
        let path = paths.state.join("playback.lock");
        let start = Instant::now();
        let max_age = Duration::from_millis(max_age_ms);

        loop {
            match fs::create_dir(&path) {
                Ok(()) => match Self::start(path.clone()) {
                    Ok(lock) => return Ok(Some(lock)),
                    Err(error) => {
                        let _ = fs::remove_dir_all(&path);
                        return Err(error);
                    }
                },
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    if lock_is_stale(&path, max_age) {
                        let _ = fs::remove_dir_all(&path);
                        continue;
                    }
                    if start.elapsed() >= max_age {
                        return Ok(None);
                    }
                    thread::sleep(Duration::from_millis(25));
                }
                Err(error) => {
                    return Err(Error::WriteFile {
                        path: path.clone(),
                        source: error,
                    });
                }
            }
        }
    }

    fn start(path: PathBuf) -> Result<Self> {
        let heartbeat_path = path.join("heartbeat");
        if let Err(source) = fs::write(&heartbeat_path, b"active") {
            let _ = fs::remove_dir_all(&path);
            return Err(Error::WriteFile {
                path: heartbeat_path.clone(),
                source,
            });
        }
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread_path = heartbeat_path;
        let thread_path_for_thread = thread_path.clone();
        let heartbeat = match thread::Builder::new()
            .name("gitsama-playback-heartbeat".to_owned())
            .spawn(move || {
                while !thread_stop.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(250));
                    if !thread_stop.load(Ordering::Relaxed) {
                        let _ = OpenOptions::new()
                            .write(true)
                            .truncate(true)
                            .open(&thread_path_for_thread);
                    }
                }
            }) {
            Ok(heartbeat) => heartbeat,
            Err(source) => {
                let _ = fs::remove_file(&thread_path);
                let _ = fs::remove_dir_all(&path);
                return Err(Error::Audio(format!(
                    "could not start playback heartbeat: {source}"
                )));
            }
        };
        Ok(Self {
            path,
            stop,
            heartbeat: Some(heartbeat),
        })
    }
}

impl Drop for PlaybackLock {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(heartbeat) = self.heartbeat.take() {
            let _ = heartbeat.join();
        }
        let _ = fs::remove_file(self.path.join("heartbeat"));
        let _ = fs::remove_dir(&self.path);
    }
}

fn lock_is_stale(path: &Path, max_age: Duration) -> bool {
    fs::metadata(path.join("heartbeat"))
        .or_else(|_| fs::metadata(path))
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|age| age > max_age)
}

pub fn log_playback_error(paths: &AppPaths, error: &Error) {
    logging::write(paths, &format!("playback failed: {error}"));
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{PlaybackLock, TestRecord, resolve_enabled, test_mode_value, write_test_record_to};
    use crate::paths::AppPaths;
    use crate::{config::Config, events::EventKind, packs::Pack};

    #[test]
    fn lock_is_released() {
        let directory = tempfile::tempdir().expect("temp");
        let paths = AppPaths::from_root(directory.path().join(".gitsama"));
        let lock = PlaybackLock::acquire(&paths, 1000)
            .expect("lock")
            .expect("available");
        assert!(paths.state.join("playback.lock").exists());
        drop(lock);
        assert!(!paths.state.join("playback.lock").exists());
    }

    #[test]
    fn test_mode_requires_explicit_value() {
        assert!(test_mode_value(Some("1")));
        assert!(!test_mode_value(Some("true")));
        assert!(!test_mode_value(None));
    }

    #[test]
    fn starter_test_log_is_jsonl_shaped() {
        let directory = tempfile::tempdir().expect("temp");
        let log = directory.path().join("events.jsonl");
        write_test_record_to(
            &log,
            &TestRecord {
                event: crate::events::EventKind::Commit,
                branch: None,
                commit_count: None,
            },
        )
        .expect("record");
        assert!(
            fs::read_to_string(log)
                .expect("read")
                .contains("\"event\":\"commit\"")
        );
    }

    #[test]
    fn disabled_massive_push_uses_enabled_push_clip() {
        let config = Config::default();
        let mut config = config;
        config.events.massive_push = false;
        let pack = Pack {
            root: std::path::PathBuf::from("."),
            manifest: crate::packs::PackManifest {
                schema_version: 1,
                id: "starter".to_owned(),
                name: "Starter".to_owned(),
                author: "GitSama".to_owned(),
                version: "1.0.0".to_owned(),
                description: "test".to_owned(),
                license: "MIT".to_owned(),
                events: [("push".to_owned(), vec!["audio/push.wav".to_owned()])]
                    .into_iter()
                    .collect(),
            },
        };
        assert_eq!(
            resolve_enabled(&config, &pack, EventKind::MassivePush).map(|(event, _)| event),
            Some(EventKind::Push)
        );

        config.events.push = false;
        assert!(resolve_enabled(&config, &pack, EventKind::MassivePush).is_none());
    }
}
