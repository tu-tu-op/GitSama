use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[cfg(not(windows))]
use std::fs;

use crate::error::{Error, Result};

pub fn executable_name() -> &'static str {
    if cfg!(windows) {
        "gitsama.exe"
    } else {
        "gitsama"
    }
}

pub fn shell_quote(path: &Path) -> String {
    let value = path.to_string_lossy();
    // Configured hooks use Git's POSIX shell, including on Windows. Rust's
    // canonicalize() adds a verbatim prefix which that shell cannot execute.
    #[cfg(windows)]
    let value = if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
        format!("//{}", unc.replace('\\', "/"))
    } else {
        value
            .strip_prefix(r"\\?\")
            .unwrap_or(&value)
            .replace('\\', "/")
    };
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// Identify the Git invocation behind our configured shell command. A shell
/// remains between Git and GitSama because the command ends in a failure guard.
pub fn hook_git_pid() -> Option<u32> {
    let parent = std::env::var("GITSAMA_GIT_PID").ok()?;
    #[cfg(not(windows))]
    {
        parent.parse().ok().filter(|pid| *pid > 1)
    }
    #[cfg(windows)]
    {
        // MSYS reports PPID=1 for a native Windows parent. Read the native
        // process tree instead; no process arguments are read or logged.
        let _ = parent;
        windows_git_parent()
    }
}

#[cfg(windows)]
fn windows_git_parent() -> Option<u32> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
            TH32CS_SNAPPROCESS,
        },
    };

    // SAFETY: These calls use a checked snapshot handle and a correctly sized
    // initialized output structure. The handle is closed before returning.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut parents = Vec::new();
        let mut found = Process32FirstW(snapshot, &mut entry);
        while found != 0 {
            let end = entry
                .szExeFile
                .iter()
                .position(|ch| *ch == 0)
                .unwrap_or(entry.szExeFile.len());
            let is_git =
                String::from_utf16_lossy(&entry.szExeFile[..end]).eq_ignore_ascii_case("git.exe");
            parents.push((entry.th32ProcessID, entry.th32ParentProcessID, is_git));
            found = Process32NextW(snapshot, &mut entry);
        }
        CloseHandle(snapshot);
        // MSYS may insert more than one shell process while emulating fork.
        let mut pid = std::process::id();
        for _ in 0..16 {
            let (_, parent, is_git) = parents.iter().find(|(id, _, _)| *id == pid)?;
            if *is_git {
                return Some(pid);
            }
            pid = *parent;
        }
        None
    }
}

#[cfg(windows)]
pub fn path_separator() -> char {
    ';'
}

pub fn installed_binary(paths: &crate::paths::AppPaths) -> PathBuf {
    paths.bin.join(executable_name())
}

pub fn remove_user_path_entry(bin: &Path) -> Result<bool> {
    #[cfg(windows)]
    {
        remove_windows_path_entry(bin)
    }
    #[cfg(not(windows))]
    {
        remove_unix_path_markers(bin)
    }
}

pub fn schedule_cleanup(executable: &Path, data_root: Option<&Path>) -> Result<()> {
    #[cfg(windows)]
    {
        let executable = windows_cmd_quote(executable);
        let cleanup = data_root.map(windows_cmd_quote);
        let mut command_text =
            String::from("cd /d \"%TEMP%\" >nul 2>nul & timeout /t 1 /nobreak >nul");
        command_text.push_str(" & del /f /q ");
        command_text.push_str(&executable);
        if let Some(root) = cleanup {
            command_text.push_str(" & rmdir /s /q ");
            command_text.push_str(&root);
        }
        Command::new("cmd.exe")
            .args(["/C", &command_text])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|error| Error::message(format!("could not schedule Windows cleanup: {error}")))
    }
    #[cfg(not(windows))]
    {
        let script = if data_root.is_some() {
            "sleep 1; rm -f -- \"$1\"; rm -rf -- \"$2\""
        } else {
            "sleep 1; rm -f -- \"$1\""
        };
        let mut command = Command::new("sh");
        command
            .args(["-c", script, "gitsama-cleanup"])
            .arg(executable)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(root) = data_root {
            command.arg(root);
        }
        command
            .spawn()
            .map(|_| ())
            .map_err(|error| Error::message(format!("could not schedule cleanup: {error}")))
    }
}

