//! Writing into system directories without being redirected by symlinks.
//!
//! Paths are resolved one component at a time with `O_NOFOLLOW`, and every change is made
//! relative to the directory descriptor that was opened. A symlink component is followed
//! only when root or the current user owns it: anybody else could have planted it to make
//! a restore running as root write somewhere else. Merged-/usr links such as `/lib` are
//! owned by root and keep working.

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};

use nix::errno::Errno;
use nix::fcntl::{openat, readlinkat, renameat, AtFlags, OFlag};
use nix::sys::stat::{fchmod, fstatat, mkdirat, Mode, SFlag};
use nix::unistd::{fchown, fchownat, symlinkat, unlinkat, Gid, Uid, UnlinkatFlags};

use crate::core::paths::normalize;
use crate::error::{ConfectError, Result};

const MAX_SYMLINKS: usize = 40;

fn dir_flags() -> OFlag {
    OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_RDONLY | OFlag::O_CLOEXEC
}

fn fail(path: &Path, errno: Errno) -> ConfectError {
    ConfectError::io_at(path, io::Error::from(errno))
}

fn refuse(path: &Path, reason: &str) -> ConfectError {
    ConfectError::InvalidPath {
        path: path.to_path_buf(),
        reason: reason.to_string(),
    }
}

/// May a symlink with this owner be followed by a process running as `euid`?
pub fn trusted_link_owner(owner: u32, euid: u32) -> bool {
    owner == 0 || owner == euid
}

/// A directory opened by resolving its path without trusting foreign symlinks.
pub struct Dir {
    fd: OwnedFd,
    path: PathBuf,
}

