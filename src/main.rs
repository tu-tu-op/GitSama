mod audio;
mod cli;
mod config;
mod contribute;
mod doctor;
mod error;
mod events;
mod git;
mod hooks;
mod logging;
mod pack_state;
mod packs;
mod paths;
mod platform;
mod push;
mod state;
mod update;

fn main() {
    let result = std::panic::catch_unwind(cli::run);

    let code = match result {
        Ok(Ok(())) => 0,
        Ok(Err(error)) => {
            if matches!(error, crate::error::Error::NpmUpgradeRequested) {
                std::process::exit(75);
            }
            if !matches!(error, crate::error::Error::UnsupportedGit(_)) {
                eprintln!("GitSama: {error}");
            }
            1
        }
        Err(_) => {
            eprintln!("GitSama could not complete that command.");
            1
        }
    };

    std::process::exit(code);
}
