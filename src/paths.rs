use std::{
    env, fs,
    path::{Path, PathBuf},
};

use crate::error::{Error, Result};

pub const APP_DIR_NAME: &str = ".gitsama";

#[derive(Clone, Debug)]
pub struct AppPaths {
    pub root: PathBuf,
    pub config: PathBuf,
    pub packs: PathBuf,
    pub state: PathBuf,
    pub logs: PathBuf,
    pub log: PathBuf,
    pub bin: PathBuf,
}

impl AppPaths {
    pub fn discover() -> Result<Self> {
        let root = match env::var_os("GITSAMA_HOME") {
            Some(value) if !value.is_empty() => PathBuf::from(value),
            _ => dirs::home_dir()
                .map(|home| home.join(APP_DIR_NAME))
                .ok_or_else(|| Error::message("could not find your home directory"))?,
        };

        Ok(Self::from_root(root))
    }

    pub fn from_root(root: PathBuf) -> Self {
        Self {
            config: root.join("config.toml"),
            packs: root.join("packs"),
            state: root.join("state"),
            logs: root.join("logs"),
            log: root.join("logs").join("gitsama.log"),
            bin: root.join("bin"),
            root,
        }
    }

    pub fn ensure_layout(&self) -> Result<()> {
        for directory in [&self.root, &self.packs, &self.state, &self.logs, &self.bin] {
            fs::create_dir_all(directory).map_err(|source| Error::WriteFile {
                path: directory.to_path_buf(),
                source,
            })?;
        }
        Ok(())
    }

    pub fn contains(&self, path: &Path) -> bool {
        path.starts_with(&self.root)
    }
}
