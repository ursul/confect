use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

use crate::cli::commands::FileStatus;
use crate::core::{CategoryManager, Repository};
use crate::error::{ConfectError, Result};

/// Tracks files between the system and the repository
pub struct FileTracker<'a> {
    repo: &'a Repository,
}

/// Files changed while refreshing the repository from the system.
#[derive(Debug, Default)]
pub struct RefreshResult {
    pub updated: Vec<PathBuf>,
    pub deleted: Vec<PathBuf>,
}

impl RefreshResult {
    pub fn is_empty(&self) -> bool {
        self.updated.is_empty() && self.deleted.is_empty()
    }

    pub fn len(&self) -> usize {
        self.updated.len() + self.deleted.len()
    }

    pub fn paths(&self) -> impl Iterator<Item = &PathBuf> {
        self.updated.iter().chain(self.deleted.iter())
    }
}

impl<'a> FileTracker<'a> {
    pub fn new(repo: &'a Repository) -> Self {
        Self { repo }
    }

    /// Add a file or directory to the repository
    pub fn add(&self, path: &Path, category: &str, _encrypt: bool) -> Result<Vec<PathBuf>> {
        let repo_base = self.repo.path();
        let category_dir = repo_base.join(category);

        let mut added_files = Vec::new();

        if path.is_dir() {
            // Walk directory
            for entry in WalkDir::new(path).follow_links(false) {
                let entry = entry?;
                let file_path = entry.path();

                // Include regular files and all symlinks
                if entry.file_type().is_file() || entry.file_type().is_symlink() {
                    self.copy_to_repo(file_path, &category_dir)?;
                    added_files.push(file_path.to_path_buf());
                }
            }
        } else if path.is_file() || path.is_symlink() {
            self.copy_to_repo(path, &category_dir)?;
            added_files.push(path.to_path_buf());
        }

        Ok(added_files)
    }

    /// Copy a file to the repository
    fn copy_to_repo(&self, system_path: &Path, category_dir: &Path) -> Result<()> {
        let path_str = system_path.to_string_lossy();
        let relative = path_str.trim_start_matches('/');
        let repo_path = category_dir.join(relative);

        // Create parent directories
        if let Some(parent) = repo_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Remove existing file/symlink if exists
        if repo_path.exists() || repo_path.is_symlink() {
            let _ = fs::remove_file(&repo_path);
        }

        // Handle symlinks - preserve them as symlinks
        let meta = fs::symlink_metadata(system_path)?;
        if meta.file_type().is_symlink() {
            let target = fs::read_link(system_path)?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(&target, &repo_path)?;
        } else if meta.is_file() {
            fs::copy(system_path, &repo_path)?;
        }
        // Skip non-regular files (sockets, devices, etc.)

        Ok(())
    }

    /// Remove a file from tracking
    pub fn remove(&self, path: &Path, delete_from_repo: bool) -> Result<Vec<PathBuf>> {
        let categories = CategoryManager::load(self.repo)?;
        let mut removed = Vec::new();

        // Find the file in the repo
        if let Some(cat) = categories.find_for_path(path) {
            let repo_path = self.repo.path().join(cat.repo_path_for(path));

            if repo_path.exists() {
                if delete_from_repo {
                    fs::remove_file(&repo_path)?;
                }
                removed.push(path.to_path_buf());
            }
        }

        Ok(removed)
    }

    /// Get the status of tracked files
    pub fn status(&self, category: Option<&str>) -> Result<HashMap<PathBuf, FileStatus>> {
        let categories = CategoryManager::load(self.repo)?;
        let mut result = HashMap::new();

        let cats_to_check: Vec<_> = if let Some(name) = category {
            vec![categories.get(name)?]
        } else {
            categories.list()
        };

        for cat in cats_to_check {
            let category_dir = self.repo.path().join(&cat.name);
            if !category_dir.exists() {
                continue;
            }

            // Walk repository files
            for entry in WalkDir::new(&category_dir).follow_links(false) {
                let entry = entry?;
                if !is_trackable_entry(&entry) {
                    continue;
                }

                let repo_file = entry.path();

                // Get system path
                if let Some(system_path) = cat.system_path_for(
                    repo_file
                        .strip_prefix(self.repo.path())
                        .unwrap_or(repo_file),
                ) {
                    let status = self.compare_files(&system_path, repo_file)?;
                    if status != FileStatus::Modified
                        || !self.files_equal(&system_path, repo_file)?
                    {
                        result.insert(system_path, status);
                    }
                }
            }
        }

        Ok(result)
    }

