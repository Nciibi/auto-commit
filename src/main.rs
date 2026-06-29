//! AutoCommit — automatic Git commits after coding sessions.
//!
//! ## Philosophy
//!
//! > Start it once, code normally, forget about Git.
//!
//! AutoCommit watches a Git repository for filesystem changes, waits
//! for a period of inactivity (default 5 s), then stages everything,
//! increments the semantic version, commits, and pushes.
//!
//! ## Architecture
//!
//! The program is organised around three concurrent loops:
//!
//! 1. **Watcher** — receives native filesystem notifications and
//!    filters them through gitignore rules.
//! 2. **Debounce** — accumulates changed paths and fires a commit
//!    signal only after a stable inactive period.
//! 3. **Main loop** — handles the commit signal, drives the git
//!    operations, and prints status to the terminal.

mod cli;
mod config;
mod debounce;
mod errors;
mod git;
mod ignore;
mod version;
mod watcher;

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;
use anyhow::Context;
use colored::*;
use debounce::DebounceEvent;
use git::GitRepo;
use ignore::IgnoreFilter;

/// Exit codes used by the application.
mod exit_code {
    pub const SUCCESS: i32 = 0;
    pub const ERROR_NOT_IN_REPO: i32 = 1;
    pub const ERROR_GENERIC: i32 = 2;
}

