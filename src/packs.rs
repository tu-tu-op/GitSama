use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};

use rand::random_range;
use serde::{Deserialize, Serialize};

use crate::{
    error::{Error, Result},
    events::EventKind,
    logging,
    paths::AppPaths,
};

pub const PACK_SCHEMA_VERSION: u32 = 1;
const STARTER_ID: &str = "starter";
const SUPPORTED_EXTENSIONS: [&str; 4] = ["wav", "mp3", "ogg", "flac"];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PackManifest {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub author: String,
    pub version: String,
    pub description: String,
    pub license: String,
    #[serde(default)]
    pub events: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Debug)]
pub struct Pack {
    pub root: PathBuf,
    pub manifest: PackManifest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackSummary {
    pub id: String,
    pub name: String,
    pub author: String,
    pub version: String,
    pub root: PathBuf,
    pub valid: bool,
}

impl Pack {
    pub fn load(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        if fs::symlink_metadata(root)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            return Err(Error::Pack(format!(
                "pack root cannot be a symlink: {}",
                root.display()
            )));
        }
        let root = fs::canonicalize(root).map_err(|source| Error::ReadFile {
            path: root.to_path_buf(),
            source,
        })?;
        if !root.is_dir() {
            return Err(Error::Pack(format!(
                "{} is not a directory",
                root.display()
            )));
        }

        let manifest_path = root.join("pack.toml");
        if fs::symlink_metadata(&manifest_path)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            return Err(Error::Pack(format!(
                "pack manifest cannot be a symlink: {}",
                manifest_path.display()
            )));
        }
        let text = fs::read_to_string(&manifest_path).map_err(|source| Error::ReadFile {
            path: manifest_path,
            source,
        })?;
        let manifest: PackManifest =
            toml::from_str(&text).map_err(|error| Error::Pack(error.to_string()))?;
        validate_manifest(&manifest)?;
        validate_audio_files(&root, &manifest)?;

        Ok(Self { root, manifest })
    }

    pub fn files_for(&self, event: EventKind) -> Vec<PathBuf> {
        self.manifest
            .events
            .get(event.as_str())
            .into_iter()
            .flatten()
            .map(|relative| self.root.join(relative))
            .collect()
    }

    pub fn resolve(&self, event: EventKind) -> Option<(EventKind, PathBuf)> {
        let mut candidates = self.files_for(event);
        let resolved_event = if candidates.is_empty() && event == EventKind::MassivePush {
            candidates = self.files_for(EventKind::Push);
            EventKind::Push
        } else {
            event
        };

        if candidates.is_empty() {
            return None;
        }

        candidates
            .get(random_range(0..candidates.len()))
            .cloned()
            .map(|path| (resolved_event, path))
    }
}

pub fn ensure_starter(paths: &AppPaths) -> Result<()> {
    paths.ensure_layout()?;
    let root = paths.packs.join(STARTER_ID);
    match fs::symlink_metadata(&root) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(Error::Pack(format!(
                "Starter pack root cannot be a symlink: {}",
                root.display()
            )));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(Error::ReadFile {
                path: root.clone(),
                source,
            });
        }
    }
    let manifest_path = root.join("pack.toml");
    if manifest_path.exists() && Pack::load(&root).is_ok() {
        return Ok(());
    }

    let audio = root.join("audio");
    if fs::symlink_metadata(&audio)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(Error::Pack(format!(
            "Starter audio directory cannot be a symlink: {}",
            audio.display()
        )));
    }
    fs::create_dir_all(&audio).map_err(|source| Error::WriteFile {
        path: audio.clone(),
        source,
    })?;

    let frequencies = [
        (EventKind::Commit, 440_u32, 180_u32),
        (EventKind::Push, 523, 220),
        (EventKind::MassivePush, 659, 320),
        (EventKind::Merge, 392, 250),
        (EventKind::BranchSwitch, 494, 170),
        (EventKind::BranchCreate, 587, 200),
        (EventKind::BranchDelete, 330, 200),
        (EventKind::Rebase, 740, 260),
    ];

    let mut events = BTreeMap::new();
    for (event, frequency, duration) in frequencies {
        let filename = format!("{}.wav", event.as_str().replace('_', "-"));
        let path = audio.join(&filename);
        if fs::symlink_metadata(&path)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            return Err(Error::Pack(format!(
                "Starter audio file cannot be a symlink: {}",
                path.display()
            )));
        }
        write_tone(&path, frequency, duration)?;
        events.insert(event.as_str().to_owned(), vec![format!("audio/{filename}")]);
    }

    let manifest = PackManifest {
        schema_version: PACK_SCHEMA_VERSION,
        id: STARTER_ID.to_owned(),
        name: "Starter".to_owned(),
        author: "GitSama".to_owned(),
        version: "1.0.0".to_owned(),
        description: "Generated tones for installation and CI verification.".to_owned(),
        license: "MIT".to_owned(),
        events,
    };
    write_manifest(&root, &manifest)?;
    Ok(())
}

