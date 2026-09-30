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

#[cfg(windows)]
pub fn current_job_info() -> String {
    use windows_sys::Win32::System::JobObjects::{
        IsProcessInJob, JOB_OBJECT_LIMIT_BREAKAWAY_OK, JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        QueryInformationJobObject,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    unsafe {
        let mut in_job: i32 = 0;
        if IsProcessInJob(GetCurrentProcess(), std::ptr::null_mut(), &mut in_job) != 0 {
            if in_job != 0 {
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                let mut ret_len: u32 = 0;
                let success = QueryInformationJobObject(
                    std::ptr::null_mut(),
                    JobObjectExtendedLimitInformation,
                    &mut info as *mut _ as *mut _,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                    &mut ret_len,
                );
                if success != 0 {
                    let flags = info.BasicLimitInformation.LimitFlags;
                    let breakaway = (flags & JOB_OBJECT_LIMIT_BREAKAWAY_OK) != 0;
                    let silent = (flags & JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK) != 0;
                    format!("in_job=true breakaway_ok={breakaway} silent_breakaway={silent}")
                } else {
                    "in_job=true breakaway_ok=unknown".to_string()
                }
            } else {
                "in_job=false".to_string()
            }
        } else {
            "in_job=unknown".to_string()
        }
    }
}

#[cfg(not(windows))]
pub fn current_job_info() -> String {
    "in_job=false".to_string()
}

#[cfg(windows)]
pub fn breakaway_is_permitted() -> Option<bool> {
    use windows_sys::Win32::System::JobObjects::{
        IsProcessInJob, JOB_OBJECT_LIMIT_BREAKAWAY_OK, JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        QueryInformationJobObject,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    unsafe {
        let mut in_job: i32 = 0;
        if IsProcessInJob(GetCurrentProcess(), std::ptr::null_mut(), &mut in_job) != 0 {
            if in_job != 0 {
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                let mut ret_len: u32 = 0;
                let success = QueryInformationJobObject(
                    std::ptr::null_mut(),
                    JobObjectExtendedLimitInformation,
                    &mut info as *mut _ as *mut _,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                    &mut ret_len,
                );
                if success != 0 {
                    let flags = info.BasicLimitInformation.LimitFlags;
                    let breakaway = (flags & JOB_OBJECT_LIMIT_BREAKAWAY_OK) != 0
                        || (flags & JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK) != 0;
                    Some(breakaway)
                } else {
                    None
                }
            } else {
                Some(true)
            }
        } else {
            None
        }
    }
}

#[cfg(not(windows))]
pub fn breakaway_is_permitted() -> Option<bool> {
    Some(true)
}

pub fn is_agent_environment() -> bool {
    const AGENT_ENV_VARS: [&str; 8] = [
        "ANTIGRAVITY_AGENT",
        "GEMINI_CLI",
        "CLAUDE_CODE",
        "AGENT_MODE",
        "AI_AGENT",
        "CODING_AGENT",
        "CURSOR_AGENT",
        "AGENT_EXECUTION",
    ];

    if AGENT_ENV_VARS
        .iter()
        .any(|var| std::env::var_os(var).is_some())
    {
        return true;
    }

    #[cfg(windows)]
    {
        if let Some(false) = breakaway_is_permitted() {
            return true;
        }
    }

    false
}

#[cfg(windows)]
pub fn spawn_tier1_breakaway(executable: &Path, event: &str) -> std::io::Result<u32> {
    use std::os::windows::process::CommandExt;

    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    if let Some(false) = breakaway_is_permitted() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "job object explicitly forbids breakaway",
        ));
    }

    let mut command = Command::new(executable);
    command
        .arg("__play")
        .arg(event)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(
            CREATE_BREAKAWAY_FROM_JOB
                | DETACHED_PROCESS
                | CREATE_NEW_PROCESS_GROUP
                | CREATE_NO_WINDOW,
        );

    command.spawn().map(|child| child.id())
}

#[cfg(not(windows))]
pub fn spawn_tier1_breakaway(executable: &Path, event: &str) -> std::io::Result<u32> {
    use std::os::unix::process::CommandExt;

    let mut command = Command::new(executable);
    command
        .arg("__play")
        .arg(event)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command.process_group(0);
    command.spawn().map(|child| child.id())
}

