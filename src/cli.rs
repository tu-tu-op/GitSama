use clap::{Parser, Subcommand};

use crate::error::Result;

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
    Uninstall,
    #[command(hide = true)]
    Hook {
        event: String,
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
    if cli.command.is_none() {
        print_dashboard();
    }
    Ok(())
}

fn print_dashboard() {
    println!("GitSama ⚔️");
    println!("Anime reactions for your Git workflow.");
    println!();
    println!("Run gitsama setup to get started.");
    println!("Run gitsama help for all commands.");
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command};
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
    }
}

