use std::{
    env,
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};

use crate::{
    audio,
    config::Config,
    contribute,
    error::{Error, Result},
    events::EventKind,
    git, hooks, pack_state, packs,
    paths::AppPaths,
    platform, update,
};

#[derive(Debug, Parser)]
#[command(
    name = "gitsama",
    version,
    about = "Anime reactions for your Git workflow",
    long_about = "GitSama reacts to meaningful local Git events with sounds from your selected pack."
)]
pub struct Cli {
    #[arg(long, help = "Check for and install the latest GitSama release")]
    pub upgrade: bool,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(
        visible_alias = "update",
        about = "Check for and install GitSama updates"
    )]
    Upgrade {
        #[arg(long, conflicts_with = "yes", help = "Check without installing")]
        check: bool,
        #[arg(long, help = "Approve installation without prompting")]
        yes: bool,
    },
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
    Mode {
        value: String,
    },
    Linger {
        value: f64,
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
    #[command(
        visible_alias = "delete",
        about = "Delete any installed pack, including built-ins"
    )]
    Remove {
        id: String,
    },
    Validate {
        path: String,
    },
    #[command(about = "Submit a tested voice pack as a GitHub pull request")]
    Contribute {
        id: String,
        #[arg(long)]
        author: Option<String>,
        #[arg(long, help = "License that permits redistribution of the audio")]
        license: Option<String>,
        #[arg(long, help = "Audio sources and redistribution permission details")]
        source: Option<String>,
    },
    #[command(
        about = "Install event-named clips from the current folder, or create an empty template"
    )]
    Scaffold {
        #[arg(required_if_eq("template", "true"))]
        name: Option<String>,
        #[arg(requires = "template")]
        directory: Option<String>,
        #[arg(
            long,
            help = "Create an editable empty pack instead of installing clips"
        )]
        template: bool,
    },
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    if cli.upgrade {
        if cli.command.is_some() {
            return Err(Error::message(
                "run gitsama --upgrade on its own, or use gitsama upgrade",
            ));
        }
        return update::run(false, false);
    }
    let notify = !matches!(
        &cli.command,
        Some(
            Command::Upgrade { .. }
                | Command::Uninstall { .. }
                | Command::Hook { .. }
                | Command::Play { .. }
                | Command::DispatchPending { .. }
        )
    );
    let result = match cli.command {
        Some(Command::Upgrade { check, yes }) => update::run(check, yes),
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
        Some(Command::Mode { value }) => set_playback_mode(&value),
        Some(Command::Linger { value }) => set_linger_cap(value),
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
                Ok(_) => Ok(()),
                Err(error) => {
                    audio::log_playback_error(&paths, &error);
                    Ok(())
                }
            }
        }
        Some(Command::DispatchPending { repository, branch }) => {
            hooks::run_pending_fail_open(&repository, &branch)
        }
    };
    if result.is_ok() && notify {
        update::notify()?;
    }
    result
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
    println!("Active Pack  {}", config.active_pack_label());
    println!("Volume       {}%", config.volume);
    println!("Big Push     {} commits", config.massive_push_threshold);
    println!("Playback     {}", config.playback_mode);
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
        config.active_pack = packs::fallback_id(&paths, "")?;
        if config.active_pack.is_empty() {
            config.enabled = false;
        }
    }

    println!("GitSama setup");
    println!();
    println!("✓ Git {}", version.short());
    println!("✓ Git 2.54 named hook system");
    println!("✓ Pack library ready");
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
    println!("Active pack: {}", config.active_pack_label());
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
    println!("Active pack  {}", config.active_pack_label());
    println!("Volume       {}%", config.volume);
    println!("Big Push     {} commits", config.massive_push_threshold);
    println!("Playback     {}", config.playback_mode);
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
        if let Some(mode) =
            prompt_optional("Playback mode [auto/detached/linger] (Enter keeps current): ")?
        {
            let mode = mode.trim().to_ascii_lowercase();
            if ["auto", "detached", "linger"].contains(&mode.as_str()) {
                config.playback_mode = mode;
            }
        }
        if let Some(value) = prompt_optional("Linger cap seconds (Enter keeps current): ")? {
            if let Ok(secs) = value.trim().parse::<f64>() {
                if (0.1..=30.0).contains(&secs) {
                    config.linger_cap_secs = secs;
                }
            }
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
    println!("Active Pack          {}", config.active_pack_label());
    println!("Volume               {}%", config.volume);
    println!(
        "Massive Push         {} commits",
        config.massive_push_threshold
    );
    println!("Playback Mode        {}", config.playback_mode);
    println!("Linger Cap           {}s", config.linger_cap_secs);
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
            let was_active = Config::load_or_default(&paths).active_pack == id;
            packs::remove(&paths, &id)?;
            println!("Removed pack {id}");
            if was_active {
                let config = Config::load_or_default(&paths);
                if config.active_pack.is_empty() {
                    println!(
                        "No voice packs remain; playback muted. Install a pack with gitsama pack scaffold, or restore a built-in with gitsama use naruto."
                    );
                } else {
                    println!("✓ Using {}", config.active_pack);
                }
            }
            Ok(())
        }
        PackCommand::Validate { path } => {
            let summary = packs::validate(Path::new(&path))?;
            println!("✓ {} ({}) is valid", summary.name, summary.id);
            Ok(())
        }
        PackCommand::Contribute {
            id,
            author,
            license,
            source,
        } => {
            packs::find(&paths, &id)?;
            if let Some(url) = pack_state::load(&paths, &id)?.contribution_url {
                println!("Pull request: {url}");
                return Ok(());
            }
            let pack = contribute::checked_pack(&paths, &id)?;
            println!(
                "Contributing {} ({id}) to tu-tu-op/GitSama.",
                pack.manifest.name
            );
            let metadata = contribute::Metadata {
                author: contribution_value(
                    "author",
                    author.or_else(|| {
                        (!pack.manifest.author.eq_ignore_ascii_case("Your Name"))
                            .then_some(pack.manifest.author.clone())
                    }),
                    "Pack author: ",
                )?,
                license: contribution_value(
                    "license",
                    license.or_else(|| {
                        (!pack.manifest.license.eq_ignore_ascii_case("user-provided"))
                            .then_some(pack.manifest.license.clone())
                    }),
                    "Audio redistribution license: ",
                )?,
                source: contribution_value(
                    "source",
                    source,
                    "Audio source URL(s) and redistribution permission details: ",
                )?,
            };
            let url = contribute::submit(&paths, &id, &metadata)?;
            println!("✓ Pull request: {url}");
            Ok(())
        }
        PackCommand::Scaffold {
            name,
            directory,
            template,
        } => {
            let current = env::current_dir().map_err(|error| Error::message(error.to_string()))?;
            if template {
                let name = name
                    .as_deref()
                    .ok_or_else(|| Error::message("--template requires a pack name"))?;
                let root = packs::scaffold(name, directory.map(PathBuf::from).unwrap_or(current))?;
                println!("Created template {}", root.display());
                return Ok(());
            }
            let summary = packs::import_folder(&paths, current, name.as_deref())?;
            pack_state::save(
                &paths,
                &summary.id,
                &pack_state::PackState {
                    created_locally: true,
                    ..Default::default()
                },
            )?;
            println!("✓ Installed {} ({})", summary.name, summary.id);
            println!("Pack path: {}", paths.packs.join(&summary.id).display());
            let pack = packs::find(&paths, &summary.id)?;
            for event in EventKind::ALL {
                let count = pack.files_for(event).len();
                if count > 0 {
                    println!("  {}: {} clip(s)", event.label(), count);
                } else if event == EventKind::MassivePush
                    && !pack.files_for(EventKind::Push).is_empty()
                {
                    println!("  {}: falls back to Push", event.label());
                } else {
                    println!("  {}: no sound", event.label());
                }
            }
            let mut config = Config::load_or_default(&paths);
            if config.active_pack.is_empty() {
                config.enabled = true;
            }
            config.active_pack = summary.id.clone();
            config.save(&paths).map_err(|error| Error::message(format!(
                "pack was installed but could not be selected: {error}; retry with gitsama use {}", summary.id
            )))?;
            println!("✓ Using {} ({})", summary.name, summary.id);
            println!("Preview your pack: gitsama test all");
            Ok(())
        }
    }
}

