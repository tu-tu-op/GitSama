use std::{
    env, fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    cli::{interactive, prompt_yes_no},
    error::{Error, Result},
    paths::AppPaths,
    platform,
};

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const RELEASE_API: &str = "https://api.github.com/repos/tu-tu-op/GitSama/releases/latest";
const DOWNLOAD_ROOT: &str = "https://github.com/tu-tu-op/GitSama/releases/download";
const CHECK_INTERVAL: u64 = 24 * 60 * 60;
const MAX_BINARY_SIZE: u64 = 100 * 1024 * 1024;

#[derive(Default, Deserialize, Serialize)]
struct CheckRecord {
    checked_at: u64,
    prompted_version: Option<String>,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
}

impl Release {
    fn version(&self) -> Result<Version> {
        let version = self.tag_name.strip_prefix('v').unwrap_or(&self.tag_name);
        let version = Version::parse(version)
            .map_err(|_| Error::message("the release has an invalid version"))?;
        if self.draft || self.prerelease || !version.pre.is_empty() || !version.build.is_empty() {
            return Err(Error::message("the release is not a stable version"));
        }
        Ok(version)
    }

    fn asset(&self, name: &str) -> Result<&Asset> {
        let expected_url = format!("{DOWNLOAD_ROOT}/{}/{name}", self.tag_name);
        self.assets
            .iter()
            .find(|asset| asset.name == name && asset.browser_download_url == expected_url)
            .ok_or_else(|| {
                Error::message(format!("the release is missing the official {name} asset"))
            })
    }
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .https_only(true)
        .timeout_global(Some(timeout))
        .timeout_resolve(Some(timeout))
        .build()
        .into()
}

fn latest_release(timeout: Duration) -> Result<Release> {
    let text = agent(timeout)
        .get(RELEASE_API)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", concat!("GitSama/", env!("CARGO_PKG_VERSION")))
        .call()
        .and_then(|mut response| {
            response
                .body_mut()
                .with_config()
                .limit(1024 * 1024)
                .read_to_string()
        })
        .map_err(|error| Error::message(format!("could not check GitSama releases: {error}")))?;
    let release: Release = serde_json::from_str(&text)
        .map_err(|_| Error::message("GitHub returned invalid release information"))?;
    release.version()?;
    Ok(release)
}

fn save_record(path: &Path, record: &CheckRecord) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::message("invalid update record path"))?;
    fs::create_dir_all(parent).map_err(|source| Error::WriteFile {
        path: parent.to_path_buf(),
        source,
    })?;
    let mut file = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| Error::message(format!("could not stage update record: {error}")))?;
    serde_json::to_writer(&mut file, record)
        .map_err(|error| Error::message(format!("could not save update record: {error}")))?;
    file.persist(path)
        .map_err(|error| Error::message(format!("could not save update record: {error}")))?;
    Ok(())
}

/// Only called after ordinary interactive commands; failures never change their result.
pub fn notify() -> Result<()> {
    if !interactive() || env::var_os("GITSAMA_NO_UPDATE_CHECK").is_some() {
        return Ok(());
    }
    let Ok(paths) = AppPaths::discover() else {
        return Ok(());
    };
    let path = paths.state.join("update-check.json");
    let mut record: CheckRecord = fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if now
        .checked_sub(record.checked_at)
        .is_some_and(|age| age < CHECK_INTERVAL)
    {
        return Ok(());
    }
    // Remember failed/offline checks too, so repeated commands stay responsive.
    record.checked_at = now;
    if save_record(&path, &record).is_err() {
        return Ok(());
    }
    let Ok(release) = latest_release(Duration::from_secs(3)) else {
        return Ok(());
    };
    let version = release.version()?.to_string();
    if release.version()? <= current_version()?
        || record.prompted_version.as_deref() == Some(&version)
    {
        return Ok(());
    }
    record.prompted_version = Some(version.clone());
    if save_record(&path, &record).is_err() {
        return Ok(());
    }
    println!("\nGitSama {version} is available (you have {CURRENT_VERSION}).");
    match prompt_yes_no("Would you like to update now? [y/N] ", false)
        .and_then(|yes| if yes { install(&release, true) } else { Ok(()) })
    {
        Err(Error::NpmUpgradeRequested) => Err(Error::NpmUpgradeRequested),
        Err(error) => {
            eprintln!("GitSama update: {error}. Retry with gitsama upgrade.");
            Ok(())
        }
        Ok(()) => Ok(()),
    }
}

fn current_version() -> Result<Version> {
    Version::parse(CURRENT_VERSION).map_err(|error| Error::message(error.to_string()))
}

pub fn run(check_only: bool, yes: bool) -> Result<()> {
    println!("Checking for GitSama updates...");
    let release = latest_release(Duration::from_secs(15))?;
    let version = release.version()?;
    if version <= current_version()? {
        println!("You are running GitSama {CURRENT_VERSION}; latest published version: {version}.");
        return Ok(());
    }
    println!("GitSama {version} is available (you have {CURRENT_VERSION}).");
    if check_only {
        println!("Run gitsama upgrade to install it.");
        return Ok(());
    }
    if !yes {
        if !interactive() {
            return Err(Error::message(
                "use gitsama upgrade --yes to approve a noninteractive update",
            ));
        }
        if !prompt_yes_no("Would you like to update now? [y/N] ", false)? {
            println!("Update postponed. Run gitsama upgrade when you are ready.");
            return Ok(());
        }
    }
    install(&release, false)
}

