use clap::{Args, Parser, Subcommand, ValueEnum};
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
    /// Named mood boards: palette, type, spacing, and the rest of a visual taste
    Design {
        #[command(subcommand)]
        action: Box<DesignAction>,
    },
}

/// Facets that can be borrowed from another mood board.
#[derive(Args, Clone, Debug)]
pub struct DesignMix {
    /// Use this board's palette
    #[arg(long, value_name = "BOARD")]
    pub palette: Option<String>,
    /// Use this board's typography
    #[arg(long = "type", alias = "typography", value_name = "BOARD")]
    pub type_board: Option<String>,
    /// Use this board's spacing scale
    #[arg(long, value_name = "BOARD")]
    pub spacing: Option<String>,
    /// Use this board's radius scale
    #[arg(long, value_name = "BOARD")]
    pub radius: Option<String>,
    /// Use this board's shadows and elevation
    #[arg(long, value_name = "BOARD")]
    pub elevation: Option<String>,
    /// Use this board's motion
    #[arg(long, value_name = "BOARD")]
    pub motion: Option<String>,
}

/// `Add` carries one flag per token group, so it is larger than the other commands.
#[allow(clippy::large_enum_variant)]
#[derive(Subcommand)]
pub enum DesignAction {
    /// Save a mood board. An existing slug is left untouched unless --replace is set.
    Add {
        /// Display name. Required unless --from supplies one.
        #[arg(long, required_unless_present = "from")]
        name: Option<String>,
        /// Identity used in the filename. Defaults to a slug of the name.
        #[arg(long)]
        slug: Option<String>,
        #[arg(long)]
        description: Option<String>,
        /// Palette role and hex color, for example background=#F6F1E8
        #[arg(long = "color", value_name = "ROLE=HEX")]
        palette: Vec<String>,
        /// Font family, for example display=Fraunces
        #[arg(long = "family", value_name = "ROLE=FAMILY")]
        family: Vec<String>,
        /// Type scale step, for example base=1rem
        #[arg(long = "scale", value_name = "STEP=SIZE")]
        scale: Vec<String>,
        /// Font weight, for example regular=400
        #[arg(long = "weight", value_name = "NAME=WEIGHT")]
        weight: Vec<String>,
        /// Spacing step, for example 4=1rem
        #[arg(long = "space", value_name = "STEP=SIZE")]
        spacing: Vec<String>,
        /// Corner radius, for example md=10px
        #[arg(long = "radius", value_name = "NAME=SIZE")]
        radius: Vec<String>,
        /// Shadow or elevation, for example md=0 10px 28px rgb(28 25 23 / 0.08)
        #[arg(long = "shadow", value_name = "NAME=SHADOW")]
        shadow: Vec<String>,
        /// Motion feel, for example calm
        #[arg(long)]
        motion: Option<String>,
        /// Motion duration, for example base=220ms
        #[arg(long = "duration", value_name = "NAME=TIME")]
        duration: Vec<String>,
        /// Motion easing, for example standard=cubic-bezier(0.2, 0, 0, 1)
        #[arg(long = "ease", value_name = "NAME=CURVE")]
        ease: Vec<String>,
        /// Component note, for example button=Solid accent fill
        #[arg(long = "component", value_name = "NAME=NOTE")]
        component: Vec<String>,
        #[arg(long)]
        vibe: Option<String>,
        /// Reference image, as path=<relative> or url=<https://...>
        #[arg(long = "reference", value_name = "KIND=VALUE")]
        reference: Vec<String>,
        /// Markdown guidance stored in the board body
        #[arg(long)]
        guidance: Option<String>,
        /// Read a mood board Markdown file and overlay any flags onto it
        #[arg(long, value_name = "FILE")]
        from: Option<PathBuf>,
        /// Write to the global Brain (~/.cerebrum/) instead of the project Brain
        #[arg(long)]
        global: bool,
        /// Make this the default board in the Brain being written
        #[arg(long)]
        default: bool,
        /// Overwrite an existing board. Required before any existing slug changes.
        #[arg(long)]
        replace: bool,
    },
    /// List mood boards from the project and global Brains
    List {
        /// Print the design index as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show one mood board. Omit the name to show the default.
    Show {
        /// Board slug or unique name. Defaults to the effective default board.
        name: Option<String>,
        /// Print tokens and agent guidance as JSON
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        mix: DesignMix,
    },
    /// Mark a board as the default in one Brain
    SetDefault {
        /// Board slug
        name: String,
        /// Update the global Brain instead of the project Brain
        #[arg(long)]
        global: bool,
    },
    /// Delete a board from one Brain. The other Brain's copy is left in place.
    Remove {
        /// Board slug
        name: String,
        /// Delete from the global Brain instead of the project Brain
        #[arg(long)]
        global: bool,
    },
    /// Export a board as Tailwind config, CSS variables, JSON tokens, or SwiftUI
    Export {
        /// tailwind, css, json, or swiftui
        #[arg(long, value_enum)]
        format: DesignExportFormat,
        /// Write to this file instead of stdout
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
        /// Board slug or unique name. Defaults to the effective default board.
        name: Option<String>,
        #[command(flatten)]
        mix: DesignMix,
    },
    /// List dream proposals. They do not change a board until apply --approve.
    Proposals {
        #[arg(long)]
        json: bool,
    },
    /// Write a proposed board, only when --approve is set and the file has a fenced board
    Apply {
        /// Proposal Markdown file
        file: PathBuf,
        /// Explicit approval. Without this flag the board is not written.
        #[arg(long)]
        approve: bool,
        /// Person approving the proposal. Defaults to cli.
        #[arg(long, value_name = "NAME")]
        reviewed_by: Option<String>,
        /// Write the board into the global Brain
        #[arg(long)]
        global: bool,
    },
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum DesignExportFormat {
    Tailwind,
    Css,
    Json,
    Swiftui,
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