fn contribution_value(flag: &str, value: Option<String>, prompt: &str) -> Result<String> {
    let value = match value {
        Some(value) => value,
        None if interactive() => prompt_line(prompt)?,
        None => {
            return Err(Error::message(format!(
                "missing contribution metadata; supply --{flag}"
            )));
        }
    };
    if value.trim().is_empty() {
        return Err(Error::message(format!("--{flag} must not be empty")));
    }
    Ok(value)
}

fn use_pack(id: &str) -> Result<()> {
    let paths = AppPaths::discover()?;
    let mut config = Config::load_or_default(&paths);
    if config.removed_builtin_packs.remove(id) {
        config.save(&paths)?;
    }
    packs::ensure_starter(&paths)?;
    packs::ensure_bundled(&paths)?;
    let pack = packs::find(&paths, id)?;
    if config.active_pack.is_empty() {
        config.enabled = true;
    }
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
    if config.active_pack.is_empty() {
        return Err(Error::Pack("no voice pack selected; install one with gitsama pack scaffold or select/restore one with gitsama use <id>".to_owned()));
    }
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
        let custom = !packs::is_built_in(&config.active_pack);
        let mut state = pack_state::load(&paths, &config.active_pack)?;
        let original = if custom {
            Some(pack_state::fingerprint(&packs::find(
                &paths,
                &config.active_pack,
            )?)?)
        } else {
            None
        };
        if custom {
            state.tested_fingerprint = None;
            pack_state::save(&paths, &config.active_pack, &state)?;
        }
        let mut played = false;
        let mut complete = true;
        for event in EventKind::ALL {
            match test_one(&paths, &config, event)? {
                audio::PlaybackOutcome::Played { .. } => played = true,
                audio::PlaybackOutcome::Skipped(audio::SkipReason::NoSound) => {}
                audio::PlaybackOutcome::Skipped(_) => complete = false,
            }
        }
        if custom && played && complete {
            let pack = packs::find(&paths, &config.active_pack)?;
            state.tested_fingerprint = Some(pack_state::fingerprint(&pack)?);
            if state.tested_fingerprint != original {
                return Err(Error::Pack(
                    "pack changed during playback; rerun gitsama test all".to_owned(),
                ));
            }
            let invite = state.created_locally
                && !state.invitation_shown
                && state.contribution_url.is_none();
            if invite {
                state.invitation_shown = true;
            }
            pack_state::save(&paths, &config.active_pack, &state)?;
            println!(
                "✓ Full preview completed for {} ({})",
                pack.manifest.name, pack.manifest.id
            );
            if invite {
                println!("Share this voice pack with the GitSama community:");
                println!("  gitsama pack contribute {}", pack.manifest.id);
            }
        }
        return Ok(());
    }

    let event = match value.parse::<usize>() {
        Ok(index) if (1..=EventKind::ALL.len()).contains(&index) => EventKind::ALL[index - 1],
        _ => EventKind::parse(&value)?,
    };
    test_one(&paths, &config, event).map(|_| ())
}

