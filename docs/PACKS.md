# Sound packs

A GitSama pack is a data-only directory. It contains a manifest and audio files; no manifest value is executed.

Fresh installations include the bundled Naruto, Bleach, and Ravi Kishan voice packs plus generated Starter tones. The bundled packs are materialized into the user's GitSama data directory on first setup. Naruto is selected by default; custom packs remain supported through the normal import commands.

## Directory layout

```text
my-pack/
├── pack.toml
├── README.md
└── audio/
    ├── commit-1.wav
    ├── commit-2.wav
    ├── push.mp3
    └── massive-push.flac
```

## Manifest schema

The current schema is version 1:

```toml
schema_version = 1

id = "my-pack"
name = "My Pack"
author = "Your Name"
version = "1.0.0"
description = "Reactions for my local Git workflow"
license = "CC BY 4.0"

[events]
commit = [
    "audio/commit-1.wav",
    "audio/commit-2.wav",
]
push = ["audio/push.mp3"]
massive_push = ["audio/massive-push.flac"]
merge = ["audio/merge.wav"]
branch_create = ["audio/branch-create.wav"]
branch_delete = ["audio/branch-delete.wav"]
branch_switch = ["audio/branch-switch.wav"]
rebase = ["audio/rebase.wav"]
pull = ["audio/pull.wav"]
```

The required metadata fields are schema_version, id, name, author, version, description, and license. Event keys are exactly commit, push, massive_push, merge, branch_switch, branch_create, branch_delete, rebase, and pull. Every value is an array of relative paths.

Pack IDs contain 1-64 lowercase letters, digits, hyphens, and underscores, and cannot start with a hyphen. Windows device names such as `con`, `nul`, `com1`, and `lpt1` are reserved on all platforms. `starter`, `naruto`, `bleach`, and `ravi-kishan` are reserved for built-in packs during folder imports. An ID is the installed directory name; commands such as `gitsama use` take this lowercase ID. Event paths may contain directories but must stay inside the pack root.

## Audio

GitSama decodes WAV, MP3, OGG, and FLAC with its built-in Rust audio stack. A file must exist, have one of those extensions, and be a regular file. The pack validator checks mappings and files; previews additionally open the audio decoder and play the selected clip. The pack validator rejects absolute paths, parent traversal, unsafe symlinks, and files outside the pack directory.

Several files can be listed for one event. GitSama chooses one at random for each event. An event with no mapping is silent. If massive_push has no usable mapping but push does, massive_push falls back to push. This also applies when massive_push is disabled while push remains enabled.

If pull has no mapping but merge does, Pull uses a Merge clip. This keeps older packs usable without changing their manifests. Pull has its own enable switch: disabling Pull silences pulls, and disabling Merge does not silence an enabled Pull event. Naruto and Ravi Kishan have dedicated Pull clips; Bleach uses Merge fallback. Pull plays only when the current branch changes during a pull, with the other internal hook sounds suppressed.

The Starter pack is generated from simple PCM tones on first setup. It contains no anime media and exists for installation checks, development, and CI. The bundled Naruto Voice Pack is included with permission to redistribute its audio and covers all v1 events.

## Import a folder of clips

Open a terminal inside a folder containing renamed clips:

```sh
cd "Bleach Custom"
gitsama pack scaffold
gitsama test all
```

`scaffold [name]` preserves the folder name as the display name and derives a lowercase ID. `gitsama pack scaffold "Bleach Bankai"` overrides the display name and installs as `bleach-bankai`. It copies clips, generates a schema-version-1 `pack.toml` and README, validates a temporary pack, then installs it under GitSama's `packs/<id>/` directory and selects it. The command prints the installed path. Source clips and source-folder history stay in their original folder.

Discovery checks the current folder and its direct `audio/` subfolder. Other files and deeper directories are ignored. Supported audio filenames must match an event:

| Event | Example filenames |
| --- | --- |
| Commit | `commit.wav`, `Commit-1.MP3`, `commit_2.ogg` |
| Push | `push.mp3`, `push-2.flac` |
| Massive Push | `massive-push.wav`, `massive_push_1.mp3` |
| Merge | `merge.flac`, `merge-1.wav` |
| Branch Switch | `branch-switch.mp3`, `branch_switch_2.wav` |
| Branch Create | `branch-create.ogg`, `branch_create-1.mp3` |
| Branch Delete | `branch-delete.wav`, `branch_delete_1.flac` |
| Rebase | `rebase.mp3`, `rebase-3.wav` |
| Pull | `pull.mp3`, `pull-2.wav` |

Extensions and event names are case insensitive. Numeric suffixes must be positive integers. Multiple variants map to the same event and playback chooses one at random. A folder needs at least one mapped event; empty event arrays remain silent, with massive push falling back to push and Pull falling back to Merge.

