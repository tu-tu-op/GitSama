# GitSama for npm

Install GitSama globally with Node.js 18 or newer and Git 2.54 or newer:

```sh
npm install --global gitsama
```

The install script downloads the matching prebuilt GitSama binary, checks its
SHA-256 digest, and registers Git's named hooks for the current user. Rust is
not required. The binary includes the default Naruto voice pack.

After installation, run `gitsama doctor` to verify the setup. For all commands,
custom sound packs, supported platforms, and source installation, see the
[GitSama README](https://github.com/tu-tu-op/GitSama#readme).
