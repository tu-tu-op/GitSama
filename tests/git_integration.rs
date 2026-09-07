use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output},
    thread,
    time::Duration,
};

use serde_json::Value;
use tempfile::{TempDir, tempdir};

struct Sandbox {
    _directory: TempDir,
    home: PathBuf,
    global_config: PathBuf,
    log: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        let directory = tempdir().expect("temporary test directory");
        let root = directory.path().to_path_buf();
        Self {
            _directory: directory,
            home: root.join("gitsama-home"),
            global_config: root.join("gitconfig"),
            log: root.join("events.jsonl"),
        }
    }

    fn git(&self, directory: Option<&Path>, args: &[&str]) -> Output {
        let mut command = Command::new("git");
        command.args(args);
        self.apply_env(&mut command);
        command.current_dir(directory.unwrap_or(self._directory.path()));
        command.output().expect("run git")
    }

    fn tool(&self, directory: Option<&Path>, args: &[&str]) -> Output {
        self.tool_path(Path::new(env!("CARGO_BIN_EXE_gitsama")), directory, args)
    }

    fn tool_path(&self, executable: &Path, directory: Option<&Path>, args: &[&str]) -> Output {
        let mut command = Command::new(executable);
        command.args(args);
        self.apply_env(&mut command);
        command.current_dir(directory.unwrap_or(self._directory.path()));
        command.output().expect("run gitsama")
    }

    fn tool_with_input(&self, args: &[&str], input: &str) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_gitsama"));
        command
            .args(args)
            .current_dir(self._directory.path())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        self.apply_env(&mut command);
        let mut child = command.spawn().expect("spawn gitsama");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(input.as_bytes())
            .expect("write hook input");
        child.wait_with_output().expect("wait gitsama")
    }

    fn apply_env(&self, command: &mut Command) {
        let root = self._directory.path();
        command
            .env("GITSAMA_HOME", &self.home)
            .env("GITSAMA_TEST_MODE", "1")
            .env("GITSAMA_NONINTERACTIVE", "1")
            .env("GITSAMA_TEST_LOG", &self.log)
            .env("GIT_CONFIG_GLOBAL", &self.global_config)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_COUNT", "0")
            .env_remove("GIT_CONFIG_PARAMETERS")
            .env_remove("GIT_CONFIG")
            .env_remove("GIT_DIR")
            .env_remove("GIT_COMMON_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("HOME", root)
            .env("USERPROFILE", root)
            .env("XDG_CONFIG_HOME", root.join("xdg"))
            .env("GIT_AUTHOR_NAME", "GitSama Test")
            .env("GIT_AUTHOR_EMAIL", "gitsama-test@example.invalid")
            .env("GIT_COMMITTER_NAME", "GitSama Test")
            .env("GIT_COMMITTER_EMAIL", "gitsama-test@example.invalid");
    }

    fn setup(&self) {
        let output = self.tool(None, &["setup"]);
        assert!(
            output.status.success(),
            "setup failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn init_repo(&self, name: &str) -> PathBuf {
        let path = self._directory.path().join(name);
        let output = self.git(
            None,
            &[
                "init",
                "--quiet",
                "-b",
                "main",
                path.to_str().expect("path"),
            ],
        );
        assert!(
            output.status.success(),
            "init failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        self.git_ok(Some(&path), &["config", "user.name", "GitSama Test"]);
        self.git_ok(
            Some(&path),
            &["config", "user.email", "gitsama-test@example.invalid"],
        );
        path
    }

    fn git_ok(&self, directory: Option<&Path>, args: &[&str]) -> Output {
        let output = self.git(directory, args);
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }

    fn tool_ok(&self, directory: Option<&Path>, args: &[&str]) -> Output {
        let output = self.tool(directory, args);
        assert!(
            output.status.success(),
            "gitsama {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }

    fn commit(&self, repo: &Path, message: &str) {
        self.git_ok(
            Some(repo),
            &["commit", "--quiet", "--allow-empty", "-m", message],
        );
    }

    fn clear_log(&self) {
        let _ = fs::remove_file(&self.log);
    }

    fn records(&self) -> Vec<Value> {
        let Ok(text) = fs::read_to_string(&self.log) else {
            return Vec::new();
        };
        text.lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    }

    fn event_names(&self) -> Vec<String> {
        self.records()
            .iter()
            .filter_map(|record| record["event"].as_str().map(ToOwned::to_owned))
            .collect()
    }

    fn wait_for_events(&self, minimum: usize) -> Vec<String> {
        for _ in 0..20 {
            let events = self.event_names();
            if events.len() >= minimum {
                return events;
            }
            thread::sleep(Duration::from_millis(50));
        }
        self.event_names()
    }
}

fn supported_named_hooks() -> bool {
    let output = Command::new("git")
        .args(["--version"])
        .output()
        .expect("git version");
    let text = String::from_utf8_lossy(&output.stdout);
    let Some(version) = text.split_whitespace().nth(2) else {
        return false;
    };
    let mut parts = version.split('.');
    let major = parts
        .next()
        .and_then(|part| part.parse::<u32>().ok())
        .unwrap_or(0);
    let minor = parts
        .next()
        .and_then(|part| part.parse::<u32>().ok())
        .unwrap_or(0);
    major > 2 || (major == 2 && minor >= 54)
}

fn skip_if_unsupported() -> bool {
    if supported_named_hooks() {
        false
    } else {
        eprintln!("skipping configured-hook integration test: Git 2.54+ is required");
        true
    }
}

#[test]
fn commit_hook_dispatches_one_event() {
    if skip_if_unsupported() {
        return;
    }
    let sandbox = Sandbox::new();
    sandbox.setup();
    let repo = sandbox.init_repo("commit-repo");
    sandbox.commit(&repo, "ignored by test log");
    assert_eq!(sandbox.event_names(), vec!["commit"]);
}

#[test]
fn setup_does_not_change_core_hooks_path() {
    if skip_if_unsupported() {
        return;
    }
    let sandbox = Sandbox::new();
    sandbox.git_ok(
        None,
        &["config", "--global", "core.hooksPath", "custom-hooks"],
    );
    sandbox.setup();
    let output = sandbox.git(None, &["config", "--global", "--get", "core.hooksPath"]);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "custom-hooks"
    );
}

#[test]
fn push_and_massive_push_are_classified() {
    if skip_if_unsupported() {
        return;
    }
    let sandbox = Sandbox::new();
    sandbox.setup();
    let repo = sandbox.init_repo("push-repo");
    let remote = sandbox._directory.path().join("remote.git");
    sandbox.git_ok(
        None,
        &[
            "init",
            "--bare",
            "--quiet",
            remote.to_str().expect("remote"),
        ],
    );
    sandbox.git_ok(
        Some(&repo),
        &["remote", "add", "origin", remote.to_str().expect("remote")],
    );
    sandbox.commit(&repo, "base");
    sandbox.git_ok(Some(&repo), &["push", "--quiet", "-u", "origin", "main"]);
    assert_eq!(sandbox.event_names(), vec!["commit", "push"]);

    sandbox.clear_log();
    sandbox.tool_ok(None, &["threshold", "3"]);
    sandbox.commit(&repo, "one");
    sandbox.commit(&repo, "two");
    sandbox.commit(&repo, "three");
    sandbox.git_ok(Some(&repo), &["push", "--quiet"]);
    assert!(sandbox.event_names().contains(&"massive_push".to_owned()));
}

#[test]
fn multiple_pushed_refs_count_shared_commits_once() {
    if skip_if_unsupported() {
        return;
    }
    let sandbox = Sandbox::new();
    sandbox.setup();
    let repo = sandbox.init_repo("multi-push-repo");
    let remote = sandbox._directory.path().join("multi-push.git");
    sandbox.git_ok(
        None,
        &[
            "init",
            "--bare",
            "--quiet",
            remote.to_str().expect("remote"),
        ],
    );
    sandbox.git_ok(
        Some(&repo),
        &["remote", "add", "origin", remote.to_str().expect("remote")],
    );
    sandbox.commit(&repo, "base");
    sandbox.git_ok(Some(&repo), &["push", "--quiet", "-u", "origin", "main"]);
    sandbox.clear_log();
    sandbox.git_ok(
        Some(&repo),
        &["commit", "--quiet", "--allow-empty", "-m", "main-one"],
    );
    sandbox.git_ok(Some(&repo), &["switch", "--quiet", "-c", "feature"]);
    sandbox.commit(&repo, "feature-one");
    sandbox.git_ok(Some(&repo), &["switch", "--quiet", "main"]);
    sandbox.commit(&repo, "main-two");
    thread::sleep(Duration::from_millis(250));
    sandbox.clear_log();
    sandbox.tool_ok(None, &["threshold", "4"]);
    sandbox.git_ok(
        Some(&repo),
        &["push", "--quiet", "origin", "main", "feature"],
    );
    let records = sandbox.records();
    assert_eq!(sandbox.event_names(), vec!["push"]);
    assert_eq!(records[0]["commit_count"].as_u64(), Some(3));
}

#[test]
fn merge_and_failed_merge_behave_correctly() {
    if skip_if_unsupported() {
        return;
    }
    let sandbox = Sandbox::new();
    sandbox.setup();
    let repo = sandbox.init_repo("merge-repo");
    sandbox.commit(&repo, "base");
    sandbox.git_ok(Some(&repo), &["switch", "--quiet", "-c", "feature"]);
    sandbox.commit(&repo, "feature");
    sandbox.git_ok(Some(&repo), &["switch", "--quiet", "main"]);
    sandbox.clear_log();
    sandbox.git_ok(Some(&repo), &["merge", "--quiet", "--no-ff", "feature"]);
    assert!(sandbox.event_names().contains(&"merge".to_owned()));

    sandbox.git_ok(Some(&repo), &["switch", "--quiet", "-c", "left"]);
    fs::write(repo.join("conflict.txt"), "left\n").expect("left file");
    sandbox.git_ok(Some(&repo), &["add", "conflict.txt"]);
    sandbox.commit(&repo, "left");
    sandbox.git_ok(Some(&repo), &["switch", "--quiet", "main"]);
    sandbox.git_ok(Some(&repo), &["switch", "--quiet", "-c", "right"]);
    fs::write(repo.join("conflict.txt"), "right\n").expect("right file");
    sandbox.git_ok(Some(&repo), &["add", "conflict.txt"]);
    sandbox.commit(&repo, "right");
    sandbox.git_ok(Some(&repo), &["switch", "--quiet", "main"]);
    sandbox.clear_log();
    let failed = sandbox.git(Some(&repo), &["merge", "--no-commit", "right"]);
    assert!(!failed.status.success());
    assert!(!sandbox.event_names().contains(&"merge".to_owned()));
    sandbox.git_ok(Some(&repo), &["merge", "--abort"]);
}

#[test]
fn branch_lifecycle_and_create_switch_deduplicate() {
    if skip_if_unsupported() {
        return;
    }
    let sandbox = Sandbox::new();
    sandbox.setup();
    let repo = sandbox.init_repo("branch-repo");
    sandbox.commit(&repo, "base");

    sandbox.clear_log();
    sandbox.git_ok(Some(&repo), &["branch", "feature"]);
    let events = sandbox.wait_for_events(1);
    assert_eq!(events, vec!["branch_create"]);

    sandbox.clear_log();
    sandbox.git_ok(Some(&repo), &["switch", "--quiet", "feature"]);
    assert_eq!(sandbox.event_names(), vec!["branch_switch"]);

    sandbox.clear_log();
    sandbox.git_ok(Some(&repo), &["switch", "--quiet", "-c", "new-feature"]);
    let events = sandbox.wait_for_events(1);
    assert_eq!(events, vec!["branch_create"]);

    sandbox.clear_log();
    sandbox.git_ok(Some(&repo), &["branch", "-d", "new-feature"]);
    let events = sandbox.wait_for_events(1);
    assert_eq!(events, vec!["branch_delete"]);

    sandbox.clear_log();
    sandbox.git_ok(Some(&repo), &["checkout", "-b", "checkout-feature"]);
    let events = sandbox.wait_for_events(1);
    assert_eq!(events, vec!["branch_create"]);
    assert_eq!(sandbox.event_names(), vec!["branch_create"]);
}

#[test]
fn rebase_fires_and_amend_does_not_fire_rebase() {
    if skip_if_unsupported() {
        return;
    }
    let sandbox = Sandbox::new();
    sandbox.setup();
    let repo = sandbox.init_repo("rewrite-repo");
    sandbox.commit(&repo, "base");
    sandbox.git_ok(Some(&repo), &["switch", "--quiet", "-c", "feature"]);
    sandbox.commit(&repo, "feature");
    sandbox.git_ok(Some(&repo), &["switch", "--quiet", "main"]);
    sandbox.commit(&repo, "main");
    sandbox.git_ok(Some(&repo), &["switch", "--quiet", "feature"]);
    sandbox.clear_log();
    sandbox.git_ok(Some(&repo), &["rebase", "main"]);
    assert!(sandbox.event_names().contains(&"rebase".to_owned()));

    sandbox.clear_log();
    sandbox.git_ok(Some(&repo), &["commit", "--quiet", "--amend", "--no-edit"]);
    assert!(!sandbox.event_names().contains(&"rebase".to_owned()));
}

#[test]
fn clone_initialization_does_not_sound_like_a_switch() {
    if skip_if_unsupported() {
        return;
    }
    let sandbox = Sandbox::new();
    sandbox.setup();
    let source = sandbox.init_repo("clone-source");
    sandbox.commit(&source, "source");
    let clone = sandbox._directory.path().join("clone-destination");
    sandbox.clear_log();
    sandbox.git_ok(
        None,
        &[
            "clone",
            "--quiet",
            source.to_str().expect("source"),
            clone.to_str().expect("clone"),
        ],
    );
    thread::sleep(Duration::from_millis(200));
    assert!(!sandbox.event_names().contains(&"branch_switch".to_owned()));
}

#[test]
fn personal_installations_do_not_cross_talk() {
    if skip_if_unsupported() {
        return;
    }
    let sandbox = Sandbox::new();
    sandbox.setup();
    let remote = sandbox._directory.path().join("shared.git");
    sandbox.git_ok(
        None,
        &[
            "init",
            "--bare",
            "--quiet",
            remote.to_str().expect("remote"),
        ],
    );

    let user_b_root = sandbox._directory.path().join("user-b");
    fs::create_dir_all(&user_b_root).expect("user b");
    let user_b_repo = user_b_root.join("repo");
    let mut init_b = Command::new("git");
    init_b.args([
        "init",
        "--quiet",
        "-b",
        "main",
        user_b_repo.to_str().expect("repo"),
    ]);
    init_b
        .env("GIT_CONFIG_GLOBAL", user_b_root.join("gitconfig"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", &user_b_root)
        .env("USERPROFILE", &user_b_root);
    assert!(init_b.output().expect("init b").status.success());
    let mut remote_b = Command::new("git");
    remote_b.current_dir(&user_b_repo).args([
        "remote",
        "add",
        "origin",
        remote.to_str().expect("remote"),
    ]);
    remote_b
        .env("GIT_CONFIG_GLOBAL", user_b_root.join("gitconfig"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", &user_b_root)
        .env("USERPROFILE", &user_b_root);
    assert!(remote_b.output().expect("remote b").status.success());
    let mut commit_b = Command::new("git");
    commit_b
        .current_dir(&user_b_repo)
        .args(["commit", "--quiet", "--allow-empty", "-m", "user b"]);
    commit_b
        .env("GIT_CONFIG_GLOBAL", user_b_root.join("gitconfig"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", &user_b_root)
        .env("USERPROFILE", &user_b_root)
        .env("GIT_AUTHOR_NAME", "B")
        .env("GIT_AUTHOR_EMAIL", "b@example.invalid")
        .env("GIT_COMMITTER_NAME", "B")
        .env("GIT_COMMITTER_EMAIL", "b@example.invalid");
    assert!(commit_b.output().expect("commit b").status.success());
    let mut push_b = Command::new("git");
    push_b
        .current_dir(&user_b_repo)
        .args(["push", "--quiet", "-u", "origin", "main"]);
    push_b
        .env("GIT_CONFIG_GLOBAL", user_b_root.join("gitconfig"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", &user_b_root)
        .env("USERPROFILE", &user_b_root);
    assert!(push_b.output().expect("push b").status.success());
    assert!(sandbox.event_names().is_empty());
}

#[test]
fn repository_opt_out_is_local_only() {
    if skip_if_unsupported() {
        return;
    }
    let sandbox = Sandbox::new();
    sandbox.setup();
    let repo_a = sandbox.init_repo("repo-a");
    let repo_b = sandbox.init_repo("repo-b");

    sandbox.tool_ok(Some(&repo_a), &["off-here"]);
    sandbox.commit(&repo_a, "muted here");
    assert!(sandbox.event_names().is_empty());

    sandbox.commit(&repo_b, "still on");
    assert_eq!(sandbox.event_names(), vec!["commit"]);

    sandbox.clear_log();
    sandbox.tool_ok(Some(&repo_a), &["on-here"]);
    sandbox.commit(&repo_a, "on again");
    assert_eq!(sandbox.event_names(), vec!["commit"]);
}

#[test]
fn traditional_repository_hooks_still_run() {
    if skip_if_unsupported() {
        return;
    }
    let sandbox = Sandbox::new();
    sandbox.setup();
    let repo = sandbox.init_repo("coexistence-repo");
    let marker = sandbox._directory.path().join("traditional-hook-ran");
    let hook = repo.join(".git/hooks/post-commit");
    let script = format!(
        "#!/bin/sh\nprintf existing >> '{}'\n",
        marker.to_string_lossy()
    );
    fs::write(&hook, script).expect("traditional hook");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(&hook).expect("hook metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&hook, permissions).expect("hook permissions");
    }
    sandbox.commit(&repo, "coexistence");
    assert!(marker.exists());
    assert_eq!(sandbox.event_names(), vec!["commit"]);
}

#[test]
fn configured_hook_commands_work_from_a_path_with_spaces() {
    if skip_if_unsupported() {
        return;
    }
    let sandbox = Sandbox::new();
    let spaced = sandbox._directory.path().join("Git Sama Test").join("bin");
    fs::create_dir_all(&spaced).expect("spaced binary directory");
    let binary_name = if cfg!(windows) {
        "gitsama.exe"
    } else {
        "gitsama"
    };
    let binary = spaced.join(binary_name);
    fs::copy(env!("CARGO_BIN_EXE_gitsama"), &binary).expect("copy binary");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(&binary)
            .expect("binary metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&binary, permissions).expect("binary permissions");
    }
    let setup = sandbox.tool_path(&binary, None, &["setup"]);
    assert!(setup.status.success(), "spaced setup failed");

    let config = sandbox.git(
        None,
        &[
            "config",
            "--global",
            "--get",
            "hook.gitsama-post-commit.command",
        ],
    );
    let command = String::from_utf8_lossy(&config.stdout);
    assert!(command.contains("Git Sama Test"));

    let repo = sandbox.init_repo("spaced-hook-repo");
    sandbox.commit(&repo, "spaced path");
    assert_eq!(sandbox.event_names(), vec!["commit"]);
}

#[test]
fn custom_pack_scaffold_import_select_and_test_workflow() {
    let sandbox = Sandbox::new();
    let parent = sandbox._directory.path().join("pack-work");
    fs::create_dir_all(&parent).expect("pack parent");
    sandbox.tool_ok(None, &["test", "commit"]);
    sandbox.clear_log();

    let scaffold = sandbox.tool(
        None,
        &[
            "pack",
            "scaffold",
            "Custom Pack",
            parent.to_str().expect("parent"),
        ],
    );
    assert!(scaffold.status.success(), "scaffold failed");
    let pack = parent.join("custom-pack");
    let source_audio = sandbox.home.join("packs/starter/audio/commit.wav");
    let custom_audio = pack.join("audio/commit.wav");
    fs::copy(source_audio, custom_audio).expect("copy starter tone");
    let manifest_path = pack.join("pack.toml");
    let manifest = fs::read_to_string(&manifest_path).expect("manifest");
    fs::write(
        &manifest_path,
        manifest.replace("commit = []", "commit = [\"audio/commit.wav\"]"),
    )
    .expect("update manifest");

    sandbox.tool_ok(None, &["pack", "validate", pack.to_str().expect("pack")]);
    sandbox.tool_ok(None, &["pack", "add", pack.to_str().expect("pack")]);
    sandbox.tool_ok(None, &["use", "custom-pack"]);
    sandbox.tool_ok(None, &["test", "commit"]);
    assert_eq!(sandbox.event_names(), vec!["commit"]);
}

#[test]
fn malformed_runtime_state_cannot_fail_a_hook() {
    let sandbox = Sandbox::new();
    fs::create_dir_all(&sandbox.home).expect("home");
    fs::write(sandbox.home.join("config.toml"), "volume = nope\n").expect("bad config");
    let output = sandbox.tool(None, &["hook", "post-commit"]);
    assert!(output.status.success());

    let malformed = sandbox.tool_with_input(
        &["hook", "pre-push", "origin"],
        "this is not a valid pre-push line\n",
    );
    assert!(malformed.status.success());

    let log_directory = sandbox._directory.path().join("log-directory");
    fs::create_dir_all(&log_directory).expect("log directory");
    let mut input = Command::new(env!("CARGO_BIN_EXE_gitsama"));
    input.args(["hook", "pre-push", "origin"]);
    input.stdin(std::process::Stdio::piped());
    sandbox.apply_env(&mut input);
    input.env("GITSAMA_TEST_LOG", &log_directory);
    let mut child = input.spawn().expect("spawn fail-open hook");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"bad input\n")
        .expect("write input");
    assert!(child.wait().expect("wait").success());
}

#[test]
fn missing_pack_and_broken_audio_cannot_fail_a_hook() {
    let sandbox = Sandbox::new();
    fs::create_dir_all(&sandbox.home).expect("home");
    fs::write(
        sandbox.home.join("config.toml"),
        "active_pack = \"missing-pack\"\n",
    )
    .expect("missing pack config");
    let missing = sandbox.tool(None, &["hook", "post-commit"]);
    assert!(missing.status.success());
    assert!(sandbox.event_names().is_empty());

    fs::write(
        sandbox.home.join("config.toml"),
        "active_pack = \"starter\"\n",
    )
    .expect("starter config");
    sandbox.tool_ok(None, &["test", "commit"]);
    sandbox.clear_log();
    let manifest = sandbox.home.join("packs/starter/pack.toml");
    let text = fs::read_to_string(&manifest).expect("starter manifest");
    fs::write(
        &manifest,
        text.replace("audio/commit.wav", "audio/missing.wav"),
    )
    .expect("broken audio mapping");
    let broken = sandbox.tool(None, &["hook", "post-commit"]);
    assert!(broken.status.success());
    assert!(sandbox.event_names().is_empty());
}

#[test]
fn push_count_failure_falls_back_to_a_normal_push_event() {
    let sandbox = Sandbox::new();
    sandbox.tool_ok(None, &["test", "commit"]);
    sandbox.clear_log();
    let line = format!(
        "refs/heads/main {} refs/heads/main {}\n",
        "a".repeat(40),
        "0".repeat(40)
    );
    let output = sandbox.tool_with_input(&["hook", "pre-push", "origin"], &line);
    assert!(output.status.success());
    assert_eq!(sandbox.event_names(), vec!["push"]);
}

#[test]
fn corrupted_branch_state_cannot_fail_checkout_hook() {
    let sandbox = Sandbox::new();
    let repo = sandbox.init_repo("corrupt-state-repo");
    sandbox.commit(&repo, "base");
    let repository = repo
        .join(".git")
        .canonicalize()
        .expect("git directory")
        .to_string_lossy()
        .into_owned();
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in format!("{repository}\0main").as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    fs::create_dir_all(sandbox.home.join("state")).expect("state");
    fs::write(
        sandbox.home.join(format!("state/pending-{hash:016x}.json")),
        "not json",
    )
    .expect("corrupted state");

    let old = "1".repeat(40);
    let new = "2".repeat(40);
    let output = sandbox.tool_path(
        Path::new(env!("CARGO_BIN_EXE_gitsama")),
        Some(&repo),
        &["hook", "post-checkout", old.as_str(), new.as_str(), "1"],
    );
    assert!(output.status.success());
}

#[test]
fn old_git_setup_makes_no_global_hook_changes() {
    if supported_named_hooks() {
        return;
    }
    let sandbox = Sandbox::new();
    let output = sandbox.tool(None, &["setup"]);
    assert!(!output.status.success());
    let message = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(message.contains("GitSama needs Git 2.54 or newer"));
    assert!(!sandbox.global_config.exists());
}
