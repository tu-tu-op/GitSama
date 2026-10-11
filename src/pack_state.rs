use std::{
    collections::{BTreeSet, hash_map::DefaultHasher},
    fs,
    hash::{Hash, Hasher},
    io::Write,
};

use serde::{Deserialize, Serialize};

use crate::{
    error::{Error, Result},
    packs::{self, Pack},
    paths::AppPaths,
};

#[derive(Default, Deserialize, Serialize)]
#[serde(default)]
pub struct PackState {
    pub created_locally: bool,
    pub tested_fingerprint: Option<String>,
    pub invitation_shown: bool,
    pub contribution_url: Option<String>,
}

fn state_path(paths: &AppPaths, id: &str) -> Result<std::path::PathBuf> {
    if !packs::is_valid_id(id) {
        return Err(Error::Pack("invalid pack id for local state".to_owned()));
    }
    Ok(paths.state.join(format!("pack-{id}.json")))
}

pub fn load(paths: &AppPaths, id: &str) -> Result<PackState> {
    let path = state_path(paths, id)?;
    match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| {
            Error::message(format!("could not parse {}: {error}", path.display()))
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(PackState::default()),
        Err(source) => Err(Error::ReadFile { path, source }),
    }
}

pub fn save(paths: &AppPaths, id: &str, state: &PackState) -> Result<()> {
    let path = state_path(paths, id)?;
    paths.ensure_layout()?;
    let mut staging =
        tempfile::NamedTempFile::new_in(&paths.state).map_err(|source| Error::WriteFile {
            path: path.clone(),
            source,
        })?;
    let bytes =
        serde_json::to_vec_pretty(state).map_err(|error| Error::message(error.to_string()))?;
    staging
        .write_all(&bytes)
        .map_err(|source| Error::WriteFile {
            path: path.clone(),
            source,
        })?;
    staging.persist(&path).map_err(|error| Error::WriteFile {
        path,
        source: error.error,
    })?;
    Ok(())
}

pub fn remove(paths: &AppPaths, id: &str) -> Result<()> {
    let path = state_path(paths, id)?;
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(Error::WriteFile { path, source }),
    }
}

// This checksum detects local edits; it is not a portable signature.
pub fn fingerprint(pack: &Pack) -> Result<String> {
    let mut hasher = DefaultHasher::new();
    let manifest =
        toml::to_string(&pack.manifest).map_err(|error| Error::Pack(error.to_string()))?;
    manifest.hash(&mut hasher);
    let files: BTreeSet<_> = pack.manifest.events.values().flatten().collect();
    for relative in files {
        relative.hash(&mut hasher);
        let path = pack.root.join(relative);
        fs::read(&path)
            .map_err(|source| Error::ReadFile { path, source })?
            .hash(&mut hasher);
    }
    Ok(format!("{:016x}", hasher.finish()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_records_persist_detect_changes_and_clear_on_removal() {
        let directory = tempfile::tempdir().expect("temp");
        let paths = AppPaths::from_root(directory.path().join("home"));
        packs::ensure_starter(&paths).expect("starter");
        let pack = packs::find(&paths, "starter").expect("pack");
        let original = fingerprint(&pack).expect("fingerprint");
        let state = PackState {
            created_locally: true,
            tested_fingerprint: Some(original.clone()),
            ..Default::default()
        };
        save(&paths, "sample", &state).expect("save");
        assert_eq!(
            load(&paths, "sample").expect("load").tested_fingerprint,
            Some(original.clone())
        );
        fs::write(pack.root.join("audio/commit.wav"), b"changed").expect("edit");
        assert_ne!(fingerprint(&pack).expect("fingerprint"), original);
        remove(&paths, "sample").expect("remove");
        assert!(!load(&paths, "sample").expect("load").created_locally);
        assert!(save(&paths, "../outside", &state).is_err());
    }
}
