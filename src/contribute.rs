use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde::Deserialize;

use crate::{
    error::{Error, Result},
    git, pack_state,
    packs::{self, Pack},
    paths::AppPaths,
};

const UPSTREAM: &str = "tu-tu-op/GitSama";

pub struct Metadata {
    pub author: String,
    pub license: String,
    pub source: String,
}

#[derive(Deserialize)]
struct User {
    login: String,
    id: u64,
}

#[derive(Deserialize)]
struct Owner {
    login: String,
}

#[derive(Deserialize)]
struct Parent {
    full_name: String,
}

#[derive(Deserialize)]
struct Repository {
    full_name: String,
    clone_url: String,
    default_branch: String,
    owner: Owner,
    #[serde(default)]
    parent: Option<Parent>,
}

pub fn checked_pack(paths: &AppPaths, id: &str) -> Result<Pack> {
    if packs::is_built_in(id) {
        return Err(Error::Pack(
            "built-in packs cannot be submitted as new contributions".to_owned(),
        ));
    }
    let pack = packs::find(paths, id)?;
    let state = pack_state::load(paths, id)?;
    if state.tested_fingerprint.as_deref() != Some(&pack_state::fingerprint(&pack)?) {
        return Err(Error::Pack(format!(
            "select this pack with gitsama use {id}, then run gitsama test all before contributing; repeat the preview after editing pack files"
        )));
    }
    Ok(pack)
}

pub fn submit(paths: &AppPaths, id: &str, metadata: &Metadata) -> Result<String> {
    git::require_supported()?;
    submit_with(paths, id, metadata, run_gh)
}

