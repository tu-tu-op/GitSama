# GitSama — Agent Working Context

> **Token-Optimized Architectural Blueprint for AI Coding Agents**  
> *Target: Instant operational understanding with minimum token consumption.*

---

## 1. Executive Summary

- **What it is**: Zero-daemon, local-only Rust CLI tool playing audio reactions (sound packs) to Git actions.
- **What it does**: Intercepts 6 native Git hooks, detects 8 workflow events (commit, push, massive push, merge, branch switch/create/delete, rebase), and plays audio without blocking Git.
- **How it does it**: Uses Git 2.54+ user-level configured hooks (`hook.*`), detached worker processes (`__play`), an atomic filesystem directory lock (`PlaybackLock`), and a stateful debounce machine to eliminate hook duplicate events.

---

## 2. Tech Stack & Dependencies

| Component | Choice | Purpose |
|---|---|---|
| **Language / Edition** | Rust 1.85+ (2024 edition) | Strict safety, fast startup (<10ms), zero runtime runtime overhead |
| **CLI Framework** | `clap 4.5` (derive) | Argument parsing for user commands & hidden hook worker commands |
| **Audio Engine** | `rodio 0.21` | In-process audio decoding/playback (`wav`, `mp3`, `ogg`, `flac`) |
| **Config / State Formats** | `toml 0.9`, `serde_json 1.0` | `config.toml`, `pack.toml`, JSON debounce state |
| **Platform Bindings** | `windows-sys 0.61` | Win32 ToolHelp32 snapshot (parent PID tracking across MSYS forks) |
| **Directories** | `dirs 4.0` | Cross-platform resolution of `~/.gitsama` |
| **Error Handling** | `thiserror 2.0` | Custom strongly typed `Error` enum |

---

## 3. Directory & File Map

```
c:\Projects\GitSama\
├── Cargo.toml / Cargo.lock      # Build config; release profile: LTO=true, strip=true, codegen-units=1
├── install.ps1 / install.sh     # User installer (checks Git >=2.54, builds/copies binary, runs setup)
├── packs/naruto/                # Bundled default sound pack (pack.toml + 8 MP3s embedded in binary)
├── docs/                        # Architecture, troubleshooting, pack docs
├── tests/git_integration.rs     # Integration test suite driving real Git processes
└── src/
    ├── main.rs                  # Entrypoint: catch_unwind wrapper around cli::run; exits 0 or 1
    ├── cli.rs                   # Clap definitions: public commands + hidden hook worker entrypoints
    ├── hooks.rs                 # Git hook registration, shell quoting, event dispatch & debounce logic
    ├── events.rs                # EventKind enum (8 variants) & string parsers
    ├── audio.rs                 # Detached playback spawn, rodio pipeline, atomic cross-process lock
    ├── push.rs                  # Pre-push stdin parser + local rev-list commit counter (zero network)
    ├── state.rs                 # ~/.gitsama/state file management (atomic claims for pending branches)
    ├── packs.rs                 # Pack manifest validation, traversal protection, audio path resolver
    ├── config.rs                # ~/.gitsama/config.toml load/save/validate + defaults
    ├── git.rs                   # Git runner, version detection (min 2.54), repo root/identity lookup
    ├── platform.rs              # Win32 vs POSIX: detached spawn, POSIX sh path quoting, PID resolution
    ├── doctor.rs                # Health check & auto-repair logic for config, binary, audio, hooks
    ├── paths.rs                 # AppPaths layout discovery (~/.gitsama or GITSAMA_HOME override)
    ├── logging.rs               # Append-only file logger for ~/.gitsama/logs/gitsama.log
    └── error.rs                 # Error enum & Result type
```

---

## 4. Event & Hook Lifecycle

```mermaid
flowchart TD
    Git[Git Operation] -->|Fires hook| GH[Git Configured Hook: hook.gitsama-*]
    GH -->|POSIX shell guard: '... hook <evt> $@ >/dev/null 2>&1 || :'| GS[gitsama hook <evt>]
    GS --> CheckRepo{Repo Opt-Out?\nhook.*.enabled=false}
    CheckRepo -- Yes --> ExitZero[Exit 0]
    CheckRepo -- No --> Logic[Filter / Debounce / Classify Event]
    Logic --> Spawner[Spawn Detached Child: gitsama __play <event>]
    Spawner --> ExitGit[Exit 0 immediately - Git unblocked]
    Spawner -.-> Detached[Detached Worker]
    Detached --> Lock{Acquire PlaybackLock\ncreate_dir state/playback.lock}
    Lock -- Stale / Timeout --> DropPlay[Exit]
    Lock -- Acquired --> Audio[Rodio Decode & Play] --> ReleaseLock[Drop Lock]
```

### Hook-to-Event Mapping Matrix

| Git Native Hook | Native Args / Stdin | GitSama Internal Handling | Resolved Event |
|---|---|---|---|
| `post-commit` | None | Direct dispatch | `commit` |
| `pre-push` | Remote name + URL (args), ref list on `stdin` | Parses stdin; runs local `git rev-list <local> --not <remote>` | `push` or `massive_push` (if count ≥ threshold) |
| `post-merge` | Is squash flag (`0`/`1`) | Direct dispatch | `merge` |
| `post-checkout` | Old HEAD, new HEAD, flag (`1`=branch, `0`=file) | Ignored if file checkout or clone init; checks pending branch state | `branch_switch` (or completes `branch_create`) |
| `reference-transaction` | State (`committed`), ref lines on `stdin` | Detects `0000..` -> OID (create) or OID -> `0000..` (delete) | `branch_create` (staged) or `branch_delete` |
| `post-rewrite` | Command name (`rebase` / `amend`) | Dispatches only if arg is `rebase` | `rebase` |