pub fn installed(paths: &AppPaths) -> Result<Vec<Pack>> {
    ensure_starter(paths)?;
    let entries = fs::read_dir(&paths.packs).map_err(|source| Error::ReadFile {
        path: paths.packs.clone(),
        source,
    })?;
    let mut packs = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                logging::write(paths, &format!("pack directory entry skipped: {error}"));
                continue;
            }
        };
        if !entry.path().is_dir() {
            continue;
        }
        match Pack::load(entry.path()) {
            Ok(pack) => packs.push(pack),
            Err(error) => logging::write(
                paths,
                &format!("invalid installed pack {}: {error}", entry.path().display()),
            ),
        }
    }
    packs.sort_by(|left, right| left.manifest.name.cmp(&right.manifest.name));
    Ok(packs)
}

pub fn summary(path: impl AsRef<Path>) -> Result<PackSummary> {
    let pack = Pack::load(path)?;
    Ok(PackSummary {
        id: pack.manifest.id.clone(),
        name: pack.manifest.name.clone(),
        author: pack.manifest.author.clone(),
        version: pack.manifest.version.clone(),
        root: pack.root,
        valid: true,
    })
}

pub fn validate(path: impl AsRef<Path>) -> Result<PackSummary> {
    summary(path)
}

pub fn install(paths: &AppPaths, source: impl AsRef<Path>) -> Result<PackSummary> {
    let source = source.as_ref();
    let source_pack = Pack::load(source)?;
    paths.ensure_layout()?;
    let destination = paths.packs.join(&source_pack.manifest.id);
    if fs::symlink_metadata(&destination).is_ok() {
        return Err(Error::Pack(format!(
            "pack '{}' is already installed; remove it before importing an update",
            source_pack.manifest.id
        )));
    }
    copy_pack(source, &destination)?;
    summary(destination)
}

pub fn remove(paths: &AppPaths, id: &str) -> Result<()> {
    validate_id(id)?;
    if id == STARTER_ID {
        return Err(Error::Pack(
            "the Starter pack is built in and cannot be removed".to_owned(),
        ));
    }
    let destination = paths.packs.join(id);
    let metadata = fs::symlink_metadata(&destination).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            Error::Pack(format!("pack '{id}' is not installed"))
        } else {
            Error::ReadFile {
                path: destination.clone(),
                source: error,
            }
        }
    })?;
    if !metadata.is_dir() || !destination.starts_with(&paths.packs) {
        return Err(Error::Pack(
            "refusing to remove an unsafe pack path".to_owned(),
        ));
    }
    fs::remove_dir_all(&destination).map_err(|source| Error::WriteFile {
        path: destination,
        source,
    })
}

pub fn find(paths: &AppPaths, id: &str) -> Result<Pack> {
    validate_id(id)?;
    let path = paths.packs.join(id);
    Pack::load(path)
}

