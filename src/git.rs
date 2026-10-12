use std::{
    env,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::error::{Error, Result};

pub const MIN_GIT_MAJOR: u64 = 2;
pub const MIN_GIT_MINOR: u64 = 54;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct GitVersion {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub suffix: String,
}

impl GitVersion {
    pub fn parse(text: &str) -> Result<Self> {
        let version = text
            .split_whitespace()
            .find(|part| {
                part.chars()
                    .next()
                    .is_some_and(|character| character.is_ascii_digit())
            })
            .ok_or_else(|| Error::Git("Git did not report a version".to_owned()))?;

        let mut parts = version.splitn(3, '.');
        let major = parts
            .next()
            .and_then(|value| value.parse().ok())
            .ok_or_else(|| Error::Git(format!("could not parse Git version from '{text}'")))?;
        let minor = parts
            .next()
            .and_then(|value| value.parse().ok())
            .ok_or_else(|| Error::Git(format!("could not parse Git version from '{text}'")))?;
        let patch_part = parts.next().unwrap_or("0");
        let patch_digits: String = patch_part
            .chars()
            .take_while(|character| character.is_ascii_digit())
            .collect();
        let patch = patch_digits.parse().unwrap_or(0);
        let suffix = patch_part
            .get(patch_digits.len()..)
            .unwrap_or_default()
            .to_owned();

        Ok(Self {
            major,
            minor,
            patch,
            suffix,
        })
    }

    pub const fn is_supported(&self) -> bool {
        self.major > MIN_GIT_MAJOR || (self.major == MIN_GIT_MAJOR && self.minor >= MIN_GIT_MINOR)
    }

    pub fn short(&self) -> String {
        format!(
            "{}.{}.{}{}",
            self.major, self.minor, self.patch, self.suffix
        )
    }
}

#[derive(Debug)]
pub struct GitOutput {
    pub status: std::process::ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

pub fn version() -> Result<GitVersion> {
    let output = run(["--version"])?;
    if !output.status.success() {
        return Err(Error::Git(output.stderr.trim().to_owned()));
    }
    GitVersion::parse(&output.stdout)
}

pub fn require_supported() -> Result<GitVersion> {
    let version = version()?;
    if version.is_supported() {
        Ok(version)
    } else {
        Err(Error::UnsupportedGit(version.short()))
    }
}

pub fn run<I, S>(args: I) -> Result<GitOutput>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run_in(None, args)
}

pub fn run_in<I, S>(directory: Option<&Path>, args: I) -> Result<GitOutput>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run_command(directory, args, None)
}

pub fn run_isolated<I, S>(directory: &Path, args: I, empty_config: &Path) -> Result<GitOutput>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run_command(Some(directory), args, Some(empty_config))
}

fn run_command<I, S>(
    directory: Option<&Path>,
    args: I,
    empty_config: Option<&Path>,
) -> Result<GitOutput>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new(git_binary());
    if let Some(empty_config) = empty_config {
        for key in [
            "GIT_DIR",
            "GIT_COMMON_DIR",
            "GIT_WORK_TREE",
            "GIT_INDEX_FILE",
            "GIT_OBJECT_DIRECTORY",
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
            "GIT_CONFIG",
            "GIT_CONFIG_PARAMETERS",
            "GIT_AUTHOR_NAME",
            "GIT_AUTHOR_EMAIL",
            "GIT_AUTHOR_DATE",
            "GIT_COMMITTER_NAME",
            "GIT_COMMITTER_EMAIL",
            "GIT_COMMITTER_DATE",
            "GIT_TEMPLATE_DIR",
            "GIT_NAMESPACE",
            "GIT_SHALLOW_FILE",
            "GIT_GRAFT_FILE",
        ] {
            command.env_remove(key);
        }
        command
            .env("GIT_CONFIG_GLOBAL", empty_config)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_COUNT", "0")
            .env("GIT_TERMINAL_PROMPT", "0")
            .args([
                "-c",
                "init.templateDir=",
                "-c",
                "credential.helper=",
                "-c",
                "credential.https://github.com.helper=!gh auth git-credential",
            ]);
    }
    command.args(args);
    if let Some(directory) = directory {
        command.current_dir(directory);
    }
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let output = command
        .output()
        .map_err(|error| Error::Git(format!("could not start Git: {error}")))?;

    Ok(GitOutput {
        status: output.status,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

pub fn checked<I, S>(args: I) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = run(args)?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        let detail = output.stderr.trim();
        Err(Error::Git(if detail.is_empty() {
            format!("Git exited with {}", output.status)
        } else {
            detail.to_owned()
        }))
    }
}

pub fn repository_root() -> Result<PathBuf> {
    let root = checked(["rev-parse", "--show-toplevel"])?;
    Ok(PathBuf::from(root.trim()))
}

pub fn git_dir() -> Result<PathBuf> {
    let value = checked(["rev-parse", "--git-dir"])?;
    let path = PathBuf::from(value.trim());
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(env::current_dir()
            .map_err(|error| Error::Git(error.to_string()))?
            .join(path))
    }
}

pub fn repository_identity() -> Result<String> {
    let path = git_dir()?;
    Ok(fs::canonicalize(&path)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned())
}

pub fn current_branch() -> Result<String> {
    Ok(checked(["symbolic-ref", "--quiet", "--short", "HEAD"])?
        .trim()
        .to_owned())
}

pub fn config_value(scope: &str, key: &str) -> Result<Option<String>> {
    let output = run(["config", scope, "--get", key])?;
    if output.status.success() {
        Ok(Some(output.stdout.trim().to_owned()))
    } else {
        Ok(None)
    }
}

pub fn config_values(scope: &str, key: &str) -> Result<Vec<String>> {
    let output = run(["config", scope, "--get-all", key])?;
    if output.status.success() {
        Ok(output
            .stdout
            .lines()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect())
    } else {
        Ok(Vec::new())
    }
}

pub fn git_binary() -> String {
    env::var("GITSAMA_GIT_BIN").unwrap_or_else(|_| "git".to_owned())
}

pub fn is_zero_oid(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|character| character == '0')
}

pub fn is_valid_oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::{GitVersion, is_zero_oid};

    #[test]
    fn parses_windows_and_release_versions() {
        let version = GitVersion::parse("git version 2.55.1.windows.1").expect("version");
        assert_eq!(version.short(), "2.55.1.windows.1");
        assert!(version.is_supported());

        let old = GitVersion::parse("git version 2.53.9").expect("version");
        assert!(!old.is_supported());
    }

    #[test]
    fn recognizes_zero_object_ids() {
        assert!(is_zero_oid(&"0".repeat(40)));
        assert!(is_zero_oid(&"0".repeat(64)));
        assert!(!is_zero_oid("0000abc"));
        assert!(!is_zero_oid(""));
    }
}
