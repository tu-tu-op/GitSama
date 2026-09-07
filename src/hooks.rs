use std::{
    env,
    io::Read,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::Duration,
};

use crate::{
    audio::{self, DispatchDetails},
    config::Config,
    error::{Error, Result},
    events::EventKind,
    git, logging,
    paths::AppPaths,
    platform, push,
    state::{self, PendingBranch},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HookSpec {
    pub friendly_name: &'static str,
    pub native_event: &'static str,
}

pub const HOOKS: [HookSpec; 6] = [
    HookSpec {
        friendly_name: "gitsama-post-commit",
        native_event: "post-commit",
    },
    HookSpec {
        friendly_name: "gitsama-pre-push",
        native_event: "pre-push",
    },
    HookSpec {
        friendly_name: "gitsama-post-merge",
        native_event: "post-merge",
    },
    HookSpec {
        friendly_name: "gitsama-post-checkout",
        native_event: "post-checkout",
    },
    HookSpec {
        friendly_name: "gitsama-reference-transaction",
        native_event: "reference-transaction",
    },
    HookSpec {
        friendly_name: "gitsama-post-rewrite",
        native_event: "post-rewrite",
    },
];

pub fn install_global(binary: &Path) -> Result<()> {
    let _ = git::require_supported()?;
    let binary = binary.canonicalize().map_err(|error| {
        Error::message(format!("could not resolve {}: {error}", binary.display()))
    })?;

    for hook in HOOKS {
        let name = format!("hook.{}", hook.friendly_name);
        unset_global(&format!("{name}.event"))?;
        unset_global(&format!("{name}.command"))?;
        unset_global(&format!("{name}.enabled"))?;
        set_global(&format!("{name}.event"), hook.native_event, true)?;
        set_global(
            &format!("{name}.command"),
            &format!(
                "{} hook {}",
                platform::shell_quote(&binary),
                hook.native_event
            ),
            false,
        )?;
        set_global(&format!("{name}.enabled"), "true", false)?;
    }
    Ok(())
}

pub fn remove_global() -> Result<()> {
    for hook in HOOKS {
        let name = format!("hook.{}", hook.friendly_name);
        for suffix in ["event", "command", "enabled"] {
            unset_global(&format!("{name}.{suffix}"))?;
        }
    }
    Ok(())
}

pub fn global_is_registered(hook: HookSpec) -> bool {
    let name = format!("hook.{}", hook.friendly_name);
    let events = git::config_values("--global", &format!("{name}.event")).unwrap_or_default();
    let command = git::config_value("--global", &format!("{name}.command"))
        .ok()
        .flatten();
    events.iter().any(|event| event == hook.native_event) && command.is_some()
}

pub fn global_is_enabled(hook: HookSpec) -> bool {
    let key = format!("hook.{}.enabled", hook.friendly_name);
    git::config_value("--global", &key)
        .ok()
        .flatten()
        .is_none_or(|value| !value.eq_ignore_ascii_case("false"))
}

pub fn local_is_disabled(hook: HookSpec) -> bool {
    let key = format!("hook.{}.enabled", hook.friendly_name);
    git::config_value("--local", &key)
        .ok()
        .flatten()
        .is_some_and(|value| value.eq_ignore_ascii_case("false"))
}

pub fn disable_here() -> Result<()> {
    let _ = git::repository_root()?;
    for hook in HOOKS {
        set_local(
            &format!("hook.{}.enabled", hook.friendly_name),
            "false",
            false,
        )?;
    }
    Ok(())
}

pub fn enable_here() -> Result<()> {
    let _ = git::repository_root()?;
    for hook in HOOKS {
        unset_local(&format!("hook.{}.enabled", hook.friendly_name))?;
    }
    Ok(())
}

pub fn list_native_hook(native_event: &str) -> Result<String> {
    let output = git::run(["hook", "list", "--show-scope", native_event])?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(Error::Git(output.stderr.trim().to_owned()))
    }
}

const BRANCH_DEDUP_MAX_AGE_MS: u64 = 1_500;
const BRANCH_DEDUP_DELAY_MS: u64 = 180;
const BRANCH_RECENT_AGE_MS: u64 = 500;

pub fn run_fail_open(native_event: &str, args: &[String]) -> Result<()> {
    let paths = AppPaths::discover().ok();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_inner(native_event, args)
    }));
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            if let Some(paths) = paths {
                logging::write(&paths, &format!("hook {native_event} failed: {error}"));
            }
        }
        Err(_) => {
            if let Some(paths) = paths {
                logging::write(&paths, &format!("hook {native_event} panicked"));
            }
        }
    }
    Ok(())
}

