<a id="gitsama"></a>

<p align="center">
  <img src="docs/assets/gitsama-readme-banner-v3.png" width="100%" alt="GitSama — Your workflow. Your sound. Custom sounds for your Git events, with a biwa-playing character.">
</p>

<p align="center">
  <strong>Anime reactions for your Git workflow.</strong><br>
  Install once. Keep using Git. Hear the moments that matter.
</p>

<p align="center">
  <a href="https://www.npmjs.com/package/gitsama"><img src="https://img.shields.io/npm/v/gitsama?style=flat-square&amp;labelColor=161b22&amp;color=ff324b" alt="npm version"></a>
  <a href="https://github.com/tu-tu-op/GitSama/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/tu-tu-op/GitSama/ci.yml?branch=main&amp;style=flat-square&amp;labelColor=161b22&amp;label=CI" alt="CI status on main"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-58a6ff?style=flat-square&amp;labelColor=161b22" alt="Source license: MIT"></a>
  <a href="#quick-start"><img src="https://img.shields.io/badge/Git-2.54%2B-f05032?style=flat-square&amp;labelColor=161b22&amp;logo=git&amp;logoColor=white" alt="Requires Git 2.54 or newer"></a>
</p>

<p align="center">
  <a href="#quick-start">Get started</a> &nbsp;·&nbsp;
  <a href="#voice-packs">Voice packs</a> &nbsp;·&nbsp;
  <a href="#create-your-own-pack">Create a pack</a> &nbsp;·&nbsp;
  <a href="#everyday-commands">Commands</a> &nbsp;·&nbsp;
  <a href="#documentation">Docs</a>
</p>

<p align="center">
  <sub>3 bundled voice packs · 9 sound events · Windows, macOS &amp; Linux</sub>
</p>

---

GitSama gives your local Git workflow a soundtrack. Commits, pushes, pulls, merges, branch changes, and rebases play clips from your chosen voice pack. It works through Git's hooks, so your terminal, editor, or coding agent can keep using the same Git commands.

<a id="install"></a>

## Quick start

You need **Git 2.54+**. The npm installer also needs **Node.js 22+** and npm.

```sh
npm install --global gitsama
```

Open a new terminal, check your setup, and hear your first sound:

```sh
gitsama doctor
gitsama test pull
```

That's it. **Naruto is selected by default.** Keep working as usual:

```sh
git add .
git commit -m "feat: give my workflow a soundtrack"
git push
```

The installer sets up GitSama for your user with a prebuilt binary and bundled packs. Rust and administrator access are not required.

<details>
<summary><strong>Other install options: Linux installer and source checkout</strong></summary>

### Linux installer

```sh
curl -fsSL https://raw.githubusercontent.com/tu-tu-op/GitSama/main/install.sh | sh
```

### Install from a source checkout

Clone the repository:

```sh
git clone https://github.com/tu-tu-op/GitSama.git
cd GitSama
```

On **Windows PowerShell**:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\install.ps1
```

On **macOS or Linux**:

```sh
chmod +x install.sh
./install.sh
```

These installers build from source when Rust and Cargo are available; otherwise they download a matching release binary. Prebuilt releases support Windows x64, Linux x64, and macOS on Intel and Apple Silicon. The Homebrew tap is not published yet.

See the [installation walkthrough](docs/USAGE.md#install) for more detail.

</details>

## Made for your workflow

- **Keep your Git habits.** Use normal Git commands from your terminal or tools that use local Git and run its hooks.
- **Choose your sound.** Switch bundled packs, import your own clips, or add random variations for an event.
- **Control the noise.** Set the volume, mute a single event, or turn GitSama off in one repository.
- **Keep work moving.** Playback runs separately. Missing clips or audio errors cannot block your Git operations.

## Voice packs

Three voice packs come bundled, alongside simple Starter tones for checking audio.

| Pack | Select it | Good to know |
| --- | --- | --- |
| **Naruto** | `gitsama use naruto` | Default pack, with a dedicated Pull clip. |
| **Bleach** | `gitsama use bleach` | Pull uses the Merge clip as a fallback. |
| **Ravi Kishan** | `gitsama use ravi-kishan` | Includes a dedicated Pull clip. |
| **Starter** | `gitsama use starter` | Generated tones for testing your setup. |

List your packs, switch, and listen:

```sh
gitsama pack list
gitsama use bleach
gitsama test all
```

Prefer a menu? Run `gitsama packs`. Commands use a pack's lowercase **ID**, such as `ravi-kishan`.

<a id="readme-create-pack"></a>

## Create your own pack

Put a few clips in a folder and name them for the events you want:

```text
My Soundtrack/
  commit-1.mp3
  commit-2.mp3
  push.wav
  pull.ogg
