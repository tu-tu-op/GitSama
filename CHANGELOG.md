# Changelog

All notable GitSama changes are recorded here.

## 0.1.2

- Preinstall Naruto, Bleach, and Ravi Kishan voice packs, preserving their clip mappings and supporting deletion and explicit restoration. Naruto remains the initial selection and includes its dedicated Pull soundtrack.
- Add a Pull sound that plays once when a pull changes the current branch, with independent controls and Merge clip fallback for older packs. Up-to-date pulls stay silent; standalone merge and rebase behavior is preserved.
- Make the README easier to browse with collapsible guides, quick navigation, and a Pull usage reference while preserving the existing documentation.

## 0.1.1

- Import folders of event-named WAV, MP3, OGG, or FLAC clips with `pack scaffold`; install and select the resulting pack automatically.
- Report completed, missing, fallback, and skipped playback accurately during previews.
- Invite creators to contribute after a successful full preview and submit only the tested pack through an isolated GitHub PR.
- Delete any installed voice pack, including built-ins, with `pack delete`; preserve source clips and support explicit restoration.
- Offer newer stable versions after interactive commands, with daily checks and an optional update prompt.
- Add `gitsama --upgrade` and `gitsama upgrade --check` / `--yes`, preserving packs and settings during upgrades.
- Keep package versions and installer defaults aligned; publish npm after release binaries are available, with optional trusted publishing in CI.
- Rebuild source installations when rerunning an installer so an old build cannot hide new changes.

## 0.1.0

Initial release:

- local, per-user Git reactions for commit, push, massive push, merge, branch switch, branch create, branch delete, and rebase
- Git 2.54 named configured hooks with coexistence for traditional repository hooks
- fail-open asynchronous playback with a bounded cross-process queue
- generated Starter tone pack and data-only custom pack tooling
- bundled Naruto Voice Pack selected by default for first-run playback
- pack validation, scaffold, import, selection, and event fallback
- global mute and event controls
- repository-local off-here and on-here controls
- doctor diagnostics, isolated test mode, installers, and release automation
