use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::core::paths::repo_relative;
use crate::error::{IoContext, Result};
use crate::track::entry::{group_name, mode_string, parse_mode, user_name, Kind, SysEntry};

pub const METADATA_VERSION: u32 = 2;

/// What the repository knows about one tracked path. This index is the source of truth
/// for which paths are stored, how (plain or encrypted) and with which permissions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntryMeta {
    pub category: String,
    pub kind: Kind,
    /// Octal permission bits such as `0640`; absent for symlinks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    pub owner: String,
    pub group: String,
    pub uid: u32,
    pub gid: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub encrypted: bool,
    /// Size and mtime of the plaintext when it was last encrypted: lets a host without
    /// the identity notice changes without decrypting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mtime_ns: Option<i64>,
}

impl EntryMeta {
    pub fn from_system(category: &str, entry: &SysEntry, encrypted: bool) -> Self {
        Self {
            category: category.to_string(),
            kind: entry.kind,
            mode: (entry.kind != Kind::Symlink).then(|| mode_string(entry.mode)),
            owner: user_name(entry.uid),
            group: group_name(entry.gid),
            uid: entry.uid,
            gid: entry.gid,
            target: entry
                .target
                .as_ref()
                .map(|t| t.to_string_lossy().into_owned()),
            encrypted,
            size: encrypted.then_some(entry.size),
            mtime_ns: encrypted.then_some(entry.mtime_ns),
        }
    }

    pub fn mode_bits(&self) -> Option<u32> {
        self.mode.as_deref().and_then(parse_mode)
    }

    /// Permission and ownership differ from the system entry.
    pub fn attributes_differ(&self, entry: &SysEntry) -> bool {
        if self.kind != entry.kind || self.uid != entry.uid || self.gid != entry.gid {
            return true;
        }
        entry.kind != Kind::Symlink && self.mode_bits() != Some(entry.mode)
    }

    /// Location of the stored copy relative to the repository root.
    pub fn repo_path(&self, system_path: &Path) -> PathBuf {
        repo_relative(&self.category, system_path, self.encrypted)
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct MetadataFile {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    entries: BTreeMap<String, EntryMeta>,
}

/// `.confect/metadata.toml`, kept sorted by path so diffs stay readable.
pub struct Metadata {
    file: PathBuf,
    entries: BTreeMap<PathBuf, EntryMeta>,
    legacy: bool,
}

impl Metadata {
    pub fn file(repo_path: &Path) -> PathBuf {
        repo_path.join(".confect").join("metadata.toml")
    }

    pub fn load(repo_path: &Path) -> Result<Self> {
        let file = Self::file(repo_path);
        let parsed: MetadataFile = if file.exists() {
            toml::from_str(&fs::read_to_string(&file).at(&file)?)?
        } else {
            MetadataFile::default()
        };
        let legacy = parsed.version < METADATA_VERSION;
        let entries = if legacy {
            BTreeMap::new()
        } else {
            parsed
                .entries
                .into_iter()
                .map(|(path, meta)| (PathBuf::from(path), meta))
                .collect()
        };
        Ok(Self {
            file,
            entries,
            legacy,
        })
    }

    /// 1.x metadata, whose entries are not usable and get rebuilt by `migrate`.
    pub fn is_legacy(&self) -> bool {
        self.legacy
    }

    pub fn save(&self) -> Result<()> {
        let file = MetadataFile {
            version: METADATA_VERSION,
            entries: self
                .entries
                .iter()
                .map(|(path, meta)| (path.to_string_lossy().into_owned(), meta.clone()))
                .collect(),
        };
        fs::write(&self.file, toml::to_string_pretty(&file)?).at(&self.file)?;
        Ok(())
    }

    pub fn get(&self, path: &Path) -> Option<&EntryMeta> {
        self.entries.get(path)
    }

    pub fn insert(&mut self, path: PathBuf, meta: EntryMeta) {
        self.entries.insert(path, meta);
    }

    pub fn remove(&mut self, path: &Path) -> Option<EntryMeta> {
        self.entries.remove(path)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&PathBuf, &EntryMeta)> {
        self.entries.iter()
    }

    pub fn in_category<'a>(
        &'a self,
        category: &'a str,
    ) -> impl Iterator<Item = (&'a PathBuf, &'a EntryMeta)> + 'a {
        self.entries
            .iter()
            .filter(move |(_, meta)| meta.category == category)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
