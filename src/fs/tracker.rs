use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::cli::commands::FileStatus;
use crate::core::{CategoryManager, Repository};
use crate::error::{ConfectError, Result};

/// Tracks files between the system and the repository
pub struct FileTracker<'a> {
    repo: &'a Repository,
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
                if entry.file_type().is_file() || entry.file_type().is_symlink() {
                    let file_path = entry.path();
                    self.copy_to_repo(file_path, &category_dir)?;
                    added_files.push(file_path.to_path_buf());
                }
            }
        } else {
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

        // Handle symlinks
        let meta = fs::symlink_metadata(system_path)?;
        if meta.file_type().is_symlink() {
            let _target = fs::read_link(system_path)?;
            // Store symlink target - we copy the actual file content
            if let Ok(real_path) = fs::canonicalize(system_path) {
                if real_path.exists() {
                    fs::copy(&real_path, &repo_path)?;
                }
            }
            // Symlink info is stored in metadata.toml
        } else {
            fs::copy(system_path, &repo_path)?;
        }

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
                if !entry.file_type().is_file() {
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

    /// Check if two files have the same content
    fn files_equal(&self, path1: &Path, path2: &Path) -> Result<bool> {
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
    pub fn refresh_all(&self) -> Result<Vec<PathBuf>> {
        let categories = CategoryManager::load(self.repo)?;
        let mut updated = Vec::new();

        for cat in categories.list() {
            let category_dir = self.repo.path().join(&cat.name);

            // Expand paths in category
            for pattern in &cat.paths {
                for path in glob::glob(pattern)?.flatten() {
                    if path.is_file() {
                        let repo_path =
                            category_dir.join(path.to_string_lossy().trim_start_matches('/'));

                        // Check if file changed
                        if !repo_path.exists() || !self.files_equal(&path, &repo_path)? {
                            self.copy_to_repo(&path, &category_dir)?;
                            updated.push(path);
                        }
                    }
                }
            }
        }

        Ok(updated)
    }

    /// Restore a file from repository to system
    pub fn restore_file(&self, system_path: &Path) -> Result<()> {
        let categories = CategoryManager::load(self.repo)?;

        if let Some(cat) = categories.find_for_path(system_path) {
            let repo_path = self.repo.path().join(cat.repo_path_for(system_path));

            if !repo_path.exists() {
                return Err(ConfectError::FileNotFound(repo_path));
            }

            // Create parent directories
            if let Some(parent) = system_path.parent() {
                fs::create_dir_all(parent)?;
            }

            // Copy file
            fs::copy(&repo_path, system_path)?;

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
                if entry.file_type().is_file() {
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
