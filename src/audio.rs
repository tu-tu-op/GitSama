use std::{
    env,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use rodio::{Decoder, OutputStreamBuilder, Sink};
use serde::Serialize;

use crate::{
    config::Config,
    error::{Error, Result},
    events::EventKind,
    logging,
    packs,
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
    let Some((resolved_event, _)) = pack.resolve(requested) else {
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

    let executable = env::current_exe()
        .map_err(|error| Error::Audio(format!("could not locate the GitSama executable: {error}")))?;
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
    let file = File::open(&path).map_err(|error| Error::Audio(format!(
        "could not open {}: {error}",
        path.display()
    )))?;
    let decoder = Decoder::try_from(file).map_err(|error| Error::Audio(error.to_string()))?;
    sink.set_volume(f32::from(config.volume) / 100.0);
    sink.append(decoder);
    sink.sleep_until_end();
    Ok(())
}

pub fn probe_output() -> Result<()> {
    let _stream = OutputStreamBuilder::open_default_stream()
        .map_err(|error| Error::Audio(error.to_string()))?;
    Ok(())
}

pub fn test_mode() -> bool {
    env::var("GITSAMA_TEST_MODE").is_ok_and(|value| value == "1")
}

fn write_test_record(record: &TestRecord) -> Result<()> {
    let path = env::var_os("GITSAMA_TEST_LOG")
        .map(PathBuf::from)
        .ok_or_else(|| Error::Audio("GITSAMA_TEST_LOG is not set".to_owned()))?;
    let text =
        serde_json::to_string(record).map_err(|error| Error::Audio(error.to_string()))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| Error::WriteFile {
            path: path.clone(),
            source: error,
        })?;
    writeln!(file, "{text}").map_err(|source| Error::WriteFile { path, source })
}

struct PlaybackLock {
    path: PathBuf,
}

impl PlaybackLock {
    fn acquire(paths: &AppPaths, max_age_ms: u64) -> Result<Option<Self>> {
        paths.ensure_layout()?;
        let path = paths.state.join("playback.lock");
        let start = Instant::now();
        let max_age = Duration::from_millis(max_age_ms);

        loop {
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Some(Self { path })),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    if lock_is_stale(&path, max_age) {
                        let _ = fs::remove_dir(&path);
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
}

impl Drop for PlaybackLock {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.path);
    }
}

fn lock_is_stale(path: &Path, max_age: Duration) -> bool {
    fs::metadata(path)
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
    use std::{env, fs};

    use super::{PlaybackLock, test_mode};
    use crate::paths::AppPaths;

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
        unsafe {
            env::set_var("GITSAMA_TEST_MODE", "1");
        }
        assert!(test_mode());
        unsafe {
            env::remove_var("GITSAMA_TEST_MODE");
        }
    }

    #[test]
    fn starter_test_log_is_jsonl_shaped() {
        let directory = tempfile::tempdir().expect("temp");
        let log = directory.path().join("events.jsonl");
        unsafe {
            env::set_var("GITSAMA_TEST_LOG", &log);
        }
        super::write_test_record(&super::TestRecord {
            event: crate::events::EventKind::Commit,
            branch: None,
            commit_count: None,
        })
        .expect("record");
        assert!(fs::read_to_string(log).expect("read").contains("\"event\":\"commit\""));
        unsafe {
            env::remove_var("GITSAMA_TEST_LOG");
        }
    }
}

