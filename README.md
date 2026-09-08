# GitSama ⚔️

Anime reactions for your Git workflow.

Commit. Push. Merge. Switch branches. GitSama reacts with sounds from your chosen pack.

Install it once and keep using Git exactly as you already do. Terminal, VS Code, coding agents — if they use your local Git, GitSama hears the event.

GitSama is a small, offline, open-source Rust CLI. It contains the generic event engine, a generated Starter tone pack, and a bundled Naruto Voice Pack included with permission to redistribute its audio. Add your own legally sourced sounds as a pack when you are ready.

## Demo

```text
$ git commit -m "fix auth"
  commit chime

$ git switch feature
  branch switch chime

$ git push
  push chime

73 outgoing commits
  massive push chime
```

The demo sounds are simple generated tones so the installation can be tested without downloading media.

## What is GitSama?

GitSama connects your local Git installation to a sound pack. Git reports a supported local event, GitSama turns it into a generic event such as commit or massive_push, and the selected pack supplies the sound.

The engine does not know anime titles, characters, quotes, or franchises. Those meanings live entirely in data-only packs.

## Why GitSama?

Git already gives meaningful moments a name. GitSama makes those moments feel a little more alive without changing how you work. There is no wrapper command to remember, no repository setup for teammates, and no account or service to maintain.

## Local by design

GitSama runs only on your computer.

If your teammate commits or pushes from their computer, GitSama on your computer does nothing.

GitSama does not watch GitHub, remote repositories, or your teammates. It reacts only when your local Git installation performs a supported event.

Installing GitSama never adds files to repositories you collaborate on. Its permanent files live under ~/.gitsama (or %USERPROFILE%/.gitsama on Windows). A repository opt-out uses that clone's local .git/config, which is not committed.

## Requirements

- Git 2.54 or newer
- Windows, macOS, or Linux
- A normal user account; administrator or root privileges are not required
- An audio output device for real playback

GitSama uses Git's [user-level configured hook system](https://git-scm.com/docs/git-hook/2.54.0.html) introduced for this workflow in Git 2.54. It registers named hooks with absolute commands, so it continues to work when an IDE has a different PATH. It never changes core.hooksPath, replaces .git/hooks/*, or removes an existing hook.

If Git is older, setup stops before changing Git configuration and explains the detected version.

## Installation

The simplest source installation is:

```sh
git clone https://github.com/tu-tu-op/GitSama.git
cd GitSama
./install.sh
```

On Windows PowerShell:

```powershell
git clone https://github.com/tu-tu-op/GitSama.git
cd GitSama
./install.ps1
```

The installers use a matching compiled release when one is available. From a source checkout they use an existing release binary or build locally with Cargo. They install the executable to the per-user GitSama bin directory, add that directory to the user PATH when needed, and run gitsama setup.

You can also download a release archive from https://github.com/tu-tu-op/GitSama/releases, place gitsama or gitsama.exe in the per-user bin directory, and run gitsama setup.

## Quick Start

```sh
gitsama
gitsama setup
gitsama test all
gitsama packs
gitsama use naruto
```

After setup, keep using Git normally:

```sh
git commit -m "fix auth"
git switch feature
git merge feature
git push
```

Setup is safe to run again. It repairs GitSama's own named hook entries without touching unrelated hooks.

## Supported Git Events

| Git moment | Native hook | GitSama event |
| --- | --- | --- |
| Successful commit | post-commit | commit |
| Push starts | pre-push | push or massive_push |
| Successful merge | post-merge | merge |
| Branch checkout or switch | post-checkout | branch_switch |
| Branch created | reference-transaction committed | branch_create |
| Branch deleted | reference-transaction committed | branch_delete |
| Rebase rewrite | post-rewrite rebase | rebase |

Push counting uses the ref data Git supplies locally. It does not contact the remote. The default massive push threshold is 50 unique outgoing commits, and counting stops as soon as the threshold is reached. If counting cannot be done safely, GitSama chooses the normal push reaction and lets Git continue.