#[cfg(windows)]
fn remove_windows_path_entry(bin: &Path) -> Result<bool> {
    let output = Command::new("reg.exe")
        .args(["query", r"HKCU\Environment", "/v", "Path"])
        .output()
        .map_err(|error| Error::message(format!("could not inspect the user PATH: {error}")))?;
    if !output.status.success() {
        return Ok(false);
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let Some(line) = text.lines().find(|line| line.contains("REG_")) else {
        return Ok(false);
    };
    let Some(type_start) = line.find("REG_") else {
        return Ok(false);
    };
    let Some(value_start) = line[type_start..].find(char::is_whitespace) else {
        return Ok(false);
    };
    let value = line[type_start + value_start..].trim();
    let wanted = normalize_windows_path(bin);
    let entries: Vec<String> = value
        .split(path_separator())
        .map(str::trim)
        .filter(|entry| !entry.is_empty() && normalize_windows_path(Path::new(entry)) != wanted)
        .map(ToOwned::to_owned)
        .collect();
    if entries.len()
        == value
            .split(path_separator())
            .filter(|entry| !entry.trim().is_empty())
            .count()
    {
        return Ok(false);
    }

    if entries.is_empty() {
        let _ = Command::new("reg.exe")
            .args([r"delete", r"HKCU\Environment", "/v", "Path", "/f"])
            .status();
    } else {
        let new_value = entries.join(";");
        let output = Command::new("reg.exe")
            .args([
                "add",
                r"HKCU\Environment",
                "/v",
                "Path",
                "/t",
                "REG_EXPAND_SZ",
                "/d",
                &new_value,
                "/f",
            ])
            .output()
            .map_err(|error| Error::message(format!("could not update the user PATH: {error}")))?;
        if !output.status.success() {
            return Err(Error::message("could not update the user PATH"));
        }
    }
    Ok(true)
}

#[cfg(windows)]
fn normalize_windows_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

#[cfg(windows)]
fn windows_cmd_quote(path: &Path) -> String {
    format!("\"{}\"", path.to_string_lossy().replace('"', "\\\""))
}

#[cfg(not(windows))]
fn remove_unix_path_markers(bin: &Path) -> Result<bool> {
    let Some(home) = dirs::home_dir() else {
        return Ok(false);
    };
    let marker = "# GitSama PATH";
    let bin_text = bin.to_string_lossy();
    let mut changed = false;
    let profiles: Vec<PathBuf> = if let Some(profile) = std::env::var_os("GITSAMA_SHELL_PROFILE") {
        vec![PathBuf::from(profile)]
    } else {
        [".profile", ".bashrc", ".zshrc"]
            .into_iter()
            .map(|profile| home.join(profile))
            .collect()
    };

    for path in profiles {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let lines: Vec<&str> = text.lines().collect();
        let mut kept = Vec::new();
        let mut index = 0;
        let mut profile_changed = false;
        while index < lines.len() {
            if lines[index].trim() == marker {
                changed = true;
                profile_changed = true;
                index += 1;
                if index < lines.len() && lines[index].contains(bin_text.as_ref()) {
                    index += 1;
                }
            } else {
                kept.push(lines[index]);
                index += 1;
            }
        }
        if profile_changed {
            let mut output = kept.join("\n");
            if text.ends_with('\n') {
                output.push('\n');
            }
            fs::write(&path, output).map_err(|source| Error::WriteFile {
                path: path.clone(),
                source,
            })?;
        }
    }
    Ok(changed)
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
        assert!(quoted.starts_with('\''));
        assert!(quoted.contains("Git Sama"));
    }

    #[test]
    fn quotes_single_quotes_and_shell_metacharacters() {
        assert_eq!(
            shell_quote(Path::new("/tmp/O'Reilly/$HOME `name`/gitsama")),
            "'/tmp/O'\\''Reilly/$HOME `name`/gitsama'"
        );
    }

    #[cfg(windows)]
    #[test]
    fn normalizes_verbatim_drive_and_unc_paths_for_git_shell() {
        assert_eq!(
            shell_quote(Path::new(r"\\?\C:\Git Sama\bin\gitsama.exe")),
            "'C:/Git Sama/bin/gitsama.exe'"
        );
        assert_eq!(
            shell_quote(Path::new(r"\\?\UNC\server\share\Git Sama\gitsama.exe")),
            "'//server/share/Git Sama/gitsama.exe'"
        );
    }
}
