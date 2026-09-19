use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::core::category::{Category, CategoryManager};
use crate::crypto::secrets;
use crate::error::{ConfectError, Result};
use crate::track::entry::{Kind, SysEntry};
use crate::track::metadata::{EntryMeta, Metadata};
use crate::track::store::Store;
use crate::track::walk::scan_category;

/// Files larger than this are compared and copied by streaming, never encrypted in memory.
const LARGE_FILE: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// New on the system.
    Add,
    /// Content and/or attributes changed on the system.
    Update { content: bool, attributes: bool },
    /// Gone from the system.
    Delete,
    /// Still on the system but now excluded or no longer covered: drop the stored copy.
    Purge,
}

impl Action {
    pub fn letter(self) -> char {
        match self {
            Action::Add => 'A',
            Action::Update { content: true, .. } => 'M',
            Action::Update { .. } => 'P',
            Action::Delete => 'D',
            Action::Purge => 'X',
        }
    }
}

#[derive(Debug, Clone)]
pub struct Change {
    pub category: String,
    pub path: PathBuf,
    pub action: Action,
    /// The system entry for Add/Update.
    pub system: Option<SysEntry>,
    /// What the repository had before, for Update/Delete/Purge.
    pub stored: Option<EntryMeta>,
    /// Whether the new copy is stored encrypted.
    pub encrypt: bool,
}

#[derive(Debug, Clone)]
pub struct SecretFinding {
    pub path: PathBuf,
    pub category: String,
    pub what: &'static str,
}

#[derive(Debug, Default)]
pub struct Plan {
    pub changes: Vec<Change>,
    pub warnings: Vec<String>,
    pub secrets: Vec<SecretFinding>,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }
}

/// Which part of the repository a command looks at.
#[derive(Debug, Default, Clone)]
pub struct Scope {
    pub category: Option<String>,
    /// Limit to these system paths and everything below them.
    pub paths: Vec<PathBuf>,
}

impl Scope {
    fn includes_category(&self, name: &str) -> bool {
        self.category.as_deref().is_none_or(|c| c == name)
    }

    fn includes_path(&self, path: &Path) -> bool {
        self.paths.is_empty() || self.paths.iter().any(|p| path.starts_with(p))
    }
}

/// Compare the system with the repository.
pub fn build(
    repo_path: &Path,
    categories: &CategoryManager,
    metadata: &Metadata,
    store: &Store,
    scope: &Scope,
) -> Result<Plan> {
    if let Some(name) = &scope.category {
        categories.get(name)?;
    }
    let mut plan = Plan::default();

    for category in categories.list() {
        if !scope.includes_category(&category.name) {
            continue;
        }
        let scan = scan_category(category, repo_path);
        plan.warnings.extend(
            scan.warnings
                .iter()
                .filter(|w| relevant_warning(w, scope))
                .cloned(),
        );

        let mut stored: BTreeMap<PathBuf, EntryMeta> = metadata
            .in_category(&category.name)
            .map(|(path, meta)| (path.clone(), meta.clone()))
            .collect();
        for (path, kind) in store.list_category(&category.name) {
            if stored.contains_key(&path) {
                continue;
            }
            let encrypted_copy = path
                .to_str()
                .and_then(|p| p.strip_suffix(".age"))
                .and_then(|plain| metadata.get(Path::new(plain)))
                .is_some_and(|meta| meta.encrypted && meta.category == category.name);
            if !encrypted_copy {
                stored.insert(path, unindexed(&category.name, kind));
            }
        }

        for (path, entry) in &scan.entries {
            // A path two legacy categories cover belongs to the more specific one; the
            // other category's copy is purged below.
            let owned = categories
                .find_for_path(path)
                .is_some_and(|owner| owner.name == category.name);
            if !scope.includes_path(path) || !owned {
                continue;
            }
            let previous = stored.remove(path);
            examine(category, path, entry, previous, store, &mut plan)?;
        }

        for (path, meta) in stored {
            if !scope.includes_path(&path) || scan.is_protected(&path) {
                continue;
            }
            let action = if path_exists(&path) {
                Action::Purge
            } else {
                Action::Delete
            };
            plan.changes.push(Change {
                category: category.name.clone(),
                path,
                action,
                system: None,
                stored: Some(meta),
                encrypt: false,
            });
        }
    }

    plan.changes.sort_by(|a, b| {
        let removing = |c: &Change| !matches!(c.action, Action::Delete | Action::Purge);
        a.path.cmp(&b.path).then(removing(a).cmp(&removing(b)))
    });
    Ok(plan)
}

fn relevant_warning(warning: &str, scope: &Scope) -> bool {
    scope.paths.is_empty()
        || scope
            .paths
            .iter()
            .any(|p| warning.contains(&*p.to_string_lossy()))
}

/// A copy found in the repository without an index entry (1.x leftovers).
fn unindexed(category: &str, kind: Kind) -> EntryMeta {
    EntryMeta {
        category: category.to_string(),
        kind,
        mode: None,
        owner: String::new(),
        group: String::new(),
        uid: u32::MAX,
        gid: u32::MAX,
        target: None,
        encrypted: false,
        size: None,
        mtime_ns: None,
    }
}