fn main() {
    let args = cli::parse();

    // Disable colours if the user asked or if stdout isn't a terminal.
    if args.no_color || !atty::is(atty::Stream::Stdout) {
        colored::control::set_override(false);
    }

    // ------------------------------------------------------------------
    // Startup
    // ------------------------------------------------------------------
    print_banner();

    // Locate the repository.
    let cwd = std::env::current_dir().expect("Cannot determine current working directory");
    let repo = match GitRepo::open(&cwd) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{} {}", "[error]".red().bold(), e);
            std::process::exit(exit_code::ERROR_NOT_IN_REPO);
        }
    };

    println!(
        "{} {}",
        "Repository:".cyan().bold(),
        repo.root.display()
    );

    // Load ignore rules.
    let ignore_filter = match IgnoreFilter::new(&repo.root) {
        Ok(f) => f,
        Err(e) => {
            eprintln!(
                "{} Failed to load gitignore rules: {}",
                "[warn]".yellow().bold(),
                e
            );
            // Non-fatal — fall back to watching everything (except .git).
            IgnoreFilter::new(&repo.root).unwrap_or_else(|_| {
                panic!("cannot create basic ignore filter")
            })
        }
    };

    // Determine branch.
    let branch = match repo.current_branch() {
        Ok(b) => b,
        Err(_) => {
            eprintln!(
                "{} Repository is in detached HEAD state — cannot determine branch.",
                "[error]".red().bold()
            );
            std::process::exit(exit_code::ERROR_GENERIC);
        }
    };
    println!("{} {}", "Branch:".cyan().bold(), branch);

    // Channels.
    let (event_tx, event_rx) = mpsc::channel::<DebounceEvent>();
    let (commit_tx, commit_rx) = mpsc::channel::<DebounceEvent>();

    // ------------------------------------------------------------------
    // Start the file watcher.
    // ------------------------------------------------------------------
    let watcher_handle = match watcher::start_watcher(&repo.root, ignore_filter, event_tx.clone()) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("{} {}", "[error]".red().bold(), e);
            std::process::exit(exit_code::ERROR_GENERIC);
        }
    };
    // Keep watcher alive for the process lifetime.
    std::mem::forget(watcher_handle);

    println!(
        "{}",
        "Watching for changes... (Ctrl+C to stop)".green().bold()
    );
    println!();

    // ------------------------------------------------------------------
    // Setup Ctrl+C handler.
    // ------------------------------------------------------------------
    let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel::<()>();
    let event_tx_for_signal = event_tx.clone();
    ctrlc::set_handler(move || {
        let _ = event_tx_for_signal.send(DebounceEvent::Shutdown);
        let _ = shutdown_tx.send(());
    })
    .expect("Error setting Ctrl+C handler");

    // ------------------------------------------------------------------
    // Spawn the debounce loop in a background thread.
    // ------------------------------------------------------------------
    let debounce_timeout = config::debounce_timeout(args.timeout);
    let event_tx_for_debounce = event_tx.clone();
    std::thread::spawn(move || {
        debounce::debounce_loop(debounce_timeout, event_rx, commit_tx);
    });

    // ------------------------------------------------------------------
    // Main loop — wait for commit signals and process them.
    // ------------------------------------------------------------------
    if args.verbose {
        println!("{}", "[verbose] Debounce timeout: {:?}".dimmed(), debounce_timeout);
    }

    // We'll re-open the repo each time to avoid stale state issues; for
    // performance the root path is captured once.
    let repo_root = repo.root.clone();

    // Wait for either a shutdown signal or the first event.
    'main: loop {
        // Check for immediate shutdown.
        if shutdown_rx.try_recv().is_ok() {
            break 'main;
        }

        match commit_rx.recv() {
            Ok(DebounceEvent::TimerExpired) => {
                if args.verbose {
                    println!("{} Debounce timer expired.", "[verbose]".dimmed());
                }

                // Re-open the repo for each cycle.
                let repo = match GitRepo::open(&repo_root) {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("{} {}", "[error]".red().bold(), e);
                        continue;
                    }
                };

                // Check if there's anything to commit.
                if repo.is_clean().unwrap_or(true) {
                    if args.verbose {
                        println!("{} No changes to commit.", "[verbose]".dimmed());
                    }
                    continue;
                }

                let msg = format!("{}\n\n{}", "Creating commit...".yellow().bold(), "".clear());
                print!("\r{}", msg);
                println!();

                // Determine the next version.
                let version_str = match repo.latest_version() {
                    Ok(Some(latest_msg)) => {
                        // Parse and increment.
                        match version::parse_version(&latest_msg) {
                            Some(v) => {
                                let next = version::increment_patch(&v);
                                version::format_version(&next)
                            }
                            None => version::INITIAL_VERSION.to_string(),
                        }
                    }
                    Ok(None) | Err(_) => version::INITIAL_VERSION.to_string(),
                };

                println!("{} {}", "Version:".cyan().bold(), version_str);

                // Commit.
                match repo.commit(&version_str) {
                    Ok(oid) => {
                        println!(
                            "{} {} ({})",
                            "Committed:".green().bold(),
                            version_str,
                            &oid[..7.min(oid.len())]
                        );
                    }
                    Err(e) => {
                        eprintln!("{} {}", "[error]".red().bold(), e);
                        continue;
                    }
                }

                // Push.
                print!("{} ", "Pushing...".cyan().bold());
                match repo.push() {
                    Ok(()) => {
                        println!("{}", "Done.".green().bold());
                    }
                    Err(errors::AutoCommitError::NoRemote) => {
                        println!("{}", "Skipped (no remote).".yellow());
                    }
                    Err(errors::AutoCommitError::NetworkError(e)) => {
                        eprintln!("{} Push failed (network): {}", "[warn]".yellow().bold(), e);
                    }
                    Err(e) => {
                        eprintln!("{} {}", "[warn]".yellow().bold(), e);
                    }
                }

                println!();
                println!("{}", "Watching...".green().bold());
            }
            Ok(DebounceEvent::Shutdown) | Err(_) => {
                break 'main;
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------------
    // Graceful shutdown.
    // ------------------------------------------------------------------
    println!();
    println!("{}", "Shutting down...".cyan().bold());
    // Resources are cleaned up via Drop / process exit.
}

/// Print the application banner.
fn print_banner() {
    println!(
        "{}",
        "AutoCommit v0.1.0 — automatic Git commits".bold()
    );
    println!(
        "{}",
        "Start it once, code normally, forget about Git.".dimmed()
    );
    println!();
}
