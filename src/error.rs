use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, ConfectError>;

#[derive(Error, Debug)]
pub enum ConfectError {
    #[error("No confect repository at {0}. Run 'confect init' first.")]
    NotInitialized(PathBuf),

    #[error("Repository already initialized at {0}")]
    AlreadyInitialized(PathBuf),

    #[error(
        "Repository at {path} uses format version {found}, this confect needs {expected}. \
         Run 'confect migrate' first."
    )]
    UnsupportedRepoVersion {
        path: PathBuf,
        found: u32,
        expected: u32,
    },

    #[error("Category '{0}' not found")]
    CategoryNotFound(String),

    #[error("Category '{0}' already exists")]
    CategoryAlreadyExists(String),

    #[error("Invalid category name '{0}': use letters, digits, '.', '_' and '-'")]
    InvalidCategoryName(String),

    #[error("Invalid host name '{0}': use letters, digits, '.', '_' and '-'")]
    InvalidHostName(String),

    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("{path} is already tracked by category '{category}'")]
    PathAlreadyTracked { path: PathBuf, category: String },

    #[error("{0} is not tracked by any category")]
    PathNotTracked(PathBuf),

    #[error("{path} is excluded in category '{category}'")]
    PathExcluded { path: PathBuf, category: String },

    #[error("{path} overlaps with '{other}' in category '{category}'")]
    OverlappingPath {
        path: PathBuf,
        other: String,
        category: String,
    },

    #[error("Refusing to track {0}: it is a system-critical location")]
    ForbiddenPath(PathBuf),

    #[error("Invalid path {path}: {reason}")]
    InvalidPath { path: PathBuf, reason: String },

    #[error("Config error: {0}")]
    Config(String),

    #[error("git is not installed or not in PATH")]
    GitNotFound,

    #[error("git {command} failed: {message}")]
    Git { command: String, message: String },

    #[error("git {command} did not finish within {seconds} s")]
    GitTimeout { command: String, seconds: u64 },

    #[error("Local branch {branch} and {remote}/{branch} have diverged; resolve it with git in the repository")]
    Diverged { remote: String, branch: String },

    #[error("{path}: {source}")]
    IoAt {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("TOML serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    #[error("Glob pattern error: {0}")]
    GlobPattern(#[from] glob::PatternError),

    #[error("System error: {0}")]
    Unix(#[from] nix::errno::Errno),

    #[error("Prompt error: {0}")]
    Dialog(#[from] dialoguer::Error),

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Decryption error for {path}: {message}")]
    Decryption { path: PathBuf, message: String },

    #[error("No age identity at {0}: encrypted files cannot be read. Run 'confect key generate' or set [encryption] identity_file.")]
    NoIdentity(PathBuf),

    #[error("No age recipients at {0}: files cannot be encrypted. Run 'confect key generate' or set [encryption] recipients_file.")]
    NoRecipients(PathBuf),

    #[error("{0} file(s) look like plaintext secrets; nothing was written (see above)")]
    PlaintextSecrets(usize),

    #[error("Another confect process holds the repository lock")]
    LockBusy,

    #[error("Aborted")]
    Aborted,

    #[error("{0}")]
    Other(String),
}

impl ConfectError {
    pub fn io_at(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        ConfectError::IoAt {
            path: path.into(),
            source,
        }
    }
}

/// Attach the path to an IO error, which std leaves out of its messages.
pub trait IoContext<T> {
    fn at(self, path: impl Into<PathBuf>) -> Result<T>;
}

impl<T> IoContext<T> for std::result::Result<T, std::io::Error> {
    fn at(self, path: impl Into<PathBuf>) -> Result<T> {
        self.map_err(|source| ConfectError::io_at(path, source))
    }
}