    /// Compare a system file with its repository copy
    fn compare_files(&self, system_path: &Path, repo_path: &Path) -> Result<FileStatus> {
        let system_exists = system_path.exists();
        let repo_exists = repo_path.exists();

        match (system_exists, repo_exists) {
            (true, true) => Ok(FileStatus::Modified), // Filtered later if equal
            (true, false) => Ok(FileStatus::Added),
            (false, true) => Ok(FileStatus::Deleted),
            (false, false) => Ok(FileStatus::Missing),
        }
    }

    /// Check if two files or symlinks have the same content
    fn files_equal(&self, path1: &Path, path2: &Path) -> Result<bool> {
        let meta1 = fs::symlink_metadata(path1)?;
        let meta2 = fs::symlink_metadata(path2)?;

        if meta1.file_type().is_symlink() || meta2.file_type().is_symlink() {
            return Ok(meta1.file_type().is_symlink()
                && meta2.file_type().is_symlink()
                && fs::read_link(path1)? == fs::read_link(path2)?);
        }

        if !meta1.is_file() || !meta2.is_file() {
            return Ok(false);
        }

        let content1 = fs::read(path1)?;
        let content2 = fs::read(path2)?;
        Ok(content1 == content2)
    }

    /// Get diff between system file and repo file
    pub fn diff_file(&self, system_path: &Path) -> Result<String> {
        let categories = CategoryManager::load(self.repo)?;

        if let Some(cat) = categories.find_for_path(system_path) {
            let repo_path = self.repo.path().join(cat.repo_path_for(system_path));

            if !repo_path.exists() {
                return Ok(format!(
                    "File only exists in system: {}",
                    system_path.display()
                ));
            }
            if !system_path.exists() {
                return Ok(format!("File only exists in repo: {}", repo_path.display()));
            }

            // Simple line-by-line diff
            let system_content = fs::read_to_string(system_path).unwrap_or_default();
            let repo_content = fs::read_to_string(&repo_path).unwrap_or_default();

            if system_content == repo_content {
                return Ok(String::new());
            }

            let mut diff = String::new();
            diff.push_str(&format!("--- a/{}\n", system_path.display()));
            diff.push_str(&format!("+++ b/{}\n", repo_path.display()));

            // Very basic diff - show changed lines
            let system_lines: Vec<_> = system_content.lines().collect();
            let repo_lines: Vec<_> = repo_content.lines().collect();

            for (i, (s, r)) in system_lines.iter().zip(repo_lines.iter()).enumerate() {
                if s != r {
                    diff.push_str(&format!("@@ -{},{} +{},{} @@\n", i + 1, 1, i + 1, 1));
                    diff.push_str(&format!("-{}\n", r));
                    diff.push_str(&format!("+{}\n", s));
                }
            }

            // Handle length differences
            if system_lines.len() > repo_lines.len() {
                for line in &system_lines[repo_lines.len()..] {
                    diff.push_str(&format!("+{}\n", line));
                }
            } else if repo_lines.len() > system_lines.len() {
                for line in &repo_lines[system_lines.len()..] {
                    diff.push_str(&format!("-{}\n", line));
                }
            }

            Ok(diff)
        } else {
            Err(ConfectError::PathNotTracked(system_path.to_path_buf()))
        }
    }

