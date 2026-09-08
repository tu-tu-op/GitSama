use std::{
    env,
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};

use crate::{
    audio,
    config::Config,
    error::{Error, Result},
    events::EventKind,
    git, hooks, packs,
    paths::AppPaths,
    platform,
};

#[derive(Debug, Parser)]
#[command(
    name = "gitsama",
    version,
    about = "Anime reactions for your Git workflow",
    long_about = "GitSama reacts to meaningful local Git events with sounds from your selected pack."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Setup,
    Status,
    Settings,
    Packs,
    Pack {
        #[command(subcommand)]
        command: PackCommand,
    },
    Use {
        id: String,
    },
    Test {
        event: Option<String>,
    },
    Volume {
        value: u8,
    },
    Threshold {
        value: u32,
    },
    Enable {
        event: String,
    },
    Disable {
        event: String,
    },
    Mute,
    Unmute,
    OffHere,
    OnHere,
    Doctor {
        #[arg(long)]
        fix: bool,
    },
    Uninstall {
        #[arg(long)]
        keep_data: bool,
    },
    #[command(hide = true)]
    Hook {
        event: String,
        #[arg(num_args = 0.., trailing_var_arg = true)]
        args: Vec<String>,
    },
    #[command(name = "__play", hide = true)]
    Play {
        event: String,
    },
    #[command(name = "__dispatch-pending", hide = true)]
    DispatchPending {
        repository: String,
        branch: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum PackCommand {
    List,
    Add {
        path: String,
    },
    Remove {
        id: String,
    },
    Validate {
        path: String,
    },
    Scaffold {
        name: String,
        directory: Option<String>,
    },
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        None => dashboard(),
        Some(Command::Setup) => setup(),
        Some(Command::Status) => status(),
        Some(Command::Settings) => settings(),
        Some(Command::Packs) => packs_picker(),
        Some(Command::Pack { command }) => pack_command(command),
        Some(Command::Use { id }) => use_pack(&id),
        Some(Command::Test { event }) => test_command(event.as_deref()),
        Some(Command::Volume { value }) => set_volume(value),
        Some(Command::Threshold { value }) => set_threshold(value),
        Some(Command::Enable { event }) => set_event(&event, true),
        Some(Command::Disable { event }) => set_event(&event, false),
        Some(Command::Mute) => set_enabled(false),
        Some(Command::Unmute) => set_enabled(true),
        Some(Command::OffHere) => off_here(),
        Some(Command::OnHere) => on_here(),
        Some(Command::Doctor { fix }) => crate::doctor::run(fix),
        Some(Command::Uninstall { keep_data }) => uninstall(keep_data),
        Some(Command::Hook { event, args }) => hooks::run_fail_open(&event, &args),
        Some(Command::Play { event }) => {
            let paths = AppPaths::discover()?;
            let config = Config::load_or_default(&paths);
            match audio::play(&paths, &config, EventKind::parse(&event)?) {
                Ok(()) => Ok(()),
                Err(error) => {
                    audio::log_playback_error(&paths, &error);
                    Ok(())
                }
            }
        }
        Some(Command::DispatchPending { repository, branch }) => {
            hooks::run_pending_fail_open(&repository, &branch)
        }
    }
}

fn dashboard() -> Result<()> {
    let paths = AppPaths::discover()?;
    let config = Config::load_or_default(&paths);
    let state = if !config.enabled {
        "MUTED"
    } else if hooks::HOOKS
        .iter()
        .any(|hook| hooks::local_is_disabled(*hook))
    {
        "DISABLED HERE"
    } else {
        "ON"
    };

    println!("GitSama ⚔️");
    println!("Anime reactions for your Git workflow.");
    println!();
    println!("Status       {state}");
    println!("Active Pack  {}", config.active_pack);
    println!("Volume       {}%", config.volume);
    println!("Big Push     {} commits", config.massive_push_threshold);
    println!();
    println!("Quick commands");
    println!();
    for command in [
        "gitsama setup",
        "gitsama packs",
        "gitsama use <pack>",
        "gitsama test",
        "gitsama settings",
        "gitsama doctor",
        "gitsama mute",
        "gitsama off-here",
    ] {
        println!("  {command}");
    }
    println!();
    println!("Run gitsama help for all commands.");
    Ok(())
}

