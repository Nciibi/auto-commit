//! CLI argument parsing using `clap`.

use clap::Parser;

/// AutoCommit — automatically create Git commits after coding sessions.
///
/// Start it inside a Git repository and code normally.  AutoCommit
/// watches for file changes, waits for inactivity, then stages,
/// versions, commits, and pushes your work.
#[derive(Parser, Debug)]
#[command(name = "autocommit", version, about, long_about = None)]
pub struct Cli {
    /// Override the inactivity timeout in seconds (default: 5).
    #[arg(short = 't', long = "timeout", value_name = "SECS")]
    pub timeout: Option<u64>,

    /// Suppress coloured terminal output.
    #[arg(short = 'n', long = "no-color")]
    pub no_color: bool,

    /// Print a more detailed event log.
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,
}

/// Parse CLI arguments.
pub fn parse() -> Cli {
    Cli::parse()
}