Branch creation takes priority over the checkout event from git switch -c and git checkout -b, so that operation produces one sound. Clone initialization and file checkouts do not produce a misleading branch-switch sound.

## Commands

```text
gitsama                         Show the friendly dashboard
gitsama help                    Show command help
gitsama setup                   Install or repair GitSama hooks
gitsama status                  Show global and repository status
gitsama settings                View or edit settings
gitsama packs                   Interactive pack picker
gitsama pack list               List installed packs
gitsama pack add <path>         Import a local pack
gitsama pack remove <id>        Remove an installed pack
gitsama pack validate <path>    Validate a pack without importing it
gitsama pack scaffold <name>    Create a new pack template
gitsama use <id>                Select the active pack
gitsama test [event|all]        Play a selected event or every event
gitsama volume <0-100>          Set volume
gitsama threshold <number>      Set massive push threshold
gitsama enable <event>          Enable one event
gitsama disable <event>         Disable one event
gitsama mute                   Mute playback globally
gitsama unmute                 Restore playback
gitsama off-here                Disable GitSama in this repository
gitsama on-here                 Inherit global behavior in this repository
gitsama doctor                 Check the installation
gitsama doctor --fix           Repair GitSama-owned configuration
gitsama uninstall               Remove GitSama and its hooks
gitsama uninstall --keep-data  Remove hooks and binary, keep packs/config
```

Hook and playback commands are internal. They are intentionally hidden from normal help and are only called by Git or GitSama's detached player process.

## Sound Packs

A pack is a directory containing pack.toml and an audio directory. The pack is data, not executable code. A manifest maps generic GitSama event names to one or more relative audio files.

Fresh installations include the generated Starter pack and the bundled Naruto Voice Pack. The Naruto pack is selected by default; custom packs can be added later with the pack commands below.

```text
my-pack/
├── pack.toml
├── README.md
└── audio/
    ├── commit.mp3
    ├── push.mp3
    ├── massive-push.mp3
    ├── merge.mp3
    ├── branch-create.mp3
    ├── branch-delete.mp3
    ├── branch-switch.mp3
    └── rebase.mp3
```

A pack may provide several files for one event; GitSama chooses one at random. An absent event is silent. If massive_push is absent while push exists and is enabled, the normal push sound is used as a fallback.

The supported formats are WAV, MP3, OGG, and FLAC through the built-in audio decoder. See docs/PACKS.md for the complete schema and a copyable template.

### Creating your own pack

```sh
gitsama pack scaffold "Pain Arc"
# put legally usable audio files in pain-arc/audio/
gitsama pack validate ./pain-arc
gitsama pack add ./pain-arc
gitsama use pain-arc
gitsama test all
```

No Rust changes, Git hook changes, or recompilation are needed. Pack paths must remain inside the pack directory, and manifests cannot contain commands. Import validation rejects absolute paths, traversal, unsafe symlinks, missing files, unsupported extensions, and future schema versions.

### Pack contribution rules

Do not add copyrighted anime clips to this repository without documented permission to redistribute them. A local pack may contain media you are legally allowed to use. A community pack submission must contain audio the contributor has permission to redistribute and must include clear license or permission information. User-provided pack audio is not automatically covered by GitSama's MIT license.

## Per-Repository Disable

```sh
gitsama off-here
gitsama on-here
gitsama status
```

off-here writes only local hook enable overrides to the repository's .git/config. It does not add a working-tree file and does not affect other repositories. on-here removes those local overrides so global behavior is inherited again.

## How It Works

```text
Local Git
   ↓
Named configured Git hook
   ↓
GitSama hook handler
   ↓
Generic GitSama event
   ↓
Pack resolver
   ↓
Detached audio process
```

Hook handlers are deliberately fail-open and silent. They consume local hook inputs, dispatch a short-lived player process, and return zero even when configuration, state, pack parsing, counting, or audio output fails. The player uses a small cross-process lock so sounds do not become an uncontrolled wall of overlapping clips. Waiting playback events expire after the configured queue age.

