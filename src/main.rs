use anyhow::Result;
use clap::Parser;
use rusqlite::auto_extension::{register_auto_extension, RawAutoExtension};
use sqlite_vec::sqlite3_vec_init;

pub mod cli;
pub mod config;
pub mod daemon;
pub mod dreamer;
pub mod git;
pub mod governance;
pub mod memory;
pub mod ui;
pub mod vector;

use cli::{Cli, Commands, HookAction};
use config::{run_config, run_stats};
use daemon::run_daemon_cmd;
use dreamer::dream_cycle;
use git::{hook_entry_from_env, hook_install, hook_remove};
use governance::{publish_runtime, run_validate};
use memory::{add_entry, init_brain, resolve_brain};
use ui::{handle_template, run_audit, run_brief, run_fix, run_map, show_status};
use vector::{run_embed, run_search, run_why};

#[tokio::main]
async fn main() -> Result<()> {
    // sqlite-vec's Rust binding declares an untyped init symbol. rusqlite 0.41
    // registers that pointer as a raw auto-extension.
    unsafe {
        let init =
            std::mem::transmute::<unsafe extern "C" fn(), RawAutoExtension>(sqlite3_vec_init);
        register_auto_extension(init)?;
    }

    let cli = Cli::parse();

    match cli.command {
        Commands::Init { global } => init_brain(resolve_brain(global)),
        Commands::Add {
            content,
            global,
            from_hook,
        } => {
            if from_hook && content.is_some() {
                anyhow::bail!("pass either memory text or --from-hook, not both");
            }
            let content = if from_hook {
                hook_entry_from_env()?
            } else {
                content.ok_or_else(|| anyhow::anyhow!("missing memory text"))?
            };
            add_entry(content, resolve_brain(global)).await
        }
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
        Commands::Search { query, json, limit } => run_search(&query, json, limit),
        Commands::Fix => run_fix(),
        Commands::Map { graph } => run_map(graph),
        Commands::Daemon { action } => run_daemon_cmd(action),
        Commands::Brief => run_brief(),
        Commands::Stats => run_stats(),
        Commands::Validate { json } => run_validate(json),
        Commands::Publish { output } => publish_runtime(output),
        Commands::Embed { brain, file } => run_embed(&brain, &file),
    }
}