fn run_gh(directory: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("gh").args(args).current_dir(directory)
        .env("GH_PROMPT_DISABLED", "1").env_remove("GH_DEBUG")
        .stdin(Stdio::null()).output()
        .map_err(|error| Error::message(format!("could not run GitHub CLI: {error}; install gh and run gh auth login --hostname github.com")))?;
    if !output.status.success() {
        return Err(Error::message(format!(
            "GitHub CLI failed: {}; check gh auth status --hostname github.com",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn parse_json<T: serde::de::DeserializeOwned>(text: &str) -> Result<T> {
    serde_json::from_str(text)
        .map_err(|error| Error::message(format!("invalid GitHub response: {error}")))
}

fn checked_git(directory: &Path, empty_config: &Path, args: &[&str]) -> Result<String> {
    let output = git::run_isolated(directory, args, empty_config)?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(Error::Git(format!(
            "{}: {}",
            args.first().unwrap_or(&"command"),
            output.stderr.trim()
        )))
    }
}

fn submit_with(
    paths: &AppPaths,
    id: &str,
    metadata: &Metadata,
    mut gh: impl FnMut(&Path, &[&str]) -> Result<String>,
) -> Result<String> {
    let mut state = pack_state::load(paths, id)?;
    if let Some(url) = &state.contribution_url {
        return Ok(url.clone());
    }
    let pack = checked_pack(paths, id)?;
    if metadata.author.trim().is_empty()
        || metadata.author.trim().eq_ignore_ascii_case("Your Name")
        || metadata.license.trim().is_empty()
        || metadata
            .license
            .trim()
            .eq_ignore_ascii_case("user-provided")
        || metadata.source.trim().is_empty()
    {
        return Err(Error::Pack("provide --author, --license, and --source with audio source and redistribution permission details".to_owned()));
    }
    let workspace = tempfile::Builder::new()
        .prefix("pack-contribution-")
        .tempdir_in(&paths.state)
        .map_err(|source| Error::WriteFile {
            path: paths.state.clone(),
            source,
        })?;
    let result = prepare_and_submit(paths, workspace.path(), &pack, metadata, &mut gh);
    match result {
        Ok(url) => {
            state.contribution_url = Some(url.clone());
            if let Err(error) = pack_state::save(paths, id, &state) {
                let saved = workspace.keep();
                return Err(Error::message(format!(
                    "pull request created: {url}, but recording it failed: {error}. Prepared work: {}",
                    saved.display()
                )));
            }
            Ok(url)
        }
        Err(error) => {
            let saved = workspace.keep();
            Err(Error::message(format!(
                "{error}\nPrepared work is saved at {}. If RECOVERY.md is present, use its commands to finish this submission; otherwise rerun gitsama pack contribute {id} after correcting the error.",
                saved.display()
            )))
        }
    }
}

fn prepare_and_submit(
    paths: &AppPaths,
    workspace: &Path,
    pack: &Pack,
    metadata: &Metadata,
    gh: &mut impl FnMut(&Path, &[&str]) -> Result<String>,
) -> Result<String> {
    let empty_config = workspace.join("empty.gitconfig");
    fs::write(&empty_config, "").map_err(|source| Error::WriteFile {
        path: empty_config.clone(),
        source,
    })?;
    gh(workspace, &["auth", "status", "--hostname", "github.com"])?;
    let user: User = parse_json(&gh(
        workspace,
        &["api", "user", "--hostname", "github.com"],
    )?)?;
    let upstream: Repository = parse_json(&gh(
        workspace,
        &["api", "repos/tu-tu-op/GitSama", "--hostname", "github.com"],
    )?)?;
    if !upstream.full_name.eq_ignore_ascii_case(UPSTREAM) {
        return Err(Error::message("unexpected upstream repository"));
    }
    let checkout = workspace.join("checkout");
    checked_git(
        workspace,
        &empty_config,
        &[
            "clone",
            "--single-branch",
            "--no-tags",
            "--branch",
            &upstream.default_branch,
            &upstream.clone_url,
            &checkout.to_string_lossy(),
        ],
    )?;
    let packs_root = checkout.join("packs");
    match fs::symlink_metadata(&packs_root) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(Error::Pack(
                "upstream packs/ is not a safe directory".to_owned(),
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(Error::ReadFile {
                path: packs_root.clone(),
                source,
            });
        }
    }
    let target = packs_root.join(&pack.manifest.id);
    match fs::symlink_metadata(&target) {
        Ok(_) => {
            return Err(Error::Pack(format!(
                "pack '{}' already exists upstream; use a new pack id",
                pack.manifest.id
            )));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(Error::ReadFile {
                path: target,
                source,
            });
        }
    }
    let expected = copy_submission(pack, metadata, &target)?;
    // Recheck after copying so a concurrent edit cannot qualify using an old preview.
    checked_pack(paths, &pack.manifest.id)?;

    let head = if user.login.eq_ignore_ascii_case(&upstream.owner.login) {
        upstream.clone_url.clone()
    } else {
        let mut fork: Repository = parse_json(&gh(
            workspace,
            &[
                "api",
                "repos/tu-tu-op/GitSama/forks",
                "--method",
                "POST",
                "--hostname",
                "github.com",
            ],
        )?)?;
        if fork.parent.is_none() {
            fork = parse_json(&gh(
                workspace,
                &[
                    "api",
                    &format!("repos/{}", fork.full_name),
                    "--hostname",
                    "github.com",
                ],
            )?)?;
        }
        if !fork.owner.login.eq_ignore_ascii_case(&user.login)
            || !fork
                .parent
                .as_ref()
                .is_some_and(|parent| parent.full_name.eq_ignore_ascii_case(UPSTREAM))
        {
            return Err(Error::message("GitHub did not return your fork of GitSama"));
        }
        let start = Instant::now();
        while !git::run_isolated(
            workspace,
            ["ls-remote", "--exit-code", &fork.clone_url, "HEAD"],
            &empty_config,
        )?
        .status
        .success()
        {
            if start.elapsed() >= Duration::from_secs(60) {
                return Err(Error::message(
                    "fork is not ready; retry the contribution shortly",
                ));
            }
            thread::sleep(Duration::from_secs(2));
        }
        fork.clone_url
    };
    let suffix = workspace.file_name().unwrap_or_default().to_string_lossy();
    let branch = format!(
        "codex/voicepack-{}-{}",
        pack.manifest.id,
        suffix.trim_start_matches("pack-contribution-")
    );
    checked_git(&checkout, &empty_config, &["checkout", "-b", &branch])?;
    checked_git(
        &checkout,
        &empty_config,
        &["config", "user.name", metadata.author.trim()],
    )?;
    checked_git(
        &checkout,
        &empty_config,
        &[
            "config",
            "user.email",
            &format!("{}+{}@users.noreply.github.com", user.id, user.login),
        ],
    )?;
    checked_git(
        &checkout,
        &empty_config,
        &["config", "credential.helper", ""],
    )?;
    checked_git(
        &checkout,
        &empty_config,
        &[
            "config",
            "credential.https://github.com.helper",
            "!gh auth git-credential",
        ],
    )?;
    checked_git(
        &checkout,
        &empty_config,
        &["remote", "add", "contribution", &head],
    )?;
    let base = checked_git(&checkout, &empty_config, &["rev-parse", "HEAD"])?;
    let prefix = format!("packs/{}", pack.manifest.id);
    checked_git(&checkout, &empty_config, &["add", "--force", "--", &prefix])?;
    verify_diff(
        &checked_git(
            &checkout,
            &empty_config,
            &["diff", "--cached", "--name-status", "-z"],
        )?,
        &expected,
    )?;
    let title = format!("Add voice pack {}", pack.manifest.id);
    checked_git(&checkout, &empty_config, &["commit", "-m", &title])?;
    verify_diff(
        &checked_git(
            &checkout,
            &empty_config,
            &["diff", "--name-status", "-z", base.trim(), "HEAD", "--"],
        )?,
        &expected,
    )?;
    let body = workspace.join("pr-body.md");
    fs::write(&body, format!("Adds {} (`{}`), a data-only voice pack.\n\nAuthor: {}\nLicense: {}\n\nAudio source and redistribution permission:\n{}\n\nValidation: pack validated and `gitsama test all` completed.\n", pack.manifest.name, pack.manifest.id, metadata.author.trim(), metadata.license.trim(), metadata.source.trim()))
        .map_err(|source| Error::WriteFile { path: body.clone(), source })?;
    let pr_head = format!("{}:{branch}", user.login);
    let recovery = format!(
        "# Finish this voice-pack submission\n\nRun these commands in your terminal.\n\n```\ncd {}\ngit push --set-upstream contribution {}\ngh pr create --repo {} --base {} --head {} --title {} --body-file {}\n```\n",
        quote(&checkout.to_string_lossy()),
        quote(&format!("HEAD:refs/heads/{branch}")),
        quote(UPSTREAM),
        quote(&upstream.default_branch),
        quote(&pr_head),
        quote(&title),
        quote(&body.to_string_lossy())
    );
    fs::write(workspace.join("RECOVERY.md"), recovery).map_err(|source| Error::WriteFile {
        path: workspace.join("RECOVERY.md"),
        source,
    })?;
    checked_git(
        &checkout,
        &empty_config,
        &[
            "push",
            "--set-upstream",
            "contribution",
            &format!("HEAD:refs/heads/{branch}"),
        ],
    )?;
    let url = gh(
        &checkout,
        &[
            "pr",
            "create",
            "--repo",
            UPSTREAM,
            "--base",
            &upstream.default_branch,
            "--head",
            &pr_head,
            "--title",
            &title,
            "--body-file",
            &body.to_string_lossy(),
        ],
    )?;
    if !url
        .trim()
        .strip_prefix("https://github.com/tu-tu-op/GitSama/pull/")
        .is_some_and(|number| {
            !number.is_empty() && number.bytes().all(|value| value.is_ascii_digit())
        })
    {
        return Err(Error::message(
            "GitHub CLI did not return a GitSama pull request URL; see RECOVERY.md",
        ));
    }
    Ok(url.trim().to_owned())
}

fn copy_submission(pack: &Pack, metadata: &Metadata, target: &Path) -> Result<BTreeSet<String>> {
    let mut manifest = pack.manifest.clone();
    manifest.author = metadata.author.trim().to_owned();
    manifest.license = metadata.license.trim().to_owned();
    let prefix = format!("packs/{}/", manifest.id);
    let mut expected = BTreeSet::from([format!("{prefix}pack.toml"), format!("{prefix}README.md")]);
    fs::create_dir_all(target).map_err(|source| Error::WriteFile {
        path: target.to_path_buf(),
        source,
    })?;
    for files in manifest.events.values_mut() {
        for relative in files {
            let original = pack.root.join(&*relative);
            let mut components = Vec::new();
            for component in Path::new(relative).components() {
                match component {
                    Component::Normal(value) => {
                        let value = value
                            .to_str()
                            .ok_or_else(|| Error::Pack("audio paths must be UTF-8".to_owned()))?;
                        if value.eq_ignore_ascii_case(".git") || value.contains('\\') {
                            return Err(Error::Pack(
                                "contributions cannot contain Git metadata or nonportable paths"
                                    .to_owned(),
                            ));
                        }
                        components.push(value);
                    }
                    Component::CurDir => {}
                    _ => return Err(Error::Pack("unsafe contribution audio path".to_owned())),
                }
            }
            let normalized = components.join("/");
            let destination = target.join(&normalized);
            if expected.insert(format!("{prefix}{normalized}")) {
                let file = fs::File::open(&original).map_err(|source| Error::ReadFile {
                    path: original.clone(),
                    source,
                })?;
                rodio::Decoder::try_from(file)
                    .map_err(|error| Error::Audio(format!("{}: {error}", original.display())))?;
                fs::create_dir_all(destination.parent().expect("audio parent")).map_err(
                    |source| Error::WriteFile {
                        path: destination.clone(),
                        source,
                    },
                )?;
                fs::copy(&original, &destination).map_err(|source| Error::WriteFile {
                    path: destination,
                    source,
                })?;
            }
            *relative = normalized;
        }
    }
    packs::write_manifest(target, &manifest)?;
    fs::write(target.join("README.md"), format!("# {}\n\n{}\n\nAuthor: {}\nVersion: {}\nLicense: {}\n\n## Audio sources and redistribution permission\n\n{}\n\n## Install\n\n```sh\ngitsama pack add ./{}\ngitsama use {}\ngitsama test all\n```\n", manifest.name, manifest.description, manifest.author, manifest.version, manifest.license, metadata.source.trim(), manifest.id, manifest.id))
        .map_err(|source| Error::WriteFile { path: target.join("README.md"), source })?;
    Pack::load(target)?;
    Ok(expected)
}

fn verify_diff(diff: &str, expected: &BTreeSet<String>) -> Result<()> {
    let fields: Vec<_> = diff.split_terminator('\0').collect();
    let mut found = BTreeSet::new();
    for entry in fields.chunks(2) {
        if entry.len() != 2 || entry[0] != "A" || !expected.contains(entry[1]) {
            return Err(Error::Pack(
                "refusing to push a diff containing changes outside the new voice pack".to_owned(),
            ));
        }
        found.insert(entry[1].to_owned());
    }
    if found != *expected {
        return Err(Error::Pack(
            "contribution diff is missing expected pack files".to_owned(),
        ));
    }
    Ok(())
}

fn quote(value: &str) -> String {
    if cfg!(windows) {
        format!("'{}'", value.replace('\'', "''"))
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    struct Fixture {
        _directory: tempfile::TempDir,
        paths: AppPaths,
        upstream: PathBuf,
        fork: PathBuf,
        empty_config: PathBuf,
        account: String,
        calls: Vec<Vec<String>>,
        fail: &'static str,
    }

    impl Fixture {
        fn new(account: &str) -> Self {
            let directory = tempfile::tempdir().expect("temp");
            let root = directory.path();
            let empty_config = root.join("gitconfig");
            fs::write(&empty_config, "").expect("config");
            let upstream = root.join("upstream");
            checked_git(
                root,
                &empty_config,
                &["init", "-b", "trunk", &upstream.to_string_lossy()],
            )
            .expect("init");
            checked_git(
                &upstream,
                &empty_config,
                &["config", "user.name", "Fixture"],
            )
            .expect("author");
            checked_git(
                &upstream,
                &empty_config,
                &["config", "user.email", "fixture@example.invalid"],
            )
            .expect("email");
            fs::write(upstream.join("README.md"), "Upstream README\n").expect("readme");
            fs::write(upstream.join("unrelated.txt"), "Unrelated upstream file\n").expect("extra");
            fs::create_dir_all(upstream.join("packs/other")).expect("other pack");
            fs::write(upstream.join("packs/other/pack.toml"), "Existing pack\n")
                .expect("other manifest");
            checked_git(&upstream, &empty_config, &["add", "."]).expect("add");
            checked_git(&upstream, &empty_config, &["commit", "-m", "Upstream base"])
                .expect("commit");
            let fork = root.join("fork.git");
            checked_git(
                root,
                &empty_config,
                &[
                    "clone",
                    "--bare",
                    &upstream.to_string_lossy(),
                    &fork.to_string_lossy(),
                ],
            )
            .expect("fork");

            let paths = AppPaths::from_root(root.join("home"));
            packs::ensure_starter(&paths).expect("starter");
            let source = root.join("Local Pack");
            fs::create_dir(&source).expect("source");
            fs::copy(
                paths.packs.join("starter/audio/commit.wav"),
                source.join("commit.wav"),
            )
            .expect("clip");
            fs::write(source.join("private.txt"), "Private source history\n").expect("private");
            checked_git(&source, &empty_config, &["init", "-b", "source-history"])
                .expect("source git");
            checked_git(&source, &empty_config, &["config", "user.name", "Source"])
                .expect("source author");
            checked_git(
                &source,
                &empty_config,
                &["config", "user.email", "source@example.invalid"],
            )
            .expect("source email");
            checked_git(&source, &empty_config, &["add", "."]).expect("source add");
            checked_git(
                &source,
                &empty_config,
                &["commit", "-m", "Private source history"],
            )
            .expect("source commit");
            packs::import_folder(&paths, &source, None).expect("import");
            let pack = packs::find(&paths, "local-pack").expect("pack");
            fs::write(pack.root.join("private.txt"), "Private installed file\n")
                .expect("installed extra");
            fs::copy(
                source.join("commit.wav"),
                pack.root.join("audio/unreferenced.wav"),
            )
            .expect("unused clip");
            let state = pack_state::PackState {
                created_locally: true,
                tested_fingerprint: Some(pack_state::fingerprint(&pack).expect("fingerprint")),
                ..Default::default()
            };
            pack_state::save(&paths, "local-pack", &state).expect("state");
            Self {
                _directory: directory,
                paths,
                upstream,
                fork,
                empty_config,
                account: account.to_owned(),
                calls: Vec::new(),
                fail: "",
            }
        }

        fn git(&self, directory: &Path, args: &[&str]) -> String {
            checked_git(directory, &self.empty_config, args).expect("fixture git")
        }

        fn gh(&mut self, _directory: &Path, args: &[&str]) -> Result<String> {
            self.calls
                .push(args.iter().map(|value| (*value).to_owned()).collect());
            match args {
                ["auth", "status", ..] if self.fail == "auth" => {
                    Err(Error::message("fixture authentication failed"))
                }
                ["auth", "status", ..] => Ok(String::new()),
                ["api", "user", ..] => Ok(json!({"login": self.account, "id": 123}).to_string()),
                ["api", "repos/tu-tu-op/GitSama", ..] => Ok(json!({
                    "full_name": UPSTREAM, "clone_url": self.upstream, "default_branch": "trunk",
                    "owner": {"login": "tu-tu-op"}
                })
                .to_string()),
                ["api", "repos/tu-tu-op/GitSama/forks", ..] => Ok(json!({
                    "full_name": format!("{}/GitSama", self.account), "clone_url": self.fork,
                    "default_branch": "trunk", "owner": {"login": self.account},
                    "parent": {"full_name": UPSTREAM}
                })
                .to_string()),
                ["pr", "create", ..] => {
                    for (flag, expected) in [
                        ("--repo", UPSTREAM),
                        ("--base", "trunk"),
                        ("--title", "Add voice pack local-pack"),
                    ] {
                        let position = args
                            .iter()
                            .position(|value| *value == flag)
                            .expect("explicit PR flag");
                        assert_eq!(args[position + 1], expected);
                    }
                    let position = args
                        .iter()
                        .position(|value| *value == "--head")
                        .expect("head");
                    assert!(
                        args[position + 1]
                            .starts_with(&format!("{}:codex/voicepack-local-pack-", self.account))
                    );
                    let position = args
                        .iter()
                        .position(|value| *value == "--body-file")
                        .expect("body");
                    let body = fs::read_to_string(args[position + 1]).expect("body file");
                    assert!(body.contains("CC0-1.0"));
                    assert!(body.contains("Original recordings, released under CC0"));
                    if self.fail == "pr" {
                        return Err(Error::message("fixture PR creation failed"));
                    }
                    Ok("https://github.com/tu-tu-op/GitSama/pull/123".to_owned())
                }
                _ => panic!("unexpected GitHub arguments: {args:?}"),
            }
        }

        fn submit(&mut self) -> Result<String> {
            let paths = self.paths.clone();
            submit_with(&paths, "local-pack", &metadata(), |directory, args| {
                self.gh(directory, args)
            })
        }

        fn prepared(&self) -> Vec<PathBuf> {
            fs::read_dir(&self.paths.state)
                .expect("state")
                .map(|entry| entry.expect("entry").path())
                .filter(|path| {
                    path.is_dir()
                        && path
                            .file_name()
                            .expect("name")
                            .to_string_lossy()
                            .starts_with("pack-contribution-")
                })
                .collect()
        }

        fn branch(&self, repository: &Path) -> String {
            self.git(
                repository,
                &[
                    "for-each-ref",
                    "--format=%(refname:short)",
                    "refs/heads/codex/",
                ],
            )
            .trim()
            .to_owned()
        }
    }

    fn metadata() -> Metadata {
        Metadata {
            author: "Contributor".to_owned(),
            license: "CC0-1.0".to_owned(),
            source: "Original recordings, released under CC0".to_owned(),
        }
    }

    fn expected() -> BTreeSet<String> {
        [
            "packs/local-pack/pack.toml",
            "packs/local-pack/README.md",
            "packs/local-pack/audio/commit.wav",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    }

    #[test]
    fn fork_submission_contains_only_pack_additions_and_retains_the_pr() {
        let mut fixture = Fixture::new("contributor");
        let base = fixture.git(&fixture.upstream, &["rev-parse", "trunk"]);
        let url = fixture.submit().expect("submit");
        assert_eq!(url, "https://github.com/tu-tu-op/GitSama/pull/123");
        let branch = fixture.branch(&fixture.fork);
        assert!(branch.starts_with("codex/voicepack-local-pack-"));
        let diff = fixture.git(
            &fixture.fork,
            &["diff", "--name-status", "-z", "trunk", &branch, "--"],
        );
        verify_diff(&diff, &expected()).expect("pack-only committed diff");
        assert_eq!(
            fixture.git(&fixture.fork, &["rev-parse", &format!("{branch}^")]),
            base
        );
        assert_eq!(
            fixture.git(&fixture.upstream, &["rev-parse", "trunk"]),
            base
        );
        assert_eq!(
            fixture.git(&fixture.fork, &["show", &format!("{branch}:README.md")]),
            "Upstream README\n"
        );
        let manifest = fixture.git(
            &fixture.fork,
            &["show", &format!("{branch}:packs/local-pack/pack.toml")],
        );
        assert!(manifest.contains("schema_version = 1"));
        assert!(manifest.contains("author = \"Contributor\""));
        assert!(manifest.contains("license = \"CC0-1.0\""));
        let readme = fixture.git(
            &fixture.fork,
            &["show", &format!("{branch}:packs/local-pack/README.md")],
        );
        assert!(readme.contains(&metadata().source));
        assert!(fixture.prepared().is_empty());
        assert_eq!(
            pack_state::load(&fixture.paths, "local-pack")
                .expect("state")
                .contribution_url,
            Some(url.clone())
        );
        let calls = fixture.calls.len();
        assert_eq!(fixture.submit().expect("repeat"), url);
        assert_eq!(fixture.calls.len(), calls);
    }

    #[test]
    fn repository_owner_uses_a_new_upstream_branch() {
        let mut fixture = Fixture::new("tu-tu-op");
        let base = fixture.git(&fixture.upstream, &["rev-parse", "trunk"]);
        fixture.submit().expect("owner submit");
        let branch = fixture.branch(&fixture.upstream);
        verify_diff(
            &fixture.git(
                &fixture.upstream,
                &["diff", "--name-status", "-z", "trunk", &branch, "--"],
            ),
            &expected(),
        )
        .expect("diff");
        assert_eq!(
            fixture.git(&fixture.upstream, &["rev-parse", "trunk"]),
            base
        );
        assert!(
            !fixture
                .calls
                .iter()
                .flatten()
                .any(|arg| arg.ends_with("/forks"))
        );
        assert!(fixture.branch(&fixture.fork).is_empty());
    }

    #[test]
    fn rejects_existing_upstream_ids_before_forking_or_pushing() {
        let mut fixture = Fixture::new("contributor");
        fs::create_dir(fixture.upstream.join("packs/local-pack")).expect("existing");
        fs::write(
            fixture.upstream.join("packs/local-pack/README.md"),
            "Already present",
        )
        .expect("readme");
        fixture.git(&fixture.upstream, &["add", "."]);
        fixture.git(&fixture.upstream, &["commit", "-m", "Existing pack"]);
        let error = fixture.submit().expect_err("existing").to_string();
        assert!(error.contains("already exists upstream"));
        assert!(
            !fixture
                .calls
                .iter()
                .flatten()
                .any(|arg| arg.ends_with("/forks"))
        );
        assert!(fixture.branch(&fixture.fork).is_empty());
    }

    #[test]
    fn preview_and_metadata_failures_do_not_contact_github() {
        let mut fixture = Fixture::new("contributor");
        let mut state = pack_state::load(&fixture.paths, "local-pack").expect("state");
        state.tested_fingerprint = None;
        pack_state::save(&fixture.paths, "local-pack", &state).expect("save");
        assert!(
            fixture
                .submit()
                .expect_err("untested")
                .to_string()
                .contains("gitsama test all")
        );
        let pack = packs::find(&fixture.paths, "local-pack").expect("pack");
        state.tested_fingerprint = Some(pack_state::fingerprint(&pack).expect("fingerprint"));
        pack_state::save(&fixture.paths, "local-pack", &state).expect("save");
        let paths = fixture.paths.clone();
        let mut incomplete = metadata();
        incomplete.source.clear();
        assert!(
            submit_with(&paths, "local-pack", &incomplete, |directory, args| fixture
                .gh(directory, args))
            .is_err()
        );
        fs::write(pack.root.join("audio/commit.wav"), "Changed after preview").expect("edit");
        assert!(
            fixture
                .submit()
                .expect_err("changed")
                .to_string()
                .contains("repeat the preview")
        );
        assert!(checked_pack(&fixture.paths, "starter").is_err());
        assert!(fixture.calls.is_empty());
        assert!(fixture.prepared().is_empty());
    }

    #[test]
    fn authentication_failure_preserves_work_without_pushing() {
        let mut fixture = Fixture::new("contributor");
        fixture.fail = "auth";
        let error = fixture.submit().expect_err("auth").to_string();
        assert!(error.contains("authentication failed"));
        assert!(error.contains("Prepared work is saved"));
        assert_eq!(fixture.prepared().len(), 1);
        assert_eq!(fixture.calls.len(), 1);
        assert!(fixture.branch(&fixture.fork).is_empty());
    }

    #[test]
    fn pr_failure_preserves_the_commit_and_recovery_commands() {
        let mut fixture = Fixture::new("contributor");
        fixture.fail = "pr";
        let error = fixture.submit().expect_err("pr").to_string();
        assert!(error.contains("RECOVERY.md"));
        let prepared = fixture.prepared();
        assert_eq!(prepared.len(), 1);
        let recovery = fs::read_to_string(prepared[0].join("RECOVERY.md")).expect("recovery");
        assert!(recovery.contains("git push --set-upstream contribution"));
        assert!(recovery.contains(
            "gh pr create --repo 'tu-tu-op/GitSama' --base 'trunk' --head 'contributor:"
        ));
        assert!(recovery.contains("--body-file"));
        let branch = fixture.branch(&fixture.fork);
        verify_diff(
            &fixture.git(
                &fixture.fork,
                &["diff", "--name-status", "-z", "trunk", &branch, "--"],
            ),
            &expected(),
        )
        .expect("diff");
        assert!(
            pack_state::load(&fixture.paths, "local-pack")
                .expect("state")
                .contribution_url
                .is_none()
        );
    }

    #[test]
    fn reused_fork_starts_from_the_latest_upstream_commit() {
        let mut fixture = Fixture::new("contributor");
        fs::write(
            fixture.upstream.join("new-upstream.txt"),
            "Fresh upstream change\n",
        )
        .expect("advance");
        fixture.git(&fixture.upstream, &["add", "."]);
        fixture.git(
            &fixture.upstream,
            &["commit", "-m", "Advance upstream beyond fork"],
        );
        let base = fixture.git(&fixture.upstream, &["rev-parse", "HEAD"]);
        fixture.submit().expect("submit to old fork");
        let branch = fixture.branch(&fixture.fork);
        assert_eq!(
            fixture.git(&fixture.fork, &["rev-parse", &format!("{branch}^")]),
            base
        );
        verify_diff(
            &fixture.git(
                &fixture.fork,
                &["diff", "--name-status", "-z", base.trim(), &branch, "--"],
            ),
            &expected(),
        )
        .expect("upstream-based diff");
    }

    #[test]
    fn push_failure_preserves_prepared_work_without_creating_a_pr() {
        let mut fixture = Fixture::new("contributor");
        let hooks = fixture.fork.join("hooks");
        fs::create_dir_all(&hooks).expect("hooks");
        let hook = hooks.join("pre-receive");
        fs::write(&hook, "#!/bin/sh\necho fixture-push-rejected >&2\nexit 1\n")
            .expect("rejecting hook");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).expect("executable hook");
        }
        let error = fixture.submit().expect_err("push rejected").to_string();
        assert!(error.contains("fixture-push-rejected"));
        assert!(error.contains("RECOVERY.md"));
        assert!(fixture.prepared()[0].join("RECOVERY.md").exists());
        assert!(
            !fixture
                .calls
                .iter()
                .any(|args| args.first().is_some_and(|arg| arg == "pr"))
        );
        assert!(fixture.branch(&fixture.fork).is_empty());
    }

    #[test]
    fn diff_verification_rejects_updates_unrelated_files_and_extra_pack_data() {
        let expected = expected();
        for diff in [
            "M\0packs/local-pack/README.md\0",
            "A\0private.txt\0",
            "A\0packs/other/audio/commit.wav\0",
            "A\0packs/local-pack/private.txt\0",
            "A\0packs/local-pack/README.md\0",
            "R100\0private.txt\0packs/local-pack/README.md\0",
        ] {
            assert!(verify_diff(diff, &expected).is_err(), "{diff:?}");
        }
    }

    #[test]
    fn export_rejects_referenced_git_metadata_and_invalid_audio() {
        let fixture = Fixture::new("contributor");
        let mut pack = packs::find(&fixture.paths, "local-pack").expect("pack");
        fs::create_dir(pack.root.join(".git")).expect("metadata");
        fs::copy(
            pack.root.join("audio/commit.wav"),
            pack.root.join(".git/private.wav"),
        )
        .expect("clip");
        pack.manifest
            .events
            .insert("commit".to_owned(), vec![".git/private.wav".to_owned()]);
        assert!(
            copy_submission(
                &pack,
                &metadata(),
                &fixture._directory.path().join("unsafe-export")
            )
            .is_err()
        );
        pack.manifest
            .events
            .insert("commit".to_owned(), vec!["audio/commit.wav".to_owned()]);
        fs::write(pack.root.join("audio/commit.wav"), "Corrupt audio").expect("corrupt");
        assert!(
            copy_submission(
                &pack,
                &metadata(),
                &fixture._directory.path().join("broken-export")
            )
            .is_err()
        );
    }
}
