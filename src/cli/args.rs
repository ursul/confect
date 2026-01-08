use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "confect")]
#[command(author, version, about = "Manage system configuration files with Git")]
#[command(propagate_version = true)]
pub struct Cli {
    /// Path to the confect repository
    #[arg(short, long, env = "CONFECT_REPO")]
    pub repo: Option<PathBuf>,

    /// Verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new confect repository
    Init {
        /// Path to create the repository (default: ~/.local/share/confect)
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// Use system-wide repository (/var/lib/confect, requires sudo)
        #[arg(long)]
        system: bool,

        /// Remote Git URL to use
        #[arg(short, long)]
        remote: Option<String>,

        /// Hostname for this machine (default: auto-detect)
        #[arg(long)]
        host: Option<String>,
    },

    /// Add a file or directory to be tracked
    Add {
        /// Path to the file or directory
        path: PathBuf,

        /// Category to add the file to
        #[arg(short, long)]
        category: Option<String>,

        /// Create category if it doesn't exist
        #[arg(long)]
        create_category: bool,

        /// Mark this file as containing secrets (will be encrypted)
        #[arg(short, long)]
        encrypt: bool,
    },

    /// Remove a file or directory from tracking
    Remove {
        /// Path to the file or directory
        path: PathBuf,

        /// Also delete from the repository (not just untrack)
        #[arg(long)]
        delete: bool,
    },

    /// Show status of tracked files
    Status {
        /// Show only files in this category
        #[arg(short, long)]
        category: Option<String>,

        /// Show detailed diff
        #[arg(short, long)]
        diff: bool,
    },

    /// Sync changes to the repository (commit and optionally push)
    Sync {
        /// Commit message
        #[arg(short, long)]
        message: Option<String>,

        /// Don't push to remote
        #[arg(long)]
        no_push: bool,

        /// Sync all hosts (requires push access to all branches)
        #[arg(long)]
        all_hosts: bool,
    },

    /// Restore files from the repository to the system
    Restore {
        /// Category to restore (default: all)
        category: Option<String>,

        /// Specific file to restore
        #[arg(short, long)]
        file: Option<PathBuf>,

        /// Show what would be done without making changes
        #[arg(long)]
        dry_run: bool,

        /// Don't ask for confirmation
        #[arg(short, long)]
        force: bool,

        /// Create backup of existing files before restoring
        #[arg(short, long)]
        backup: bool,
    },

    /// Manage categories
    #[command(subcommand)]
    Category(CategoryCommands),

    /// Show repository information
    Info,

    /// Set up automatic backup timer (systemd)
    SetupTimer {
        /// Timer schedule (systemd OnCalendar format)
        #[arg(short, long, default_value = "hourly")]
        schedule: String,

        /// Remove the timer instead of creating it
        #[arg(long)]
        remove: bool,
    },

    /// Pull latest changes from remote
    Pull {
        /// Also restore files after pulling
        #[arg(short, long)]
        restore: bool,
    },

    /// Show diff between system files and repository
    Diff {
        /// Category to diff
        category: Option<String>,

        /// Specific file to diff
        #[arg(short, long)]
        file: Option<PathBuf>,
    },

    /// Update confect to the latest version
    #[command(name = "self-update")]
    SelfUpdate {
        /// Check for updates without installing
        #[arg(long)]
        check: bool,
    },
}

#[derive(Subcommand)]
pub enum CategoryCommands {
    /// List all categories
    List,

    /// Show files in a category
    Show {
        /// Category name
        name: String,
    },

    /// Create a new category
    Create {
        /// Category name
        name: String,

        /// Description of the category
        #[arg(short, long)]
        description: Option<String>,

        /// Paths to include (glob patterns supported)
        #[arg(short, long, required = true)]
        path: Vec<String>,

        /// Patterns for files that should be encrypted
        #[arg(short, long)]
        encrypt: Vec<String>,
    },

    /// Delete a category
    Delete {
        /// Category name
        name: String,

        /// Don't ask for confirmation
        #[arg(short, long)]
        force: bool,

        /// Also remove files from repository
        #[arg(long)]
        remove_files: bool,
    },

    /// Add a path to an existing category
    AddPath {
        /// Category name
        name: String,

        /// Path to add (glob pattern supported)
        path: String,

        /// Mark as encrypted
        #[arg(short, long)]
        encrypt: bool,
    },

    /// Remove a path from a category
    RemovePath {
        /// Category name
        name: String,

        /// Path to remove
        path: String,
    },
}
