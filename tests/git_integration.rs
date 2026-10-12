use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output},
    thread,
    time::{Duration, Instant},
};

use serde_json::Value;
use tempfile::{TempDir, tempdir};

const NATIVE_HOOKS: [&str; 6] = [
    "post-commit",
    "pre-push",
    "post-merge",
    "post-checkout",
    "reference-transaction",
    "post-rewrite",
];

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
            .env_remove("GIT_REFLOG_ACTION")
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

    fn bare_remote(&self, name: &str) -> PathBuf {
        let remote = self._directory.path().join(name);
        self.git_ok(
            None,
            &[
                "init",
                "--bare",
                "--quiet",
                "-b",
                "main",
                remote.to_str().expect("remote"),
            ],
        );
        // A local transport launches receive-pack under the pushing user's
        // environment. Model a separate remote account without GitSama, so
        // its ref updates don't contaminate client-side push assertions.
        for hook in NATIVE_HOOKS {
            self.git_ok(
                Some(&remote),
                &[
                    "config",
                    "--local",
                    &format!("hook.gitsama-{hook}.enabled"),
                    "false",
                ],
            );
        }
        remote
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

    fn commit_file(&self, repo: &Path, file: &str, contents: &str) {
        fs::write(repo.join(file), contents).expect("write fixture file");
        self.git_ok(Some(repo), &["add", file]);
        self.commit(repo, file);
    }

    fn pull_pair(&self, name: &str) -> (PathBuf, PathBuf) {
        let source = self.init_repo(&format!("{name}-source"));
        self.tool_ok(Some(&source), &["off-here"]);
        self.commit_file(&source, "base.txt", "base\n");
        let client = self._directory.path().join(format!("{name}-client"));
        self.git_ok(
            None,
            &[
                "clone",
                "--quiet",
                source.to_str().expect("source path"),
                client.to_str().expect("client path"),
            ],
        );
        (source, client)
    }

    fn clear_log(&self) {
        // Detached branch workers must finish before the next action starts.
        self.wait_for_events(0);
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
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut quiet_since = Instant::now();
        let mut previous = self.event_names();
        while Instant::now() < deadline {
            let events = self.event_names();
            if events != previous {
                quiet_since = Instant::now();
                previous.clone_from(&events);
            }
            // Cover the detached dispatch delay and observe a quiet period,
            // so exact-count assertions also catch late duplicate events.
            if events.len() >= minimum && quiet_since.elapsed() >= Duration::from_millis(750) {
                return events;
            }
            thread::sleep(Duration::from_millis(25));
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
        assert!(
            std::env::var("GITSAMA_REQUIRE_GIT_254").as_deref() != Ok("1"),
            "Git 2.54+ is required for CI: configured-hook integration tests must not be skipped"
        );
        eprintln!("skipping configured-hook integration test: Git 2.54+ is required");
        true
    }
}

#[test]
fn ci_has_supported_git_for_configured_hooks() {
    if std::env::var("GITSAMA_REQUIRE_GIT_254").as_deref() == Ok("1") {
        assert!(supported_named_hooks(), "CI requires Git 2.54 or newer");
    }
}

#[test]
fn hook_logs_never_include_remote_credentials() {
    let sandbox = Sandbox::new();
    let secret = "GITSAMA_SYNTHETIC_SECRET";
    let output = sandbox.tool(
        None,
        &[
            "hook",
            "pre-push",
            "origin",
            &format!("https://audit:{secret}@example.invalid/repo.git"),
        ],
    );
    assert!(output.status.success());
    let log = fs::read_to_string(sandbox.home.join("logs/gitsama.log")).expect("diagnostic log");
    assert!(log.contains("hook entered: pre-push"));
    assert!(!log.contains(secret));
    assert!(!log.contains("example.invalid"));
}

#[test]
fn uninstall_requires_consent_and_preserves_unrelated_data() {
    let sandbox = Sandbox::new();
    sandbox.setup();
    let denied = sandbox.tool(None, &["uninstall"]);
    assert!(!denied.status.success());
    assert!(String::from_utf8_lossy(&denied.stderr).contains("--yes"));
    assert!(sandbox.home.join("config.toml").exists());
    fs::write(sandbox.home.join("private.txt"), "must survive").expect("extra data");
    let denied = sandbox.tool(None, &["uninstall", "--yes"]);
    assert!(!denied.status.success());
    assert!(sandbox.home.join("private.txt").exists());
    sandbox.tool_ok(None, &["uninstall", "--keep-data", "--yes"]);
    assert!(sandbox.home.join("private.txt").exists());
    assert!(sandbox.home.join("config.toml").exists());
}

#[test]
fn managed_uninstall_handles_literal_percent_paths_without_a_shell() {
    let sandbox = Sandbox::new();
    sandbox.setup();
    let executable = sandbox.home.join("bin").join(if cfg!(windows) {
        "Git Sama %GITSAMA_AUDIT_VALUE%.exe"
    } else {
        "Git Sama %GITSAMA_AUDIT_VALUE%"
    });
    fs::copy(env!("CARGO_BIN_EXE_gitsama"), &executable).expect("managed binary");
    let output = sandbox.tool_path(&executable, None, &["uninstall", "--yes"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!sandbox.home.exists(), "owned data root should be removed");
    assert!(
        Path::new(env!("CARGO_BIN_EXE_gitsama")).exists(),
        "test build must survive"
    );
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
    assert_eq!(sandbox.wait_for_events(1), vec!["commit"]);
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
    let remote = sandbox.bare_remote("remote.git");
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
    let remote = sandbox.bare_remote("multi-push.git");
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
    sandbox.git_ok(Some(&repo), &["switch", "--quiet", "left"]);
    sandbox.clear_log();
    let failed = sandbox.git(Some(&repo), &["merge", "--no-commit", "right"]);
    assert!(!failed.status.success());
    let unmerged = sandbox.git_ok(Some(&repo), &["ls-files", "--unmerged"]);
    assert!(String::from_utf8_lossy(&unmerged.stdout).contains("conflict.txt"));
    assert!(!sandbox.event_names().contains(&"merge".to_owned()));
    sandbox.git_ok(Some(&repo), &["merge", "--abort"]);
}

#[test]
fn pull_plays_once_only_when_the_checked_out_branch_changes() {
    if skip_if_unsupported() {
        return;
    }
    let sandbox = Sandbox::new();
    sandbox.setup();
    for (name, strategy, diverge) in [
        ("fast-forward", "--ff-only", false),
        ("merge", "--no-rebase", true),
        ("rebase-fast-forward", "--rebase", false),
        ("rebase", "--rebase", true),
    ] {
        let (source, client) = sandbox.pull_pair(name);
        if name == "fast-forward" {
            // The full branch ref must work even when a tag has the same name.
            sandbox.git_ok(Some(&client), &["tag", "main"]);
        }
        if diverge {
            sandbox.commit_file(&client, "local-1.txt", "local 1\n");
            sandbox.commit_file(&client, "local-2.txt", "local 2\n");
        }
        sandbox.commit_file(&source, "remote.txt", "remote\n");
        sandbox.clear_log();
        sandbox.git_ok(Some(&client), &["pull", strategy]);
        assert_eq!(sandbox.wait_for_events(1), vec!["pull"], "{name}");
        assert_eq!(
            fs::read_to_string(client.join("remote.txt")).unwrap(),
            "remote\n"
        );
        assert_eq!(sandbox.records()[0]["branch"].as_str(), Some("main"));

        sandbox.clear_log();
        sandbox.git_ok(Some(&client), &["pull", strategy]);
        assert!(sandbox.wait_for_events(0).is_empty(), "up-to-date {name}");
    }
}

#[test]
fn pull_failures_and_remote_tracking_updates_stay_silent() {
    if skip_if_unsupported() {
        return;
    }
    let sandbox = Sandbox::new();
    sandbox.setup();
    let (source, client) = sandbox.pull_pair("failed-pull");
    sandbox.commit_file(&client, "conflict.txt", "local\n");
    sandbox.commit_file(&source, "conflict.txt", "remote\n");
    let head = sandbox
        .git_ok(Some(&client), &["rev-parse", "refs/heads/main"])
        .stdout;

    for strategy in ["--ff-only", "--no-rebase", "--rebase"] {
        sandbox.clear_log();
        let failed = sandbox.git(Some(&client), &["pull", strategy]);
        assert!(!failed.status.success(), "{strategy}");
        assert!(sandbox.wait_for_events(0).is_empty(), "{strategy}");
        assert_eq!(
            sandbox
                .git_ok(Some(&client), &["rev-parse", "refs/heads/main"])
                .stdout,
            head
        );
        match strategy {
            "--no-rebase" => {
                sandbox.git_ok(Some(&client), &["merge", "--abort"]);
            }
            "--rebase" => {
                sandbox.git_ok(Some(&client), &["rebase", "--abort"]);
            }
            _ => {}
        }
    }

    sandbox.commit_file(&source, "another-remote.txt", "remote\n");
    sandbox.clear_log();
    sandbox.git_ok(Some(&client), &["pull", "--dry-run"]);
    assert!(sandbox.wait_for_events(0).is_empty());
    sandbox.git_ok(Some(&client), &["fetch", "origin"]);
    assert!(sandbox.wait_for_events(0).is_empty());
}

#[test]
fn pull_controls_and_legacy_pack_fallback_are_independent() {
    if skip_if_unsupported() {
        return;
    }
    let sandbox = Sandbox::new();
    sandbox.setup();
    let (source, client) = sandbox.pull_pair("pull-controls");
    let manifest = sandbox.home.join("packs/naruto/pack.toml");
    let text = fs::read_to_string(&manifest).expect("manifest");
    let mut legacy: toml::Value = toml::from_str(&text).expect("manifest TOML");
    legacy["events"]
        .as_table_mut()
        .expect("events")
        .remove("pull");
    fs::write(&manifest, toml::to_string_pretty(&legacy).unwrap()).expect("legacy pack");
    for event in ["merge", "rebase", "commit"] {
        sandbox.tool_ok(None, &["disable", event]);
    }
    let preview = sandbox.tool_ok(None, &["test", "pull"]);
    assert!(String::from_utf8_lossy(&preview.stdout).contains("fallback to Merge"));
    sandbox.commit_file(&source, "remote.txt", "remote\n");
    sandbox.clear_log();
    sandbox.git_ok(Some(&client), &["pull", "--ff-only"]);
    assert_eq!(sandbox.wait_for_events(1), vec!["pull"]);

    sandbox.tool_ok(None, &["disable", "pull"]);
    sandbox.commit_file(&source, "remote.txt", "new remote\n");
    sandbox.clear_log();
    sandbox.git_ok(Some(&client), &["pull", "--ff-only"]);
    assert!(sandbox.wait_for_events(0).is_empty());
    assert_eq!(
        fs::read_to_string(client.join("remote.txt")).unwrap(),
        "new remote\n"
    );
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
    sandbox.git_ok(Some(&repo), &["switch", "--quiet", "main"]);
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
    sandbox.commit_file(&repo, "base.txt", "base\n");
    sandbox.git_ok(Some(&repo), &["switch", "--quiet", "-c", "feature"]);
    sandbox.commit_file(&repo, "feature.txt", "feature\n");
    sandbox.git_ok(Some(&repo), &["switch", "--quiet", "main"]);
    sandbox.commit_file(&repo, "main.txt", "main\n");
    sandbox.git_ok(Some(&repo), &["switch", "--quiet", "feature"]);
    sandbox.clear_log();
    sandbox.git_ok(Some(&repo), &["rebase", "main"]);
    let events = sandbox.wait_for_events(1);
    assert_eq!(events.iter().filter(|event| *event == "rebase").count(), 1);

    sandbox.clear_log();
    sandbox.git_ok(Some(&repo), &["commit", "--quiet", "--amend", "--no-edit"]);
    assert_eq!(sandbox.wait_for_events(1), vec!["commit"]);
}

#[test]
fn separate_create_and_switch_commands_keep_their_own_events() {
    if skip_if_unsupported() {
        return;
    }
    let sandbox = Sandbox::new();
    sandbox.setup();
    let repo = sandbox.init_repo("separate-branch-actions");
    sandbox.commit(&repo, "base");
    sandbox.clear_log();
    sandbox.git_ok(Some(&repo), &["branch", "feature"]);
    // No delay: this must not consume the preceding Git process's event.
    sandbox.git_ok(Some(&repo), &["switch", "--quiet", "feature"]);
    let mut events = sandbox.wait_for_events(2);
    events.sort();
    assert_eq!(events, vec!["branch_create", "branch_switch"]);
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
    let remote = sandbox.bare_remote("shared.git");

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
            "--template",
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
fn scaffold_installs_and_selects_clips_from_the_current_folder() {
    let sandbox = Sandbox::new();
    sandbox.tool_ok(None, &["test", "commit"]);
    sandbox.clear_log();
    let source = sandbox._directory.path().join("Bleach Custom");
    fs::create_dir_all(source.join("audio")).expect("source");
    fs::create_dir(source.join(".git")).expect("unrelated history");
    let tone = sandbox.home.join("packs/starter/audio/commit.wav");
    fs::copy(&tone, source.join("commit.wav")).expect("clip");
    fs::copy(&tone, source.join("audio/push.wav")).expect("clip");
    fs::write(source.join("private.txt"), "private").expect("extra");
    let output = sandbox.tool(Some(&source), &["pack", "scaffold"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("Using Bleach Custom (bleach-custom)"));
    assert!(text.contains("falls back to Push"));
    assert!(text.contains("Merge: no sound"));
    assert!(text.contains("gitsama test all"));
    let installed = sandbox.home.join("packs/bleach-custom");
    assert!(installed.join("audio/commit.wav").exists());
    assert!(installed.join("audio/push.wav").exists());
    assert!(!installed.join("private.txt").exists());
    assert!(!installed.join(".git").exists());
    assert!(!source.join("pack.toml").exists());
    let config = fs::read_to_string(sandbox.home.join("config.toml")).expect("config");
    assert!(config.contains("active_pack = \"bleach-custom\""));
    sandbox.tool_ok(
        None,
        &["pack", "validate", installed.to_str().expect("path")],
    );
    sandbox.tool_ok(None, &["test", "all"]);
    assert_eq!(sandbox.event_names(), vec!["commit", "push", "push"]);
    let rename = sandbox.tool(Some(&source), &["pack", "scaffold", "Bankai Pack"]);
    assert!(
        rename.status.success(),
        "{}",
        String::from_utf8_lossy(&rename.stderr)
    );
    assert!(sandbox.home.join("packs/bankai-pack/pack.toml").exists());
}

#[test]
fn full_preview_records_only_completed_playback_and_removal_clears_state() {
    let sandbox = Sandbox::new();
    sandbox.tool_ok(None, &["test", "commit"]);
    let source = sandbox._directory.path().join("Preview Pack");
    fs::create_dir(&source).expect("source");
    fs::copy(
        sandbox.home.join("packs/starter/audio/commit.wav"),
        source.join("commit.wav"),
    )
    .expect("tone");
    sandbox.tool_ok(Some(&source), &["pack", "scaffold"]);
    let state_path = sandbox.home.join("state/pack-preview-pack.json");
    let state = || -> Value {
        serde_json::from_slice(&fs::read(&state_path).expect("state")).expect("json")
    };
    assert!(state()["created_locally"].as_bool().expect("created"));
    assert!(state()["tested_fingerprint"].is_null());
    sandbox.tool_ok(None, &["test", "commit"]);
    assert!(state()["tested_fingerprint"].is_null());
    let preview = sandbox.tool(None, &["test", "all"]);
    assert!(preview.status.success());
    let text = String::from_utf8_lossy(&preview.stdout);
    assert!(text.contains("Played Commit: commit.wav"));
    assert!(text.contains("Skipped Push: no sound mapped"));
    assert!(text.contains("Full preview completed"));
    assert!(state()["tested_fingerprint"].is_string());
    sandbox.tool_ok(None, &["mute"]);
    let muted = sandbox.tool(None, &["test", "all"]);
    assert!(String::from_utf8_lossy(&muted.stdout).contains("GitSama is muted"));
    assert!(state()["tested_fingerprint"].is_null());
    sandbox.tool_ok(None, &["unmute"]);
    sandbox.tool_ok(None, &["volume", "0"]);
    let silent = sandbox.tool(None, &["test", "all"]);
    assert!(String::from_utf8_lossy(&silent.stdout).contains("volume is zero"));
    assert!(state()["tested_fingerprint"].is_null());
    sandbox.tool_ok(None, &["volume", "80"]);
    let config_path = sandbox.home.join("config.toml");
    let config = fs::read_to_string(&config_path).expect("config");
    fs::write(
        &config_path,
        config.replace("queue_max_age_ms = 5000", "queue_max_age_ms = 1"),
    )
    .expect("queue cap");
    let lock = sandbox.home.join("state/playback.lock");
    fs::create_dir(&lock).expect("lock");
    fs::write(lock.join("owner_pid"), std::process::id().to_string()).expect("owner");
    let heartbeat = fs::File::create(lock.join("heartbeat")).expect("heartbeat");
    heartbeat
        .set_times(
            fs::FileTimes::new()
                .set_modified(std::time::SystemTime::now() + Duration::from_secs(60)),
        )
        .expect("fresh heartbeat");
    drop(heartbeat);
    let busy = sandbox.tool(None, &["test", "all"]);
    assert!(String::from_utf8_lossy(&busy.stdout).contains("playback queue timed out"));
    assert!(state()["tested_fingerprint"].is_null());
    fs::remove_dir_all(lock).expect("unlock");
    fs::write(&config_path, config).expect("restore config");
    let installed_clip = sandbox.home.join("packs/preview-pack/audio/commit.wav");
    fs::write(&installed_clip, b"corrupt audio").expect("corrupt clip");
    let broken = sandbox.tool(None, &["test", "all"]);
    assert!(!broken.status.success());
    assert!(state()["tested_fingerprint"].is_null());
    fs::copy(source.join("commit.wav"), installed_clip).expect("repair clip");
    sandbox.tool_ok(None, &["test", "all"]);
    assert!(state()["tested_fingerprint"].is_string());
    sandbox.tool_ok(None, &["pack", "remove", "preview-pack"]);
    assert!(!state_path.exists());
}

#[test]
fn contribution_invitation_requires_a_full_preview_and_appears_once_per_installation() {
    let sandbox = Sandbox::new();
    let built_in = sandbox.tool_ok(None, &["test", "all"]);
    assert!(!String::from_utf8_lossy(&built_in.stdout).contains("pack contribute"));
    let source = sandbox._directory.path().join("Invitations");
    fs::create_dir(&source).expect("source");
    let tone = sandbox.home.join("packs/starter/audio/push.wav");
    fs::copy(&tone, source.join("push.wav")).expect("clip");
    sandbox.tool_ok(Some(&source), &["pack", "scaffold"]);
    let single = sandbox.tool_ok(None, &["test", "push"]);
    assert!(!String::from_utf8_lossy(&single.stdout).contains("pack contribute"));
    let untested = sandbox.tool(None, &["pack", "contribute", "invitations"]);
    assert!(!untested.status.success());
    assert!(String::from_utf8_lossy(&untested.stderr).contains("gitsama test all"));
    sandbox.tool_ok(None, &["mute"]);
    let muted = sandbox.tool_ok(None, &["test", "all"]);
    assert!(!String::from_utf8_lossy(&muted.stdout).contains("pack contribute"));
    sandbox.tool_ok(None, &["unmute"]);
    sandbox.tool_ok(None, &["volume", "0"]);
    let silent = sandbox.tool_ok(None, &["test", "all"]);
    assert!(!String::from_utf8_lossy(&silent.stdout).contains("pack contribute"));
    sandbox.tool_ok(None, &["volume", "80"]);
    let installed = sandbox.home.join("packs/invitations/audio/push.wav");
    fs::write(&installed, b"corrupt").expect("corrupt");
    let failed = sandbox.tool(None, &["test", "all"]);
    assert!(!failed.status.success());
    assert!(!String::from_utf8_lossy(&failed.stdout).contains("pack contribute"));
    fs::copy(&tone, &installed).expect("restore");
    let full = sandbox.tool_ok(None, &["test", "all"]);
    let text = String::from_utf8_lossy(&full.stdout);
    assert!(text.contains("fallback to Push"));
    assert_eq!(
        text.matches("gitsama pack contribute invitations").count(),
        1
    );
    let repeat = sandbox.tool_ok(None, &["test", "all"]);
    assert!(!String::from_utf8_lossy(&repeat.stdout).contains("pack contribute"));
    let missing = sandbox.tool(None, &["pack", "contribute", "invitations"]);
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("--author"));
    let missing = sandbox.tool(
        None,
        &[
            "pack",
            "contribute",
            "invitations",
            "--author",
            "Contributor",
            "--license",
            "CC0-1.0",
        ],
    );
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("--source"));
    sandbox.tool_ok(None, &["pack", "remove", "invitations"]);
    sandbox.tool_ok(Some(&source), &["pack", "scaffold"]);
    let reinstalled = sandbox.tool_ok(None, &["test", "all"]);
    assert!(String::from_utf8_lossy(&reinstalled.stdout).contains("pack contribute invitations"));
}

#[test]
fn delete_any_pack_preserves_sources_and_remembers_builtin_deletions() {
    let sandbox = Sandbox::new();
    sandbox.tool_ok(None, &["test", "commit"]);
    let source = sandbox._directory.path().join("Delete Pack");
    fs::create_dir(&source).expect("source");
    fs::copy(
        sandbox.home.join("packs/starter/audio/commit.wav"),
        source.join("commit.wav"),
    )
    .expect("source clip");
    sandbox.tool_ok(Some(&source), &["pack", "scaffold"]);
    sandbox.tool_ok(None, &["test", "all"]);
    let deleted = sandbox.tool_ok(None, &["pack", "delete", "delete-pack"]);
    assert!(String::from_utf8_lossy(&deleted.stdout).contains("Using naruto"));
    assert!(!sandbox.home.join("packs/delete-pack").exists());
    assert!(!sandbox.home.join("state/pack-delete-pack.json").exists());
    assert!(source.join("commit.wav").exists());
    sandbox.tool_ok(None, &["pack", "remove", "starter"]);
    assert!(!sandbox.home.join("packs/starter").exists());
    for id in ["bleach", "ravi-kishan"] {
        sandbox.tool_ok(None, &["pack", "delete", id]);
        assert!(!sandbox.home.join(format!("packs/{id}")).exists());
    }
    let last = sandbox.tool_ok(None, &["pack", "delete", "naruto"]);
    assert!(
        String::from_utf8_lossy(&last.stdout).contains("No voice packs remain; playback muted")
    );
    let config = || -> toml::Value {
        toml::from_str(&fs::read_to_string(sandbox.home.join("config.toml")).expect("config"))
            .expect("toml")
    };
    assert_eq!(config()["active_pack"].as_str(), Some(""));
    assert_eq!(config()["enabled"].as_bool(), Some(false));
    assert_eq!(
        config()["removed_builtin_packs"]
            .as_array()
            .expect("removed")
            .len(),
        4
    );
    let list = sandbox.tool_ok(None, &["pack", "list"]);
    assert!(String::from_utf8_lossy(&list.stdout).trim().is_empty());
    let preview = sandbox.tool(None, &["test", "all"]);
    assert!(!preview.status.success());
    assert!(String::from_utf8_lossy(&preview.stderr).contains("no voice pack selected"));
    let status = sandbox.tool_ok(None, &["status"]);
    assert!(String::from_utf8_lossy(&status.stdout).contains("Active pack  None"));
    sandbox.tool_ok(None, &["doctor", "--fix"]);
    sandbox.tool_ok(None, &["setup"]);
    assert!(!sandbox.home.join("packs/naruto").exists());
    assert!(!sandbox.home.join("packs/starter").exists());
    assert!(!sandbox.home.join("packs/bleach").exists());
    assert!(!sandbox.home.join("packs/ravi-kishan").exists());
    sandbox.tool_ok(None, &["use", "naruto"]);
    assert!(sandbox.home.join("packs/naruto/pack.toml").exists());
    assert!(!sandbox.home.join("packs/starter").exists());
    assert_eq!(config()["active_pack"].as_str(), Some("naruto"));
    assert_eq!(config()["enabled"].as_bool(), Some(true));
    sandbox.tool_ok(None, &["test", "commit"]);
    sandbox.tool_ok(None, &["pack", "delete", "naruto"]);
    sandbox.tool_ok(Some(&source), &["pack", "scaffold", "Fresh Pack"]);
    assert_eq!(config()["active_pack"].as_str(), Some("fresh-pack"));
    assert_eq!(config()["enabled"].as_bool(), Some(true));
    sandbox.tool_ok(None, &["test", "all"]);
    sandbox.tool_ok(None, &["pack", "delete", "fresh-pack"]);
    sandbox.tool_ok(None, &["use", "starter"]);
    assert!(sandbox.home.join("packs/starter/pack.toml").exists());
    assert!(!sandbox.home.join("packs/naruto").exists());
    for id in ["bleach", "ravi-kishan"] {
        sandbox.tool_ok(None, &["use", id]);
        assert!(sandbox.home.join(format!("packs/{id}/pack.toml")).exists());
        assert_eq!(config()["active_pack"].as_str(), Some(id));
        sandbox.tool_ok(None, &["pack", "delete", id]);
        assert!(!sandbox.home.join(format!("packs/{id}")).exists());
    }
}

#[test]
fn delete_rejects_unsafe_or_missing_ids_and_can_remove_a_broken_pack() {
    let sandbox = Sandbox::new();
    sandbox.tool_ok(None, &["test", "commit"]);
    for id in ["../naruto", "naruto/..", "Naruto", "missing-pack"] {
        let output = sandbox.tool(None, &["pack", "delete", id]);
        assert!(!output.status.success(), "{id}");
        assert!(sandbox.home.join("packs/naruto/pack.toml").exists());
    }
    let broken = sandbox.home.join("packs/broken");
    fs::create_dir(&broken).expect("broken pack");
    fs::write(broken.join("pack.toml"), "invalid TOML").expect("manifest");
    sandbox.tool_ok(None, &["pack", "delete", "broken"]);
    assert!(!broken.exists());
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

#[test]
fn fallback_chain_forces_tier3_linger_mode() {
    if !supported_named_hooks() {
        return;
    }
    let sandbox = Sandbox::new();
    sandbox.setup();
    let repo = sandbox.init_repo("tier3-repo");

    let mut command = Command::new("git");
    command.args(["commit", "--quiet", "--allow-empty", "-m", "tier3 test"]);
    sandbox.apply_env(&mut command);
    command.env("GITSAMA_FORCE_TIER", "3");
    command.current_dir(&repo);
    let output = command.output().expect("run git commit");
    assert!(output.status.success());

    let events = sandbox.wait_for_events(1);
    assert_eq!(events, vec!["commit"]);
}

#[test]
fn fallback_chain_tier1_failure_falls_back_to_tier3() {
    if !supported_named_hooks() {
        return;
    }
    let sandbox = Sandbox::new();
    sandbox.setup();
    let repo = sandbox.init_repo("fallback-repo");

    let config_path = sandbox.home.join("config.toml");
    let mut config_text = fs::read_to_string(&config_path).unwrap_or_default();
    config_text.push_str("\nplayback_mode = 'linger'\nlinger_cap_secs = 2.0\n");
    fs::write(&config_path, config_text).expect("write config");

    sandbox.commit(&repo, "linger commit");
    let events = sandbox.wait_for_events(1);
    assert_eq!(events, vec!["commit"]);
}

#[test]
fn dead_process_lock_is_reclaimed_by_hook() {
    if !supported_named_hooks() {
        return;
    }
    let sandbox = Sandbox::new();
    sandbox.setup();
    let repo = sandbox.init_repo("lock-recovery-repo");

    let lock_dir = sandbox.home.join("state").join("playback.lock");
    fs::create_dir_all(&lock_dir).expect("lock dir");
    fs::write(lock_dir.join("owner_pid"), "999999999").expect("write dead pid");
    fs::write(lock_dir.join("heartbeat"), "active").expect("write heartbeat");

    sandbox.commit(&repo, "recovery commit");
    let events = sandbox.wait_for_events(1);
    assert_eq!(events, vec!["commit"]);
}
