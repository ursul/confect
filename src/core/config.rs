use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

use crate::error::{Result, ConfectError};

/// Global confect configuration (~/.config/confect/config.toml)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub global: GlobalConfig,
    #[serde(default)]
    pub encryption: EncryptionConfig,
    #[serde(default)]
    pub hosts: HostsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    #[serde(default = "default_remote")]
    pub default_remote: String,
    #[serde(default = "default_true")]
    pub auto_push: bool,
    #[serde(default)]
    pub editor: Option<String>,
    #[serde(default)]
    pub repo_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub public_key: Option<String>,
    #[serde(default)]
    pub recipients_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostsConfig {
    #[serde(default = "default_strategy")]
    pub strategy: String,
    #[serde(default)]
    pub current: Option<String>,
}

fn default_remote() -> String {
    "origin".to_string()
}

fn default_true() -> bool {
    true
}

fn default_strategy() -> String {
    "branch".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            global: GlobalConfig::default(),
            encryption: EncryptionConfig::default(),
            hosts: HostsConfig::default(),
        }
    }
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            default_remote: default_remote(),
            auto_push: true,
            editor: None,
            repo_path: None,
        }
    }
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            public_key: None,
            recipients_file: None,
        }
    }
}

impl Default for HostsConfig {
    fn default() -> Self {
        Self {
            strategy: default_strategy(),
            current: None,
        }
    }
}

impl Config {
    /// Get the path to the global config file
    pub fn global_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| ConfectError::Config("Could not find config directory".to_string()))?;
        Ok(config_dir.join("confect").join("config.toml"))
    }

    /// Load global configuration
    pub fn load_global() -> Result<Self> {
        let path = Self::global_path()?;

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save global configuration
    pub fn save_global(&self) -> Result<()> {
        let path = Self::global_path()?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }

    /// Initialize global config with hostname
    pub fn init_global(hostname: &str) -> Result<Self> {
        let mut config = Self::default();
        config.hosts.current = Some(hostname.to_string());
        config.save_global()?;
        Ok(config)
    }

    /// Get the default repository path (XDG-compatible)
    pub fn default_repo_path() -> PathBuf {
        dirs::data_dir()
            .map(|d| d.join("confect"))
            .unwrap_or_else(|| PathBuf::from("/var/lib/confect"))
    }

    /// Get the system-wide repository path
    pub fn system_repo_path() -> PathBuf {
        PathBuf::from("/var/lib/confect")
    }

    /// Get the configured repository path
    pub fn repo_path(&self) -> PathBuf {
        self.global
            .repo_path
            .clone()
            .unwrap_or_else(Self::default_repo_path)
    }
}

/// Repository-local configuration (.confect/config.toml)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoConfig {
    #[serde(default)]
    pub repository: RepoMeta,
    #[serde(default)]
    pub hosts: RepoHostsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RepoMeta {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub created: Option<String>,
}

fn default_version() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RepoHostsConfig {
    #[serde(default = "default_strategy")]
    pub strategy: String,
    #[serde(default)]
    pub list: std::collections::HashMap<String, HostEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostEntry {
    pub branch: String,
}

impl Default for RepoConfig {
    fn default() -> Self {
        Self {
            repository: RepoMeta::default(),
            hosts: RepoHostsConfig::default(),
        }
    }
}

impl RepoConfig {
    /// Save to repository path
    pub fn save(&self, repo_path: &Path) -> Result<()> {
        let confect_dir = repo_path.join(".confect");
        fs::create_dir_all(&confect_dir)?;

        let config_path = confect_dir.join("config.toml");
        let content = toml::to_string_pretty(self)?;
        fs::write(&config_path, content)?;
        Ok(())
    }
}
