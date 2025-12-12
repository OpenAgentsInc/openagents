//! Taskmaster CLI - Issue tracker for OpenAgents
//!
//! Full-featured issue management ported from Beads.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process;
use taskmaster::SqliteRepository;

mod commands;
use commands::*;

/// Taskmaster - Issue tracker for OpenAgents
#[derive(Parser)]
#[command(name = "taskmaster")]
#[command(about = "Full-featured issue tracker for OpenAgents", long_about = None)]
#[command(version)]
struct Cli {
    /// Path to database file
    #[arg(short, long, env = "TASKMASTER_DB", default_value = ".openagents/taskmaster.db")]
    db: PathBuf,

    /// ID prefix for new issues
    #[arg(short, long, env = "TASKMASTER_PREFIX", default_value = "tm")]
    prefix: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize database
    Init,

    /// Create a new issue
    Create(CreateArgs),

    /// Show issue details
    Show(ShowArgs),

    /// List issues
    List(ListArgs),

    /// Update an issue
    Update(UpdateArgs),

    /// Delete an issue (soft delete)
    Delete(DeleteArgs),

    /// Restore a deleted issue
    Restore(RestoreArgs),

    /// Start working on an issue
    Start(StartArgs),

    /// Close an issue
    Close(CloseArgs),

    /// Reopen a closed issue
    Reopen(ReopenArgs),

    /// Block an issue
    Block(BlockArgs),

    /// Unblock an issue
    Unblock(UnblockArgs),

    /// Show ready issues
    Ready(ReadyArgs),

    /// Search issues
    Search(SearchArgs),

    /// Find stale issues
    Stale(StaleArgs),

    /// Dependency management
    Dep(DepArgs),

    /// Label management
    Label(LabelArgs),

    /// Comment management
    Comment(CommentArgs),

    /// Show statistics
    Stats(StatsArgs),

    /// Run health checks
    Doctor(DoctorArgs),

    /// Clean up expired tombstones
    Cleanup(CleanupArgs),

    /// Show audit events
    Events(EventsArgs),
}


fn main() {
    let cli = Cli::parse();

    // Open database
    let repo = match SqliteRepository::open(&cli.db) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error opening database: {}", e);
            process::exit(1);
        }
    };

    // Execute command
    let result = match cli.command {
        Commands::Init => init::run(&repo),
        Commands::Create(args) => create::run(&repo, &cli.prefix, args),
        Commands::Show(args) => show::run(&repo, args),
        Commands::List(args) => list::run(&repo, args),
        Commands::Update(args) => update::run(&repo, args),
        Commands::Delete(args) => delete::run(&repo, args),
        Commands::Restore(args) => restore::run(&repo, args),
        Commands::Start(args) => start::run(&repo, args),
        Commands::Close(args) => close::run(&repo, args),
        Commands::Reopen(args) => reopen::run(&repo, args),
        Commands::Block(args) => block::run(&repo, args),
        Commands::Unblock(args) => unblock::run(&repo, args),
        Commands::Ready(args) => ready::run(&repo, args),
        Commands::Search(args) => search::run(&repo, args),
        Commands::Stale(args) => stale::run(&repo, args),
        Commands::Dep(args) => dep::run(&repo, args),
        Commands::Label(args) => label::run(&repo, args),
        Commands::Comment(args) => comment::run(&repo, args),
        Commands::Stats(args) => stats::run(&repo, args),
        Commands::Doctor(args) => doctor::run(&repo, args),
        Commands::Cleanup(args) => cleanup::run(&repo, args),
        Commands::Events(args) => events::run(&repo, args),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}
