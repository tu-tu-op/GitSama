# Contributing to GitSama

Thanks for helping make GitSama useful and joyful.

## Development setup

Install Rust stable and Git. Clone the repository, then run:

```sh
cargo build
cargo test
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

GitSama targets Git 2.54 or newer because it uses Git's named configured hook system. The test suite isolates global Git configuration with temporary paths. It also supports a no-device test mode through GITSAMA_TEST_MODE=1 and GITSAMA_TEST_LOG.

## Code style

Keep runtime hook paths small, fail-open, silent, and free from panics. Use the standard library where it keeps the implementation clear. Do not introduce network services, telemetry, background daemons, or repository working-tree files.

Add meaningful tests for new behavior. Run formatting, clippy with warnings denied, and the full test suite before opening a pull request.

## Pull requests

Describe the user-visible behavior, the supported platforms you exercised, and the checks you ran. Keep commits focused and avoid unrelated formatting churn. Documentation should be updated with behavior changes.

For bug reports, include the GitSama version, operating system, Git version, command that exposed the issue, and safe output from gitsama doctor. Remove repository names, private URLs, commit messages, credentials, and personal paths before posting.

## Pack contributions

Packs are data-only directories. Do not add executable files or commands to a manifest. Community pack submissions must include audio the contributor has permission to redistribute, with clear licensing or permission information. Do not submit copyrighted anime dialogue or music without redistribution rights.

GitSama's source code is MIT licensed. That license does not automatically apply to user-provided pack audio.

See docs/PACKS.md for the format and validation rules.

### Submit a folder-based voice pack

Rename your clips for events such as `commit-1.mp3`, `push.wav`, or `branch-switch.ogg`, then run inside their folder:

```sh
gitsama pack scaffold "My Voice Pack"
gitsama test all
```

Scaffolding installs and selects the pack. After a successful full preview it offers `gitsama pack contribute my-voice-pack`. Supply attribution and redistribution details:

```sh
gh auth login --hostname github.com
gitsama pack contribute my-voice-pack --author "Contributor" --license "CC0-1.0" --source "Original recordings by me; released under CC0-1.0."
```

GitHub CLI must be installed and authenticated. Interactive submission prompts for missing metadata; automation must provide all missing fields through the flags. The source description should identify every clip's origin and include source links and permission details where applicable. Use the actual media license; the example above applies to original recordings you have chosen to release under CC0.

The command starts from upstream's current default branch in an isolated checkout, uses your fork (or a new upstream branch for repository owners), verifies additions under `packs/my-voice-pack/`, and creates a PR. It exports the pack manifest, a generated README containing your metadata, and referenced audio. It prints and retains the PR URL; failures retain prepared work with recovery instructions.

For a manually authored pack, use `gitsama pack scaffold --template "My Voice Pack" [parent]`, populate `pack.toml`, then validate, add, select, and run `gitsama test all` before contributing. Repeat the full preview after changes to the installed manifest or audio. Choose a new ID if that ID already exists upstream.

### Pack PR checklist

- The manifest uses schema version 1 and a unique lowercase ID.
- Every referenced clip can be decoded and the pack has completed `gitsama test all`.
- Author, version, description, audio licensing, source links, and redistribution permission are accurate.
- The diff adds only the new pack's manifest, contribution README, and referenced audio under `packs/<id>/`.

Pack submissions are reviewed as contributions. Adding a community pack to the repository does not automatically embed it in the binary as a bundled default.

