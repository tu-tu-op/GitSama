use std::{env, fs};

use crate::{audio, config::Config, error::Result, git, hooks, packs, paths::AppPaths};

pub fn run(fix: bool) -> Result<()> {
    let paths = AppPaths::discover()?;
    paths.ensure_layout()?;
    println!("GitSama Doctor");
    println!();

    let version = match git::version() {
        Ok(version) if version.is_supported() => {
            ok(&format!("Git {} found", version.short()));
            Some(version)
        }
        Ok(version) => {
            warn(&format!(
                "Git {} found; GitSama requires Git 2.54 or newer",
                version.short()
            ));
            None
        }
        Err(error) => {
            warn(&format!("Git could not be checked: {error}"));
            None
        }
    };

    let executable = env::current_exe().ok();
    if let Some(executable) = &executable {
        if executable.exists() {
            ok(&format!("GitSama binary found at {}", executable.display()));
        } else {
            warn("GitSama binary path does not exist");
        }
    } else {
        warn("GitSama binary path could not be determined");
    }

    let mut config = Config::load_or_default(&paths);
    let config_valid = match fs::read_to_string(&paths.config) {
        Ok(text) => match Config::parse(&text) {
            Ok(parsed) => {
                config = parsed;
                ok("Config readable");
                true
            }
            Err(error) => {
                warn(&format!("Config needs repair: {error}"));
                false
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            ok("Config will use defaults until setup");
            false
        }
        Err(error) => {
            warn(&format!("Config could not be read: {error}"));
            false
        }
    };

    if fix && !config_valid {
        config = Config::default();
        config.save(&paths)?;
        ok("Config repaired");
    }

    if let Err(error) = packs::ensure_starter(&paths) {
        warn(&format!("Starter pack unavailable: {error}"));
    }

    match packs::find(&paths, &config.active_pack) {
        Ok(pack) => ok(&format!("Active pack valid: {}", pack.manifest.name)),
        Err(error) => {
            warn(&format!("Active pack invalid: {error}"));
            if fix {
                config.active_pack = "starter".to_owned();
                config.save(&paths)?;
                ok("Active pack reset to Starter");
            }
        }
    }

    if audio::test_mode() {
        ok("Audio test mode enabled");
    } else {
        match audio::probe_output() {
            Ok(()) => ok("Audio output available"),
            Err(error) => warn(&format!("Audio output unavailable: {error}")),
        }
    }

    if version.is_some() {
        if fix {
            if let Some(executable) = executable {
                hooks::install_global(&executable)?;
                ok("GitSama named hooks repaired");
            }
        }
        let mut registered = 0;
        for hook in hooks::HOOKS {
            if hooks::global_is_registered(hook) {
                ok(&format!("{} registered globally", hook.native_event));
                match hooks::list_native_hook(hook.native_event) {
                    Ok(_) => ok(&format!(
                        "{} is visible to Git's hook runner",
                        hook.native_event
                    )),
                    Err(error) => warn(&format!(
                        "{} could not be listed by Git: {error}",
                        hook.native_event
                    )),
                }
                registered += 1;
            } else {
                warn(&format!("{} is not registered globally", hook.native_event));
            }
        }
        if registered == hooks::HOOKS.len() {
            ok("All GitSama named hooks are registered");
        }
    } else {
        warn("Named hook checks skipped until Git 2.54 or newer is installed");
    }

    match git::config_value("--global", "core.hooksPath") {
        Ok(Some(value)) => ok(&format!("core.hooksPath remains unchanged ({value})")),
        Ok(None) => ok("core.hooksPath remains unset"),
        Err(error) => warn(&format!("core.hooksPath could not be inspected: {error}")),
    }
    ok("Existing repository hooks remain available; GitSama does not replace .git/hooks");

    println!();
    println!("Diagnostic log: {}", paths.log.display());
    println!("No network, telemetry, or remote monitoring is used.");
    Ok(())
}

fn ok(message: &str) {
    println!("✓ {message}");
}

fn warn(message: &str) {
    println!("! {message}");
}