    /// Refresh all tracked files from system to repository
    pub fn refresh_all(&self) -> Result<RefreshResult> {
        let categories = CategoryManager::load(self.repo)?;
        let mut result = RefreshResult::default();

        for cat in categories.list() {
            let category_dir = self.repo.path().join(&cat.name);
            let mut seen_system_paths = HashSet::new();

            // Expand paths in category. Exact directory paths are tracked recursively.
            for pattern in &cat.paths {
                self.refresh_pattern(
                    &cat,
                    pattern,
                    &category_dir,
                    &mut seen_system_paths,
                    &mut result.updated,
                )?;
            }

            if !category_dir.exists() {
                continue;
            }

            let mut repo_files = Vec::new();
            for entry in WalkDir::new(&category_dir).follow_links(false) {
                let entry = entry?;
                if is_trackable_entry(&entry) {
                    repo_files.push(entry.path().to_path_buf());
                }
            }

            for repo_file in repo_files {
                if let Some(system_path) = cat.system_path_for(
                    repo_file
                        .strip_prefix(self.repo.path())
                        .unwrap_or(&repo_file),
                ) {
                    if !seen_system_paths.contains(&system_path) && cat.matches(&system_path) {
                        fs::remove_file(&repo_file)?;
                        result.deleted.push(system_path);
                    }
                }
            }

            remove_empty_dirs(&category_dir)?;
        }

        Ok(result)
    }

    fn refresh_pattern(
        &self,
        cat: &crate::core::Category,
        pattern: &str,
        category_dir: &Path,
        seen_system_paths: &mut HashSet<PathBuf>,
        updated: &mut Vec<PathBuf>,
    ) -> Result<()> {
        for path in glob::glob(pattern)? {
            let path = path?;
            if !cat.matches(&path) {
                continue;
            }

            if path.is_dir() {
                let mut entries = WalkDir::new(&path).follow_links(false).into_iter();
                while let Some(entry) = entries.next() {
                    let entry = entry?;
                    let entry_path = entry.path();

                    if entry.file_type().is_dir() && !cat.matches(entry_path) {
                        entries.skip_current_dir();
                        continue;
                    }

                    if is_trackable_entry(&entry) {
                        self.refresh_file(entry_path, category_dir, seen_system_paths, updated)?;
                    }
                }
            } else if is_trackable_path(&path) {
                self.refresh_file(&path, category_dir, seen_system_paths, updated)?;
            }
        }

        Ok(())
    }

    fn refresh_file(
        &self,
        path: &Path,
        category_dir: &Path,
        seen_system_paths: &mut HashSet<PathBuf>,
        updated: &mut Vec<PathBuf>,
    ) -> Result<()> {
        seen_system_paths.insert(path.to_path_buf());

        let repo_path = category_dir.join(path.to_string_lossy().trim_start_matches('/'));

        if !repo_path.exists() || !self.files_equal(path, &repo_path)? {
            self.copy_to_repo(path, category_dir)?;
            updated.push(path.to_path_buf());
        }

        Ok(())
    }

    /// Restore a file from repository to system
    pub fn restore_file(&self, system_path: &Path) -> Result<()> {
        let categories = CategoryManager::load(self.repo)?;

        if let Some(cat) = categories.find_for_path(system_path) {
            let repo_path = self.repo.path().join(cat.repo_path_for(system_path));

            // Check if exists (file or symlink)
            if !repo_path.exists() && !repo_path.is_symlink() {
                return Err(ConfectError::FileNotFound(repo_path));
            }

            // Create parent directories
            if let Some(parent) = system_path.parent() {
                fs::create_dir_all(parent)?;
            }

            // Remove existing file/symlink
            if system_path.exists() || system_path.is_symlink() {
                let _ = fs::remove_file(system_path);
            }

            // Restore - handle symlinks
            let meta = fs::symlink_metadata(&repo_path)?;
            if meta.file_type().is_symlink() {
                let target = fs::read_link(&repo_path)?;
                #[cfg(unix)]
                std::os::unix::fs::symlink(&target, system_path)?;
            } else {
                fs::copy(&repo_path, system_path)?;
            }

            Ok(())
        } else {
            Err(ConfectError::PathNotTracked(system_path.to_path_buf()))
        }
    }

