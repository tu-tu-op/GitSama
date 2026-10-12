# Publishing GitSama releases

Pushing a development branch does not update existing installations. Merge the
release changes into `main`, then publish a version tag. The release workflow
builds the Windows, Linux, and macOS binaries and uploads their SHA-256 checksums
and build provenance attestations to GitHub Releases. It uploads all assets to a
draft before publishing. Repository release immutability is enabled, so a
published release's assets and tag cannot be replaced. By default, a maintainer
publishes the matching npm package from their PC after those files are ready.

## Prepare a version

Keep these values aligned:

- The package version in `Cargo.toml` and the `gitsama` entry in `Cargo.lock`.
- The version in `npm/package.json`.
- The default version in `install.ps1` and `install.sh`.
- The release tag, such as `v0.1.2`.

Update `CHANGELOG.md` and review the Windows, Linux, and macOS CI and Security
results before merging. The workflow rejects a tag whose commit is not merged into `main` or
whose version does not match the package and installer versions.

## Publish 0.1.2

After the release branch has been merged, run from the repository:

```powershell
git switch main
git pull --ff-only origin main
git tag v0.1.2
git push origin v0.1.2
```

Watch the **Release** workflow in GitHub Actions. Wait for **Publish GitHub
Release** to succeed and check that the release has binaries and checksums for
Windows x64, Linux x64, macOS Intel, and macOS Apple Silicon.

Then publish npm from the same repository checkout:

```powershell
npm login --auth-type=web --registry=https://registry.npmjs.org
cd npm
npm run prepare-release
npm pack --dry-run
npm publish --access public --registry=https://registry.npmjs.org
cd ..
npm view gitsama version --registry=https://registry.npmjs.org
```

Complete any npm authentication prompts in the browser. The final command should
show `0.1.2`. Announce the update after both GitHub and npm publication are complete.

`prepare-release` downloads the four raw binary checksums from the matching
published GitHub release and writes `npm/release-checksums.json`. This generated
file is ignored by Git and included in the npm package. Review it and confirm
`npm pack --dry-run` includes it. `prepublishOnly` refuses a missing or invalid
manifest, and postinstall verifies against these packaged hashes rather than a
separately downloaded checksum. Re-run preparation after changing the version.

If you downloaded the build artifacts locally, verify their binaries and pin
their hashes without fetching another copy of the checksums:

```sh
npm run prepare-release -- ../release-assets
```

The optional CI npm job uses this route with artifacts from the same workflow
run that created the immutable GitHub release. To verify the build provenance of
a downloaded asset with GitHub CLI:

```sh
gh attestation verify ./gitsama-v0.1.2-windows-x86_64.exe --repo tu-tu-op/GitSama
```

Users running 0.1.0 need one update through npm or their source installer to get
the upgrade command. Later versions can use `gitsama --upgrade` or accept the
interactive notification. A package published on npm must have the matching
GitHub release binaries; its install script downloads those exact assets.
Standalone upgrades require an immutable release. The repository setting does
not make already published 0.1.0 or 0.1.1 releases immutable retroactively.

## Optional: publish npm automatically

The npm job is disabled by default. To enable it, configure npm trusted
publishing and then set the GitHub repository Actions variable
`NPM_TRUSTED_PUBLISHING` to `true` under **Settings → Secrets and variables →
Actions → Variables**.

In the `gitsama` package settings on npmjs.com, add a GitHub Actions trusted
publisher with:

| Setting | Value |
| --- | --- |
| Repository owner | `tu-tu-op` |
| Repository | `GitSama` |
| Workflow filename | `release.yml` |
| Environment | Leave empty; this workflow does not declare an environment. |
| Allowed action | Enable direct publishing with `npm publish`. |

Save both settings before pushing a release tag. The workflow grants
`id-token: write` and uses Node 24 with pinned npm 11.21.0. It publishes npm after
the GitHub release and skips versions already present in the registry when rerun. A local
npm login is not needed for this route. See the official
[npm trusted publishing instructions](https://docs.npmjs.com/trusted-publishers/).

## If publishing fails

If a platform build fails, fix it before publishing a new version. If GitHub
publishing succeeds and npm authentication fails, sign in again and repeat the
npm publication step. If using automatic publication, correct the trusted
publisher settings and rerun the failed npm job. Keep the existing published
version tag.

Do not rerun asset publication for a completed immutable release or try to
replace its binaries. A code or packaging correction after publication needs a
new version and tag. Retry only the failed npm job from the original workflow
run when GitHub publication has already succeeded.

Do not publish a development package with a version that has no release assets.
The Homebrew tap is not published yet; it requires its own formula and release
process before it can be offered as an installation method.