fn path_exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn examine(
    category: &Category,
    path: &Path,
    entry: &SysEntry,
    previous: Option<EntryMeta>,
    store: &Store,
    plan: &mut Plan,
) -> Result<()> {
    let encrypt = entry.kind == Kind::File && category.should_encrypt(path);

    let (content_changed, attributes_changed) = match &previous {
        None => (true, true),
        Some(meta) => {
            let content = match content_differs(entry, meta, encrypt, store) {
                Ok(changed) => changed,
                Err(Unreadable(reason)) => {
                    plan.warnings.push(format!(
                        "cannot read {}: {}; its stored copy is kept",
                        path.display(),
                        reason
                    ));
                    false
                }
            };
            (content, meta.attributes_differ(entry))
        }
    };

    if !content_changed && !attributes_changed {
        return Ok(());
    }

    if content_changed && entry.kind == Kind::File && !encrypt && !category.allows_plaintext(path) {
        match read_head(path) {
            Ok(head) => {
                if let Some(what) = secrets::detect(&head) {
                    plan.secrets.push(SecretFinding {
                        path: path.to_path_buf(),
                        category: category.name.clone(),
                        what,
                    });
                }
            }
            Err(err) => {
                let outcome = if previous.is_some() {
                    "its stored copy is kept"
                } else {
                    "it is not stored"
                };
                plan.warnings.push(format!(
                    "cannot read {}: {}; {}",
                    path.display(),
                    err,
                    outcome
                ));
                return Ok(());
            }
        }
    }

    let action = match previous {
        None => Action::Add,
        Some(_) => Action::Update {
            content: content_changed,
            attributes: attributes_changed,
        },
    };
    plan.changes.push(Change {
        category: category.name.clone(),
        path: path.to_path_buf(),
        action,
        system: Some(entry.clone()),
        stored: previous,
        encrypt,
    });
    Ok(())
}

struct Unreadable(String);

fn content_differs(
    entry: &SysEntry,
    meta: &EntryMeta,
    encrypt: bool,
    store: &Store,
) -> std::result::Result<bool, Unreadable> {
    if meta.kind != entry.kind || meta.encrypted != encrypt {
        return Ok(true);
    }
    match entry.kind {
        Kind::Dir => Ok(false),
        Kind::Symlink => {
            let stored = store.read_link(&entry.path, meta).ok();
            Ok(stored.as_deref() != entry.target.as_deref())
        }
        Kind::File if meta.encrypted => {
            if store.crypto().has_identity() {
                let stored = match store.read(&entry.path, meta) {
                    Ok(stored) => stored,
                    Err(_) => return Ok(true),
                };
                let current = fs::read(&entry.path).map_err(|e| Unreadable(e.to_string()))?;
                Ok(stored != current)
            } else {
                Ok(meta.size != Some(entry.size) || meta.mtime_ns != Some(entry.mtime_ns))
            }
        }
        Kind::File => match store.same_plain_content(&entry.path, meta) {
            Ok(same) => Ok(!same),
            Err(ConfectError::IoAt { path, source }) if path == entry.path => {
                Err(Unreadable(source.to_string()))
            }
            Err(_) => Ok(true),
        },
    }
}

fn read_head(path: &Path) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let mut file = fs::File::open(path)?;
    let mut buf = Vec::new();
    file.by_ref()
        .take(secrets::SCAN_LIMIT as u64)
        .read_to_end(&mut buf)?;
    Ok(buf)
}

/// Write the plan into the repository working tree and the metadata index.
pub fn apply(plan: &Plan, store: &Store, metadata: &mut Metadata) -> Result<Vec<String>> {
    let mut failures = Vec::new();
    for change in &plan.changes {
        if let Err(err) = apply_change(change, store, metadata) {
            failures.push(format!("{}: {}", change.path.display(), err));
        }
    }
    Ok(failures)
}

fn apply_change(change: &Change, store: &Store, metadata: &mut Metadata) -> Result<()> {
    match change.action {
        Action::Delete | Action::Purge => {
            store.remove(&change.category, &change.path)?;
            if metadata
                .get(&change.path)
                .is_some_and(|meta| meta.category == change.category)
            {
                metadata.remove(&change.path);
            }
        }
        Action::Add | Action::Update { .. } => {
            let entry = change
                .system
                .as_ref()
                .expect("add/update carry the system entry");
            let content = matches!(
                change.action,
                Action::Add | Action::Update { content: true, .. }
            );
            if content {
                write_content(change, entry, store)?;
            }
            let mut meta = EntryMeta::from_system(&change.category, entry, change.encrypt);
            if !content {
                if let Some(previous) = &change.stored {
                    meta.size = previous.size;
                    meta.mtime_ns = previous.mtime_ns;
                }
            }
            metadata.insert(change.path.clone(), meta);
        }
    }
    Ok(())
}

fn write_content(change: &Change, entry: &SysEntry, store: &Store) -> Result<()> {
    let executable = entry.mode & 0o100 != 0;
    match entry.kind {
        Kind::Dir => Ok(()),
        Kind::Symlink => {
            let target = entry
                .target
                .as_deref()
                .expect("symlinks carry their target");
            store.write_symlink(&change.category, &change.path, target)
        }
        Kind::File if !change.encrypt && entry.size > LARGE_FILE => {
            store.copy_file(&change.category, &change.path, executable)
        }
        Kind::File => {
            let content = fs::read(&change.path).map_err(|e| {
                if e.kind() == ErrorKind::NotFound {
                    ConfectError::FileNotFound(change.path.clone())
                } else {
                    ConfectError::io_at(&change.path, e)
                }
            })?;
            store.write_file(
                &change.category,
                &change.path,
                &content,
                change.encrypt,
                executable,
            )
        }
    }
}