pub fn scaffold(name: &str, parent: impl AsRef<Path>) -> Result<PathBuf> {
    if name.trim().is_empty() {
        return Err(Error::Pack("pack name cannot be empty".to_owned()));
    }
    let id = slugify(name);
    let parent = parent.as_ref();
    let root = parent.join(&id);
    if fs::symlink_metadata(&root).is_ok() {
        return Err(Error::Pack(format!(
            "destination {} already exists",
            root.display()
        )));
    }
    let audio = root.join("audio");
    fs::create_dir_all(&audio).map_err(|source| Error::WriteFile {
        path: audio,
        source,
    })?;

    let mut events = BTreeMap::new();
    for event in EventKind::ALL {
        events.insert(event.as_str().to_owned(), Vec::new());
    }
    let manifest = PackManifest {
        schema_version: PACK_SCHEMA_VERSION,
        id: id.clone(),
        name: name.trim().to_owned(),
        author: "Your Name".to_owned(),
        version: "1.0.0".to_owned(),
        description: "A GitSama sound pack.".to_owned(),
        license: "user-provided".to_owned(),
        events,
    };
    write_manifest(&root, &manifest)?;
    let readme = format!(
        "# {name}

Add audio files under audio/, then map them in pack.toml.

Supported formats: WAV, MP3, OGG, FLAC.

Validate and install:

    gitsama pack validate .
    gitsama pack add .
    gitsama use {id}
    gitsama test all

Only include audio you are legally allowed to redistribute.
"
    );
    fs::write(root.join("README.md"), readme).map_err(|source| Error::WriteFile {
        path: root.join("README.md"),
        source,
    })?;
    Ok(root)
}

pub fn slugify(name: &str) -> String {
    let mut slug = String::new();
    for character in name.trim().chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
        } else if character == '-' || character == '_' {
            slug.push(character);
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-').to_owned();
    if slug.is_empty() {
        "pack".to_owned()
    } else {
        slug
    }
}

fn validate_manifest(manifest: &PackManifest) -> Result<()> {
    if manifest.schema_version != PACK_SCHEMA_VERSION {
        return Err(Error::Pack(format!(
            "schema {} is unsupported; this build supports schema {}",
            manifest.schema_version, PACK_SCHEMA_VERSION
        )));
    }
    validate_id(&manifest.id)?;
    for field in [
        ("name", &manifest.name),
        ("author", &manifest.author),
        ("version", &manifest.version),
        ("description", &manifest.description),
        ("license", &manifest.license),
    ] {
        if field.1.trim().is_empty() {
            return Err(Error::Pack(format!("{} cannot be empty", field.0)));
        }
    }
    for event in manifest.events.keys() {
        let parsed = EventKind::parse(event)?;
        if event != parsed.as_str() {
            return Err(Error::Pack(format!(
                "event '{event}' must use the canonical name '{}'",
                parsed.as_str()
            )));
        }
    }
    Ok(())
}

fn validate_id(id: &str) -> Result<()> {
    if !is_valid_id(id) {
        return Err(Error::Pack(format!(
            "id '{id}' must use lowercase letters, digits, '-' or '_', and cannot start with '-'"
        )));
    }
    Ok(())
}

pub fn is_valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '-'
                || character == '_'
        })
        && !id.starts_with('-')
}

