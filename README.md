<div align="center">
  <h1>⚡ AutoCommit</h1>
  <p>
    <strong>Start it once, code normally, forget about Git.</strong>
  </p>
  <p>
    <a href="#-features">Features</a> •
    <a href="#-installation">Installation</a> •
    <a href="#-usage">Usage</a> •
    <a href="#-how-it-works">How It Works</a> •
    <a href="#-architecture">Architecture</a> •
    <a href="#-configuration">Configuration</a>
  </p>
  <p>
    <img src="https://img.shields.io/badge/rust-stable-orange?logo=rust" alt="Rust Stable">
    <img src="https://img.shields.io/badge/license-MIT-blue" alt="MIT License">
    <img src="https://img.shields.io/badge/platform-windows%20%7C%20macOS%20%7C%20linux-lightgrey" alt="Cross-platform">
  </p>
</div>

---

**AutoCommit** is a cross-platform CLI tool that watches a Git repository and automatically creates commits when you finish working. No more forgotten `git add`, `git commit`, or `git push` — just start AutoCommit and code.

```bash
# Start in any Git repository
autocommit

# That's it. Code normally. AutoCommit handles the rest.
```

---

## ✨ Features

- **Automatic File Watching** — monitors your entire repository using native filesystem notifications
- **Smart Debouncing** — waits for 5 seconds of inactivity before committing (configurable)
- **Semantic Versioning** — reads the latest `vX.Y.Z` from commit history, increments PATCH
- **Automatic Pushing** — pushes to your tracking branch after every commit
- **Gitignore-Aware** — respects `.gitignore`, nested `.gitignore`, `.git/info/exclude`, and global excludes
- **Graceful Shutdown** — clean exit on `Ctrl+C` with all resources released
- **Coloured Terminal** — clear, colour-coded status output (supports `--no-color`)
- **Zero Configuration** — works out of the box in any Git repository
- **No `unsafe` Code** — 100% safe Rust

## 📦 Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/Nciibi/auto-commit.git
cd auto-commit

# Build release binary
cargo build --release

# (Optional) Install to PATH
cp target/release/autocommit ~/.cargo/bin/
```

### Prerequisites

- [Rust](https://rustup.rs/) 1.70+ (stable) — the installer will guide you through setup
- A Git repository to work in (local or cloned)

### Make `autocommit` available anywhere (add it to PATH)

After building, the `autocommit.exe` file lives inside the project folder. To run it from any terminal, you need to copy it to a folder on your PATH.

<details>
<summary><b>🪟 Windows (click to expand)</b></summary>

#### Option A — One command (easiest)

Open **PowerShell** and run:

```powershell
Copy-Item target\release\autocommit.exe "$env:USERPROFILE\.cargo\bin\" -Force
```

Then type `autocommit` in any terminal to use it. If it says the command wasn't found, restart your terminal or log out and back in.

#### Option B — Manual (if you prefer clicking)

1. Open the project folder (`auto-commit`)
2. Go into `target` → `release`
3. Copy the file `autocommit.exe`
4. Open a new File Explorer window and paste this in the address bar: `%USERPROFILE%\.cargo\bin`
5. Press Enter — if the folder doesn't exist yet, create it
6. Paste `autocommit.exe` into that folder

> **Still not working?** Restart your terminal (or log out and back in) so Windows refreshes your PATH.

</details>

<details>
<summary><b>🍎 macOS / 🐧 Linux (click to expand)</b></summary>

Run this in your terminal:

```bash
cp target/release/autocommit ~/.cargo/bin/
```

Now type `autocommit` from anywhere. Restart your terminal if it's not found right away.

</details>

### Verify it worked

Open a **new** terminal window and type:

```bash
autocommit --version
```

If you see `autocommit 0.1.0`, you're all set.

---

## 🚀 Usage

```bash
# Basic usage — start watching the current directory
autocommit

# Custom inactivity timeout (seconds)
autocommit --timeout 10

# Suppress coloured output
autocommit --no-color

# Verbose logging
autocommit --verbose

# Combine flags
autocommit --timeout 30 --verbose
```

### What You'll See

```
AutoCommit v0.1.0 — automatic Git commits
Start it once, code normally, forget about Git.

