# GitSama

GitSama plays a sound when something important happens in your local Git workflow, such as a commit, push, merge, branch change, or rebase.

Install it once and keep using Git normally. GitSama works below your terminal, editor, or coding agent through Git's configured hooks.

[Install](#install) · [Update](#update-gitsama) · [Create a voice pack](#create-a-voice-pack-from-your-clips) · [Delete or restore a pack](#delete-or-restore-a-voice-pack) · [Share a pack](#share-a-voice-pack-optional) · [Troubleshooting](#troubleshooting)

## Install

GitSama is installed once per user. It does not add files to your repositories. Packaged installs use prebuilt binaries, so Rust is not required. All install methods require Git 2.54 or newer.

The npm package is available. The Homebrew tap is not published yet; macOS users can use npm or the source installer below.

### npm (Windows, macOS, and Linux)

Requires Node.js 18 or newer and npm:

~~~sh
npm install --global gitsama
~~~

The npm package downloads the prebuilt binary for your OS and architecture, verifies its SHA-256 checksum, and registers GitSama's named hooks for your user. The bundled Naruto voice pack is part of the binary.

### Windows

~~~powershell
npm install --global gitsama
~~~

### macOS

~~~sh
npm install --global gitsama
~~~

### Linux

~~~sh
curl -fsSL https://raw.githubusercontent.com/tu-tu-op/GitSama/main/install.sh | sh
~~~

The release binaries currently support Windows x64, Linux x64, macOS Intel, and macOS Apple Silicon. Administrator or root access is not required for the per-user installers.

### Install from a source checkout

Source installation is also available. It builds from source when Rust and Cargo are installed; otherwise it downloads the matching release binary.

#### Windows PowerShell

~~~powershell
git clone https://github.com/tu-tu-op/GitSama.git
cd GitSama
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\install.ps1
~~~

The execution-policy option applies only to this PowerShell process. It does not change your permanent Windows policy.

#### macOS or Linux

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

gitsama doctor checks Git, the installed binary, sound output, the active pack, and every GitSama hook. gitsama test all previews each event and reports completed playback, missing sounds, fallback playback, and skips.

## Update GitSama

After an interactive command such as `gitsama pack scaffold` or `gitsama status` finishes, GitSama checks for a newer stable release. It checks at most once a day and offers each new version once:

~~~text
GitSama 0.1.2 is available (you have 0.1.1).
Would you like to update now? [y/N]
~~~

Type `y` to update, or press Enter to continue using your current version. You can also update whenever you want:

~~~sh
gitsama --upgrade
~~~

Other update commands:

| Command | What it does |
| --- | --- |
| `gitsama upgrade` | Check now and ask before installing an update. |
| `gitsama upgrade --check` | Check now without installing anything. |
| `gitsama upgrade --yes` | Approve installation without a prompt, including in scripts. |

For a source or standalone installation, GitSama downloads the official binary for your platform, checks its SHA-256 checksum and version, and replaces the executable. For a global npm installation, it updates the whole npm package after the running binary exits. Packs, volume, settings, and contribution records are preserved. A local npm installation prints a command to run in its project; Homebrew installations use `brew upgrade gitsama`.

Git hooks, audio workers, help/version output, uninstallation, and noninteractive commands do not perform automatic update checks. Offline or failed checks leave your normal command working. Set `GITSAMA_NO_UPDATE_CHECK=1` to disable automatic checks; the explicit upgrade commands still work.

**Already using 0.1.0?** Update once through your original installation method to get the new upgrade command. For npm:

~~~sh
npm install --global gitsama@latest
~~~

For a source installation, pull the updated source and rerun its installer; it rebuilds the current code when Cargo is available.

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

### Everyday use

| Command | What it does |
| --- | --- |
| `gitsama` | Show the dashboard. |
| `gitsama help` | Show available commands. |
| `gitsama status` | Show the active pack, volume, and playback status. |
| `gitsama packs` | Choose an installed pack interactively. |
| `gitsama use <id>` | Select a pack; restore a deleted built-in if needed. |
| `gitsama test commit` | Preview one commit sound. |
| `gitsama test all` | Preview every event in the active pack. |
| `gitsama --upgrade` | Check for a new version and ask before installing it. |

### Pack commands

| Command | What it does |
| --- | --- |
| `gitsama pack list` | List installed packs and their IDs. |
| `gitsama pack scaffold [name]` | Create, install, and select a pack from clips in the current folder. |
| `gitsama pack delete <id>` | Delete any installed pack, including Naruto or Starter. |
| `gitsama pack remove <id>` | Same as `pack delete`. |
| `gitsama pack contribute <id>` | Share a tested pack through a GitHub pull request. |
| `gitsama pack add <path>` | Install a prepared pack containing `pack.toml`. |
| `gitsama pack validate <path>` | Check the manifest and referenced files in a pack folder. |
| `gitsama pack scaffold --template <name> [parent]` | Create an empty pack to fill in manually. |

Replace `<id>` with the lowercase ID shown by `gitsama pack list`, such as `bleach`. Arguments in square brackets are optional. The walkthrough below shows complete examples.

### Settings and maintenance

| Command | What it does |
| --- | --- |
| `gitsama volume 60` | Set the volume to 60%; accepts 0–100. |
| `gitsama mute` / `gitsama unmute` | Mute or resume playback. |
| `gitsama settings` | View or edit settings. |
| `gitsama threshold 25` | Use the massive-push sound for pushes of at least 25 commits. |
| `gitsama enable <event>` / `gitsama disable <event>` | Turn a Git event's sound on or off. |
| `gitsama off-here` / `gitsama on-here` | Disable or enable GitSama in the current repository. |
| `gitsama setup` | Install or repair GitSama hooks. |
| `gitsama doctor` / `gitsama doctor --fix` | Check the installation or repair GitSama settings and hooks. |
| `gitsama uninstall` | Remove GitSama, its hooks, and its saved data. |
| `gitsama uninstall --keep-data` | Remove the binary and hooks while preserving packs and settings. |

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

GitSama's hooks, playback, pack creation, validation, and previews run on your computer:

- It does not watch GitHub or remote repositories.
- It does not monitor teammates or other computers.
- It has no account, server, database, telemetry, analytics, or background network service.
- Hooks and previews keep repository activity and pack usage local.
- It does not add files to a project repository.

Interactive update checks request public release information from GitHub. Upgrading downloads release files or updates the npm package. These requests contain no repository activity, clip names, or pack usage. Automatic checks can be disabled with `GITSAMA_NO_UPDATE_CHECK=1`.

The explicit `gitsama pack contribute` command uses your authenticated GitHub CLI to publish the selected pack's manifest, contribution README, and referenced audio through a pull request.

Permanent files are stored in:

- Windows: %USERPROFILE%\.gitsama
- macOS and Linux: ~/.gitsama

The only repository-local setting is the opt-out written by gitsama off-here to that repository's .git/config. It is not committed.

## Voice packs

A voice pack is a collection of audio clips assigned to Git events. Fresh installations include:

- **Naruto** (`naruto`): the default voice pack, bundled with permission to redistribute its audio.
- **Starter** (`starter`): simple generated tones for checking sound output.

Use `gitsama pack list` to see installed packs. A pack's **name** is its display label; its **ID** is what you type in commands. For example, **Bleach Bankai** has the ID `bleach-bankai`.

### Create a voice pack from your clips

#### 1. Put your clips in a folder and name them for events

For example, create a folder named `Bleach` and add clips you are allowed to use. GitSama supports **WAV, MP3, OGG, and FLAC**.

| Sound for | Example filename |
| --- | --- |
| Commit | `commit.mp3` |
| Push | `push.mp3` |
| Massive push | `massive-push.mp3` |
| Merge | `merge.mp3` |
| Switch branches | `branch-switch.mp3` |
| Create a branch | `branch-create.mp3` |
| Delete a branch | `branch-delete.mp3` |
| Rebase | `rebase.mp3` |

The `.mp3` extension in these examples can be replaced with any supported format. Case differences and underscores are accepted: `Commit.WAV` and `branch_switch.ogg` both work. For multiple clips for one event, use positive numbers, such as `commit-1.mp3` and `commit-2.mp3`; GitSama picks one at random during playback.

Clips can be directly in the folder or in its `audio/` subfolder:

~~~text
Bleach/
  commit-1.mp3
  commit-2.mp3
  push.wav
  audio/
    branch-switch.ogg
~~~

You can start with just one clip. Events with no clip stay silent. If there is no separate massive-push clip, GitSama uses the push clip for massive pushes.

#### 2. Open a terminal in that folder and install the pack

If `Bleach` is in your Downloads folder, run this in **Windows PowerShell**:

~~~powershell
cd "$HOME\Downloads\Bleach"
gitsama pack scaffold
~~~

On **macOS or Linux**:

~~~sh
cd ~/Downloads/Bleach
gitsama pack scaffold
~~~

If your terminal is already inside the clips folder, run just `gitsama pack scaffold`.

This command:

- Creates the pack's settings file, `pack.toml`, with the clips already assigned to events.
- Copies the pack into your GitSama data directory and prints the installed path.
- Selects the new pack, ready for previewing.

The display name comes from the folder name: `Bleach` becomes the ID `bleach`. Your original clips stay in their source folder. The generated `pack.toml` is in the installed copy at the printed path.

To choose a different name during creation, replace the scaffold command with:

~~~sh
gitsama pack scaffold "Bleach Bankai"
~~~

That creates the display name **Bleach Bankai** and ID `bleach-bankai`. GitSama prints corrective instructions if a clip name is unrecognized, filenames conflict, the name is reserved, or that ID is already installed.

The IDs `naruto` and `starter` are reserved for built-in packs. Use a different name for a custom pack, such as `My Naruto Pack`.

#### 3. Listen to your pack

~~~sh
gitsama test all
~~~

GitSama previews one clip for each event that has a sound and reports the result:

| Preview output | Meaning |
| --- | --- |
| `Played Commit: commit-1.mp3` | The clip finished playing. |
| `Played Massive Push: push.wav (fallback to Push)` | The push clip was used for massive push. |
| `Skipped Merge: no sound mapped` | This pack has no merge clip; partial packs allow this. |
| `Skipped ...: GitSama is muted` | Run `gitsama unmute`, then preview again. |
| `Skipped ...: volume is zero` | Run `gitsama volume 80`, then preview again. |
| `Skipped ...: playback queue timed out` | Wait for other playback to finish, then retry. |

An error reading a clip or opening the sound output stops the preview and reports the problem. For a quick preview of one event, use `gitsama test commit`.

After a successful full preview of a newly scaffolded pack, GitSama prints an optional [invitation to share it](#share-a-voice-pack-optional). It appears once per installation. At least one clip must finish, and every event with an available sound must finish. Missing clips in a partial pack are allowed; single-event tests, muted or zero-volume runs, timeouts, and playback errors do not qualify.

To change clips or event assignments later, edit the installed copy at the printed path and run `gitsama test all` again. Editing the source folder does not update the installed copy.

### Switch voice packs

List packs, select one by its lowercase ID, and preview it:

~~~sh
gitsama pack list
gitsama use bleach
gitsama test all
~~~

Use `gitsama use naruto` to select Naruto or `gitsama use starter` to select the test tones.

### Delete or restore a voice pack

Delete a pack using its lowercase ID:

~~~sh
gitsama pack delete bleach
~~~

`gitsama pack remove bleach` does the same thing. Deletion removes the installed pack and its local preview/contribution records. Your source soundtrack folder is preserved.

- If you delete the active pack, GitSama selects another installed pack.
- If you delete the last pack, playback is muted and status shows `Active pack: None`.
- Selecting or scaffolding a pack after deleting the last one resumes playback.

Built-in packs can also be deleted. For example:

~~~sh
gitsama pack delete naruto
~~~

Deleted built-ins stay deleted during listing, setup, and repair. To reinstall and select one explicitly:

~~~sh
gitsama use naruto
~~~

Use `starter` in these commands to delete or restore Starter instead.

### Install a shared pack that contains pack.toml

If you downloaded a prepared pack named `my-voice-pack`, run:

~~~sh
gitsama pack validate ./my-voice-pack
gitsama pack add ./my-voice-pack
gitsama use my-voice-pack
gitsama test all
~~~

`pack add` prints the pack's ID; use that ID in the `use` command if it differs from the folder name. The prepared folder must contain `pack.toml` and its referenced clips. For a folder of renamed clips, follow the scaffold walkthrough above.

<details>
<summary>Advanced: create an empty template and assign clips manually</summary>

~~~sh
gitsama pack scaffold --template "My Custom Pack" ./packs
cd ./packs/my-custom-pack
# Add clips under audio/ and edit the event arrays in pack.toml.
gitsama pack validate .
gitsama pack add .
gitsama use my-custom-pack
gitsama test all
~~~

The parent folder `./packs` is optional. An empty template can validate but plays no sound until you assign clips in `pack.toml`. See [Pack documentation](docs/PACKS.md) for the full format, supported filenames, and validation rules.

</details>

## Share a voice pack (optional)

You can contribute a pack to GitSama by opening a **pull request**: a proposed addition that maintainers can review. Share clips you created or have permission to redistribute. The GitSama source-code license does not grant rights to other people's audio.

### 1. Complete a full preview

Select your pack and run:

~~~sh
gitsama use bleach
gitsama test all
~~~

After the first successful full preview of a locally scaffolded pack, GitSama suggests `gitsama pack contribute bleach`. Sharing is your choice. Repeat the full preview after editing the installed `pack.toml` or referenced clips. A failed full preview clears the previous successful test.

### 2. Sign in with GitHub CLI

Install [GitHub CLI](https://cli.github.com/), then sign in:

~~~sh
gh auth login --hostname github.com
~~~

### 3. Submit the pack with its audio details

~~~sh
gitsama pack contribute bleach
~~~

In an interactive terminal, GitSama asks for missing details:

| Detail | What to provide |
| --- | --- |
| Author | Your name or contributor handle. |
| Audio license | The license or permission terms allowing the clips to be redistributed. |
| Source | Where each clip came from, source links where applicable, and redistribution permission details. |

If the pack already has real author and license values in `pack.toml`, those can be used. In scripts or other noninteractive terminals, supply missing details with `--author`, `--license`, and `--source`.

For example, **for your own recordings that you release under CC0**, replace the author with your name:

~~~sh
gitsama pack contribute bleach --author "Your Name Here" --license "CC0-1.0" --source "Original recordings by me; released under CC0-1.0 for redistribution."
~~~

GitSama prepares the submission in a separate checkout, creates or reuses your GitHub fork, and opens the pull request. For repository owners, it uses a new branch in the project. Its new files are the pack's `pack.toml`, a README with attribution and licensing details, and referenced audio under `packs/bleach/`. The PR link is printed and saved locally; repeating the command shows the saved link.

If submission fails, GitSama prints the saved preparation folder and recovery instructions. Follow its `RECOVERY.md` when available. IDs already present in the GitSama repository need a different name. See [Contribution guidance](CONTRIBUTING.md#pack-contributions) for the review checklist and submission details.

A contributed pack can be downloaded and installed with `pack add`. Including it in the repository does not automatically bundle it into every GitSama installation.

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

GitSama saves each pack's creation, full-preview, invitation, and PR-link records under `state/pack-<id>.json`. Deleting the pack clears those records. `GITSAMA_HOME` can point GitSama at a different data directory.

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

If the preview says `no sound mapped`, that event has no assigned clip. This is normal for a partial pack. For an empty template, add clips and fill in its event arrays in `pack.toml`.

### A pack command fails

| Problem | How to fix it |
| --- | --- |
| `git: 'pack' is not a git command` | Use `gitsama pack add <path>` to install a prepared pack. |
| An uppercase name such as `Bleach` is rejected | Run `gitsama pack list` and use its lowercase ID, such as `gitsama use bleach`. |
| `pack.toml` cannot be found | For a folder of clips, run `gitsama pack scaffold`. Its generated manifest is at the printed installed path. For a prepared pack, pass the folder containing `pack.toml` to `pack add` or `pack validate`. |
| An audio filename is unmatched | Rename the clip using an event name from the [filename table](#1-put-your-clips-in-a-folder-and-name-them-for-events). |
| Audio filenames conflict | Rename duplicate clips to distinct variants, such as `commit-1.mp3` and `commit-2.mp3`. Names are compared without regard to case. |
| The pack ID is already installed | Choose another name, or delete that installed pack before importing its replacement. |
| Status shows `Active pack: None` | Select or restore a pack with `gitsama use <id>`, or create one with `gitsama pack scaffold`. |
| Contribution asks for a full test | Select the pack and rerun `gitsama test all`; changes to the installed manifest or clips require another full preview. |
| A new command such as `pack delete` is not recognized | Update or reinstall GitSama with a build that includes these commands. For a source checkout, rerun the [source installer](#install-from-a-source-checkout). |

See [Troubleshooting](docs/TROUBLESHOOTING.md) for contribution recovery, GitHub authentication, and additional audio diagnostics.

### An IDE or coding agent does not play sounds

Confirm that the IDE uses the same local Git installation and that the operation does not bypass hooks. GitSama works below the editor layer with VS Code, JetBrains IDEs, terminal tools, and coding agents that use local Git. For coding agents and sandboxes that terminate process trees upon command exit (e.g. Antigravity), see [Troubleshooting Coding Agents and Sandboxes](docs/TROUBLESHOOTING.md#coding-agents-and-sandboxed-environments) for the 3-tier fallback chain and linger mode settings.

## Uninstall

~~~sh
gitsama uninstall
~~~

The uninstall command removes GitSama's named global hooks, its identifiable PATH entry, the managed executable, and the per-user GitSama directory. It leaves unrelated hooks and repositories alone.

For an npm installation, use:

~~~sh
npm uninstall --global gitsama
~~~

The npm uninstall lifecycle removes GitSama's named hooks and preserves packs and configuration.

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

### Do I need Rust or Node.js?

No Rust toolchain is required when installing a prebuilt release or using a package manager. The npm installation method requires Node.js 18 or newer and npm. A source build requires Rust and Cargo.

### Does GitSama watch GitHub?

It does not watch Git repositories or GitHub activity. Git events and playback are handled locally. Interactive commands can check GitHub's public releases for updates, and the explicit `pack contribute` command contacts GitHub to submit a pack PR.

### Does it add files to my project?

No. Its permanent files live in the per-user GitSama directory. The only repository-local setting is an opt-out in .git/config.

### Do I need to type gitsama commit?

No. Continue using normal Git commands.

### Can I add sounds from other anime or games?

Yes, as a data-only pack containing media you are legally allowed to use. Put renamed clips in a folder, run `gitsama pack scaffold` there, then run `gitsama test all`.

### Can sound playback stop a commit or push?

No. GitSama's hook handling is fail-open and playback is detached. A broken pack, missing file, audio failure, or counting error cannot block Git.

## License

GitSama source code is licensed under the MIT License. User-created packs and their audio remain under their own applicable licenses.
