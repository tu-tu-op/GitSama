# Troubleshooting

Start with:

```sh
gitsama doctor
gitsama status
gitsama test commit
```

## Git version

GitSama requires Git 2.54 or newer. If setup detects an older version, upgrade Git and rerun gitsama setup. No GitSama hook configuration is written when the check fails.

## No sound

Check that the active pack exists and validates:

```sh
gitsama pack list
gitsama pack validate ~/.gitsama/packs/my-pack
gitsama test all
```

Check the log path printed by gitsama doctor. The audio backend needs an available output device. CI and headless testing can use GITSAMA_TEST_MODE=1.

## Git operation still works

This is intentional. GitSama hooks are fail-open. If a pack, state file, log, decoder, or audio device fails, Git continues and the diagnostic log records a short reason.

## IDE or coding agent

GitSama reacts when the tool invokes the same local Git installation with hooks enabled. Confirm the IDE's Git path and check whether the operation uses a hook-bypass option such as --no-verify.

## Coding agents and sandboxed environments

Some coding agents (such as Antigravity) and IDE sandboxes execute each command within a Windows Job Object or container that terminates the entire process tree as soon as the top-level command returns.

Because detached background workers spawned by standard mechanisms inherit the job object, short-lived commands (like `git commit`) exit before the background worker can finish initializing the audio device and playing the sound. Longer commands (like `git push`) may play briefly before being killed.

### The 3-Tier Fallback Chain

GitSama automatically detects constrained job environments and uses an ordered fallback chain:
1. **Tier 1 (Breakaway)**: Spawns the worker with `CREATE_BREAKAWAY_FROM_JOB` so it escapes the parent job.
2. **Tier 2 (Outside Launcher)**: If breakaway is forbidden by the job, GitSama launches the worker via WMI (`Win32_Process` on Windows) or `systemd-run` (Linux), creating the worker completely outside the container tree. The successful tier is cached in `~/.gitsama/state/launcher_tier.txt`.
3. **Tier 3 (Linger Mode)**: If container escape is unavailable, GitSama plays the sound directly inside the hook process, capped at `linger_cap_secs` (default: 2.5s). Playback stops cleanly at the cap, guaranteeing the full sound plays without delaying Git indefinitely.

### Configuration

You can configure playback behavior via:
```sh
gitsama mode linger       # Force in-process capped playback
gitsama mode detached     # Force background worker only
gitsama mode auto         # Default: automatic fallback chain
gitsama linger 3.0        # Set linger cap in seconds (0.1 - 30.0)
```

### Reading diagnostic breadcrumbs

Check `~/.gitsama/logs/gitsama.log` for process lifecycle breadcrumbs:
- `hook entered: <event> [in_job=... breakaway_ok=...]`: shows whether process is containerized.
- `attempting Tier 1... / Tier 2... / Tier 3...`: shows fallback tier progression.
- `worker started`: worker process started.
- `lock acquired`: cross-process playback lock acquired.
- `audio device opened`: audio subsystem initialized.
- `first sound started`: audio decoding finished and audio sink playing.
- `playback finished`: playback completed to the end.
- `lock released`: lock directory cleaned up.

If `worker started` is followed immediately by silence with no `playback finished` and an orphaned lock, the worker was terminated by a parent Job Object. GitSama automatically reclaims dead-process locks instantly.

## Repository is quiet

Run gitsama status inside the repository. off-here stores local hook enable overrides in .git/config; on-here removes them. No working-tree file is created.

## Safe diagnostics

Share only GitSama version, operating system, Git version, and redacted gitsama doctor output. Do not share repository names, private remote URLs, commit messages, credentials, or private pack media.

## Removing GitSama

Use gitsama uninstall for a complete removal. Use gitsama uninstall --keep-data when you want to remove the executable and hooks but keep packs and configuration for a later reinstall.
