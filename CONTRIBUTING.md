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

