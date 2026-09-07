# Sound packs

A GitSama pack is a data-only directory. It contains a manifest and audio files; no manifest value is executed.

## Directory layout

§§§text
my-pack/
├── pack.toml
├── README.md
└── audio/
    ├── commit-1.wav
    ├── commit-2.wav
    ├── push.mp3
    └── massive-push.flac
§§§

## Manifest schema

The current schema is version 1:

§§§toml
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
§§§

The required metadata fields are schema_version, id, name, author, version, description, and license. Event keys are exactly commit, push, massive_push, merge, branch_switch, branch_create, branch_delete, and rebase. Every value is an array of relative paths.

Pack ids use lowercase letters, digits, hyphens, and underscores. An id is the installed directory name. Event paths may contain directories but must stay inside the pack root.

## Audio

GitSama decodes WAV, MP3, OGG, and FLAC with its built-in Rust audio stack. A file must exist, have one of those extensions, and be a regular file. The pack validator rejects absolute paths, parent traversal, unsafe symlinks, and files outside the pack directory.

Several files can be listed for one event. GitSama chooses one at random for each event. An event with no mapping is silent. If massive_push has no usable mapping but push does, massive_push falls back to push. This also applies when massive_push is disabled while push remains enabled.

The Starter pack is generated from simple PCM tones on first setup. It contains no anime media and exists for installation checks, development, and CI.

## Create a pack

§§§sh
gitsama pack scaffold "My Custom Pack"
cd my-custom-pack
# add files under audio/ and edit pack.toml
gitsama pack validate .
gitsama pack add .
gitsama use my-custom-pack
gitsama test all
§§§

The optional scaffold directory is a parent directory:

§§§sh
gitsama pack scaffold "My Custom Pack" ./packs
§§§

This creates ./packs/my-custom-pack.

## Install and remove

Validate before importing. Import copies the validated directory into the user's GitSama home:

§§§sh
gitsama pack add ./my-custom-pack
gitsama pack list
gitsama pack remove my-custom-pack
§§§

The importer refuses to replace an existing pack. Remove the old pack explicitly before importing an updated copy.

## Safety and copyright

Manifests cannot contain shell commands, executable commands, or remote download instructions. GitSama never executes pack files.

Do not include copyrighted anime dialogue, music, or sound effects in this repository. You can create a local pack from media you are legally allowed to use. A public pack contribution must include permission to redistribute every audio file and clear license information. The GitSama MIT license does not apply automatically to pack audio.

