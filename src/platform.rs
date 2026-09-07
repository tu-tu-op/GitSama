use std::path::Path;

pub fn executable_name() -> &'static str {
    if cfg!(windows) {
        "gitsama.exe"
    } else {
        "gitsama"
    }
}

pub fn shell_quote(path: &Path) -> String {
    let value = path.to_string_lossy();
    if cfg!(windows) {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

pub fn path_separator() -> char {
    if cfg!(windows) { ';' } else { ':' }
}

#[cfg(test)]
mod tests {
    use super::shell_quote;
    use std::path::Path;

    #[test]
    fn quotes_paths_with_spaces() {
        let quoted = shell_quote(Path::new(if cfg!(windows) {
            r"C:\Git Sama\bin\gitsama.exe"
        } else {
            "/tmp/Git Sama/bin/gitsama"
        }));
        assert!(quoted.starts_with(if cfg!(windows) { '"' } else { '\'' }));
        assert!(quoted.contains("Git Sama"));
    }

    #[test]
    fn quotes_single_quotes_on_unix() {
        if !cfg!(windows) {
            assert_eq!(
                shell_quote(Path::new("/tmp/O'Reilly/gitsama")),
                "'/tmp/O'\\''Reilly/gitsama'"
            );
        }
    }
}