fn validate_audio_files(root: &Path, manifest: &PackManifest) -> Result<()> {
    for (event, files) in &manifest.events {
        let event = EventKind::parse(event)?;
        for relative in files {
            let relative_path = safe_relative_path(relative)?;
            let path = root.join(relative_path);
            let metadata = fs::symlink_metadata(&path).map_err(|source| {
                Error::Pack(format!(
                    "{} for event {}: {source}",
                    path.display(),
                    event.as_str()
                ))
            })?;
            if metadata.file_type().is_symlink() {
                return Err(Error::Pack(format!(
                    "symlink audio path is not allowed: {}",
                    path.display()
                )));
            }
            let canonical = fs::canonicalize(&path)
                .map_err(|source| Error::Pack(format!("{}: {source}", path.display())))?;
            if !canonical.starts_with(root) {
                return Err(Error::Pack(format!(
                    "audio path escapes pack root: {relative}"
                )));
            }
            if !metadata.is_file() {
                return Err(Error::Pack(format!(
                    "audio path is not a regular file: {relative}"
                )));
            }
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            if !SUPPORTED_EXTENSIONS.contains(&extension.as_str()) {
                return Err(Error::Pack(format!(
                    "unsupported audio extension '.{extension}' for {relative}"
                )));
            }
        }
    }
    Ok(())
}

fn safe_relative_path(value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    if value.trim().is_empty() || path.is_absolute() {
        return Err(Error::Pack(format!("audio path must be relative: {value}")));
    }
    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(Error::Pack(format!("unsafe audio path: {value}")));
            }
        }
    }
    Ok(path.to_path_buf())
}

fn copy_pack(source: &Path, destination: &Path) -> Result<()> {
    copy_directory(source, destination)
}

fn copy_directory(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination).map_err(|source_error| Error::WriteFile {
        path: destination.to_path_buf(),
        source: source_error,
    })?;
    for entry in fs::read_dir(source).map_err(|source_error| Error::ReadFile {
        path: source.to_path_buf(),
        source: source_error,
    })? {
        let entry = entry.map_err(|source_error| Error::ReadFile {
            path: source.to_path_buf(),
            source: source_error,
        })?;
        let file_type = entry.file_type().map_err(|source_error| Error::ReadFile {
            path: entry.path(),
            source: source_error,
        })?;
        let target = destination.join(entry.file_name());
        if file_type.is_symlink() {
            return Err(Error::Pack(format!(
                "pack contains an unsafe symlink: {}",
                entry.path().display()
            )));
        }
        if file_type.is_dir() {
            copy_directory(&entry.path(), &target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), &target).map_err(|source_error| Error::WriteFile {
                path: target,
                source: source_error,
            })?;
        }
    }
    Ok(())
}

fn write_manifest(root: &Path, manifest: &PackManifest) -> Result<()> {
    let text = toml::to_string_pretty(manifest)
        .map_err(|error| Error::Pack(format!("could not serialize manifest: {error}")))?;
    fs::write(root.join("pack.toml"), text).map_err(|source| Error::WriteFile {
        path: root.join("pack.toml"),
        source,
    })
}

