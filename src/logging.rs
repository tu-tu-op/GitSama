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
    if fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(std::io::Error::other("refusing to write a symlink log"));
    }
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

    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    // Bound individual entries too, and prevent line/terminal control injection.
    let message: String = message
        .chars()
        .take(4096)
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect();
    writeln!(file, "{} {}", now_millis(), message)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::append_bounded;
    use std::fs;

    #[test]
    fn log_entries_are_bounded_and_cannot_inject_lines() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("gitsama.log");
        append_bounded(&path, &format!("one\n\u{1b}[2J{}", "x".repeat(5000))).expect("append");
        let text = fs::read_to_string(&path).expect("read");
        assert_eq!(text.lines().count(), 1);
        assert!(!text.contains('\u{1b}'));
        assert!(text.len() < 4200);
    }

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