    /// List all files in a category
    pub fn list_files_in_category(&self, category_name: &str) -> Result<Vec<PathBuf>> {
        let categories = CategoryManager::load(self.repo)?;
        let cat = categories.get(category_name)?;

        let category_dir = self.repo.path().join(&cat.name);
        let mut files = Vec::new();

        if category_dir.exists() {
            for entry in WalkDir::new(&category_dir).follow_links(false) {
                let entry = entry?;
                if is_trackable_entry(&entry) {
                    let repo_path = entry.path();
                    if let Some(system_path) = cat.system_path_for(
                        repo_path
                            .strip_prefix(self.repo.path())
                            .unwrap_or(repo_path),
                    ) {
                        files.push(system_path);
                    }
                }
            }
        }

        Ok(files)
    }

    /// List all tracked files
    pub fn list_all_tracked_files(&self) -> Result<Vec<PathBuf>> {
        let categories = CategoryManager::load(self.repo)?;
        let mut files = Vec::new();

        for cat in categories.list() {
            files.extend(self.list_files_in_category(&cat.name)?);
        }

        Ok(files)
    }

    /// Count files in a category
    pub fn count_files_in_category(&self, category_name: &str) -> Result<usize> {
        Ok(self.list_files_in_category(category_name)?.len())
    }

    /// Count all tracked files
    pub fn count_all_files(&self) -> Result<usize> {
        Ok(self.list_all_tracked_files()?.len())
    }

    /// Get category name for a path
    pub fn get_category(&self, path: &Path) -> Result<String> {
        let categories = CategoryManager::load(self.repo)?;

        categories
            .find_for_path(path)
            .map(|c| c.name.clone())
            .ok_or_else(|| ConfectError::PathNotTracked(path.to_path_buf()))
    }
}

fn is_trackable_entry(entry: &DirEntry) -> bool {
    let file_type = entry.file_type();
    file_type.is_file() || file_type.is_symlink()
}

fn is_trackable_path(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|meta| meta.is_file() || meta.file_type().is_symlink())
        .unwrap_or(false)
}

fn remove_empty_dirs(root: &Path) -> Result<()> {
    let mut dirs = Vec::new();

    for entry in WalkDir::new(root).min_depth(1).follow_links(false) {
        let entry = entry?;
        if entry.file_type().is_dir() {
            dirs.push(entry.path().to_path_buf());
        }
    }

    dirs.sort_by_key(|path| std::cmp::Reverse(path.components().count()));

    for dir in dirs {
        if fs::read_dir(&dir)?.next().is_none() {
            fs::remove_dir(&dir)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{CategoryManager, Repository};
    use tempfile::tempdir;

    #[test]
    fn refresh_all_adds_new_files_and_removes_deleted_files_in_tracked_directory() -> Result<()> {
        let temp = tempdir()?;
        let repo_dir = temp.path().join("repo");
        let source_dir = temp.path().join("source");
        fs::create_dir_all(&source_dir)?;

        let old_file = source_dir.join("old.conf");
        fs::write(&old_file, "old")?;

        let repo = Repository::init(&repo_dir, "test-host")?;
        let mut categories = CategoryManager::load(&repo)?;
        categories.create(
            "configs",
            None,
            vec![source_dir.to_string_lossy().to_string()],
        )?;
        categories.save()?;

        let tracker = FileTracker::new(&repo);
        tracker.add(&source_dir, "configs", false)?;

        let nested_dir = source_dir.join("nested");
        fs::create_dir_all(&nested_dir)?;
        let new_file = nested_dir.join("new.conf");
        fs::write(&new_file, "new")?;
        fs::remove_file(&old_file)?;

        let refreshed = tracker.refresh_all()?;

        let repo_old_file = repo_dir.join("configs").join(
            old_file
                .strip_prefix("/")
                .expect("tempdir path is absolute"),
        );
        let repo_new_file = repo_dir.join("configs").join(
            new_file
                .strip_prefix("/")
                .expect("tempdir path is absolute"),
        );

        assert!(refreshed.updated.contains(&new_file));
        assert!(refreshed.deleted.contains(&old_file));
        assert!(!repo_old_file.exists());
        assert_eq!(fs::read_to_string(repo_new_file)?, "new");

        Ok(())
    }
}