impl Dir {
    /// Open `path`, creating missing directories (mode 0755) when `create` is set.
    pub fn open(path: &Path, create: bool) -> Result<Self> {
        let euid = nix::unistd::geteuid().as_raw();
        let mut pending: Vec<OsString> = components(&normalize(path))?;
        pending.reverse();
        let mut current = PathBuf::from("/");
        let mut fd = open_root(path)?;
        let mut links = 0;

        while let Some(name) = pending.pop() {
            match openat(&fd, name.as_os_str(), dir_flags(), Mode::empty()) {
                Ok(next) => {
                    fd = next;
                    current.push(&name);
                }
                Err(Errno::ENOENT) if create => {
                    match mkdirat(&fd, name.as_os_str(), Mode::from_bits_truncate(0o755)) {
                        Ok(()) | Err(Errno::EEXIST) => {}
                        Err(errno) => return Err(fail(&current.join(&name), errno)),
                    }
                    fd = openat(&fd, name.as_os_str(), dir_flags(), Mode::empty())
                        .map_err(|e| fail(&current.join(&name), e))?;
                    current.push(&name);
                }
                Err(Errno::ELOOP) | Err(Errno::ENOTDIR) => {
                    let here = current.join(&name);
                    let stat = fstatat(&fd, name.as_os_str(), AtFlags::AT_SYMLINK_NOFOLLOW)
                        .map_err(|e| fail(&here, e))?;
                    if SFlag::from_bits_truncate(stat.st_mode) & SFlag::S_IFMT != SFlag::S_IFLNK {
                        return Err(refuse(&here, "not a directory"));
                    }
                    if !trusted_link_owner(stat.st_uid, euid) {
                        return Err(refuse(
                            &here,
                            "a symlink owned by another user is in the way; not following it",
                        ));
                    }
                    links += 1;
                    if links > MAX_SYMLINKS {
                        return Err(refuse(path, "too many symlinks"));
                    }
                    let target = PathBuf::from(
                        readlinkat(&fd, name.as_os_str()).map_err(|e| fail(&here, e))?,
                    );
                    // `current` holds no symlinks, so resolving `..` lexically is exact.
                    let resolved = normalize(&if target.is_absolute() {
                        target
                    } else {
                        current.join(target)
                    });
                    let mut replacement = components(&resolved)?;
                    replacement.reverse();
                    pending.extend(replacement);
                    current = PathBuf::from("/");
                    fd = open_root(path)?;
                }
                Err(errno) => return Err(fail(&current.join(&name), errno)),
            }
        }
        Ok(Self { fd, path: current })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn stat(&self, name: &OsStr) -> Result<Option<nix::sys::stat::FileStat>> {
        match fstatat(&self.fd, name, AtFlags::AT_SYMLINK_NOFOLLOW) {
            Ok(stat) => Ok(Some(stat)),
            Err(Errno::ENOENT) => Ok(None),
            Err(errno) => Err(fail(&self.path.join(name), errno)),
        }
    }

    pub fn is_directory(&self, name: &OsStr) -> Result<bool> {
        Ok(self.stat(name)?.is_some_and(|s| {
            SFlag::from_bits_truncate(s.st_mode) & SFlag::S_IFMT == SFlag::S_IFDIR
        }))
    }

    /// Create a new temporary file in this directory (never following a planted link).
    pub fn create_temp(&self) -> Result<(OsString, File)> {
        for attempt in 0..1000u32 {
            let name = OsString::from(format!(
                ".confect-restore-{}-{}",
                std::process::id(),
                attempt
            ));
            let flags = OFlag::O_CREAT
                | OFlag::O_EXCL
                | OFlag::O_WRONLY
                | OFlag::O_NOFOLLOW
                | OFlag::O_CLOEXEC;
            match openat(
                &self.fd,
                name.as_os_str(),
                flags,
                Mode::from_bits_truncate(0o600),
            ) {
                Ok(fd) => return Ok((name, File::from(fd))),
                Err(Errno::EEXIST) => continue,
                Err(errno) => return Err(fail(&self.path.join(&name), errno)),
            }
        }
        Err(refuse(&self.path, "cannot create a temporary file"))
    }

    /// Create a symlink under a fresh temporary name.
    pub fn create_temp_link(&self, target: &Path) -> Result<OsString> {
        for attempt in 0..1000u32 {
            let name = OsString::from(format!(
                ".confect-restore-{}-{}",
                std::process::id(),
                attempt
            ));
            match symlinkat(target, &self.fd, name.as_os_str()) {
                Ok(()) => return Ok(name),
                Err(Errno::EEXIST) => continue,
                Err(errno) => return Err(fail(&self.path.join(&name), errno)),
            }
        }
        Err(refuse(&self.path, "cannot create a temporary symlink"))
    }

    pub fn chown_link(&self, name: &OsStr, uid: u32, gid: u32) -> std::result::Result<(), Errno> {
        fchownat(
            &self.fd,
            name,
            Some(Uid::from_raw(uid)),
            Some(Gid::from_raw(gid)),
            AtFlags::AT_SYMLINK_NOFOLLOW,
        )
    }

    /// Atomically replace `name` with `temp`; a symlink at `name` is replaced, not followed.
    pub fn replace(&self, temp: &OsStr, name: &OsStr) -> Result<()> {
        renameat(&self.fd, temp, &self.fd, name).map_err(|errno| {
            let _ = unlinkat(&self.fd, temp, UnlinkatFlags::NoRemoveDir);
            fail(&self.path.join(name), errno)
        })
    }

    pub fn remove_temp(&self, temp: &OsStr) {
        let _ = unlinkat(&self.fd, temp, UnlinkatFlags::NoRemoveDir);
    }

    /// Copy the current `name` to `<name>.confect-backup.<stamp>` in the same directory.
    pub fn backup(&self, name: &OsStr, stamp: &str) -> Result<Option<PathBuf>> {
        let Some(stat) = self.stat(name)? else {
            return Ok(None);
        };
        let mut backup = name.to_os_string();
        backup.push(format!(".confect-backup.{}", stamp));
        let kind = SFlag::from_bits_truncate(stat.st_mode) & SFlag::S_IFMT;

        if kind == SFlag::S_IFLNK {
            let target = readlinkat(&self.fd, name).map_err(|e| fail(&self.path.join(name), e))?;
            symlinkat(target.as_os_str(), &self.fd, backup.as_os_str())
                .map_err(|e| fail(&self.path.join(&backup), e))?;
            return Ok(Some(self.path.join(backup)));
        }
        if kind != SFlag::S_IFREG {
            return Ok(None);
        }
        let source = openat(
            &self.fd,
            name,
            OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|e| fail(&self.path.join(name), e))?;
        let destination = openat(
            &self.fd,
            backup.as_os_str(),
            OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_WRONLY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::from_bits_truncate(stat.st_mode & 0o7777),
        )
        .map_err(|e| fail(&self.path.join(&backup), e))?;
        let mut source = File::from(source);
        let mut destination = File::from(destination);
        io::copy(&mut source, &mut destination)
            .map_err(|e| ConfectError::io_at(self.path.join(&backup), e))?;
        Ok(Some(self.path.join(backup)))
    }

    /// Set owner and mode of the directory itself.
    pub fn set_attributes(
        &self,
        uid: Option<u32>,
        gid: Option<u32>,
        mode: Option<u32>,
    ) -> std::result::Result<(), Errno> {
        if uid.is_some() || gid.is_some() {
            fchown(&self.fd, uid.map(Uid::from_raw), gid.map(Gid::from_raw))?;
        }
        if let Some(mode) = mode {
            fchmod(&self.fd, Mode::from_bits_truncate(mode))?;
        }
        Ok(())
    }

    pub fn owner(&self) -> Result<(u32, u32)> {
        let stat = nix::sys::stat::fstat(self.fd.as_fd()).map_err(|e| fail(&self.path, e))?;
        Ok((stat.st_uid, stat.st_gid))
    }
}

fn open_root(original: &Path) -> Result<OwnedFd> {
    nix::fcntl::open("/", dir_flags(), Mode::empty()).map_err(|e| fail(original, e))
}

/// Components of a normalized absolute path.
fn components(path: &Path) -> Result<Vec<OsString>> {
    let mut result = Vec::new();
    for component in path.components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(name) => result.push(name.to_os_string()),
            Component::ParentDir | Component::Prefix(_) => {
                return Err(refuse(path, "unsupported path"))
            }
        }
    }
    if !path.is_absolute() {
        return Err(refuse(path, "expected an absolute path"));
    }
    Ok(result)
}

