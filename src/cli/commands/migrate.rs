use console::style;
use std::path::Path;

use crate::cli::commands::audit;
use crate::cli::ui;
use crate::core::config::{RepoConfig, RepoMeta, REPO_FORMAT_VERSION};
use crate::core::Repository;
use crate::crypto::Crypto;
use crate::error::{ConfectError, IoContext, Result};
use crate::track::entry::{group_name, mode_string, user_name};
use crate::track::{EntryMeta, Kind, Metadata, Store, SysEntry};

/// Convert a 1.x repository: sorted category file, a real metadata index captured from
/// the system, the repository format version and private permissions.
pub fn run(explicit_repo: Option<&Path>, yes: bool) -> Result<()> {
    let mut repo = Repository::open_any_version(explicit_repo)?;
    let version = repo.repo_config().repository.version;
    let metadata = Metadata::load(repo.path())?;
    if version == REPO_FORMAT_VERSION && !metadata.is_legacy() {
        ui::success("The repository is already in the current format.");
        return Ok(());
    }
    let _lock = repo.lock()?;

    println!(
        "Migrating {} (format {} → {}, branch {})",
        style(repo.path().display()).cyan(),
        version,
        REPO_FORMAT_VERSION,
        repo.branch()
    );
    if !ui::confirm(
        "Rewrite the repository metadata and commit the result?",
        yes,
    )? {
        return Err(ConfectError::Aborted);
    }

    let categories = repo.categories()?;
    for category in categories.list() {
        for pattern in &category.paths {
            if !pattern.starts_with('/') {
                ui::warn(&format!(
                    "category '{}' has a relative path {}; replace it with an absolute one",
                    category.name, pattern
                ));
            }
        }
    }
    categories.save()?;

    let crypto = Crypto::new(
        repo.config().identity_file()?,
        repo.config().recipients_file()?,
    );
    let store = Store::new(repo.path(), &crypto);
    let mut rebuilt = Metadata::load(repo.path())?;
    let mut indexed = 0;
    for category in categories.list() {
        for (path, kind) in store.list_category(&category.name) {
            let meta = match SysEntry::read(&path) {
                Ok(Some(entry)) if entry.kind == kind => {
                    EntryMeta::from_system(&category.name, &entry, false)
                }
                _ => from_repo_copy(&store, &category.name, &path, kind)?,
            };
            rebuilt.insert(path, meta);
            indexed += 1;
        }
    }
    rebuilt.save()?;

    let created = repo.repo_config().repository.created.clone();
    let host = repo.host().to_string();
    repo.set_repo_config(RepoConfig {
        repository: RepoMeta {
            version: REPO_FORMAT_VERSION,
            created,
            host: Some(host.clone()),
        },
    })?;
    if repo.ensure_private()? {
        ui::info("Restricted the repository directory to its owner (0700)");
    }

    let git = repo.git();
    git.add_all()?;
    if git.has_staged_changes()? {
        git.commit("Migrate repository to confect 2 format", &host)?;
    }
    ui::success(&format!(
        "Indexed {} stored path(s) and committed the migration",
        indexed
    ));

    let findings = audit::scan_tree(repo.path());
    if !findings.is_empty() {
        println!();
        ui::warn(&format!(
            "{} stored file(s) are plaintext secrets. Encrypt or exclude them, rotate them, \
             and check the history with 'confect audit --history':",
            findings.len()
        ));
        for finding in findings {
            println!("  ! {} ({})", finding.path, finding.what);
        }
    }
    println!();
    println!(
        "Run {} next: it records directory permissions and anything that changed.",
        style("confect sync").cyan()
    );
    Ok(())
}

/// Index a stored copy whose system file is gone, using the copy's own attributes.
fn from_repo_copy(store: &Store, category: &str, path: &Path, kind: Kind) -> Result<EntryMeta> {
    use std::os::unix::fs::MetadataExt;
    let relative = crate::core::paths::repo_relative(category, path, false);
    let copy = store.absolute(&relative);
    let meta = std::fs::symlink_metadata(&copy).at(&copy)?;
    Ok(EntryMeta {
        category: category.to_string(),
        kind,
        mode: (kind != Kind::Symlink).then(|| mode_string(meta.mode())),
        owner: user_name(meta.uid()),
        group: group_name(meta.gid()),
        uid: meta.uid(),
        gid: meta.gid(),
        target: if kind == Kind::Symlink {
            std::fs::read_link(&copy)
                .ok()
                .map(|t| t.to_string_lossy().into_owned())
        } else {
            None
        },
        encrypted: false,
        size: None,
        mtime_ns: None,
    })
}
