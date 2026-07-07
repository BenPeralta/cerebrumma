use anyhow::Result;
use clap::Parser;
use rusqlite::ffi::sqlite3_auto_extension;
use sqlite_vec::sqlite3_vec_init;

pub mod cli;
pub mod config;
pub mod daemon;
pub mod dreamer;
pub mod git;
pub mod memory;
pub mod ui;
pub mod vector;

use cli::{Cli, Commands, HookAction};
use config::{run_config, run_stats};
use daemon::run_daemon_cmd;
use dreamer::dream_cycle;
use git::{hook_install, hook_remove};
use memory::{add_entry, init_brain, resolve_brain};
use ui::{handle_template, run_audit, run_brief, run_fix, run_map, show_status};
use vector::{run_search, run_why};

#[tokio::main]
async fn main() -> Result<()> {
    // Register sqlite-vec extension globally
    unsafe {
        sqlite3_auto_extension(Some(std::mem::transmute(sqlite3_vec_init as *const ())));
    }

    let cli = Cli::parse();

    match cli.command {
        Commands::Init { global } => init_brain(resolve_brain(global)),
        Commands::Add { content, global } => add_entry(content, resolve_brain(global)).await,
        Commands::Status => show_status(),
        Commands::Template { list, apply } => handle_template(list, apply),
        Commands::Watch => daemon::watch_daemon().await,
        Commands::Hook { action } => match action {
            HookAction::Install => hook_install(),
            HookAction::Remove => hook_remove(),
        },
        Commands::Dream { ingest, auto } => dream_cycle(ingest, auto).await,
        Commands::Audit => run_audit(),
        Commands::Config { action } => run_config(action),
        Commands::Why { query } => run_why(&query),
        Commands::Search { query } => run_search(&query),
        Commands::Fix => run_fix(),
        Commands::Map { graph } => run_map(graph),
        Commands::Daemon { action } => run_daemon_cmd(action),
        Commands::Brief => run_brief(),
        Commands::Stats => run_stats(),
    }
}