fn setup() -> Result<()> {
    let paths = AppPaths::discover()?;
    paths.ensure_layout()?;
    let version = match git::version() {
        Ok(version) if version.is_supported() => version,
        Ok(version) => {
            print_old_git(&version.short());
            return Err(Error::UnsupportedGit(version.short()));
        }
        Err(error) => {
            println!("GitSama could not find a usable Git installation.");
            println!("Install Git 2.54 or newer, then run gitsama setup again.");
            return Err(error);
        }
    };

    packs::ensure_starter(&paths)?;
    packs::ensure_bundled(&paths)?;
    let mut config = Config::load_or_default(&paths);
    if packs::find(&paths, &config.active_pack).is_err() {
        config.active_pack = packs::DEFAULT_PACK_ID.to_owned();
    }

    println!("GitSama setup");
    println!();
    println!("✓ Git {}", version.short());
    println!("✓ Git 2.54 named hook system");
    println!("✓ Built-in packs ready");
    if audio::test_mode() {
        println!("✓ Audio test mode enabled");
    } else {
        match audio::probe_output() {
            Ok(()) => println!("✓ Audio output ready"),
            Err(error) => println!("! Audio output unavailable: {error}"),
        }
    }

    if interactive() {
        println!();
        println!("Choose your first pack:");
        for pack in packs::installed(&paths)? {
            println!("  {} ({})", pack.manifest.name, pack.manifest.id);
        }
        if let Some(pack_id) = prompt_optional("Pack id (Enter keeps current): ")? {
            let pack = packs::find(&paths, &pack_id)?;
            config.active_pack = pack.manifest.id;
        }

        if let Some(value) = prompt_optional("Volume 0-100 (Enter keeps current): ")? {
            config.volume = parse_volume(&value)?;
        }
        if let Some(value) = prompt_optional("Massive push threshold (Enter keeps current): ")? {
            config.massive_push_threshold = parse_threshold(&value)?;
        }
        println!();
        println!("Choose events:");
        for event in EventKind::ALL {
            let enabled = prompt_yes_no(
                &format!("Enable {}? [Y/n] ", event.label()),
                config.is_event_enabled(event),
            )?;
            config.set_event_enabled(event, enabled);
        }
    }

    config.save(&paths)?;

    let executable = env::current_exe()
        .map_err(|error| Error::message(format!("could not find the GitSama binary: {error}")))?;
    hooks::install_global(&executable)?;

    println!("✓ GitSama hooks registered");
    println!();
    println!("Active pack: {}", config.active_pack);
    println!("Volume: {}%", config.volume);
    println!("Massive push threshold: {}", config.massive_push_threshold);

    if interactive() && prompt_yes_no("Test a sound now? [Y/n] ", true)? {
        test_one(&paths, &config, EventKind::Commit)?;
    }

    println!();
    println!("✓ GitSama is ready. Just use Git normally.");
    Ok(())
}

fn print_old_git(version: &str) {
    println!("GitSama needs Git 2.54 or newer.");
    println!();
    println!("Detected:");
    println!("  Git {version}");
    println!();
    println!("Upgrade Git and run:");
    println!();
    println!("  gitsama setup");
}

fn status() -> Result<()> {
    let paths = AppPaths::discover()?;
    let config = Config::load_or_default(&paths);
    let repository = git::repository_root().ok();
    let local_disabled = repository.is_some()
        && hooks::HOOKS
            .iter()
            .any(|hook| hooks::local_is_disabled(*hook));
    let global_hooks = hooks::HOOKS
        .iter()
        .filter(|hook| hooks::global_is_registered(**hook))
        .count();
    let globally_disabled = hooks::HOOKS
        .iter()
        .any(|hook| !hooks::global_is_enabled(*hook));

    let state = if local_disabled {
        "disabled in this repository"
    } else if !config.enabled || globally_disabled {
        "muted"
    } else if global_hooks != hooks::HOOKS.len() {
        "not set up"
    } else {
        "globally enabled"
    };

    println!("GitSama status");
    println!();
    println!("State        {state}");
    println!("Active pack  {}", config.active_pack);
    println!("Volume       {}%", config.volume);
    println!("Big Push     {} commits", config.massive_push_threshold);
    println!(
        "Hooks        {global_hooks}/{} registered",
        hooks::HOOKS.len()
    );
    if let Some(repository) = repository {
        println!("Repository   {}", repository.display());
    }
    Ok(())
}

