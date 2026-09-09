# GitSama

GitSama plays a sound when something important happens in your local Git workflow, such as a commit, push, merge, branch change, or rebase.

Install it once and keep using Git normally. GitSama works below your terminal, editor, or coding agent through Git's configured hooks.

## Install

GitSama is installed once per user. It does not add files to your repositories.

You need:

- Git 2.54 or newer
- Windows, macOS, or Linux
- An audio output device for playback

### Windows PowerShell

~~~powershell
git clone https://github.com/tu-tu-op/GitSama.git
cd GitSama
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\install.ps1
~~~

The execution-policy option applies only to this PowerShell process. It does not change your permanent Windows policy.

### macOS or Linux

~~~sh
git clone https://github.com/tu-tu-op/GitSama.git
cd GitSama
chmod +x install.sh
./install.sh
~~~

Open a new terminal after installation so the new PATH entry is available.

The installer checks your Git version, builds or downloads the correct release binary, installs it in your per-user GitSama directory, adds that directory to your PATH, and registers the Git hooks. Administrator or root access is not required.

## Verify the installation

Run these commands in a new terminal:

~~~sh
gitsama --version
gitsama doctor
gitsama status
gitsama test all
~~~

gitsama doctor checks Git, the installed binary, sound output, the active pack, and every GitSama hook. gitsama test all plays one sound for each supported event.

## Use Git normally

After setup, do not change your Git workflow:

~~~sh
git status
git add .
git commit -m "your message"
git switch -c feature
git push
~~~

GitSama reacts automatically when the matching local Git event completes.

## Common commands

~~~text
gitsama                         Show the dashboard
gitsama help                    Show all commands
gitsama setup                   Install or repair GitSama hooks
gitsama status                  Show global and repository status
gitsama doctor                  Check the installation
gitsama doctor --fix            Repair GitSama-owned configuration
gitsama settings                View or edit settings
gitsama packs                   Choose an installed pack
gitsama pack list               List installed packs
gitsama use <id>                Select a pack
gitsama test [event|all]        Play one event or every event
gitsama volume <0-100>          Set the volume
gitsama threshold <number>      Set the massive-push threshold
gitsama enable <event>          Enable an event
gitsama disable <event>         Disable an event
gitsama mute                    Mute playback globally
gitsama unmute                  Restore playback
gitsama off-here                Disable GitSama in this repository
gitsama on-here                 Use the global setting in this repository
gitsama uninstall               Remove GitSama and its hooks
gitsama uninstall --keep-data   Remove the binary and hooks but keep data
~~~

Hook and playback commands are internal. Git and GitSama call them automatically.

## Supported Git events

| Git event | Native hook | Sound event |
| --- | --- | --- |
| Successful commit | post-commit | commit |
| Push starts | pre-push | push or massive_push |
| Successful merge | post-merge | merge |
| Branch checkout or switch | post-checkout | branch_switch |
| Branch created | reference-transaction | branch_create |
| Branch deleted | reference-transaction | branch_delete |
| Rebase rewrite | post-rewrite | rebase |

GitSama counts outgoing commits locally for a massive push. It does not contact the remote. The default threshold is 50 unique outgoing commits, and counting stops as soon as the threshold is known.

Branch creation takes priority over the checkout event produced by git switch -c and git checkout -b, so those commands produce one sound instead of two. Clone initialization and file checkouts do not produce a misleading branch-switch sound.

## How GitSama works

Git reports a local event to a named Git hook. GitSama converts that event into a generic event, finds the matching sound in the active pack, and starts a short-lived playback process.

The hook process is fail-open. A missing sound, invalid pack, audio error, state error, or counting error cannot block a commit, push, merge, or other Git operation. Playback uses a small cross-process lock so several sounds do not overlap without control.

GitSama uses Git's user-level configured hook system from Git 2.54. It registers named hooks with absolute commands, so Git can find the installed binary even when an IDE has a different PATH. It does not change core.hooksPath, replace .git/hooks/*, or remove existing hooks.

## Local by design

GitSama runs on your computer only:

- It does not watch GitHub or remote repositories.
- It does not monitor teammates or other computers.
- It has no account, server, database, telemetry, analytics, or background network service.
- It does not send repository names, commit messages, credentials, remote activity, pack usage, or audio anywhere.
- It does not add files to a project repository.

Permanent files are stored in:

- Windows: %USERPROFILE%\.gitsama
- macOS and Linux: ~/.gitsama

The only repository-local setting is the opt-out written by gitsama off-here to that repository's .git/config. It is not committed.

## Sound packs

A sound pack is a data-only directory containing pack.toml and an audio directory. Fresh installations include:

- starter: generated tones used for installation checks, development, and CI
- naruto: the bundled Naruto Voice Pack, included with permission to redistribute its audio

The Naruto pack is selected by default. GitSama supports WAV, MP3, OGG, and FLAC files. If an event has several files, GitSama chooses one at random. If an event has no sound, it is silent. If massive_push has no usable sound but push does, GitSama falls back to the push sound.

### Create and install a custom pack

