# Project Specification: AutoCommit

## Overview

Build a cross-platform CLI tool called **AutoCommit** in **Rust**.

The purpose of AutoCommit is to automatically create Git commits whenever the developer finishes a coding session. The tool is started inside a Git repository and runs continuously until stopped.

The philosophy is:

> Start it once, code normally, forget about Git.

---

# Command

The tool should be executable as:

```bash
autocommit
```

The current working directory is the project root.

No configuration should be required.

---

# Startup

When started:

1. Verify the current directory is inside a Git repository.
2. Find the repository root.
3. Load all Git ignore rules exactly as Git does.
4. Start recursively watching the repository.
5. Never watch the `.git` directory.
6. Display a small terminal status.

Example:

```
AutoCommit

Repository:
D:\Projects\Messenger

Watching repository...
Waiting for changes...
```

---

# File Watching

Watch the entire repository recursively.

The tool should detect:

* File modifications
* File creation
* File deletion
* File rename
* Directory creation
* Directory deletion

Use efficient native filesystem notifications.

---

# Ignore Rules

The tool must respect Git ignore rules exactly.

Ignored files should never trigger commits.

This includes:

* .gitignore
* nested .gitignore files
* .git/info/exclude
* global Git ignore

Use the Rust `ignore` crate instead of implementing ignore matching manually.

---

# Debouncing

Do NOT commit immediately.

Whenever a change occurs:

* Start a debounce timer.
* If another change occurs before the timer expires, reset the timer.
* Only commit after the repository has been inactive for a configurable duration.

Default inactivity duration:

5 seconds.

Example:

```
save
save
save
save

(wait 5 seconds)

commit
```

---

# Before Committing

When the debounce timer expires:

Run the equivalent of:

```
git add -A
```

If there are no staged changes:

Return to waiting.

Do not create empty commits.

---

# Versioning

The latest commit message is expected to contain a semantic version.

Examples:

v0.0.1
v0.2.14
v5.10.3

Parse the latest version.

Increment only the PATCH number.

Examples:

v0.2.1 -> v0.2.2

v1.8.99 -> v1.8.100

If no version commit exists yet:

Start at

v0.0.1

---

# Commit

Automatically execute:

```
git add -A

git commit -m "vX.Y.Z"

git push
```

Only push if the commit succeeds.

---

# Branch

Use the current checked-out branch.

Do not assume "main" or "master".

---

# Terminal Output

Display useful status updates.

Example:

```
Watching...

Change detected:
src/main.rs

Waiting for inactivity...

Creating commit...

Commit:
v0.3.7

Pushing...

Done.

Watching...
```

Use colored output if possible.

---

# Graceful Exit

When Ctrl+C is pressed:

Stop the watcher.

Release all resources.

Exit cleanly.

---

# Error Handling

Handle:

* Not inside a Git repository
* Git not installed
* Push rejected
* Merge conflicts
* Network unavailable
* Invalid version format
* Missing remote
* Repository in detached HEAD
* Permission denied

Errors should be informative and should not crash unexpectedly.

---

# Architecture

Organize the project into modules.

Suggested layout:

```
src/

main.rs

watcher.rs
git.rs
version.rs
ignore.rs
debounce.rs
cli.rs
errors.rs
config.rs
```

---

# Recommended Crates

Use modern stable crates.

Suggested:

* notify
* ignore
* git2
* semver
* anyhow
* thiserror
* clap
* tokio
* colored
* ctrlc

Avoid shelling out to Git where the functionality is available through `git2`. Use `git2` for repository operations whenever practical.

---

# Code Quality

Requirements:

* Rust stable
* Well documented
* Modular
* No unsafe code
* Idiomatic Rust
* Unit tests for version parsing and incrementing
* Clear comments
* Robust error handling

---

# Future Extensibility

Design the code so these features can be added later without major refactoring:

* Config file
* Conventional commits
* Git tags
* Changelog generation
* GitHub Releases
* TUI dashboard
* Hooks before commit
* Auto-format before commit
* Auto-run tests before commit
* Commit statistics
* Notifications
* Multi-repository mode

The initial implementation should focus only on the automatic watcher, semantic version incrementing, committing, and pushing.