#[cfg(windows)]
pub fn spawn_tier2_outside(executable: &Path, event: &str) -> std::io::Result<u32> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const DETACHED_PROCESS: u32 = 0x0000_0008;

    let exe_path = executable.display().to_string().replace('\'', "''");
    // PowerShell's creation flags do not apply to the separate process WMI creates.
    let script = format!(
        "$startup = New-CimInstance -ClassName Win32_ProcessStartup -ClientOnly -Property @{{CreateFlags = [uint32]{DETACHED_PROCESS}; ShowWindow = [uint16]0}} -ErrorAction Stop; \
         $res = Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{{CommandLine = '\"{}\" __play {}'; ProcessStartupInformation = $startup}}; if ($res.ReturnValue -eq 0) {{ $res.ProcessId }} else {{ exit 1 }}",
        exe_path, event
    );

    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .stdin(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(pid) = stdout
            .split_whitespace()
            .filter_map(|s| s.parse::<u32>().ok())
            .next()
        {
            return Ok(pid);
        }
    }
    Err(std::io::Error::other(format!(
        "WMI process creation failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    )))
}

#[cfg(not(windows))]
pub fn spawn_tier2_outside(executable: &Path, event: &str) -> std::io::Result<u32> {
    let output = Command::new("systemd-run")
        .args([
            "--user",
            "--scope",
            "--quiet",
            executable.to_str().unwrap_or("gitsama"),
            "__play",
            event,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output();
    match output {
        Ok(out) if out.status.success() => Ok(0),
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "transient user scope not available",
        )),
    }
}

#[cfg(windows)]
pub fn spawn_detached(command: &mut Command) -> std::io::Result<u32> {
    use std::os::windows::process::CommandExt;

    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    // Break away from any enclosing job object so background workers (audio playback,
    // pending branch dedup) survive job closure in IDEs, CI runners, and agent sandboxes.
    command.creation_flags(
        CREATE_BREAKAWAY_FROM_JOB | DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW,
    );

    match command.spawn() {
        Ok(child) => Ok(child.id()),
        Err(_) => {
            // Fallback: If the job explicitly forbids breakaway, retry without CREATE_BREAKAWAY_FROM_JOB
            // while still detaching from the parent console and process group.
            command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
            command.spawn().map(|child| child.id())
        }
    }
}

#[cfg(not(windows))]
pub fn spawn_detached(command: &mut Command) -> std::io::Result<u32> {
    use std::os::unix::process::CommandExt;

    // Decouple process group so child does not receive SIGHUP or process group signals
    // when parent Git runner completes.
    command.process_group(0);
    command.spawn().map(|child| child.id())
}

#[cfg(windows)]
pub fn process_exists(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_INVALID_PARAMETER, GetLastError};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    if pid == 0 {
        return false;
    }

    const STILL_ACTIVE: u32 = 259;

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            let error = GetLastError();
            return error != ERROR_INVALID_PARAMETER;
        }
        let mut exit_code: u32 = 0;
        let success = GetExitCodeProcess(handle, &mut exit_code);
        CloseHandle(handle);
        success != 0 && exit_code == STILL_ACTIVE
    }
}

#[cfg(not(windows))]
pub fn process_exists(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    let status = std::process::Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    match status {
        Ok(s) => s.success(),
        Err(_) => std::path::Path::new(&format!("/proc/{pid}")).exists(),
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

    #[test]
    fn spawn_detached_starts_process_cleanly() {
        let mut command = if cfg!(windows) {
            let mut cmd = std::process::Command::new("cmd.exe");
            cmd.args(["/C", "exit 0"]);
            cmd
        } else {
            std::process::Command::new("true")
        };
        command.stdin(std::process::Stdio::null());
        command.stdout(std::process::Stdio::null());
        command.stderr(std::process::Stdio::null());
        assert!(super::spawn_detached(&mut command).is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn tier2_worker_starts_without_a_console() {
        use std::{
            fs, thread,
            time::{Duration, Instant},
        };

        let directory = tempfile::tempdir().expect("worker directory");
        // Exercise the launcher's quoting as well as its window settings.
        let executable = directory.path().join("Git Sama O'Reilly.exe");
        fs::copy(
            std::env::current_exe().expect("test executable"),
            &executable,
        )
        .expect("copy console probe");
        let pid = super::spawn_tier2_outside(&executable, "--ignored")
            .expect("launch console probe through WMI");
        let deadline = Instant::now() + Duration::from_secs(10);
        while super::process_exists(pid) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(25));
        }
        assert!(!super::process_exists(pid), "console probe did not finish");
        assert_eq!(
            fs::read_to_string(executable.with_extension("console")).expect("console probe result"),
            "false",
            "the WMI worker must not receive a console window"
        );
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "subprocess probe invoked by tier2_worker_starts_without_a_console"]
    fn __play_console_probe() {
        let executable = std::env::current_exe().expect("probe executable");
        // SAFETY: GetConsoleWindow has no preconditions and returns a borrowed handle.
        let has_console =
            unsafe { !windows_sys::Win32::System::Console::GetConsoleWindow().is_null() };
        std::fs::write(
            executable.with_extension("console"),
            has_console.to_string(),
        )
        .expect("write console probe result");
    }

    #[test]
    fn process_exists_checks_current_and_invalid_pid() {
        assert!(super::process_exists(std::process::id()));
        assert!(!super::process_exists(999_999_999));
    }
}