Repository: /home/user/projects/my-app
Branch: main
Watching for changes... (Ctrl+C to stop)

[change detected: src/main.rs]
Waiting for inactivity...

Creating commit...
Version: v0.3.7
Committed: v0.3.7 (a1b2c3d)
Pushing... Done.

Watching...
```

## 🔧 How It Works

```
┌──────────────┐    ┌──────────────┐    ┌──────────────┐
│  File Event  │───▶│   Debounce   │───▶│     Git      │
│  (notify)    │    │   Timer      │    │  Operations  │
│              │    │   (5s)       │    │              │
│ Filters out  │    │ Resets on    │    │ git add -A   │
│ ignored      │    │ each new     │    │ git commit   │
│ paths        │    │ event        │    │ git push     │
└──────────────┘    └──────────────┘    └──────────────┘
```

### The Lifecycle of a Change

1. **Watch** — `notify` crate delivers filesystem events in real-time
2. **Filter** — each path is checked against gitignore rules; ignored paths are discarded
3. **Debounce** — the timer resets on every new event, so rapid saves don't trigger commits
4. **Stage** — on timeout expiry, all changes are staged via `git add -A`
5. **Version** — the latest `vX.Y.Z` is parsed from commit history; PATCH is incremented
6. **Commit** — a commit is created with the new version as the message
7. **Push** — the commit is pushed to the tracking remote via `git push` (your system's Git credential helpers handle auth)
8. **Loop** — the watcher continues, ready for the next change cycle

## 🏗 Architecture

```
src/
├── main.rs        # Entry point, banner, event loop, graceful shutdown
├── cli.rs         # CLI argument parsing (clap)
├── config.rs      # Configuration defaults (debounce timeout)
├── debounce.rs    # Debounce timer logic
├── errors.rs      # Comprehensive error types (thiserror)
├── git.rs         # Git operations via git2 (commit, push, versioning, add)
├── ignore.rs      # Gitignore filtering via git2
├── version.rs     # Semantic version parsing & PATCH incrementing
└── watcher.rs     # Filesystem watcher via notify
```

### Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| **Threads over async** | Simpler error handling, no futures runtime, natural blocking for channel operations |
| **Hybrid approach** | `git2` for repo discovery / status / version history; CLI (`git add`, `git commit`, `git push`) for write operations so system credential helpers work out of the box |
| **`semver` crate** | Correct semver parsing (handles pre-release, build metadata) |
| **Channel-based IPC** | Clean separation between watcher, debounce, and main loop |
| **No config file (yet)** | Future extensibility — the config module is ready for file-based overrides |

## ⚙️ Configuration

### CLI Options

| Flag | Description | Default |
|------|-------------|---------|
| `-t, --timeout <SECS>` | Inactivity timeout in seconds | `5` |
| `-n, --no-color` | Disable coloured output | `false` |
| `-v, --verbose` | Detailed event logging | `false` |
| `-h, --help` | Print help | |
| `-V, --version` | Print version | |

### Planned (Future)

- **Config file** — per-project settings via `autocommit.toml`
- **Conventional commits** — `feat:`, `fix:`, etc.
- **Git tags** — automatic tagging on commit
- **Changelog** — auto-generated release notes
- **Hooks** — pre-commit formatting, test running
- **TUI dashboard** — live status view

## 🧪 Development

```bash
# Run tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Check for warnings
cargo check

# Build release
cargo build --release

# Run linting
cargo clippy
```

### Testing Philosophy

- **Unit tests** for pure logic (version parsing, config, debounce)
- **Integration tests** for git and ignore modules (using `tempfile` repos)
- **No flaky tests** — all tests are deterministic and self-contained

## 🐛 Error Handling

AutoCommit handles these scenarios gracefully:

| Scenario | Behaviour |
|----------|-----------|
| Not in a Git repo | Clear error message, exit code 1 |
| Detached HEAD | Error message, exit code 2 |
| No remote configured | Commit succeeds, push skipped (yellow warning) |
| Push rejected / network error | Warning logged, continues watching |
| Permission denied | Informative error message |
| Merge conflict | Error message, resolves manually |

## 📄 License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.

## 🤝 Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

---

<div align="center">
  <sub>Built with ❤️ and Rust</sub>
</div>