```

Open a terminal in that folder, then run:

```sh
gitsama pack scaffold
gitsama test all
```

GitSama creates the manifest, installs a copy, and selects your new pack. Your original clips stay in place. The example folder becomes the pack ID `my-soundtrack`.

**WAV, MP3, OGG, and FLAC** are supported. Numbered clips become random variations; a single clip is enough to start. Clips can also live in an `audio/` subfolder. Unmapped events stay silent, with Massive Push falling back to Push and Pull falling back to Merge.

[Full walkthrough](docs/USAGE.md#readme-create-pack) · [All filenames and manifest options](docs/PACKS.md) · [Install a shared pack](docs/USAGE.md#install-a-shared-pack-that-contains-packtoml)

<a id="common-commands"></a>

## Everyday commands

| What you want | Command |
| --- | --- |
| See your setup | `gitsama` or `gitsama status` |
| Choose a voice pack | `gitsama packs` or `gitsama use <id>` |
| Preview a sound / all sounds | `gitsama test pull` / `gitsama test all` |
| Set the volume | `gitsama volume 60` |
| Mute / unmute | `gitsama mute` / `gitsama unmute` |
| Silence / enable an event | `gitsama disable pull` / `gitsama enable pull` |
| Disable / enable in this repo | `gitsama off-here` / `gitsama on-here` |
| Change the big-push threshold | `gitsama threshold 25` |
| Delete a pack | `gitsama pack delete <id>` |
| Check / repair your setup | `gitsama doctor` / `gitsama doctor --fix` |
| Update GitSama | `gitsama --upgrade` |

Replace `<id>` with an ID from `gitsama pack list`. Run `gitsama help` for all commands, or open the [complete command reference](docs/USAGE.md#common-commands).

## Nine events. Your soundtrack.

| Event | When it plays |
| --- | --- |
| **Commit** | After a successful commit. |
| **Push** | When a push starts, through Git's pre-push hook. |
| **Massive Push** | Instead of Push when the outgoing commit count reaches your threshold; 50 by default. |
| **Pull** | Once when a pull changes the current branch. |
| **Merge** | After a successful standalone merge. |
| **Branch Create** | When you create a local branch. |
| **Branch Switch** | When you switch branches. |
| **Branch Delete** | When you delete a local branch. |
| **Rebase** | After a standalone rebase rewrites commits. |

> [!TIP]
> A pull plays one Pull sound, including merge-based and rebase-based pulls. An up-to-date pull stays silent. Creating a branch with `git switch -c` also plays just one sound.

[Hook mappings and playback rules](docs/USAGE.md#supported-git-events)

## Local by design

Hooks, playback, and pack previews run on your computer. GitSama has no account requirement, telemetry, or always-running service. It installs per user, adds no files to your projects, and keeps your existing Git hooks in place.

Interactive commands can check public GitHub releases for updates. Set `GITSAMA_NO_UPDATE_CHECK=1` to disable automatic checks. Sharing a pack is an explicit action through `gitsama pack contribute`.

[How it works](docs/ARCHITECTURE.md) · [Privacy and file locations](docs/USAGE.md#local-by-design) · [Security policy](SECURITY.md)

## Documentation

| Guide | What's inside |
| --- | --- |
| [User guide](docs/USAGE.md) | Installation, settings, updates, sharing, uninstalling, and detailed walkthroughs. |
| [Sound packs](docs/PACKS.md) | Supported filenames, `pack.toml`, audio formats, and validation. |
| [Troubleshooting](docs/TROUBLESHOOTING.md) | No sound, Git versions, IDEs, coding agents, and recovery. |
| [Architecture](docs/ARCHITECTURE.md) | Hooks, event detection, playback, and data flow. |
| [Contributing](CONTRIBUTING.md) | Development setup, checks, and pack submissions. |
| [Changelog](CHANGELOG.md) | Changes between releases. |

## FAQ

<details>
<summary><strong>Will this work in my editor or coding agent?</strong></summary>

Yes, when it uses your local Git installation and runs Git's hooks. Tools that bypass hooks do not trigger sounds. For agents that terminate child processes after commands finish, see the [coding-agent troubleshooting guide](docs/TROUBLESHOOTING.md#coding-agents-and-sandboxed-environments).

</details>

<details>
<summary><strong>Does everyone in a repository hear the sounds?</strong></summary>

Only you. GitSama reacts to Git operations on your computer, under your user account. Your teammates need their own installation to hear their own sounds.

</details>

<details>
<summary><strong>Can I use my own recordings, anime, or game clips?</strong></summary>

Yes. Use clips you have permission to use and follow the [pack walkthrough](#create-your-own-pack). Shared packs need their own audio licensing and attribution; GitSama's MIT source license does not cover third-party media.

</details>

<details>
<summary><strong>How do I remove GitSama?</strong></summary>

For an npm install, run `npm uninstall --global gitsama`; it removes GitSama's hooks and preserves your packs and settings. For a standalone or source install, run `gitsama uninstall`, or `gitsama uninstall --keep-data` to preserve your packs and settings. See the [full uninstall guide](docs/USAGE.md#uninstall).

</details>

[More questions and answers](docs/USAGE.md#faq)

## Build with us

Bug reports, improvements, and voice packs are welcome. Start with [Contributing](CONTRIBUTING.md), [report a bug](https://github.com/tu-tu-op/GitSama/issues/new), or [share a tested pack](docs/USAGE.md#share-a-voice-pack-optional).

For pack contributions, include the audio's source, attribution, and permission to redistribute it.

---

<p align="center">
  <strong>Your workflow. Your sound.</strong><br>
  <sub>Source code: <a href="LICENSE">MIT</a> · Voice packs and audio retain their own licenses.</sub><br><br>
  <a href="#gitsama">Back to top ↑</a>
</p>
