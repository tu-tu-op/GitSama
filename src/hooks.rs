use std::path::Path;

use crate::{
    error::{Error, Result},
    events::EventKind,
    git,
    platform,
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
    let binary = binary
        .canonicalize()
        .map_err(|error| Error::message(format!("could not resolve {}: {error}", binary.display())))?;

    for hook in HOOKS {
        let name = format!("hook.{}", hook.friendly_name);
        unset_global(&format!("{name}.event"))?;
        unset_global(&format!("{name}.command"))?;
        unset_global(&format!("{name}.enabled"))?;
        unset_global(&format!("{name}.parallel"))?;
        set_global(
            &format!("{name}.event"),
            hook.native_event,
            true,
        )?;
        set_global(
            &format!("{name}.command"),
            &format!("{} hook {}", platform::shell_quote(&binary), hook.native_event),
            false,
        )?;
        set_global(&format!("{name}.enabled"), "true", false)?;
        set_global(&format!("{name}.parallel"), "true", false)?;
    }
    Ok(())
}

pub fn remove_global() -> Result<()> {
    for hook in HOOKS {
        let name = format!("hook.{}", hook.friendly_name);
        for suffix in ["event", "command", "enabled", "parallel"] {
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
        .map_or(true, |value| value != "false")
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

pub fn spec_for_native(native_event: &str) -> Option<HookSpec> {
    HOOKS
        .iter()
        .copied()
        .find(|hook| hook.native_event == native_event)
}

pub fn native_for_event(event: EventKind) -> &'static str {
    match event {
        EventKind::Commit => "post-commit",
        EventKind::Push | EventKind::MassivePush => "pre-push",
        EventKind::Merge => "post-merge",
        EventKind::BranchSwitch => "post-checkout",
        EventKind::BranchCreate | EventKind::BranchDelete => "reference-transaction",
        EventKind::Rebase => "post-rewrite",
    }
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

    use super::{HOOKS, native_for_event};
    use crate::{events::EventKind, platform::shell_quote};

    #[test]
    fn has_one_named_hook_for_each_native_event() {
        assert_eq!(HOOKS.len(), 6);
        assert!(HOOKS.iter().all(|hook| hook.friendly_name.starts_with("gitsama-")));
    }

    #[test]
    fn event_mapping_is_explicit() {
        assert_eq!(native_for_event(EventKind::Commit), "post-commit");
        assert_eq!(native_for_event(EventKind::MassivePush), "pre-push");
        assert_eq!(native_for_event(EventKind::BranchCreate), "reference-transaction");
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