/// Split an absolute path into its parent directory and final name.
pub fn split(path: &Path) -> Result<(&Path, &OsStr)> {
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) if !name.as_bytes().is_empty() => Ok((parent, name)),
        _ => Err(refuse(path, "no file name")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn only_root_or_own_symlinks_are_trusted() {
        assert!(trusted_link_owner(0, 0));
        assert!(trusted_link_owner(0, 1000));
        assert!(trusted_link_owner(1000, 1000));
        assert!(!trusted_link_owner(1001, 0));
        assert!(!trusted_link_owner(1001, 1000));
    }

    #[test]
    fn open_creates_missing_directories_and_follows_own_links() {
        let root = tempfile::tempdir().unwrap();
        let real = root.path().join("real");
        fs::create_dir(&real).unwrap();
        std::os::unix::fs::symlink(&real, root.path().join("link")).unwrap();

        let dir = Dir::open(&root.path().join("link/new/deeper"), true).unwrap();
        assert_eq!(dir.path(), real.join("new/deeper"));
        assert!(real.join("new/deeper").is_dir());
    }

    #[test]
    fn replace_swaps_a_symlink_itself() {
        let root = tempfile::tempdir().unwrap();
        let victim = root.path().join("victim");
        fs::write(&victim, "precious").unwrap();
        std::os::unix::fs::symlink(&victim, root.path().join("target")).unwrap();

        let dir = Dir::open(root.path(), false).unwrap();
        let (temp, mut file) = dir.create_temp().unwrap();
        file.write_all(b"new").unwrap();
        dir.replace(&temp, OsStr::new("target")).unwrap();
        assert_eq!(fs::read_to_string(&victim).unwrap(), "precious");
        assert_eq!(
            fs::read_to_string(root.path().join("target")).unwrap(),
            "new"
        );
    }

    #[test]
    fn backup_never_writes_through_a_planted_symlink() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("app.conf"), "current").unwrap();
        fs::write(root.path().join("victim"), "precious").unwrap();
        std::os::unix::fs::symlink(
            root.path().join("victim"),
            root.path().join("app.conf.confect-backup.STAMP"),
        )
        .unwrap();

        let dir = Dir::open(root.path(), false).unwrap();
        assert!(dir.backup(OsStr::new("app.conf"), "STAMP").is_err());
        assert_eq!(
            fs::read_to_string(root.path().join("victim")).unwrap(),
            "precious"
        );
        let saved = dir
            .backup(OsStr::new("app.conf"), "OTHER")
            .unwrap()
            .unwrap();
        assert_eq!(fs::read_to_string(saved).unwrap(), "current");
    }
}
