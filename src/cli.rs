use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "cerebrum",
    author,
    version,
    about = "Portable, git-tracked memory and governance for AI agents"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new .cerebrum/ Brain in the current directory (or globally with --global)
    Init {
        #[arg(long, help = "Create the global Brain at ~/.cerebrum/")]
        global: bool,
    },
    /// Add a new entry to episodic memory
    Add {
        /// Memory text. Omit when using --from-hook.
        #[arg(required_unless_present = "from_hook")]
        content: Option<String>,
        #[arg(
            long,
            help = "Write to the global Brain (~/.cerebrum/) instead of local"
        )]
        global: bool,
        /// Read a git-hook payload from the environment (no shell interpolation)
        #[arg(long)]
        from_hook: bool,
    },
    /// Show Brain status
    Status,
    /// List or apply community templates
    Template {
        #[arg(short, long)]
        list: bool,
        #[arg(short, long)]
        apply: Option<String>,
    },
    /// Watch for file changes and auto-capture to episodic memory
    Watch,
    /// Install or remove the git post-commit hook
    Hook {
        #[command(subcommand)]
        action: HookAction,
    },
    /// Reflect on episodic memory and promote insights (dream cycle)
    Dream {
        /// Ingest an LLM reflection file and promote it into the Brain
        #[arg(long, value_name = "FILE")]
        ingest: Option<PathBuf>,
        /// Automatically run the dream cycle using a configured API key
        #[arg(long)]
        auto: bool,
    },
    /// Audit the current project and generate a seed prompt for the Brain
    Audit,
    /// Configure API keys and Brain settings
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Trace the origin of a specific rule or insight
    Why { query: String },
    /// Semantic search across all memory layers
    Search {
        query: String,
        /// Print machine-readable JSON (local and global indexes)
        #[arg(long)]
        json: bool,
        /// Maximum matches to return
        #[arg(long, default_value_t = 5)]
        limit: usize,
    },
    /// Digital Janitor: Resolve technical debt flagged in audits
    Fix,
    /// Visualize the Brain: Generate a premium HTML report
    Map {
        /// Render the force-directed neural web (nodes + inferred connections)
        #[arg(long)]
        graph: bool,
    },
    /// Persistent Intelligence: Manage background brain activity
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
    /// The Proactive Briefing: Give the AI a 'State of the Union' report
    Brief,
    /// View Brain health and usage statistics
    Stats,
    /// Validate governed Business Brain entries and fail on unsafe conflicts
    Validate {
        /// Print the validation report as JSON
        #[arg(long)]
        json: bool,
    },
    /// Publish approved Business Brain knowledge as a deterministic runtime snapshot
    Publish {
        /// Output path (defaults to .cerebrum/runtime/brain.json)
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
    },
    /// Index one memory file already on disk. Content is read from stdin.
    Embed {
        /// Brain root (.cerebrum or ~/.cerebrum)
        #[arg(long)]
        brain: PathBuf,
        /// Memory file to associate with the vector
        #[arg(long)]
        file: PathBuf,
    },
}

#[derive(Subcommand)]
pub enum DaemonAction {
    /// Start the background watcher and dreamer
    Start,
    /// Stop background activity
    Stop,
    /// Check daemon status
    Status,
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Set a configuration value.
    ///
    /// API keys (`gemini_key`, `anthropic_key`) are not accepted on the command
    /// line. They are read from the environment or an interactive prompt.
    Set {
        key: String,
        /// Value for non-secret settings such as `provider` or `gemini_model`.
        value: Option<String>,
    },
    /// Show current configuration (keys are masked)
    Get,
}

#[derive(Subcommand)]
pub enum HookAction {
    /// Install the post-commit hook into .git/hooks/
    Install,
    /// Remove the post-commit hook
    Remove,
}
