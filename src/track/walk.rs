use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use glob::MatchOptions;
use walkdir::WalkDir;

use crate::core::category::Category;
use crate::core::paths::{has_glob, is_backup_name};
use crate::track::entry::{Kind, SysEntry};

/// What a category currently looks like on the system.
#[derive(Debug, Default)]
pub struct Scan {
    pub entries: BTreeMap<PathBuf, SysEntry>,
    /// Paths whose stored copies must be kept although nothing was found there: a
    /// tracked root that disappeared or a directory that could not be read.
    pub protected: Vec<PathBuf>,
    pub warnings: Vec<String>,
}

impl Scan {
    pub fn is_protected(&self, path: &Path) -> bool {
        self.protected.iter().any(|p| path.starts_with(p))
    }

    fn protect(&mut self, path: &Path, warning: String) {
        self.protected.push(path.to_path_buf());
        self.warnings.push(warning);
    }
}

pub fn scan_category(category: &Category, repo_path: &Path) -> Scan {
    let mut scan = Scan::default();
    for pattern in &category.paths {
        if has_glob(pattern) {
            let options = MatchOptions {
                case_sensitive: true,
                require_literal_separator: true,
                require_literal_leading_dot: false,
            };
            match glob::glob_with(pattern, options) {
                Ok(paths) => {
                    for item in paths {
                        match item {
                            Ok(path) => visit(category, repo_path, &path, &mut scan),
                            Err(err) => {
                                let path = err.path().to_path_buf();
                                scan.protect(
                                    &path,
                                    format!("cannot read {}: {}", path.display(), err.error()),
                                );
                            }
                        }
                    }
                }
                Err(err) => scan.warnings.push(format!(
                    "invalid pattern {} in category '{}': {}",
                    pattern, category.name, err
                )),
            }
        } else {
            visit(category, repo_path, Path::new(pattern), &mut scan);
        }
    }
    scan
}

fn visit(category: &Category, repo_path: &Path, root: &Path, scan: &mut Scan) {
    if category.is_excluded(root) || root.starts_with(repo_path) {
        return;
    }
    let entry = match SysEntry::read(root) {
        Ok(Some(entry)) => entry,
        Ok(None) => return,
        Err(err) if err.kind() == ErrorKind::NotFound => {
            scan.protect(
                root,
                format!(
                    "{} is missing; its stored copies are kept (untrack it with 'confect remove')",
                    root.display()
                ),
            );
            return;
        }
        Err(err) => {
            scan.protect(root, format!("cannot read {}: {}", root.display(), err));
            return;
        }
    };
    if entry.kind != Kind::Dir {
        scan.entries.insert(root.to_path_buf(), entry);
        return;
    }

    let mut walker = WalkDir::new(root).follow_links(false).into_iter();
    while let Some(item) = walker.next() {
        let item = match item {
            Ok(item) => item,
            Err(err) => {
                let path = err.path().unwrap_or(root).to_path_buf();
                let reason = err
                    .io_error()
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| err.to_string());
                scan.protect(&path, format!("cannot read {}: {}", path.display(), reason));
                continue;
            }
        };
        let path = item.path();
        let is_dir = item.file_type().is_dir();

        let name = item.file_name();
        let Some(name) = name.to_str() else {
            scan.warnings.push(format!(
                "skipping {}: the name is not valid UTF-8",
                path.display()
            ));
            if is_dir {
                walker.skip_current_dir();
            }
            continue;
        };
        let skip = category.is_excluded(path)
            || path.starts_with(repo_path)
            || (is_dir && name == ".git" && item.depth() > 0)
            || (!is_dir && is_backup_name(name));
        if skip {
            if is_dir {
                walker.skip_current_dir();
            }
            continue;
        }

        match SysEntry::read(path) {
            Ok(Some(entry)) => {
                scan.entries.insert(path.to_path_buf(), entry);
            }
            Ok(None) => {}
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => {
                scan.protect(path, format!("cannot read {}: {}", path.display(), err));
                if is_dir {
                    walker.skip_current_dir();
                }
            }
        }
    }
}
