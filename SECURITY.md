# Security policy

## Report a vulnerability

Use [GitHub's private vulnerability reporting](https://github.com/tu-tu-op/GitSama/security/advisories/new) for security issues. Include the GitSama version, installation method, operating system, affected command, and a minimal reproduction using disposable files and repositories. Explain the expected behavior and observed impact.

Do not put working credentials, private repository contents, or an unreleased exploit in a public issue. Ordinary bugs can use GitHub Issues. Security fixes land on the default branch and ship in a new release; older versions do not receive automatic backports.

## Security boundary

GitSama is a local CLI that runs with the current user's permissions. Its Git hooks must leave Git operations working when playback, pack validation, state, or update checks fail. Packs are data: manifests and audio are parsed, and pack files are never executed. Explicit pack contributions use the user's authenticated GitHub CLI.

GitSama is not a sandbox for an already compromised user account or a malicious audio decoder vulnerability. Keep the OS, Git, Node.js, and GitSama updated. Install the npm package from npmjs.com or binaries from this repository's releases.

## Protections in 0.1.2

- npm upgrades invoke an absolute npm CLI through Node, outside the caller's repository. They do not launch `npm.cmd` from a project folder.
- npm packages include hashes for their exact release binaries. Postinstall rejects bytes that do not match those packaged hashes, even if a remote checksum file was also replaced.
- Standalone upgrades require GitHub to mark the release immutable, verify its SHA-256 checksum and asset digest when present, and check the candidate's version before replacement.
- Release assets receive build provenance attestations. The workflow builds and tests with read-only source permissions, uploads a complete draft, then publishes it. Only the publishing job can write releases.
- Actions are pinned to commit hashes. CI audits locked Rust dependencies, scans reachable Git history with redacted output, and checks the Windows, Linux, and macOS product flows. Dependabot tracks Cargo dependencies and Actions.
- Logs omit hook arguments, bound each entry, and remove control characters. Unix data directories use mode `0700`; state/config files and logs use `0600`. State changes use atomic replacement.
- Pack imports stage only a validated manifest, referenced audio, and an optional README. Path traversal, links, nonregular files, terminal controls, and bidirectional control characters are rejected.
- File counts, file sizes, hook input, queue waits, and playback have limits. See [pack limits](docs/PACKS.md#resource-limits).
- Uninstall requires consent, checks that the data directory belongs to GitSama, and refuses user-home roots, repositories, the caller's working directory, and unrelated top-level files. Windows self-removal uses no command shell.

## Compatibility and limits

The npm package requires Node.js 22 or newer. Noninteractive uninstall requires `--yes`; npm uninstall passes it while preserving user data. A custom `GITSAMA_HOME` must be a dedicated real directory, not a symlink, filesystem root, user home, or Git repository. Normal commands still work from inside an installed pack; leave that directory before deleting the entire GitSama home.

Pack audio is limited to 100 MiB per file and 512 MiB total, with at most 4,096 event mappings. Manifests and optional imported READMEs are limited to 1 MiB each. Paths use `/`, at most 1,024 bytes and 32 components. A clip plays for at most ten minutes; the existing linger limit can stop it sooner. Queue waits are capped at 30 seconds. Hook input is capped at 4 MiB and one reference transaction can dispatch at most 128 branch events.

These limits reduce resource exhaustion; they do not promise that every malformed audio file can be decoded safely or cheaply. The locked dependency audit checks published advisories and cannot detect every new vulnerability. Disabling update notifications does not disable an explicit upgrade.

## Repository and publisher administration

On 2026-10-12, release immutability, private vulnerability reporting, dependency graph, Dependabot alerts, malware alerts, and Dependabot security updates were enabled for this repository. GitHub secret scanning and push protection were already enabled. Immutable releases apply to future publications; previously published mutable releases are not retroactively protected.

The active `Protect main` ruleset requires all five CI/security checks from
GitHub Actions and requires branches to be up to date before merging. Its
existing PR requirement, force-push block, deletion restriction, and empty
bypass list are preserved. The new security checks run when this branch is
pushed; a local commit alone cannot produce their GitHub check results.

Before publishing the hardened version:

1. Review CI for all supported platforms and keep `Rust dependency audit`, `Secret scan`, `Check (ubuntu-latest)`, `Check (windows-latest)`, and `Check (macos-latest)` required in the main branch ruleset. The PR must contain the security workflow for its new checks to report. Keep force pushes and branch deletion blocked. For a solo maintainer, require passing checks and a PR without requiring an unavailable second reviewer.
2. Protect maintainer GitHub and npm accounts with strong two-factor authentication, preferably a passkey or hardware security key. Remove unused publishing tokens and review collaborators, deploy keys, and installed apps. Repository code cannot enforce account-level authentication.
3. Prefer npm trusted publishing when configured; otherwise complete npm's authentication prompts personally. Follow [release instructions](docs/RELEASING.md) to embed the correct binary hashes before publishing. Never reuse a published version or replace its release assets.

Published users receive these code protections after upgrading to a release containing them. A source branch or local commit does not update existing installations.