fn settings() -> Result<()> {
    let paths = AppPaths::discover()?;
    let mut config = Config::load_or_default(&paths);
    println!("GitSama Settings");
    println!();
    print_config(&config);

    if interactive() {
        println!();
        if let Some(pack_id) = prompt_optional("Active pack (Enter keeps current): ")? {
            let pack = packs::find(&paths, &pack_id)?;
            config.active_pack = pack.manifest.id;
        }
        if let Some(value) = prompt_optional("New volume (Enter keeps current): ")? {
            config.volume = parse_volume(&value)?;
        }
        if let Some(value) = prompt_optional("New massive push threshold (Enter keeps current): ")?
        {
            config.massive_push_threshold = parse_threshold(&value)?;
        }
        for event in EventKind::ALL {
            let enabled = prompt_yes_no(
                &format!("Enable {}? [Y/n] ", event.label()),
                config.is_event_enabled(event),
            )?;
            config.set_event_enabled(event, enabled);
        }
        config.save(&paths)?;
        println!("Settings saved.");
    }
    Ok(())
}

fn print_config(config: &Config) {
    println!("Active Pack          {}", config.active_pack);
    println!("Volume               {}%", config.volume);
    println!(
        "Massive Push         {} commits",
        config.massive_push_threshold
    );
    println!();
    println!("Events");
    for event in EventKind::ALL {
        println!(
            "  {:<18} {}",
            event.label(),
            if config.is_event_enabled(event) {
                "✓"
            } else {
                "—"
            }
        );
    }
}

fn packs_picker() -> Result<()> {
    let paths = AppPaths::discover()?;
    let config = Config::load_or_default(&paths);
    let installed = packs::installed(&paths)?;
    println!("GitSama Packs");
    println!();
    for pack in &installed {
        let marker = if pack.manifest.id == config.active_pack {
            "✓"
        } else {
            " "
        };
        println!("  {marker} {} ({})", pack.manifest.name, pack.manifest.id);
    }
    if interactive() {
        if let Some(id) = prompt_optional("Choose a pack id (or q to go back): ")? {
            if !id.eq_ignore_ascii_case("q") {
                use_pack(&id)?;
            }
        }
    }
    Ok(())
}

fn pack_command(command: PackCommand) -> Result<()> {
    let paths = AppPaths::discover()?;
    match command {
        PackCommand::List => {
            for pack in packs::installed(&paths)? {
                let active = if Config::load_or_default(&paths).active_pack == pack.manifest.id {
                    " *"
                } else {
                    ""
                };
                println!("{}{} - {}", pack.manifest.id, active, pack.manifest.name);
            }
            Ok(())
        }
        PackCommand::Add { path } => {
            let summary = packs::install(&paths, Path::new(&path))?;
            println!("Added {} ({})", summary.name, summary.id);
            Ok(())
        }
        PackCommand::Remove { id } => {
            packs::remove(&paths, &id)?;
            println!("Removed pack {id}");
            Ok(())
        }
        PackCommand::Validate { path } => {
            let summary = packs::validate(Path::new(&path))?;
            println!("✓ {} ({}) is valid", summary.name, summary.id);
            Ok(())
        }
        PackCommand::Scaffold { name, directory } => {
            let parent = directory
                .map(PathBuf::from)
                .unwrap_or(env::current_dir().map_err(|error| Error::message(error.to_string()))?);
            let root = packs::scaffold(&name, parent)?;
            println!("Created {}", root.display());
            Ok(())
        }
    }
}

fn use_pack(id: &str) -> Result<()> {
    let paths = AppPaths::discover()?;
    packs::ensure_starter(&paths)?;
    packs::ensure_bundled(&paths)?;
    let pack = packs::find(&paths, id)?;
    let mut config = Config::load_or_default(&paths);
    config.active_pack = pack.manifest.id.clone();
    config.save(&paths)?;
    println!("✓ Using {} ({})", pack.manifest.name, pack.manifest.id);
    Ok(())
}