fn test_one(paths: &AppPaths, config: &Config, event: EventKind) -> Result<audio::PlaybackOutcome> {
    let outcome = audio::play(paths, config, event)?;
    match &outcome {
        audio::PlaybackOutcome::Played {
            resolved_event,
            path,
        } => {
            let filename = path.file_name().unwrap_or_default().to_string_lossy();
            if *resolved_event != event {
                println!(
                    "  Played {}: {filename} (fallback to {})",
                    event.label(),
                    resolved_event.label()
                );
            } else {
                println!("  Played {}: {filename}", event.label());
            }
        }
        audio::PlaybackOutcome::Skipped(reason) => {
            println!("  Skipped {}: {}", event.label(), reason.message())
        }
    }
    Ok(outcome)
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

fn set_playback_mode(value: &str) -> Result<()> {
    let mode = value.trim().to_ascii_lowercase();
    if !["auto", "detached", "linger"].contains(&mode.as_str()) {
        return Err(Error::message(
            "playback mode must be 'auto', 'detached', or 'linger'",
        ));
    }
    let paths = AppPaths::discover()?;
    let mut config = Config::load_or_default(&paths);
    config.playback_mode = mode.clone();
    config.save(&paths)?;
    println!("✓ Playback mode set to '{mode}'");
    Ok(())
}

fn set_linger_cap(value: f64) -> Result<()> {
    if value <= 0.0 || value > 30.0 {
        return Err(Error::message(
            "linger cap must be between 0.1 and 30.0 seconds",
        ));
    }
    let paths = AppPaths::discover()?;
    let mut config = Config::load_or_default(&paths);
    config.linger_cap_secs = value;
    config.save(&paths)?;
    println!("✓ Linger cap set to {value}s");
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

pub(crate) fn interactive() -> bool {
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

pub(crate) fn prompt_yes_no(prompt: &str, default: bool) -> Result<bool> {
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

    #[test]
    fn scaffold_import_and_template_arguments_are_distinct() {
        assert!(Cli::try_parse_from(["gitsama", "pack", "scaffold"]).is_ok());
        assert!(Cli::try_parse_from(["gitsama", "pack", "scaffold", "Bleach"]).is_ok());
        assert!(
            Cli::try_parse_from([
                "gitsama",
                "pack",
                "scaffold",
                "--template",
                "Bleach",
                "packs"
            ])
            .is_ok()
        );
        assert!(Cli::try_parse_from(["gitsama", "pack", "scaffold", "--template"]).is_err());
        assert!(Cli::try_parse_from(["gitsama", "pack", "scaffold", "Bleach", "packs"]).is_err());
    }

    #[test]
    fn contribution_accepts_metadata_flags() {
        assert!(
            Cli::try_parse_from([
                "gitsama",
                "pack",
                "contribute",
                "bleach",
                "--author",
                "Contributor",
                "--license",
                "CC0-1.0",
                "--source",
                "Original recordings"
            ])
            .is_ok()
        );
        assert!(Cli::try_parse_from(["gitsama", "pack", "contribute"]).is_err());
    }

    #[test]
    fn delete_and_remove_accept_the_same_pack_id() {
        for command in ["delete", "remove"] {
            let cli = Cli::try_parse_from(["gitsama", "pack", command, "naruto"]).expect("parse");
            assert!(matches!(cli.command, Some(Command::Pack {
                command: super::PackCommand::Remove { id }
            }) if id == "naruto"));
        }
    }
}