Scaffolding stops before installation for unmatched supported audio filenames, case-insensitive basename collisions between folders, symlinks or nonregular audio files, reserved IDs, or an already installed ID. The error explains what to rename or fix. For example, rename `bankai.mp3` to `commit.mp3`, and distinguish two `commit.mp3` clips as `commit-1.mp3` and `commit-2.mp3`.

To update an imported pack, edit its installed copy at the printed path, or explicitly remove it and scaffold again from the source folder. Editing the source folder does not update the installed copy.

## Create an empty template

```sh
gitsama pack scaffold --template "My Custom Pack"
cd my-custom-pack
# add files under audio/ and edit pack.toml
gitsama pack validate .
gitsama pack add .
gitsama use my-custom-pack
gitsama test all
```

Template mode creates empty event arrays and requires a name. Its optional directory is a parent directory:

```sh
gitsama pack scaffold --template "My Custom Pack" ./packs
```

This creates ./packs/my-custom-pack.

An empty template can validate but plays no clips until its event arrays are populated. `pack add` installs the manually authored directory; `gitsama use my-custom-pack` selects its ID.

## Install and remove

Validate before importing. Import copies the validated directory into the user's GitSama home:

```sh
gitsama pack add ./my-custom-pack
gitsama pack list
gitsama pack delete my-custom-pack
```

The importer refuses to replace an existing pack. Remove the old pack explicitly before importing an updated copy.

`pack delete <id>` and `pack remove <id>` delete any installed pack, including `naruto`, `bleach`, `ravi-kishan`, and `starter`. The installed directory and local preview/contribution records are removed; the original source folder is preserved. Invalid IDs, paths outside the installed packs directory, and symlink pack roots are rejected. Broken packs can be deleted even if their manifest no longer validates.

Deleting the active pack selects a remaining valid pack, preferring Naruto, then Starter, then another remaining pack. Deleting the last pack clears the active selection and mutes playback. Afterward, selecting or scaffolding a pack resumes playback.

Deleted built-ins stay absent during `pack list`, `setup`, and `doctor --fix`. To restore one, run `gitsama use <id>` with `naruto`, `bleach`, `ravi-kishan`, or `starter`; this reinstalls and selects that built-in pack.

## Preview and contribute

`gitsama test all` previews one randomly selected clip for each event, including massive-push fallback. It prints `Played` after the clip finishes and identifies fallback playback. Missing mappings, muting, zero volume, and queue timeouts are reported as `Skipped`; decoder or output-device errors fail the preview. Event enable switches apply to Git hooks; explicit previews let you listen to mapped events.

A completed full preview requires at least one completed clip and completion of every available event preview. Missing mappings in a partial pack are allowed. Single-event tests, muted or zero-volume runs, timeouts, and errors never create a successful full-preview record. A failed full preview clears the previous success. Manifest or referenced-audio changes require another full preview before contribution.

The first successful full preview of a locally scaffolded installation prints `gitsama pack contribute <id>` once. Built-in packs and manually imported packs do not receive that invitation. Manually authored custom packs can still be contributed after a successful full preview.

Install GitHub CLI, authenticate, then submit with attribution:

```sh
gh auth login --hostname github.com
gitsama pack contribute my-custom-pack --author "Contributor" --license "CC0-1.0" --source "Original recordings; released under CC0-1.0 for redistribution."
```

Interactive use collects missing metadata; noninteractive use requires flags for missing values. Author and license can default to real values in the manifest. `--source` must explain where all audio came from and why it may be redistributed. Overrides are written to the submitted manifest and README.

GitSama uses a fresh isolated checkout of `tu-tu-op/GitSama`'s current default branch, creates or reuses your fork, and submits a unique branch. Repository owners submit through a new upstream branch. IDs already present upstream are rejected. The submission contains the manifest, generated contribution README, and referenced audio; staged and committed diffs must consist of additions within this new pack. PR creation uses explicit repository, base, head, title, and body-file arguments, as documented by [GitHub CLI](https://cli.github.com/manual/gh_pr_create).

The PR URL is printed and saved. Repeating the command shows the saved URL. Failed submissions retain their preparation directory under GitSama's `state/`; use its `RECOVERY.md` when available. Creation, preview, invitation, and submission records are stored in `state/pack-<id>.json` outside pack files and cleared when the pack is removed. A scaffolded reinstallation receives a new invitation after its first successful full preview.

## Safety and copyright

Manifests cannot contain shell commands, executable commands, or remote download instructions. GitSama never executes pack files.

Do not include copyrighted anime dialogue, music, or sound effects in this repository without permission to redistribute them. You can create a local pack from media you are legally allowed to use. A public pack contribution must include permission to redistribute every audio file and clear license information. The GitSama MIT license does not apply automatically to pack audio.