fn test_command(value: Option<&str>) -> Result<()> {
    let paths = AppPaths::discover()?;
    paths.ensure_layout()?;
    packs::ensure_starter(&paths)?;
    packs::ensure_bundled(&paths)?;
    let config = Config::load_or_default(&paths);
    let value = match value {
        Some(value) => value.to_owned(),
        None if interactive() => {
            println!("Choose a sound to test:");
            for (index, event) in EventKind::ALL.iter().enumerate() {
                println!("  {}. {}", index + 1, event.label());
            }
            println!("  9. Play All");
            let answer = prompt_line("Choice [1-9]: ")?;
            if answer.eq_ignore_ascii_case("q") {
                return Ok(());
            }
            if answer.trim() == "9" {
                "all".to_owned()
            } else {
                answer.trim().to_owned()
            }
        }
        None => "all".to_owned(),
    };

    if value.eq_ignore_ascii_case("all") {
        for event in EventKind::ALL {
            test_one(&paths, &config, event)?;
        }
        return Ok(());
    }

    let event = match value.parse::<usize>() {
        Ok(index) if (1..=EventKind::ALL.len()).contains(&index) => EventKind::ALL[index - 1],
        _ => EventKind::parse(&value)?,
    };
    test_one(&paths, &config, event)
}

fn test_one(paths: &AppPaths, config: &Config, event: EventKind) -> Result<()> {
    println!("  Playing {}", event.label());
    if audio::test_mode() {
        audio::dispatch(paths, config, event, audio::DispatchDetails::default())
    } else {
        audio::play(paths, config, event)
    }
}

fn set_volume(value: u8) -> Result<()> {
    if value > 100 {
        return Err(Error::message("volume must be between 0 and 100"));
    }
    let paths = AppPaths::discover()?;
    let mut config = Config::load_or_default(&paths);
    config.volume = value;
    config.save(&paths)?;
    println!("✓ Volume set to {value}%");
    Ok(())
}

fn set_threshold(value: u32) -> Result<()> {
    let value = parse_threshold(&value.to_string())?;
    let paths = AppPaths::discover()?;
    let mut config = Config::load_or_default(&paths);
    config.massive_push_threshold = value;
    config.save(&paths)?;
    println!("✓ Massive push threshold set to {value} commits");
    Ok(())
}

fn parse_volume(value: &str) -> Result<u8> {
    let value = value
        .trim()
        .parse::<u8>()
        .map_err(|_| Error::message("volume must be a number from 0 to 100"))?;
    if value > 100 {
        return Err(Error::message("volume must be between 0 and 100"));
    }
    Ok(value)
}

fn parse_threshold(value: &str) -> Result<u32> {
    let value = value
        .trim()
        .parse::<u32>()
        .map_err(|_| Error::message("massive push threshold must be a positive number"))?;
    if value == 0 {
        return Err(Error::message("massive push threshold must be at least 1"));
    }
    Ok(value)
}

fn set_event(value: &str, enabled: bool) -> Result<()> {
    let event = EventKind::parse(value)?;
    let paths = AppPaths::discover()?;
    let mut config = Config::load_or_default(&paths);
    config.set_event_enabled(event, enabled);
    config.save(&paths)?;
    println!(
        "✓ {} {}",
        event.label(),
        if enabled { "enabled" } else { "disabled" }
    );
    Ok(())
}

fn set_enabled(enabled: bool) -> Result<()> {
    let paths = AppPaths::discover()?;
    let mut config = Config::load_or_default(&paths);
    config.enabled = enabled;
    config.save(&paths)?;
    println!("✓ GitSama {}", if enabled { "unmuted" } else { "muted" });
    Ok(())
}

fn off_here() -> Result<()> {
    let root = git::repository_root()
        .map_err(|_| Error::message("run gitsama off-here inside a Git repository"))?;
    hooks::disable_here()?;
    println!("✓ GitSama disabled for:");
    println!();
    println!("  {}", root.display());
    println!();
    println!("Other repositories are unaffected.");
    Ok(())
}