---

## 5. Critical Invariants & Architectural Tricks

### 1. Non-Destructive Hook Registration (Git 2.54+)
- **Does NOT** touch `.git/hooks/*`.
- **Does NOT** set `core.hooksPath`.
- Registers hooks via global git config:
  ```gitconfig
  [hook "gitsama-post-commit"]
      event = post-commit
      command = GITSAMA_GIT_PID="$PPID" '/path/to/gitsama' hook post-commit "$@" >/dev/null 2>&1 || :
      enabled = true
  ```
- Coexists peacefully with Husky, Lefthook, and repository-level hooks.

### 2. Strict Fail-Open Guarantee
- Every hook command in Git config ends with `|| :`.
- `main.rs` wraps all execution in `std::panic::catch_unwind`.
- `hooks.rs` wraps inner handlers in `catch_unwind` and logs to `~/.gitsama/logs/gitsama.log`.
- No failure (missing audio, corrupted config, invalid pack, audio driver crash) will ever abort or slow down a Git operation.

### 3. Branch Creation vs Switch Debounce Machine
- `git switch -c <name>` or `git checkout -b <name>` triggers **both** `reference-transaction` and `post-checkout`.
- **Solution**:
  1. `reference-transaction` sees new branch ref creation, writes a short-lived `PendingBranch` state file to `~/.gitsama/state/`, and spawns `__dispatch-pending` with a 180ms delay.
  2. If `post-checkout` fires within 180ms, it claims/consumes the `PendingBranch` file and immediately triggers `branch_create` (suppressing `branch_switch`).
  3. When `__dispatch-pending` wakes up, its claim fails (file gone) and it exits quietly.
  4. If user ran `git branch <name>` without checking out, `post-checkout` never fires; `__dispatch-pending` claims the file after 180ms and plays `branch_create`.

### 4. Zero-Network Massive Push Counting (`push.rs`)
- Outgoing commit count is determined solely by streaming `git rev-list <local_oid> --not <remote_tracking_tips>`.
- Stops parsing the stream as soon as the threshold (default: 50) is reached.
- Never contacts remote servers; falls back to standard `push` on any rev-list parsing error.

### 5. Cross-Process Synchronization (`audio.rs`)
- Playback runs in a separate detached process (`CREATE_NO_WINDOW | DETACHED_PROCESS` on Windows, fork/setsid on Unix).
- Prevents overlapping/scrambled sounds using `PlaybackLock`:
  - Atomic directory creation: `fs::create_dir("~/.gitsama/state/playback.lock")`.
  - Spawns background thread updating `playback.lock/heartbeat` timestamp every 250ms.
  - Competing instances check heartbeat staleness (stale if > `queue_max_age_ms`, default 5000ms).

### 6. Windows / MSYS / Git Quirks (`platform.rs`)
- Git hooks on Windows run inside MSYS `sh.exe`.
- Path canonicalization produces Windows UNC paths (`\\?\C:\...`), which MSYS sh fails to execute. `platform::shell_quote` normalizes them to POSIX paths (`/C/...` or `'C:/...'`).
- Parent PID in MSYS sh (`$PPID`) returns 1. `platform::windows_git_parent()` uses Win32 `ToolHelp32Snapshot` to walk process parents up to 16 levels to locate the real `git.exe` PID.

---

## 6. Runtime Layout & File Locations

All user state is stored outside project repositories:

| Path | Purpose |
|---|---|
| `~/.gitsama/config.toml` | User settings (active pack, volume 0-100, threshold, mute, per-event toggles) |
| `~/.gitsama/bin/` | Permanent binary location added to user `PATH` |
| `~/.gitsama/packs/` | Installed custom and bundled sound packs |
| `~/.gitsama/state/` | Runtime locks (`playback.lock/`) and debounce tokens (`pending-*.json`) |
| `~/.gitsama/logs/gitsama.log` | Diagnostic logging for hook and playback errors |
| `<repo>/.git/config` | Local opt-out only: `hook.gitsama-*.enabled = false` (via `gitsama off-here`) |

*Note: Set `GITSAMA_HOME=/custom/path` to isolate all runtime paths (used by tests).*

---

## 7. Developer & Test Cheatsheet

```sh
# Build
cargo build
cargo build --release

# Run Integration Tests
cargo test

# Integration Test Mode (skips audio device, writes JSON records to file)
GITSAMA_TEST_MODE=1 GITSAMA_TEST_LOG=/path/to/log.json cargo test

# CLI Operations
cargo run -- status                   # Inspect global & current repo hook status
cargo run -- doctor [--fix]          # Health check & auto-repair hooks/config
cargo run -- test [event|all]        # Test play sounds
cargo run -- pack list               # List available sound packs
cargo run -- pack scaffold <name>    # Create a custom pack template
cargo run -- off-here / on-here      # Disable / re-enable GitSama in current repo
```
