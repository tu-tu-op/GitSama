# GitSama for npm

Install GitSama globally with Node.js 18 or newer and Git 2.54 or newer:

```sh
npm install --global gitsama
```

The install script downloads the matching prebuilt GitSama binary, checks its
SHA-256 digest, and registers Git's named hooks for the current user. Rust is
not required. The binary includes Naruto, Bleach, and Ravi Kishan voice packs.

After installation, run `gitsama doctor` to verify the setup. For all commands,
custom sound packs, supported platforms, and source installation, see the
[GitSama README](https://github.com/tu-tu-op/GitSama#readme).

## Updates

GitSama checks for updates after interactive commands, at most once a day. When
a new stable release is available, it asks whether you want to update. Accepting
updates the global npm package after the native binary exits, preserving your
voice packs and settings.

```sh
gitsama --upgrade
gitsama upgrade --check
```

Version 0.1.0 users need one npm update to obtain this command:

```sh
npm install --global gitsama@latest
```

Set `GITSAMA_NO_UPDATE_CHECK=1` to disable automatic checks. Hooks and
noninteractive commands do not check automatically. Local npm installations
print the update command to run in their project.
