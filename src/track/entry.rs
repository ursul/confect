use nix::unistd::{Gid, Group, Uid, User};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    File,
    Symlink,
    Dir,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::File => "file",
            Kind::Symlink => "symlink",
            Kind::Dir => "directory",
        }
    }
}

/// A file, symlink or directory as it currently is on the system.
#[derive(Debug, Clone)]
pub struct SysEntry {
    pub path: PathBuf,
    pub kind: Kind,
    /// Permission bits including setuid/setgid/sticky (no file type bits).
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub size: u64,
    pub mtime_ns: i64,
    pub target: Option<PathBuf>,
}

impl SysEntry {
    /// `lstat` the path; `Ok(None)` for sockets, FIFOs and devices, which are not tracked.
    pub fn read(path: &Path) -> io::Result<Option<Self>> {
        let meta = fs::symlink_metadata(path)?;
        let file_type = meta.file_type();
        let kind = if file_type.is_symlink() {
            Kind::Symlink
        } else if file_type.is_dir() {
            Kind::Dir
        } else if file_type.is_file() {
            Kind::File
        } else {
            // Sockets, FIFOs and devices.
            return Ok(None);
        };
        let target = if kind == Kind::Symlink {
            Some(fs::read_link(path)?)
        } else {
            None
        };
        Ok(Some(Self {
            path: path.to_path_buf(),
            kind,
            mode: meta.mode() & 0o7777,
            uid: meta.uid(),
            gid: meta.gid(),
            size: meta.size(),
            mtime_ns: meta.mtime() * 1_000_000_000 + meta.mtime_nsec(),
            target,
        }))
    }
}

pub fn user_name(uid: u32) -> String {
    User::from_uid(Uid::from_raw(uid))
        .ok()
        .flatten()
        .map(|u| u.name)
        .unwrap_or_else(|| uid.to_string())
}

pub fn group_name(gid: u32) -> String {
    Group::from_gid(Gid::from_raw(gid))
        .ok()
        .flatten()
        .map(|g| g.name)
        .unwrap_or_else(|| gid.to_string())
}

/// Resolve an owner on this host by name first (UIDs differ between machines).
pub fn resolve_uid(name: &str, fallback: u32) -> u32 {
    User::from_name(name)
        .ok()
        .flatten()
        .map(|u| u.uid.as_raw())
        .unwrap_or(fallback)
}

pub fn resolve_gid(name: &str, fallback: u32) -> u32 {
    Group::from_name(name)
        .ok()
        .flatten()
        .map(|g| g.gid.as_raw())
        .unwrap_or(fallback)
}

pub fn mode_string(mode: u32) -> String {
    format!("{:04o}", mode & 0o7777)
}

pub fn parse_mode(mode: &str) -> Option<u32> {
    u32::from_str_radix(mode, 8).ok().map(|m| m & 0o7777)
}
