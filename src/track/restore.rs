use std::fs::{self, File};
use std::io::{self, ErrorKind, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use nix::errno::Errno;
use nix::unistd::{fchown, Gid, Uid};

use crate::error::{ConfectError, IoContext, Result};
use crate::track::entry::{resolve_gid, resolve_uid, Kind, SysEntry};
use crate::track::metadata::{EntryMeta, Metadata};
use crate::track::safe_fs::{split, Dir};
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
    let owner_differs = resolve_uid(&meta.owner).is_some_and(|uid| uid != current.uid)
        || resolve_gid(&meta.group).is_some_and(|gid| gid != current.gid);
    let attributes = current.kind != meta.kind
        || owner_differs
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
///
/// Every write goes through [`Dir`], so a symlink planted anywhere on the way by another
/// user cannot redirect it.
pub fn restore(items: &[RestoreItem], store: &Store, backup: bool) -> RestoreReport {
    let mut report = RestoreReport::default();
    let stamp = chrono::Local::now().format("%Y%m%dT%H%M%S").to_string();

    let (dirs, others): (Vec<_>, Vec<_>) = items
        .iter()
        .filter(|item| item.state != RestoreState::Unchanged)
        .partition(|item| item.meta.kind == Kind::Dir);

    for item in &dirs {
        if let Err(err) = Dir::open(&item.path, true) {
            report.failures.push((item.path.clone(), err.to_string()));
        }
    }

    for item in &others {
        let backup_stamp =
            (backup && item.state == RestoreState::Overwrite).then_some(stamp.as_str());
        let result = match item.meta.kind {
            Kind::Symlink => restore_symlink(item, store, backup_stamp, &mut report),
            Kind::File => restore_file(item, store, backup_stamp, &mut report),
            Kind::Dir => Ok(()),
        };
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
        match set_dir_attributes(item, &mut report.warnings) {
            Ok(()) => report.restored += 1,
            Err(err) => report.failures.push((item.path.clone(), err.to_string())),
        }
    }
    report
}

/// Owner to set, or `None` with a warning when this host cannot map it.
fn wanted_owner(item: &RestoreItem, warnings: &mut Vec<String>) -> Option<(u32, u32)> {
    match (resolve_uid(&item.meta.owner), resolve_gid(&item.meta.group)) {
        (Some(uid), Some(gid)) => Some((uid, gid)),
        _ => {
            warnings.push(format!(
                "{}: owner {}:{} does not exist on this host; left as created",
                item.path.display(),
                item.meta.owner,
                item.meta.group
            ));
            None
        }
    }
}

fn not_root_warning(item: &RestoreItem) -> String {
    format!(
        "{}: cannot set owner {}:{} (not root)",
        item.path.display(),
        item.meta.owner,
        item.meta.group
    )
}

fn restore_file(
    item: &RestoreItem,
    store: &Store,
    backup: Option<&str>,
    report: &mut RestoreReport,
) -> Result<()> {
    let (parent, name) = split(&item.path)?;
    let dir = Dir::open(parent, true)?;
    if dir.is_directory(name)? {
        return Err(ConfectError::InvalidPath {
            path: item.path.clone(),
            reason: "a directory is in the way; not replacing it".into(),
        });
    }

    let content = if item.meta.encrypted {
        Some(store.read(&item.path, &item.meta)?)
    } else {
        None
    };
    let (temp, mut file) = dir.create_temp()?;
    let written = (|| -> Result<()> {
        match &content {
            Some(content) => file.write_all(content).at(&item.path)?,
            None => {
                let source = store.absolute(&item.meta.repo_path(&item.path));
                let mut reader = File::open(&source).at(&source)?;
                io::copy(&mut reader, &mut file).at(&item.path)?;
            }
        }
        if let Some((uid, gid)) = wanted_owner(item, &mut report.warnings) {
            use std::os::unix::fs::MetadataExt;
            let current = file.metadata().at(&item.path)?;
            if current.uid() != uid || current.gid() != gid {
                match fchown(&file, Some(Uid::from_raw(uid)), Some(Gid::from_raw(gid))) {
                    Ok(()) => {}
                    Err(Errno::EPERM) => report.warnings.push(not_root_warning(item)),
                    Err(errno) => return Err(ConfectError::Unix(errno)),
                }
            }
        }
        // After chown, which may clear setuid/setgid bits.
        if let Some(mode) = item.meta.mode_bits() {
            file.set_permissions(fs::Permissions::from_mode(mode))
                .at(&item.path)?;
        }
        file.sync_all().at(&item.path)?;
        if let Some(stamp) = backup {
            if let Some(saved) = dir.backup(name, stamp)? {
                report.backups.push(saved);
            }
        }
        Ok(())
    })();
    if let Err(err) = written {
        dir.remove_temp(&temp);
        return Err(err);
    }
    dir.replace(&temp, name)
}

fn restore_symlink(
    item: &RestoreItem,
    store: &Store,
    backup: Option<&str>,
    report: &mut RestoreReport,
) -> Result<()> {
    let (parent, name) = split(&item.path)?;
    let dir = Dir::open(parent, true)?;
    if dir.is_directory(name)? {
        return Err(ConfectError::InvalidPath {
            path: item.path.clone(),
            reason: "a directory is in the way; not replacing it".into(),
        });
    }
    let target = store.read_link(&item.path, &item.meta)?;
    let temp = dir.create_temp_link(&target)?;
    let prepared = (|| -> Result<()> {
        if let Some((uid, gid)) = wanted_owner(item, &mut report.warnings) {
            match dir.chown_link(&temp, uid, gid) {
                Ok(()) => {}
                Err(Errno::EPERM) => {
                    let (current_uid, current_gid) = (
                        nix::unistd::geteuid().as_raw(),
                        nix::unistd::getegid().as_raw(),
                    );
                    if current_uid != uid || current_gid != gid {
                        report.warnings.push(not_root_warning(item));
                    }
                }
                Err(errno) => return Err(ConfectError::Unix(errno)),
            }
        }
        if let Some(stamp) = backup {
            if let Some(saved) = dir.backup(name, stamp)? {
                report.backups.push(saved);
            }
        }
        Ok(())
    })();
    if let Err(err) = prepared {
        dir.remove_temp(&temp);
        return Err(err);
    }
    dir.replace(&temp, name)
}

fn set_dir_attributes(item: &RestoreItem, warnings: &mut Vec<String>) -> Result<()> {
    let dir = Dir::open(&item.path, false)?;
    let owner = wanted_owner(item, warnings);
    let (current_uid, current_gid) = dir.owner()?;
    let chown = owner.filter(|&(uid, gid)| uid != current_uid || gid != current_gid);
    match dir.set_attributes(chown.map(|o| o.0), chown.map(|o| o.1), None) {
        Ok(()) => {}
        Err(Errno::EPERM) => warnings.push(not_root_warning(item)),
        Err(errno) => return Err(ConfectError::Unix(errno)),
    }
    dir.set_attributes(None, None, item.meta.mode_bits())
        .map_err(ConfectError::Unix)
}
