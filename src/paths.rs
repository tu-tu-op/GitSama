use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};

use crate::error::{Error, Result};

pub const APP_DIR_NAME: &str = ".gitsama";
const HOME_MARKER: &str = ".gitsama-home";
const HOME_MARKER_CONTENT: &[u8] = b"gitsama-home-v1\n";

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::message("invalid state file path"))?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|source| Error::WriteFile {
        path: path.to_path_buf(),
        source,
    })?;
    file.write_all(bytes)
        .and_then(|_| file.as_file().sync_all())
        .map_err(|source| Error::WriteFile {
            path: path.to_path_buf(),
            source,
        })?;
    file.persist(path).map_err(|error| Error::WriteFile {
        path: path.to_path_buf(),
        source: error.error,
    })?;
    Ok(())
}

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
        self.validate_root_location()?;
        for directory in [&self.root, &self.packs, &self.state, &self.logs, &self.bin] {
            if fs::symlink_metadata(directory)
                .is_ok_and(|metadata| metadata.file_type().is_symlink() || !metadata.is_dir())
            {
                return Err(Error::message(
                    "GitSama data directories must be real directories, not links",
                ));
            }
            fs::create_dir_all(directory).map_err(|source| Error::WriteFile {
                path: directory.to_path_buf(),
                source,
            })?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).map_err(
                    |source| Error::WriteFile {
                        path: directory.to_path_buf(),
                        source,
                    },
                )?;
            }
        }
        let marker = self.root.join(HOME_MARKER);
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&marker) {
            Ok(mut file) => {
                file.write_all(HOME_MARKER_CONTENT)
                    .map_err(|source| Error::WriteFile {
                        path: marker,
                        source,
                    })?
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(source) => {
                return Err(Error::WriteFile {
                    path: marker,
                    source,
                });
            }
        }
        Ok(())
    }

    pub fn validate_root_location(&self) -> Result<PathBuf> {
        if fs::symlink_metadata(&self.root).is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            return Err(Error::message(
                "GitSama home must not be a symlink; set GITSAMA_HOME to the real data directory",
            ));
        }
        let root = match fs::canonicalize(&self.root) {
            Ok(root) => root,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                std::path::absolute(&self.root)
                    .map_err(|error| Error::message(error.to_string()))?
            }
            Err(source) => {
                return Err(Error::ReadFile {
                    path: self.root.clone(),
                    source,
                });
            }
        };
        if root.parent().is_none() {
            return Err(Error::message(
                "refusing to use a filesystem root as GitSama home",
            ));
        }
        if let Some(home) = dirs::home_dir() {
            if let Ok(home) = fs::canonicalize(home) {
                if home.starts_with(&root) {
                    return Err(Error::message(
                        "GitSama home cannot contain your user home; select a dedicated data directory",
                    ));
                }
            }
        }
        if fs::symlink_metadata(root.join(".git")).is_ok()
            || (root.join("HEAD").is_file()
                && root.join("objects").is_dir()
                && root.join("refs").is_dir())
        {
            return Err(Error::message(
                "refusing to use a Git repository as GitSama home",
            ));
        }
        Ok(root)
    }

    pub fn validate_removal(&self) -> Result<PathBuf> {
        let root = self.validate_root_location()?;
        if env::current_dir()
            .and_then(fs::canonicalize)
            .is_ok_and(|working| working.starts_with(&root))
        {
            return Err(Error::message(
                "refusing to remove your working directory; run uninstall outside the GitSama data directory or use --keep-data",
            ));
        }
        let marker = root.join(HOME_MARKER);
        let marked = fs::symlink_metadata(&marker).is_ok_and(|metadata| {
            metadata.is_file()
                && !metadata.file_type().is_symlink()
                && metadata.len() == HOME_MARKER_CONTENT.len() as u64
        }) && fs::read(&marker).is_ok_and(|bytes| bytes == HOME_MARKER_CONTENT);
        // Older releases have no marker, but their complete layout remains recognizable.
        let legacy = fs::symlink_metadata(&self.config)
            .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
            && [&self.packs, &self.state, &self.logs, &self.bin]
                .into_iter()
                .all(|path| {
                    fs::symlink_metadata(path).is_ok_and(|metadata| {
                        metadata.is_dir() && !metadata.file_type().is_symlink()
                    })
                });
        if !marked && !legacy {
            return Err(Error::message(
                "refusing to remove a directory that is not an identifiable GitSama home; use --keep-data",
            ));
        }
        for entry in fs::read_dir(&root).map_err(|source| Error::ReadFile {
            path: root.clone(),
            source,
        })? {
            let entry = entry.map_err(|source| Error::ReadFile {
                path: root.clone(),
                source,
            })?;
            if !matches!(
                entry.file_name().to_str(),
                Some(
                    "packs"
                        | "state"
                        | "logs"
                        | "bin"
                        | "config.toml"
                        | HOME_MARKER
                        | ".DS_Store"
                        | "Thumbs.db"
                        | "desktop.ini"
                )
            ) {
                return Err(Error::message(
                    "GitSama home contains unrelated files; use --keep-data or move those files before uninstalling",
                ));
            }
        }
        Ok(root)
    }
}

#[cfg(test)]
mod tests {
    use super::AppPaths;

    #[cfg(unix)]
    #[test]
    fn atomic_state_writes_replace_links_without_touching_external_files() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let directory = tempfile::tempdir().expect("temp");
        let paths = AppPaths::from_root(directory.path().join("data"));
        paths.ensure_layout().expect("layout");
        let outside = directory.path().join("private.txt");
        std::fs::write(&outside, "must survive").expect("outside");
        symlink(&outside, &paths.config).expect("link");
        super::write_atomic(&paths.config, b"volume = 80\n").expect("save");
        assert_eq!(
            std::fs::read_to_string(outside).expect("outside"),
            "must survive"
        );
        assert!(
            !std::fs::symlink_metadata(&paths.config)
                .expect("config")
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            std::fs::metadata(&paths.config)
                .expect("config")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(&paths.root)
                .expect("root")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
    #[test]
    fn cleanup_requires_an_owned_directory_and_protects_home_and_repositories() {
        let directory = tempfile::tempdir().expect("temp");
        let paths = AppPaths::from_root(directory.path().join("data"));
        std::fs::create_dir_all(&paths.root).expect("root");
        assert!(paths.validate_removal().is_err());
        paths.ensure_layout().expect("layout");
        assert!(paths.validate_removal().is_ok());
        std::fs::write(paths.root.join("private.txt"), "unrelated data").expect("extra");
        assert!(paths.validate_removal().is_err());
        std::fs::remove_file(paths.root.join("private.txt")).expect("remove extra");
        std::fs::create_dir(paths.root.join(".git")).expect("git metadata");
        assert!(paths.validate_removal().is_err());
        assert!(
            AppPaths::from_root(std::env::current_dir().expect("cwd"))
                .validate_root_location()
                .is_err()
        );
        if let Some(home) = dirs::home_dir() {
            assert!(AppPaths::from_root(home).validate_root_location().is_err());
        }
    }
}