pub fn run_pending_fail_open(repository: &str, branch: &str) -> Result<()> {
    let paths = AppPaths::discover().ok();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_pending(repository, branch)
    }));
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            if let Some(paths) = paths {
                logging::write(&paths, &format!("pending branch dispatch failed: {error}"));
            }
        }
        Err(_) => {
            if let Some(paths) = paths {
                logging::write(&paths, "pending branch dispatch panicked");
            }
        }
    }
    Ok(())
}

fn run_inner(native_event: &str, args: &[String]) -> Result<()> {
    let paths = AppPaths::discover()?;
    let config = Config::load_or_default(&paths);
    match native_event {
        "post-commit" => dispatch(
            &paths,
            &config,
            EventKind::Commit,
            DispatchDetails::default(),
        ),
        "post-merge" => dispatch(
            &paths,
            &config,
            EventKind::Merge,
            DispatchDetails::default(),
        ),
        "pre-push" => handle_push(&paths, &config, args),
        "post-checkout" => handle_checkout(&paths, &config, args),
        "reference-transaction" => handle_transaction(&paths, &config, args),
        "post-rewrite" => {
            if args.first().is_some_and(|value| value == "rebase") {
                dispatch(
                    &paths,
                    &config,
                    EventKind::Rebase,
                    DispatchDetails::default(),
                )?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn dispatch(
    paths: &AppPaths,
    config: &Config,
    event: EventKind,
    details: DispatchDetails,
) -> Result<()> {
    match audio::dispatch(paths, config, event, details) {
        Ok(()) => Ok(()),
        Err(error) => {
            audio::log_playback_error(paths, &error);
            Err(error)
        }
    }
}

fn handle_push(paths: &AppPaths, config: &Config, args: &[String]) -> Result<()> {
    let mut input = String::new();
    if let Err(error) = std::io::stdin().read_to_string(&mut input) {
        logging::write(paths, &format!("pre-push input unavailable: {error}"));
        return dispatch(paths, config, EventKind::Push, DispatchDetails::default());
    }
    let refs = match push::parse_stdin(&input) {
        Ok(refs) => refs,
        Err(error) => {
            logging::write(paths, &format!("pre-push input invalid: {error}"));
            return dispatch(paths, config, EventKind::Push, DispatchDetails::default());
        }
    };
    let remote = args.first().map(String::as_str).unwrap_or("origin");
    let classification = match push::classify(&refs, remote, config.massive_push_threshold) {
        Ok(classification) => classification,
        Err(error) => {
            logging::write(paths, &format!("push count failed: {error}"));
            push::PushClassification {
                event: EventKind::Push,
                commit_count: 0,
            }
        }
    };
    dispatch(
        paths,
        config,
        classification.event,
        DispatchDetails {
            branch: None,
            commit_count: Some(classification.commit_count),
        },
    )
}

fn handle_checkout(paths: &AppPaths, config: &Config, args: &[String]) -> Result<()> {
    if args.len() < 3 || args[2] != "1" || git::is_zero_oid(&args[0]) || git::is_zero_oid(&args[1])
    {
        return Ok(());
    }
    let branch = match git::current_branch() {
        Ok(branch) if !branch.is_empty() => branch,
        _ => return Ok(()),
    };
    let repository = git::repository_identity()?;
    if config.is_event_enabled(EventKind::BranchCreate) {
        if let Some(pending) =
            state::take_pending(paths, &repository, &branch, BRANCH_DEDUP_MAX_AGE_MS)?
        {
            return dispatch(
                paths,
                config,
                EventKind::BranchCreate,
                DispatchDetails {
                    branch: Some(pending.branch),
                    commit_count: None,
                },
            );
        }
        if state::take_recent(paths, &repository, &branch)? {
            return Ok(());
        }
    }
    dispatch(
        paths,
        config,
        EventKind::BranchSwitch,
        DispatchDetails {
            branch: Some(branch),
            commit_count: None,
        },
    )
}

fn handle_transaction(paths: &AppPaths, config: &Config, args: &[String]) -> Result<()> {
    if args.first().map(String::as_str) != Some("committed") {
        return Ok(());
    }
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .map_err(|error| Error::Git(format!("could not read reference transaction: {error}")))?;
    let repository = git::repository_identity()?;
    for line in input.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() != 3 || !parts[0].starts_with("refs/heads/") {
            continue;
        }
        let branch = parts[0].trim_start_matches("refs/heads/");
        let old_zero = git::is_zero_oid(parts[1]);
        let new_zero = git::is_zero_oid(parts[2]);
        if old_zero && !new_zero && config.is_event_enabled(EventKind::BranchCreate) {
            let pending = PendingBranch {
                repository: repository.clone(),
                branch: branch.to_owned(),
                object_id: parts[2].to_owned(),
                created_at_ms: logging::now_millis(),
            };
            state::write_pending(paths, &pending)?;
            if let Err(error) = spawn_pending(&repository, branch) {
                logging::write(paths, &format!("pending branch process failed: {error}"));
                let _ = state::take_pending(paths, &repository, branch, BRANCH_DEDUP_MAX_AGE_MS);
                dispatch(
                    paths,
                    config,
                    EventKind::BranchCreate,
                    DispatchDetails {
                        branch: Some(branch.to_owned()),
                        commit_count: None,
                    },
                )?;
            }
        } else if !old_zero && new_zero {
            dispatch(
                paths,
                config,
                EventKind::BranchDelete,
                DispatchDetails {
                    branch: Some(branch.to_owned()),
                    commit_count: None,
                },
            )?;
        }
    }
    Ok(())
}

fn spawn_pending(repository: &str, branch: &str) -> Result<()> {
    let executable = env::current_exe()
        .map_err(|error| Error::Audio(format!("could not locate GitSama: {error}")))?;
    Command::new(executable)
        .arg("__dispatch-pending")
        .arg(repository)
        .arg(branch)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|error| Error::Audio(format!("could not start pending dispatch: {error}")))
}

fn run_pending(repository: &str, branch: &str) -> Result<()> {
    thread::sleep(Duration::from_millis(BRANCH_DEDUP_DELAY_MS));
    let paths = AppPaths::discover()?;
    let config = Config::load_or_default(&paths);
    let Some(pending) = state::take_pending(&paths, repository, branch, BRANCH_DEDUP_MAX_AGE_MS)?
    else {
        return Ok(());
    };
    state::mark_recent(&paths, repository, &pending.branch, BRANCH_RECENT_AGE_MS)?;
    dispatch(
        &paths,
        &config,
        EventKind::BranchCreate,
        DispatchDetails {
            branch: Some(pending.branch.clone()),
            commit_count: None,
        },
    )
}

fn set_global(key: &str, value: &str, append: bool) -> Result<()> {
    set_scope("--global", key, value, append)
}

fn set_local(key: &str, value: &str, append: bool) -> Result<()> {
    set_scope("--local", key, value, append)
}

fn set_scope(scope: &str, key: &str, value: &str, append: bool) -> Result<()> {
    let mut args = vec!["config".to_owned(), scope.to_owned()];
    if append {
        args.push("--add".to_owned());
    }
    args.push(key.to_owned());
    args.push(value.to_owned());
    let output = git::run(args)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(Error::Git(if output.stderr.trim().is_empty() {
            format!("could not set Git config {key}")
        } else {
            output.stderr.trim().to_owned()
        }))
    }
}

fn unset_global(key: &str) -> Result<()> {
    unset_scope("--global", key)
}

fn unset_local(key: &str) -> Result<()> {
    unset_scope("--local", key)
}

fn unset_scope(scope: &str, key: &str) -> Result<()> {
    let output = git::run(["config", scope, "--unset-all", key])?;
    if output.status.success()
        || output
            .stderr
            .to_ascii_lowercase()
            .contains("key does not contain a section")
        || output.stderr.to_ascii_lowercase().contains("no such key")
    {
        Ok(())
    } else {
        // Git uses exit status 5 for an absent key. It is safe to treat that
        // specific case as an idempotent success while preserving other errors.
        let detail = output.stderr.trim().to_ascii_lowercase();
        if output.status.code() == Some(5)
            || detail.contains("not found")
            || detail.contains("no such")
        {
            Ok(())
        } else {
            Err(Error::Git(output.stderr.trim().to_owned()))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::HOOKS;
    use crate::platform::shell_quote;

    #[test]
    fn has_one_named_hook_for_each_native_event() {
        assert_eq!(HOOKS.len(), 6);
        assert!(
            HOOKS
                .iter()
                .all(|hook| hook.friendly_name.starts_with("gitsama-"))
        );
    }

    #[test]
    fn command_is_safe_for_spaced_paths() {
        let path = shell_quote(Path::new(if cfg!(windows) {
            r"C:\Users\Name\Git Sama\gitsama.exe"
        } else {
            "/Users/name/Git Sama/gitsama"
        }));
        let command = format!("{path} hook pre-push");
        assert!(command.contains("hook pre-push"));
        assert!(command.contains("Git Sama"));
    }
}
