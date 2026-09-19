use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use nix::fcntl::AtFlags;
use nix::unistd::{fchownat, Gid, Uid};

use crate::error::{ConfectError, IoContext, Result};
use crate::track::entry::{resolve_gid, resolve_uid, Kind, SysEntry};
use crate::track::metadata::{EntryMeta, Metadata};
use crate::track::store::{files_equal, Store};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreState {
    /// Nothing at the path yet.
    Create,
    /// Content differs from the stored copy.
    Overwrite,
    /// Only permissions or ownership differ.
    Attributes,
    Unchanged,
}

#[derive(Debug, Clone)]
pub struct RestoreItem {
    pub path: PathBuf,
    pub meta: EntryMeta,
    pub state: RestoreState,
}

#[derive(Debug, Default)]
pub struct RestoreReport {
    pub restored: usize,
    pub backups: Vec<PathBuf>,
    pub warnings: Vec<String>,
    pub failures: Vec<(PathBuf, String)>,
}

/// Decide what restoring each selected entry would change.
pub fn prepare(
    metadata: &Metadata,
    store: &Store,
    selected: &[PathBuf],
) -> Vec<(RestoreItem, Option<String>)> {
    let mut items = Vec::new();
    for path in selected {
        let Some(meta) = metadata.get(path) else {
            continue;
        };
        let (state, problem) = match state_of(path, meta, store) {
            Ok(state) => (state, None),
            Err(err) => (RestoreState::Overwrite, Some(err.to_string())),
        };
        items.push((
            RestoreItem {
                path: path.clone(),
                meta: meta.clone(),
                state,
            },
            problem,
        ));
    }
    items
}

fn state_of(path: &Path, meta: &EntryMeta, store: &Store) -> Result<RestoreState> {
    let current = match SysEntry::read(path) {
        Ok(Some(entry)) => entry,
        Ok(None) => return Ok(RestoreState::Overwrite),
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(RestoreState::Create),
        Err(err) => return Err(ConfectError::io_at(path, err)),
    };
    let wanted_uid = resolve_uid(&meta.owner, meta.uid);
    let wanted_gid = resolve_gid(&meta.group, meta.gid);
    let attributes = current.kind != meta.kind
        || current.uid != wanted_uid
        || current.gid != wanted_gid
        || (meta.kind != Kind::Symlink && meta.mode_bits() != Some(current.mode));
    let content = match (meta.kind, current.kind) {
        (Kind::Dir, Kind::Dir) => false,
        (Kind::Symlink, Kind::Symlink) => Some(store.read_link(path, meta)?) != current.target,
        (Kind::File, Kind::File) if meta.encrypted => {
            store.read(path, meta)? != fs::read(path).at(path)?
        }
        (Kind::File, Kind::File) => !files_equal(path, &store.absolute(&meta.repo_path(path)))?,
        _ => true,
    };
    Ok(if content {
        RestoreState::Overwrite
    } else if attributes {
        RestoreState::Attributes
    } else {
        RestoreState::Unchanged
    })
}

/// Write the selected entries back to the system.
pub fn restore(items: &[RestoreItem], store: &Store, backup: bool) -> RestoreReport {
    let mut report = RestoreReport::default();
    let stamp = chrono::Local::now().format("%Y%m%dT%H%M%S").to_string();

    let (dirs, others): (Vec<_>, Vec<_>) = items
        .iter()
        .filter(|item| item.state != RestoreState::Unchanged)
        .partition(|item| item.meta.kind == Kind::Dir);

    for item in &dirs {
        if let Err(err) = ensure_dir(&item.path) {
            report.failures.push((item.path.clone(), err.to_string()));
        }
    }

    for item in &others {
        let result = (|| -> Result<()> {
            if backup && item.state == RestoreState::Overwrite {
                if let Some(saved) = make_backup(&item.path, &stamp)? {
                    report.backups.push(saved);
                }
            }
            match item.meta.kind {
                Kind::Symlink => restore_symlink(item, store, &mut report.warnings),
                Kind::File => restore_file(item, store, &mut report.warnings),
                Kind::Dir => Ok(()),
            }
        })();
        match result {
            Ok(()) => report.restored += 1,
            Err(err) => report.failures.push((item.path.clone(), err.to_string())),
        }
    }

    // Directory permissions last and deepest first: a restrictive mode must not block
    // writing the files inside it.
    let mut dirs = dirs;
    dirs.sort_by_key(|item| std::cmp::Reverse(item.path.components().count()));
    for item in dirs {
        match set_attributes_nofollow(&item.path, &item.meta, &mut report.warnings) {
            Ok(()) => report.restored += 1,
            Err(err) => report.failures.push((item.path.clone(), err.to_string())),
        }
    }
    report
}

