use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "confect")]
#[command(author, version, about = "Keep system configuration files in Git")]
#[command(propagate_version = true)]
pub struct Cli {
    /// Repository to use instead of the configured one
    #[arg(long, global = true, env = "CONFECT_REPO", value_name = "PATH")]
    pub repo: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create a repository, or clone an existing one with --from
    Init(InitArgs),

    /// Start tracking files or directories
    Add {
        /// Paths to track
        #[arg(required = true)]
        paths: Vec<PathBuf>,

        /// Category to add them to
        #[arg(short, long)]
        category: String,

        /// Create the category if it does not exist
        #[arg(long)]
        create_category: bool,

        /// Store the files encrypted with age
        #[arg(short, long)]
        encrypt: bool,
    },

    /// Stop tracking paths and drop their stored copies
    Remove {
        /// Paths to untrack
        #[arg(required = true)]
        paths: Vec<PathBuf>,
    },

    /// Show how the system differs from the repository
    Status {
        /// Limit to these paths
        paths: Vec<PathBuf>,

        /// Limit to one category
        #[arg(short, long)]
        category: Option<String>,

        /// Show content diffs as well
        #[arg(short, long)]
        diff: bool,

        /// Exit with status 3 when anything differs (for monitoring)
        #[arg(long)]
        exit_code: bool,
    },

    /// Show content differences between the repository and the system
    Diff {
        /// Limit to these paths
        paths: Vec<PathBuf>,

        /// Limit to one category
        #[arg(short, long)]
        category: Option<String>,
    },

    /// Record the system state in the repository, commit and push
    Sync {
        /// Commit message
        #[arg(short, long)]
        message: Option<String>,

        /// Do not push, whatever auto_push says
        #[arg(long, conflicts_with = "push")]
        no_push: bool,

        /// Push even if auto_push is off
        #[arg(long)]
        push: bool,
    },

    /// Write stored files back to the system
    Restore {
        /// Limit to these paths
        paths: Vec<PathBuf>,

        /// Limit to one category
        #[arg(short, long)]
        category: Option<String>,

        /// Only show what would change
        #[arg(short = 'n', long)]
        dry_run: bool,

        /// Do not ask for confirmation
        #[arg(short, long)]
        yes: bool,

        /// Keep a timestamped copy of every file that gets overwritten
        #[arg(short, long)]
        backup: bool,
    },

    /// Fast-forward this host's branch from the remote
    Pull {
        /// Restore the pulled files afterwards, keeping backups of overwritten files
        #[arg(long)]
        restore: bool,

        /// Do not ask for confirmation before restoring
        #[arg(short, long, requires = "restore")]
        yes: bool,
    },

    /// Push this host's branch to the remote
    Push,

    /// Manage categories
    #[command(subcommand)]
    Category(CategoryCommands),

    /// Look for plaintext secrets in the repository
    Audit {
        /// Search every commit, not only the current files
        #[arg(long)]
        history: bool,
    },

    /// Manage the age key used for encrypted files
    #[command(subcommand)]
    Key(KeyCommands),

    /// Convert a 1.x repository to the current format
    Migrate {
        /// Do not ask for confirmation
        #[arg(short, long)]
        yes: bool,
    },

    /// Show repository information
    Info,

    /// Install a systemd timer that runs 'confect sync'
    SetupTimer {
        /// OnCalendar schedule
        #[arg(short, long, default_value = "daily")]
        schedule: String,

        /// Commit message used by the timer
        #[arg(short, long, default_value = "Automatic backup")]
        message: String,

        /// Install as a user unit (default for non-root users)
        #[arg(long)]
        user: bool,

        /// Stop, disable and delete the timer instead
        #[arg(long)]
        remove: bool,
    },

    /// Update confect from the GitHub release, verifying its checksum
    #[command(name = "self-update")]
    SelfUpdate {
        /// Only check whether a newer release exists
        #[arg(long)]
        check: bool,

        /// Do not ask for confirmation
        #[arg(short, long)]
        yes: bool,
    },
}

#[derive(Args)]
pub struct InitArgs {
    /// Repository directory (default: ~/.local/share/confect)
    #[arg(short, long, conflicts_with = "system")]
    pub path: Option<PathBuf>,

    /// Use the system-wide repository /var/lib/confect
    #[arg(long)]
    pub system: bool,

    /// Remote to push to
    #[arg(long, conflicts_with = "from")]
    pub remote: Option<String>,

    /// Clone this repository and continue this host's branch in it
    #[arg(long, value_name = "URL")]
    pub from: Option<String>,

    /// Host name, which is also the branch name host/<name> (default: hostname)
    #[arg(long)]
    pub host: Option<String>,
}

#[derive(Subcommand)]
pub enum CategoryCommands {
    /// List categories
    List,

    /// Show a category's patterns and files
    Show { name: String },

    /// Create a category
    Create {
        name: String,

        /// Paths or patterns to track
        #[arg(short, long, required = true)]
        path: Vec<String>,

        /// Description
        #[arg(short, long)]
        description: Option<String>,

        /// Patterns of files to store encrypted
        #[arg(short, long)]
        encrypt: Vec<String>,

        /// Patterns to leave out
        #[arg(short = 'x', long)]
        exclude: Vec<String>,

        /// Patterns the secret guard lets through in plaintext
        #[arg(long)]
        allow_plaintext: Vec<String>,
    },

    /// Delete a category and its stored copies
    Delete {
        name: String,

        /// Do not ask for confirmation
        #[arg(short, long)]
        yes: bool,
    },

    /// Add a path or pattern to a category
    AddPath { name: String, path: String },

    /// Remove a path or pattern from a category and drop its copies
    RemovePath { name: String, path: String },

    /// Manage exclusions
    #[command(subcommand)]
    Exclude(PatternCommands),

    /// Manage patterns of files stored encrypted
    #[command(subcommand)]
    Encrypt(PatternCommands),

    /// Manage patterns the secret guard lets through in plaintext
    #[command(subcommand, name = "allow-plaintext")]
    AllowPlaintext(PatternCommands),
}

#[derive(Subcommand)]
pub enum PatternCommands {
    /// Add a pattern
    Add { name: String, pattern: String },
    /// Remove a pattern
    Remove { name: String, pattern: String },
}

#[derive(Subcommand)]
pub enum KeyCommands {
    /// Create the age identity and recipients files
    Generate,
    /// Print the recipients encrypted files are written for
    Show,
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_consistent() {
        Cli::command().debug_assert();
    }
}
