use chrono::{DateTime, Utc};
use nix::unistd::{Gid, Group, Uid, User};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use crate::core::Repository;
use crate::error::Result;

/// Metadata for a single file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    /// File mode (permissions)
    pub mode: u32,
    /// Owner UID
    pub uid: u32,
    /// Group GID
    pub gid: u32,
    /// Owner username
    pub owner: String,
    /// Group name
    pub group: String,
    /// Modification time
    #[serde(default)]
    pub mtime: Option<DateTime<Utc>>,
    /// Symlink target (if this is a symlink)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symlink_target: Option<PathBuf>,
}

impl FileMetadata {
    /// Collect metadata from a file
    pub fn from_path(path: &Path) -> Result<Self> {
        let meta = fs::symlink_metadata(path)?;

        let symlink_target = if meta.file_type().is_symlink() {
            Some(fs::read_link(path)?)
        } else {
            None
        };

        let uid = meta.uid();
        let gid = meta.gid();

        let owner = User::from_uid(Uid::from_raw(uid))
            .ok()
            .flatten()
            .map(|u| u.name)
            .unwrap_or_else(|| uid.to_string());

        let group = Group::from_gid(Gid::from_raw(gid))
            .ok()
            .flatten()
            .map(|g| g.name)
            .unwrap_or_else(|| gid.to_string());

        let mtime = DateTime::from_timestamp(meta.mtime(), 0);

        Ok(Self {
            mode: meta.permissions().mode(),
            uid,
            gid,
            owner,
            group,
            mtime,
            symlink_target,
        })
    }

    /// Apply metadata to a file
    pub fn apply_to(&self, path: &Path) -> Result<()> {
        // Handle symlinks specially
        if let Some(ref target) = self.symlink_target {
            // Remove existing file if any
            if path.exists() || path.symlink_metadata().is_ok() {
                fs::remove_file(path)?;
            }
            // Create symlink
            std::os::unix::fs::symlink(target, path)?;
            // Note: lchown for symlinks requires special handling
            // For now we skip ownership on symlinks as it's less critical
        } else {
            // Regular file - set permissions
            let perms = fs::Permissions::from_mode(self.mode);
            fs::set_permissions(path, perms)?;

            // Set ownership
            nix::unistd::chown(
                path,
                Some(Uid::from_raw(self.uid)),
                Some(Gid::from_raw(self.gid)),
            )?;
        }

        Ok(())
    }

    /// Get human-readable mode string (e.g., "0644")
    pub fn mode_string(&self) -> String {
        format!("{:04o}", self.mode & 0o7777)
    }
}

/// Metadata file structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct MetadataFile {
    #[serde(default)]
    files: HashMap<String, FileMetadata>,
}

/// Store for file metadata
pub struct MetadataStore {
    entries: HashMap<PathBuf, FileMetadata>,
    repo_path: PathBuf,
}

impl MetadataStore {
    /// Load metadata from repository
    pub fn load(repo: &Repository) -> Result<Self> {
        let repo_path = repo.path().to_path_buf();
        let metadata_file = repo_path.join(".confect").join("metadata.toml");

        let entries = if metadata_file.exists() {
            let content = fs::read_to_string(&metadata_file)?;
            let file: MetadataFile = toml::from_str(&content)?;

            file.files
                .into_iter()
                .map(|(path, meta)| (PathBuf::from(path), meta))
                .collect()
        } else {
            HashMap::new()
        };

        Ok(Self { entries, repo_path })
    }

    /// Save metadata to repository
    pub fn save(&self) -> Result<()> {
        let confect_dir = self.repo_path.join(".confect");
        fs::create_dir_all(&confect_dir)?;

        let metadata_file = confect_dir.join("metadata.toml");

        let file = MetadataFile {
            files: self
                .entries
                .iter()
                .map(|(path, meta)| (path.to_string_lossy().to_string(), meta.clone()))
                .collect(),
        };

        let content = toml::to_string_pretty(&file)?;
        fs::write(&metadata_file, content)?;
        Ok(())
    }

    /// Update metadata for a file from the current system state
    pub fn update_from_system(&mut self, path: &Path) -> Result<()> {
        let meta = FileMetadata::from_path(path)?;
        self.entries.insert(path.to_path_buf(), meta);
        Ok(())
    }

    /// Get metadata for a file
    pub fn get(&self, path: &Path) -> Option<&FileMetadata> {
        self.entries.get(path)
    }

    /// Apply metadata to a file
    pub fn apply_to(&self, path: &Path) -> Result<()> {
        if let Some(meta) = self.entries.get(path) {
            meta.apply_to(path)?;
        }
        Ok(())
    }

    /// Apply all stored metadata
    pub fn apply_all(&self) -> Result<()> {
        for (path, meta) in &self.entries {
            if path.exists() || meta.symlink_target.is_some() {
                meta.apply_to(path)?;
            }
        }
        Ok(())
    }

    /// Remove metadata for a file
    pub fn remove(&mut self, path: &Path) {
        self.entries.remove(path);
    }
}
