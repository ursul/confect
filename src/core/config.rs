use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{ConfectError, IoContext, Result};

pub const SYSTEM_REPO_PATH: &str = "/var/lib/confect";
pub const REPO_FORMAT_VERSION: u32 = 2;

/// Global confect configuration (`~/.config/confect/config.toml`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub global: GlobalConfig,
    #[serde(default)]
    pub encryption: EncryptionConfig,
    #[serde(default, skip_serializing_if = "HostsConfig::is_empty")]
    pub hosts: HostsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo_path: Option<PathBuf>,
    #[serde(default = "default_remote")]
    pub default_remote: String,
    #[serde(default = "default_true")]
    pub auto_push: bool,
    /// Seconds a git network operation may take before it is killed.
    #[serde(default = "default_network_timeout")]
    pub network_timeout: u64,
    /// Accepted from 1.x configs, not used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EncryptionConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_file: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipients_file: Option<PathBuf>,
    /// Accepted from 1.x configs, not used.
    #[serde(default, skip_serializing)]
    pub enabled: Option<bool>,
    /// Accepted from 1.x configs, not used.
    #[serde(default, skip_serializing)]
    pub public_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostsConfig {
    /// Host name used when the repository does not record one (1.x repositories).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<String>,
    /// Accepted from 1.x configs, not used.
    #[serde(default, skip_serializing)]
    pub strategy: Option<String>,
}

impl HostsConfig {
    fn is_empty(&self) -> bool {
        self.current.is_none()
    }
}

fn default_remote() -> String {
    "origin".to_string()
}

fn default_true() -> bool {
    true
}

fn default_network_timeout() -> u64 {
    300
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            repo_path: None,
            default_remote: default_remote(),
            auto_push: true,
            network_timeout: default_network_timeout(),
            editor: None,
        }
    }
}

const KNOWN_KEYS: &[(&str, &[&str])] = &[
    (
        "global",
        &[
            "repo_path",
            "default_remote",
            "auto_push",
            "network_timeout",
            "editor",
        ],
    ),
    (
        "encryption",
        &["identity_file", "recipients_file", "enabled", "public_key"],
    ),
    ("hosts", &["current", "strategy"]),
];

impl Config {
    pub fn dir() -> Result<PathBuf> {
        dirs::config_dir()
            .map(|d| d.join("confect"))
            .ok_or_else(|| ConfectError::Config("cannot determine the config directory".into()))
    }

    pub fn path() -> Result<PathBuf> {
        Ok(Self::dir()?.join("config.toml"))
    }

    /// Load the global configuration, warning about keys confect does not understand.
    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(&path).at(&path)?;
        for warning in unknown_keys(&content) {
            eprintln!("Warning: {}: {}", path.display(), warning);
        }
        toml::from_str(&content)
            .map_err(|e| ConfectError::Config(format!("{}: {}", path.display(), e)))
    }

    /// Save the configuration. Callers load it first, so existing settings survive.
    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).at(parent)?;
        }
        fs::write(&path, toml::to_string_pretty(self)?).at(&path)?;
        Ok(())
    }

    pub fn default_repo_path() -> PathBuf {
        dirs::data_dir()
            .map(|d| d.join("confect"))
            .unwrap_or_else(|| PathBuf::from(SYSTEM_REPO_PATH))
    }

    pub fn repo_path(&self) -> PathBuf {
        self.global
            .repo_path
            .clone()
            .unwrap_or_else(Self::default_repo_path)
    }

    pub fn identity_file(&self) -> Result<PathBuf> {
        match &self.encryption.identity_file {
            Some(path) => Ok(path.clone()),
            None => Ok(Self::dir()?.join("age-identity.txt")),
        }
    }

    pub fn recipients_file(&self) -> Result<PathBuf> {
        match &self.encryption.recipients_file {
            Some(path) => Ok(path.clone()),
            None => Ok(Self::dir()?.join("age-recipients.txt")),
        }
    }
}

fn unknown_keys(content: &str) -> Vec<String> {
    let Ok(table) = content.parse::<toml::Table>() else {
        return Vec::new();
    };
    let mut warnings = Vec::new();
    for (section, value) in &table {
        let Some(known) = KNOWN_KEYS.iter().find(|(name, _)| name == section) else {
            let hint = match section.as_str() {
                "repository" => " (the repository path is [global] repo_path)",
                "sync" => " (auto_push belongs to [global])",
                _ => "",
            };
            warnings.push(format!("unknown section [{}] is ignored{}", section, hint));
            continue;
        };
        if let Some(keys) = value.as_table() {
            for key in keys.keys() {
                if !known.1.contains(&key.as_str()) {
                    warnings.push(format!("unknown key {}.{} is ignored", section, key));
                }
            }
        }
    }
    warnings
}

/// Repository-local configuration (`.confect/config.toml`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RepoConfig {
    #[serde(default)]
    pub repository: RepoMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoMeta {
    #[serde(default = "legacy_version")]
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
}

impl Default for RepoMeta {
    fn default() -> Self {
        Self {
            version: REPO_FORMAT_VERSION,
            created: None,
            host: None,
        }
    }
}

fn legacy_version() -> u32 {
    1
}

impl RepoConfig {
    pub fn file(repo_path: &Path) -> PathBuf {
        repo_path.join(".confect").join("config.toml")
    }

    pub fn load(repo_path: &Path) -> Result<Self> {
        let path = Self::file(repo_path);
        if !path.exists() {
            return Ok(Self {
                repository: RepoMeta {
                    version: legacy_version(),
                    ..RepoMeta::default()
                },
            });
        }
        let content = fs::read_to_string(&path).at(&path)?;
        Ok(toml::from_str(&content)?)
    }

    pub fn save(&self, repo_path: &Path) -> Result<()> {
        let path = Self::file(repo_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).at(parent)?;
        }
        fs::write(&path, toml::to_string_pretty(self)?).at(&path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_keys_are_reported_with_hints() {
        let warnings =
            unknown_keys("[global]\nrepo_path = \"/x\"\nfoo = 1\n[repository]\npath = \"/y\"\n");
        assert!(warnings.iter().any(|w| w.contains("global.foo")));
        assert!(warnings.iter().any(|w| w.contains("[global] repo_path")));
    }

    #[test]
    fn legacy_1x_config_parses() {
        let config: Config = toml::from_str(
            "[global]\ndefault_remote = \"origin\"\nauto_push = false\nrepo_path = \"/var/lib/confect\"\n\n[encryption]\nenabled = false\n\n[hosts]\nstrategy = \"branch\"\ncurrent = \"h\"\n",
        )
        .unwrap();
        assert!(!config.global.auto_push);
        assert_eq!(config.hosts.current.as_deref(), Some("h"));
        assert_eq!(config.global.network_timeout, 300);
    }
}
