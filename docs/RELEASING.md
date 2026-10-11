# Publishing GitSama releases

Pushing a development branch does not update existing installations. Merge the
release changes into `main`, then publish a version tag. The release workflow
builds the Windows, Linux, and macOS binaries, uploads their SHA-256 checksums to
GitHub Releases, and publishes the matching npm package afterward.

## Configure npm publishing once

The workflow uses npm trusted publishing; a local npm login or an npm token is
not required for this workflow.

In the `gitsama` package settings on npmjs.com, add a GitHub Actions trusted
publisher with:

| Setting | Value |
| --- | --- |
| Repository owner | `tu-tu-op` |
| Repository | `GitSama` |
| Workflow filename | `release.yml` |
| Environment | Leave empty; this workflow does not declare an environment. |
| Allowed action | Enable direct publishing with `npm publish`. |

Save the configuration before pushing the release tag. The workflow grants
`id-token: write` and uses Node 24 with npm 11. See the official
[npm trusted publishing instructions](https://docs.npmjs.com/trusted-publishers/).

## Prepare a version

Keep these values aligned:

- The package version in `Cargo.toml` and the `gitsama` entry in `Cargo.lock`.
- The version in `npm/package.json`.
- The default version in `install.ps1` and `install.sh`.
- The release tag, such as `v0.1.1`.

Update `CHANGELOG.md` and review the Windows, Linux, and macOS CI results before
merging. The workflow rejects a tag whose commit is not merged into `main` or
whose version does not match the package and installer versions.

## Publish 0.1.1

After the release branch has been merged, run from the repository:

```powershell
git switch main
git pull --ff-only origin main
git tag v0.1.1
git push origin v0.1.1
```

Watch the **Release** workflow in GitHub Actions. Wait for the binaries and npm
package to finish publishing before announcing the update. The npm publish job
skips versions already present in the registry when rerun.

Users running 0.1.0 need one update through npm or their source installer to get
the upgrade command. Later versions can use `gitsama --upgrade` or accept the
interactive notification. A package published on npm must have the matching
GitHub release binaries; its install script downloads those exact assets.

## If publishing fails

If a platform build fails, fix it before publishing a new version. If GitHub
publishing succeeds and npm authentication fails, correct the trusted publisher
settings and rerun the failed npm job. Keep the existing published version tag.

For a manual npm publication from an authenticated PC, wait for all release
assets, then run from `npm/`:

```sh
npm login
npm publish --access public
```

Do not publish a development package with a version that has no release assets.
The Homebrew tap is not published yet; it requires its own formula and release
process before it can be offered as an installation method.