## Works With IDEs and Coding Agents

GitSama works below the editor layer:

```text
VS Code       → Git → GitSama
Codex         → Git → GitSama
Claude Code   → Git → GitSama
Terminal      → Git → GitSama
JetBrains     → Git → GitSama
```

Any tool that uses your local Git and allows the relevant Git hooks will trigger GitSama. An operation that bypasses hooks with Git's own bypass options may bypass GitSama too. GitSama is an entertainment tool, not enforcement software.

## Privacy

GitSama has no backend, database, account, authentication, cloud service, telemetry, analytics, GitHub OAuth, or background network service. Runtime behavior is offline. It does not send repository names, commit messages, credentials, remote activity, pack usage, or audio anywhere. GitHub is used only as a release-artifact host for people who choose to download a release.

The diagnostic log is local at ~/.gitsama/logs/gitsama.log. It contains operational errors without commit messages, repository contents, credentials, or secrets.

## Troubleshooting

Run:

```sh
gitsama doctor
gitsama doctor --fix
gitsama status
```

If a sound is silent, confirm the active pack with gitsama status, validate it with gitsama pack validate, and run gitsama test commit. Check the log path shown by gitsama doctor.

If setup reports an old Git version, upgrade Git to 2.54 or newer and run gitsama setup again. Setup does not partially install hooks on an unsupported Git version.

If an IDE does not trigger sounds, check whether it uses the same local Git installation and whether the operation bypasses hooks. The hooks are user-level Git configuration, so repository files and core.hooksPath are not involved.

## Doctor

gitsama doctor checks Git discovery and version, the binary path, configuration, the active pack, audio backend availability, each GitSama named hook, repository opt-out state, and the diagnostic log location. --fix repairs GitSama-owned global configuration only.

## Uninstall

```sh
gitsama uninstall
```

The command explains the files it will remove, removes GitSama's named global hook entries first, removes its identifiable user PATH entry, and then removes the per-user GitSama directory. It leaves unrelated hooks and repositories alone. Use gitsama uninstall --keep-data if you want to remove the hooks and executable while keeping packs or configuration for a later reinstall.

## Development

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo run -- test all
```

Integration tests use temporary repositories and isolated Git configuration. They set GITSAMA_TEST_MODE=1, GITSAMA_TEST_LOG, and GITSAMA_HOME so CI never needs an audio device and never touches a developer's real Git configuration. Tests that exercise Git 2.54 configured hooks report a clear skip when the local Git is older than the supported minimum.

Read docs/ARCHITECTURE.md for design details and CONTRIBUTING.md before opening a pull request.

## Contributing

Bug reports and pull requests are welcome. Please include the platform, Git version, GitSama version, and relevant safe gitsama doctor output. Do not include repository names, private URLs, commit messages, credentials, or private audio files.

Pack contributions must follow the audio licensing rule above. The source code is MIT licensed; pack audio needs its own permission and license information.

## License

GitSama source code is licensed under the MIT License. User-created packs and their audio remain under their own applicable licenses.

## FAQ

### Does everyone in a repository hear the sounds?

No. GitSama is installed per user. Only local Git operations run through the installation that has GitSama configured can trigger it.

### Does GitSama watch GitHub?

No. There is no server, polling, remote monitoring, or GitHub API use.

### Does it add files to my project?

No. Permanent files live in the per-user ~/.gitsama directory. The only repository-local setting is an opt-out in .git/config.

### Do I need to type gitsama commit?

No. Continue using Git normally.

### Can I add Naruto, JJK, Bleach, Dragon Ball, or meme sounds?

Yes, as data-only packs containing media you are legally allowed to use. Use gitsama pack scaffold, add the files, validate, import, select, and test.

### Can sound playback stop a commit or push?

No. Hook handling is fail-open and the player is detached. A broken pack, missing file, audio device failure, or counting error cannot block Git.