fn on_here() -> Result<()> {
    let root = git::repository_root()
        .map_err(|_| Error::message("run gitsama on-here inside a Git repository"))?;
    hooks::enable_here()?;
    println!("✓ GitSama enabled again for:");
    println!();
    println!("  {}", root.display());
    Ok(())
}

fn uninstall(keep_data: bool) -> Result<()> {
    let paths = AppPaths::discover()?;
    println!("GitSama uninstall");
    println!();
    println!("This removes GitSama's named global hooks and:");
    if keep_data {
        println!("  the GitSama executable and user PATH entry");
        println!("  (packs and configuration are preserved)");
    } else {
        println!("  {}", paths.root.display());
    }
    println!();
    if interactive() && !prompt_yes_no("Continue? [y/N] ", false)? {
        println!("Nothing removed.");
        return Ok(());
    }
    hooks::remove_global()?;
    let _ = platform::remove_user_path_entry(&paths.bin);
    let executable = env::current_exe().ok();
    let managed_executable = platform::installed_binary(&paths);
    let root_is_executable = executable.as_ref().is_some_and(|executable| {
        let root = paths
            .root
            .canonicalize()
            .unwrap_or_else(|_| paths.root.clone());
        let executable = executable
            .canonicalize()
            .unwrap_or_else(|_| executable.clone());
        executable.starts_with(root)
    });
    if let Some(executable) = executable.filter(|_| root_is_executable) {
        platform::schedule_cleanup(&executable, (!keep_data).then_some(paths.root.as_path()))?;
    } else {
        if managed_executable.exists() {
            std::fs::remove_file(&managed_executable).map_err(|source| Error::WriteFile {
                path: managed_executable.clone(),
                source,
            })?;
        }
        if !keep_data && paths.root.exists() {
            std::fs::remove_dir_all(&paths.root).map_err(|source| Error::WriteFile {
                path: paths.root.clone(),
                source,
            })?;
        }
    }
    println!("✓ GitSama hooks removed.");
    if keep_data {
        println!("✓ Packs and configuration preserved.");
    } else {
        println!("✓ GitSama home removal scheduled.");
    }
    println!("GitSama's named hooks were removed before cleanup.");
    Ok(())
}

fn interactive() -> bool {
    io::stdin().is_terminal()
        && io::stdout().is_terminal()
        && env::var("GITSAMA_NONINTERACTIVE").is_err()
}

fn prompt_line(prompt: &str) -> Result<String> {
    print!("{prompt}");
    io::stdout()
        .flush()
        .map_err(|error| Error::message(error.to_string()))?;
    let mut value = String::new();
    io::stdin()
        .read_line(&mut value)
        .map_err(|error| Error::message(error.to_string()))?;
    Ok(value.trim().to_owned())
}

fn prompt_optional(prompt: &str) -> Result<Option<String>> {
    let value = prompt_line(prompt)?;
    if value.is_empty() {
        Ok(None)
    } else {
        Ok(Some(value))
    }
}

fn prompt_yes_no(prompt: &str, default: bool) -> Result<bool> {
    let value = prompt_line(prompt)?;
    if value.is_empty() {
        return Ok(default);
    }
    Ok(matches!(value.chars().next(), Some('y' | 'Y')))
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command, parse_threshold, parse_volume};
    use clap::Parser;

    #[test]
    fn root_command_is_friendly() {
        let cli = Cli::try_parse_from(["gitsama"]).expect("parse");
        assert!(cli.command.is_none());
    }

    #[test]
    fn internal_commands_parse() {
        let cli = Cli::try_parse_from(["gitsama", "hook", "post-commit"]).expect("parse");
        assert!(matches!(cli.command, Some(Command::Hook { .. })));
        let cli = Cli::try_parse_from(["gitsama", "hook", "post-rewrite", "rebase"])
            .expect("hook arguments");
        assert!(matches!(
            cli.command,
            Some(Command::Hook { args, .. }) if args == vec!["rebase"]
        ));
    }

    #[test]
    fn numeric_settings_are_bounded() {
        assert!(parse_volume("80").is_ok());
        assert!(parse_volume("101").is_err());
        assert!(parse_threshold("1").is_ok());
        assert!(parse_threshold("0").is_err());
    }
}