fn write_tone(path: &Path, frequency: u32, duration_ms: u32) -> Result<()> {
    let sample_rate = 44_100_u32;
    let sample_count = sample_rate * duration_ms / 1_000;
    let data_size = sample_count * 2;
    let mut bytes = Vec::with_capacity((44 + data_size) as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_size).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_size.to_le_bytes());

    for index in 0..sample_count {
        let time = index as f32 / sample_rate as f32;
        let fade_in = (index as f32 / (sample_rate as f32 * 0.01)).min(1.0);
        let fade_out = ((sample_count - index) as f32 / (sample_rate as f32 * 0.04)).min(1.0);
        let envelope = fade_in * fade_out * 0.28;
        let sample = (2.0 * std::f32::consts::PI * frequency as f32 * time).sin() * envelope;
        bytes.extend_from_slice(&((sample * i16::MAX as f32) as i16).to_le_bytes());
    }

    fs::write(path, bytes).map_err(|source| Error::WriteFile {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs};

    use super::{
        PACK_SCHEMA_VERSION, PackManifest, safe_relative_path, slugify, validate, validate_manifest,
    };
    use crate::{events::EventKind, paths::AppPaths};

    fn manifest(path: &str) -> PackManifest {
        let mut events = BTreeMap::new();
        events.insert("commit".to_owned(), vec![path.to_owned()]);
        PackManifest {
            schema_version: PACK_SCHEMA_VERSION,
            id: "sample".to_owned(),
            name: "Sample".to_owned(),
            author: "Tester".to_owned(),
            version: "1.0.0".to_owned(),
            description: "Test".to_owned(),
            license: "test".to_owned(),
            events,
        }
    }

    #[test]
    fn validates_safe_paths_and_rejects_traversal() {
        assert!(safe_relative_path("audio/commit.wav").is_ok());
        assert!(safe_relative_path("../outside.wav").is_err());
        assert!(safe_relative_path("/absolute.wav").is_err());
        assert!(safe_relative_path("").is_err());
    }

    #[test]
    fn rejects_unknown_events_and_future_schema() {
        let mut value = manifest("audio/commit.wav");
        value.events.insert("status".to_owned(), vec![]);
        assert!(validate_manifest(&value).is_err());
        value.events.remove("status");
        value.events.insert("branch-create".to_owned(), vec![]);
        assert!(validate_manifest(&value).is_err());
        value.events.remove("branch-create");
        value.schema_version = PACK_SCHEMA_VERSION + 1;
        assert!(validate_manifest(&value).is_err());
    }

    #[test]
    fn validates_extension_and_existing_file() {
        let directory = tempfile::tempdir().expect("temp");
        fs::create_dir(directory.path().join("audio")).expect("audio");
        fs::write(directory.path().join("audio/commit.wav"), b"RIFF").expect("audio file");
        let manifest_path = directory.path().join("pack.toml");
        let text = toml::to_string(&manifest("audio/commit.wav")).expect("manifest");
        fs::write(&manifest_path, text).expect("manifest file");
        assert!(validate(directory.path()).is_ok());

        fs::write(directory.path().join("audio/commit.exe"), b"not audio").expect("bad file");
        let bad = manifest("audio/commit.exe");
        fs::write(&manifest_path, toml::to_string(&bad).expect("manifest")).expect("manifest");
        assert!(validate(directory.path()).is_err());
    }

    #[test]
    fn slugifies_pack_names() {
        assert_eq!(slugify("Pain Arc"), "pain-arc");
        assert_eq!(slugify("  My__Pack  "), "my__pack");
        assert_eq!(
            EventKind::parse("branch-create").expect("event"),
            EventKind::BranchCreate
        );
    }

    #[test]
    fn scaffold_rejects_an_empty_name() {
        let directory = tempfile::tempdir().expect("temp");
        assert!(super::scaffold(" ", directory.path()).is_err());
    }

    #[test]
    fn resolves_multiple_files_and_massive_push_fallback() {
        let directory = tempfile::tempdir().expect("temp");
        let mut events = BTreeMap::new();
        events.insert(
            "push".to_owned(),
            vec!["audio/push-1.wav".to_owned(), "audio/push-2.wav".to_owned()],
        );
        let pack = super::Pack {
            root: directory.path().to_path_buf(),
            manifest: manifest("audio/push-1.wav"),
        };
        let mut pack = pack;
        pack.manifest.events = events;
        let (resolved, path) = pack.resolve(EventKind::MassivePush).expect("push fallback");
        assert_eq!(resolved, EventKind::Push);
        assert!(path.ends_with("push-1.wav") || path.ends_with("push-2.wav"));
        assert!(pack.resolve(EventKind::Merge).is_none());
    }

    #[test]
    fn repairs_a_damaged_starter_pack() {
        let directory = tempfile::tempdir().expect("temp");
        let paths = AppPaths::from_root(directory.path().join(".gitsama"));
        super::ensure_starter(&paths).expect("starter");
        fs::remove_file(paths.packs.join("starter/audio/commit.wav")).expect("remove tone");
        super::ensure_starter(&paths).expect("repair starter");
        assert!(super::find(&paths, "starter").is_ok());
    }
}
