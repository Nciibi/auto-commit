# Contributing to AutoCommit

Thank you for your interest in contributing! This document outlines the guidelines for contributing to AutoCommit.

## 🧠 Code of Conduct

By participating, you agree to maintain a respectful, inclusive environment. Be constructive, assume good faith, and help us build something great.

## 🐛 Reporting Bugs

Open an issue with:

- A **clear title** and **description**
- Steps to **reproduce** (include repository structure if relevant)
- Expected vs actual behaviour
- Platform and Rust version (`rustc --version`)

## 💡 Feature Requests

Open an issue with:

- What you'd like to see added
- Why it would be useful (real-world use case)
- Any prior art or examples

## 🛠 Development Setup

```bash
# Clone and build
git clone https://github.com/Nciibi/auto-commit.git
cd auto-commit
cargo build

# Run tests
cargo test

# Run clippy
cargo clippy
```

## 📝 Pull Request Guidelines

1. **One change per PR** — keep it focused
2. **Write tests** — cover the new behaviour and edge cases
3. **No new warnings** — the project compiles with zero warnings
4. **No `unsafe` code** — this is a project policy
5. **Document public APIs** — doc comments on `pub` items
6. **Update the README** — if adding or changing user-facing behaviour

### PR Checklist

- [ ] `cargo check` — zero warnings
- [ ] `cargo test` — all tests pass
- [ ] `cargo clippy` — no new lint warnings
- [ ] Documentation updated (if needed)
- [ ] Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/) (optional but appreciated)

## 📁 Module Guidelines

| Module | Responsibility | Test Coverage |
|--------|---------------|---------------|
| `version.rs` | Parse, increment, format semver | ✅ Required |
| `debounce.rs` | Timer reset logic, channel comms | ✅ Required |
| `config.rs` | Configuration defaults | ✅ Required |
| `ignore.rs` | Gitignore checking | ✅ Required |
| `git.rs` | Repository operations | ✅ Recommended |
| `watcher.rs` | File system watching | ✅ Manual |
| `cli.rs` | Argument parsing | ⬜ Optional |
| `errors.rs` | Error types | ⬜ Optional |

## 🏗 Architectural Notes

- **Thread-based, not async** — channels over tokio. Keep it that way unless there's a compelling reason.
- **`git2`, not shell** — avoid shelling out to `git` CLI. Use `git2` for all repo operations.
- **Error chaining** — use `thiserror` for library errors, `anyhow` at the top level.
- **Future-proofing** — the `config.rs` module is designed to support file-based overrides; don't break that contract.

## 📋 Commit Format

We prefer [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add --timeout flag
fix: handle detached HEAD gracefully
docs: update README with architecture diagram
test: add debounce reset test
refactor: extract git operations into module
```

## ❓ Questions

Open a [Discussion](https://github.com/Nciibi/auto-commit/discussions) for questions, ideas, or help getting started.