~~~sh
gitsama pack scaffold "My Custom Pack"
cd my-custom-pack
# Add legally usable files under audio/ and edit pack.toml.
gitsama pack validate .
gitsama pack add .
gitsama use my-custom-pack
gitsama test all
~~~

You can create the pack under a different parent directory:

~~~sh
gitsama pack scaffold "My Custom Pack" ./packs
~~~

This creates ./packs/my-custom-pack.

A pack has this general layout:

~~~text
my-pack/
|-- pack.toml
|-- README.md
|-- audio/
    |-- commit.mp3
    |-- push.mp3
    |-- massive-push.mp3
    |-- merge.mp3
    |-- branch-create.mp3
    |-- branch-delete.mp3
    |-- branch-switch.mp3
    |-- rebase.mp3
~~~

The manifest maps generic event names to relative audio paths. It cannot contain commands. Validation rejects absolute paths, parent traversal, unsafe symlinks, missing files, unsupported extensions, and files outside the pack directory. See docs/PACKS.md for the complete manifest schema.

Do not add copyrighted audio to this repository without permission to redistribute it. Local packs may contain media you are legally allowed to use. Every public pack contribution must include permission and license information for its audio.

## Disable GitSama in one repository

Run these commands from inside the repository:

~~~sh
gitsama off-here
gitsama status
gitsama on-here
~~~

off-here changes only that repository's local Git configuration. Other repositories are unaffected. on-here restores the global setting.

## Configuration and data

The main configuration file is stored at:

- Windows: %USERPROFILE%\.gitsama\config.toml
- macOS and Linux: ~/.gitsama/config.toml

Use GitSama commands instead of editing the file directly:

~~~sh
gitsama settings
gitsama volume 60
gitsama threshold 25
gitsama disable rebase
gitsama mute
gitsama unmute
~~~

The diagnostic log is stored at ~/.gitsama/logs/gitsama.log on macOS and Linux, or %USERPROFILE%\.gitsama\logs\gitsama.log on Windows.

## Troubleshooting

Start with:

~~~sh
gitsama doctor
gitsama doctor --fix
gitsama status
~~~

### gitsama is not found

Open a new terminal after installation. If it is still not found, confirm that the per-user GitSama bin directory is on PATH:

- Windows: %USERPROFILE%\.gitsama\bin
- macOS and Linux: ~/.gitsama/bin

You can run the installed binary directly to confirm that it exists, then open a new terminal and try gitsama --version again.

### Setup reports an old Git version

GitSama requires Git 2.54 or newer. Upgrade Git and run:

~~~sh
gitsama setup
~~~

GitSama checks the version before changing Git configuration.

### A sound does not play

Check the active pack and audio output:

~~~sh
gitsama status
gitsama pack validate <path>
gitsama test commit
~~~

Then check the log path reported by gitsama doctor. Playback errors do not stop Git operations.

### An IDE does not trigger sounds

Confirm that the IDE uses the same local Git installation and that the operation does not bypass hooks. GitSama works below the editor layer with VS Code, JetBrains IDEs, terminal tools, and coding agents that use local Git.

## Uninstall

~~~sh
gitsama uninstall
~~~

The uninstall command removes GitSama's named global hooks, its identifiable PATH entry, the executable, and the per-user GitSama directory. It leaves unrelated hooks and repositories alone.

Use this form to remove the executable and hooks while keeping packs and configuration:

~~~sh
gitsama uninstall --keep-data
~~~

## Development

From a source checkout:

~~~sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo run -- test all
~~~

Integration tests use temporary repositories and isolated Git configuration. They set GITSAMA_TEST_MODE=1, GITSAMA_TEST_LOG, and GITSAMA_HOME so tests do not need a real audio device or modify a developer's Git configuration.

Read docs/ARCHITECTURE.md for implementation details, docs/PACKS.md for pack development, and docs/TROUBLESHOOTING.md for additional diagnostics.

## Contributing

Bug reports and pull requests are welcome. Include the platform, Git version, GitSama version, and safe gitsama doctor output. Do not include repository names, private URLs, commit messages, credentials, or private audio files.

The source code is licensed under MIT. Pack audio needs its own permission and license information.

## FAQ

### Does everyone in a repository hear the sounds?

No. GitSama is installed per user. Only local Git operations performed on a computer with GitSama configured can trigger it.

### Does GitSama watch GitHub?

No. It has no server, polling, remote monitoring, or GitHub API integration.

### Does it add files to my project?

No. Its permanent files live in the per-user GitSama directory. The only repository-local setting is an opt-out in .git/config.

### Do I need to type gitsama commit?

No. Continue using normal Git commands.

### Can I add sounds from other anime or games?

Yes, as a data-only pack containing media you are legally allowed to use. Scaffold, validate, import, select, and test the pack.

### Can sound playback stop a commit or push?

No. GitSama's hook handling is fail-open and playback is detached. A broken pack, missing file, audio failure, or counting error cannot block Git.

## License

GitSama source code is licensed under the MIT License. User-created packs and their audio remain under their own applicable licenses.