fn asset_name(version: &Version) -> Result<String> {
    let target = match (env::consts::OS, env::consts::ARCH) {
        ("windows", "x86_64") => "windows-x86_64.exe",
        ("linux", "x86_64") => "linux-x86_64",
        ("macos", "x86_64") => "macos-x86_64",
        ("macos", "aarch64") => "macos-aarch64",
        _ => {
            return Err(Error::message(
                "no prebuilt update supports this platform; update from source",
            ));
        }
    };
    Ok(format!("gitsama-v{version}-{target}"))
}

fn download(asset: &Asset, limit: u64) -> Result<Vec<u8>> {
    if asset.size == 0 || asset.size > limit {
        return Err(Error::message("the release asset has an invalid size"));
    }
    let bytes = agent(Duration::from_secs(60))
        .get(&asset.browser_download_url)
        .call()
        .and_then(|mut response| response.body_mut().with_config().limit(limit).read_to_vec())
        .map_err(|error| Error::message(format!("could not download {}: {error}", asset.name)))?;
    if bytes.len() as u64 != asset.size {
        return Err(Error::message("the release download is incomplete"));
    }
    Ok(bytes)
}

fn install(release: &Release, automatic: bool) -> Result<()> {
    let version = release.version()?;
    // The npm wrapper installs the whole package after this native process exits,
    // avoiding a locked executable on Windows and keeping npm metadata in sync.
    if let Some(request_path) = env::var_os("GITSAMA_NPM_UPGRADE_FILE") {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&request_path)
            .map_err(|error| Error::message(format!("could not request npm update: {error}")))?;
        serde_json::to_writer(
            &mut file,
            &serde_json::json!({ "version": version.to_string(), "automatic": automatic }),
        )
        .map_err(|error| Error::message(format!("could not request npm update: {error}")))?;
        return Err(Error::NpmUpgradeRequested);
    }
    let executable = env::current_exe().map_err(|error| {
        Error::message(format!("could not find the installed executable: {error}"))
    })?;
    if executable
        .components()
        .any(|component| component.as_os_str() == "Cellar")
    {
        return Err(Error::message(
            "this installation is managed by Homebrew; run brew upgrade gitsama",
        ));
    }
    if executable.parent().is_some_and(|parent| {
        parent.file_name().is_some_and(|name| name == "native")
            && parent
                .parent()
                .is_some_and(|root| root.join("package.json").is_file())
    }) {
        return Err(Error::message(
            "this binary is managed by npm; run npm install --global gitsama@latest",
        ));
    }
    let name = asset_name(&version)?;
    let binary_asset = release.asset(&name)?;
    let checksum_asset = release.asset(&format!("{name}.sha256"))?;
    println!("Downloading GitSama {version}...");
    let checksum_bytes = download(checksum_asset, 1024)?;
    let checksum_text = std::str::from_utf8(&checksum_bytes)
        .map_err(|_| Error::message("the release checksum is not valid text"))?;
    let mut checksum_fields = checksum_text.split_whitespace();
    let expected = checksum_fields.next().unwrap_or_default();
    let filename = checksum_fields
        .next()
        .map(|name| name.trim_start_matches('*'));
    if expected.len() != 64
        || !expected.bytes().all(|byte| byte.is_ascii_hexdigit())
        || filename.is_some_and(|filename| filename != name)
        || checksum_fields.next().is_some()
    {
        return Err(Error::message("the release checksum is invalid"));
    }
    let binary = download(binary_asset, MAX_BINARY_SIZE)?;
    if !format!("{:x}", Sha256::digest(&binary)).eq_ignore_ascii_case(expected) {
        return Err(Error::message(
            "the downloaded binary failed its SHA-256 check; update cancelled",
        ));
    }
    let stage = tempfile::tempdir()
        .map_err(|error| Error::message(format!("could not stage the update: {error}")))?;
    let candidate = stage.path().join(platform::executable_name());
    let mut file = fs::File::create(&candidate).map_err(|source| Error::WriteFile {
        path: candidate.clone(),
        source,
    })?;
    file.write_all(&binary).map_err(|source| Error::WriteFile {
        path: candidate.clone(),
        source,
    })?;
    file.sync_all().map_err(|source| Error::WriteFile {
        path: candidate.clone(),
        source,
    })?;
    drop(file);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&candidate, fs::Permissions::from_mode(0o755)).map_err(|error| {
            Error::message(format!("could not make the update executable: {error}"))
        })?;
    }
    verify_binary(&candidate, &version)?;
    self_replace::self_replace(&candidate)
        .map_err(|error| Error::message(format!("could not replace the installed binary: {error}; reinstall GitSama using your original installer")))?;
    println!("Updated GitSama to {version}. Your packs and settings are preserved.");
    println!("The next gitsama command will use the new version.");
    Ok(())
}

fn verify_binary(path: &Path, version: &Version) -> Result<()> {
    let mut child = Command::new(path)
        .arg("--version")
        .env("GITSAMA_NONINTERACTIVE", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| {
            Error::message(format!("could not open the downloaded binary: {error}"))
        })?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Error::message(
                    "the downloaded binary did not report its version; update cancelled",
                ));
            }
        }
    }
    let output = child.wait_with_output().map_err(|error| {
        Error::message(format!(
            "could not read the downloaded binary version: {error}"
        ))
    })?;
    if !output.status.success()
        || String::from_utf8_lossy(&output.stdout).trim() != format!("gitsama {version}")
    {
        return Err(Error::message(
            "the downloaded binary has the wrong version; update cancelled",
        ));
    }
    Ok(())
}