fn ensure_dir(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => Ok(()),
        Ok(_) => Err(ConfectError::InvalidPath {
            path: path.to_path_buf(),
            reason: "expected a directory, found something else; not replacing it".into(),
        }),
        Err(err) if err.kind() == ErrorKind::NotFound => fs::create_dir_all(path).at(path),
        Err(err) => Err(ConfectError::io_at(path, err)),
    }
}

fn parent_of(path: &Path) -> Result<&Path> {
    let parent = path.parent().ok_or_else(|| ConfectError::InvalidPath {
        path: path.to_path_buf(),
        reason: "no parent directory".into(),
    })?;
    fs::create_dir_all(parent).at(parent)?;
    Ok(parent)
}

fn refuse_directory(path: &Path) -> Result<()> {
    if let Ok(meta) = fs::symlink_metadata(path) {
        if meta.is_dir() {
            return Err(ConfectError::InvalidPath {
                path: path.to_path_buf(),
                reason: "a directory is in the way; not replacing it".into(),
            });
        }
    }
    Ok(())
}

fn restore_file(item: &RestoreItem, store: &Store, warnings: &mut Vec<String>) -> Result<()> {
    refuse_directory(&item.path)?;
    let parent = parent_of(&item.path)?;
    let mut temp = tempfile::Builder::new()
        .prefix(".confect-restore-")
        .tempfile_in(parent)
        .at(parent)?;

    if item.meta.encrypted {
        let content = store.read(&item.path, &item.meta)?;
        temp.write_all(&content).at(&item.path)?;
    } else {
        let source = store.absolute(&item.meta.repo_path(&item.path));
        let mut reader = File::open(&source).at(&source)?;
        io::copy(&mut reader, temp.as_file_mut()).at(&item.path)?;
    }

    if let Some(mode) = item.meta.mode_bits() {
        temp.as_file()
            .set_permissions(fs::Permissions::from_mode(mode))
            .at(&item.path)?;
    }
    chown_fd(temp.as_file(), &item.path, &item.meta, warnings)?;
    // chown may clear setuid/setgid bits; set the mode once more afterwards.
    if let Some(mode) = item.meta.mode_bits() {
        temp.as_file()
            .set_permissions(fs::Permissions::from_mode(mode))
            .at(&item.path)?;
    }
    temp.as_file().sync_all().at(&item.path)?;
    temp.persist(&item.path)
        .map_err(|e| ConfectError::io_at(&item.path, e.error))?;
    Ok(())
}

fn restore_symlink(item: &RestoreItem, store: &Store, warnings: &mut Vec<String>) -> Result<()> {
    refuse_directory(&item.path)?;
    let parent = parent_of(&item.path)?;
    let target = store.read_link(&item.path, &item.meta)?;
    let temp = unique_link(parent, &target)?;
    let result = chown_nofollow(&temp, &item.meta, warnings)
        .and_then(|_| fs::rename(&temp, &item.path).at(&item.path));
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn unique_link(parent: &Path, target: &Path) -> Result<PathBuf> {
    for attempt in 0..100u32 {
        let candidate = parent.join(format!(
            ".confect-restore-{}-{}",
            std::process::id(),
            attempt
        ));
        match std::os::unix::fs::symlink(target, &candidate) {
            Ok(()) => return Ok(candidate),
            Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(ConfectError::io_at(candidate, err)),
        }
    }
    Err(ConfectError::Other(format!(
        "cannot create a temporary symlink in {}",
        parent.display()
    )))
}

fn wanted_owner(meta: &EntryMeta) -> (u32, u32) {
    (
        resolve_uid(&meta.owner, meta.uid),
        resolve_gid(&meta.group, meta.gid),
    )
}

fn chown_fd(file: &File, path: &Path, meta: &EntryMeta, warnings: &mut Vec<String>) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    let (uid, gid) = wanted_owner(meta);
    let current = file.metadata().at(path)?;
    if current.uid() == uid && current.gid() == gid {
        return Ok(());
    }
    match nix::unistd::fchown(file, Some(Uid::from_raw(uid)), Some(Gid::from_raw(gid))) {
        Ok(()) => Ok(()),
        Err(nix::errno::Errno::EPERM) => {
            warnings.push(format!(
                "{}: cannot set owner {}:{} (not root)",
                path.display(),
                meta.owner,
                meta.group
            ));
            Ok(())
        }
        Err(errno) => Err(ConfectError::Unix(errno)),
    }
}

