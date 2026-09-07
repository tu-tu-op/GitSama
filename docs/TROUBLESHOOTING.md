# Troubleshooting

Start with:

```sh
gitsama doctor
gitsama status
gitsama test commit
```

## Git version

GitSama requires Git 2.54 or newer. If setup detects an older version, upgrade Git and rerun gitsama setup. No GitSama hook configuration is written when the check fails.

## No sound

Check that the active pack exists and validates:

```sh
gitsama pack list
gitsama pack validate ~/.gitsama/packs/my-pack
gitsama test all
```

Check the log path printed by gitsama doctor. The audio backend needs an available output device. CI and headless testing can use GITSAMA_TEST_MODE=1.

## Git operation still works

This is intentional. GitSama hooks are fail-open. If a pack, state file, log, decoder, or audio device fails, Git continues and the diagnostic log records a short reason.

## IDE or coding agent

GitSama reacts when the tool invokes the same local Git installation with hooks enabled. Confirm the IDE's Git path and check whether the operation uses a hook-bypass option such as --no-verify.

## Repository is quiet

Run gitsama status inside the repository. off-here stores local hook enable overrides in .git/config; on-here removes them. No working-tree file is created.

## Safe diagnostics

Share only GitSama version, operating system, Git version, and redacted gitsama doctor output. Do not share repository names, private remote URLs, commit messages, credentials, or private pack media.

## Removing GitSama

Use gitsama uninstall for a complete removal. Use gitsama uninstall --keep-data when you want to remove the executable and hooks but keep packs and configuration for a later reinstall.
