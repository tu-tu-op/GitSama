use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_LOG_BYTES: u64 = 256 * 1024;

pub fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis())
}

pub fn write(paths: &crate::paths::AppPaths, message: &str) {
    let _ = append_bounded(&paths.log, message);
}

fn append_bounded(path: &Path, message: &str) -> std::io::Result<()> {
    if let Ok(metadata) = fs::metadata(path) {
        if metadata.len() >= MAX_LOG_BYTES {
            let rotated = path.with_extension("log.1");
            let _ = fs::remove_file(&rotated);
            let _ = fs::rename(path, rotated);
        }
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{} {}", now_millis(), message)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::append_bounded;
    use std::fs;

    #[test]
    fn rotates_a_large_log() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("gitsama.log");
        fs::write(&path, vec![b'x'; 256 * 1024]).expect("write log");

        append_bounded(&path, "new line").expect("append");
        assert!(path.with_extension("log.1").exists());
        assert!(fs::read_to_string(path).expect("read").contains("new line"));
    }
}