fn chown_nofollow(path: &Path, meta: &EntryMeta, warnings: &mut Vec<String>) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    let (uid, gid) = wanted_owner(meta);
    let current = fs::symlink_metadata(path).at(path)?;
    if current.uid() == uid && current.gid() == gid {
        return Ok(());
    }
    match fchownat(
        nix::fcntl::AT_FDCWD,
        path,
        Some(Uid::from_raw(uid)),
        Some(Gid::from_raw(gid)),
        AtFlags::AT_SYMLINK_NOFOLLOW,
    ) {
        Ok(()) => Ok(()),
        Err(nix::errno::Errno::EPERM) => {
            warnings.push(format!(
                "{}: cannot set owner {}:{} (not root)",
                path.display(),
                meta.owner,
                meta.group
            ));
            Ok(())
        }
        Err(errno) => Err(ConfectError::Unix(errno)),
    }
}

/// Apply mode and owner to a directory, refusing to follow a symlink planted there.
fn set_attributes_nofollow(
    path: &Path,
    meta: &EntryMeta,
    warnings: &mut Vec<String>,
) -> Result<()> {
    let current = fs::symlink_metadata(path).at(path)?;
    if !current.is_dir() {
        return Err(ConfectError::InvalidPath {
            path: path.to_path_buf(),
            reason: "expected a directory".into(),
        });
    }
    chown_nofollow(path, meta, warnings)?;
    if let Some(mode) = meta.mode_bits() {
        let dir = File::open(path).at(path)?;
        dir.set_permissions(fs::Permissions::from_mode(mode))
            .at(path)?;
    }
    Ok(())
}

/// Save the current file next to it without ever writing through a symlink.
fn make_backup(path: &Path, stamp: &str) -> Result<Option<PathBuf>> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(ConfectError::io_at(path, err)),
    };
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".confect-backup.{}", stamp));
    let backup = path.with_file_name(name);

    if meta.file_type().is_symlink() {
        let target = fs::read_link(path).at(path)?;
        std::os::unix::fs::symlink(target, &backup).at(&backup)?;
        return Ok(Some(backup));
    }
    if !meta.is_file() {
        return Ok(None);
    }
    let mut source = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(path)
        .at(path)?;
    let mut destination = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(meta.permissions().mode() & 0o7777)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(&backup)
        .at(&backup)?;
    io::copy(&mut source, &mut destination).at(&backup)?;
    Ok(Some(backup))
}

#[cfg(test)]
mod tests {
    use super::make_backup;
    use std::fs;

    #[test]
    fn backup_never_writes_through_a_planted_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("app.conf");
        let victim = dir.path().join("victim");
        fs::write(&file, "current").unwrap();
        fs::write(&victim, "precious").unwrap();
        std::os::unix::fs::symlink(&victim, dir.path().join("app.conf.confect-backup.STAMP"))
            .unwrap();

        assert!(make_backup(&file, "STAMP").is_err());
        assert_eq!(fs::read_to_string(&victim).unwrap(), "precious");

        let saved = make_backup(&file, "OTHER").unwrap().unwrap();
        assert_eq!(fs::read_to_string(saved).unwrap(), "current");
    }
}
