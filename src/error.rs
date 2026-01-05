use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, ConfectError>;

#[derive(Error, Debug)]
pub enum ConfectError {
    #[error("Repository not initialized. Run 'confect init' first.")]
    NotInitialized,

    #[error("Repository already initialized at {0}")]
    AlreadyInitialized(PathBuf),

    #[error("Category '{0}' not found")]
    CategoryNotFound(String),

    #[error("Category '{0}' already exists")]
    CategoryAlreadyExists(String),

    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Path already tracked: {0}")]
    PathAlreadyTracked(PathBuf),

    #[error("Path not tracked: {0}")]
    PathNotTracked(PathBuf),

    #[error("Forbidden path: {0}. Cannot track system-critical directories.")]
    ForbiddenPath(PathBuf),

    #[error("Invalid path: {0}")]
    InvalidPath(PathBuf),

    #[error("No changes to sync")]
    NoChanges,

    #[error("Config file error: {0}")]
    Config(String),

    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("TOML serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    #[error("Glob pattern error: {0}")]
    GlobPattern(#[from] glob::PatternError),

    #[error("Glob error: {0}")]
    Glob(#[from] glob::GlobError),

    #[error("Walk directory error: {0}")]
    WalkDir(#[from] walkdir::Error),

    #[error("Unix error: {0}")]
    Unix(#[from] nix::errno::Errno),

    #[error("Dialog error: {0}")]
    Dialog(#[from] dialoguer::Error),

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Decryption error: {0}")]
    Decryption(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(PathBuf),

    #[error("Host '{0}' not found in repository")]
    HostNotFound(String),

    #[error("{0}")]
    Other(String),
}
