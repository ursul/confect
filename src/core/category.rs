use glob::{MatchOptions, Pattern};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::core::paths::{glob_base, has_glob};
use crate::error::{ConfectError, IoContext, Result};

const MATCH_OPTIONS: MatchOptions = MatchOptions {
    case_sensitive: true,
    require_literal_separator: true,
    require_literal_leading_dot: false,
};

/// A category groups related configuration files together.
///
/// All pattern lists share one syntax: an absolute path covers itself and everything
/// below it, an absolute glob covers what it matches and everything below that, and a
/// pattern without `/` (such as `*.pem`) matches a file or directory name at any depth.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Category {
    #[serde(skip)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub encrypt: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,
    /// Files the secret guard lets through in plaintext on purpose.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allow_plaintext: Vec<String>,
}

impl Category {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Self::default()
        }
    }

    /// Whether the path lies inside one of the category's paths (ignoring exclusions).
    pub fn covers(&self, path: &Path) -> bool {
        self.paths.iter().any(|p| pattern_covers(p, path))
    }

    pub fn is_excluded(&self, path: &Path) -> bool {
        self.exclude.iter().any(|p| pattern_covers(p, path))
    }

    /// Whether the path belongs to this category.
    pub fn matches(&self, path: &Path) -> bool {
        self.covers(path) && !self.is_excluded(path)
    }

    pub fn should_encrypt(&self, path: &Path) -> bool {
        self.encrypt.iter().any(|p| pattern_covers(p, path))
    }

    pub fn allows_plaintext(&self, path: &Path) -> bool {
        self.allow_plaintext.iter().any(|p| pattern_covers(p, path))
    }

    /// Length of the most specific path pattern covering `path`, for tie-breaking.
    fn specificity(&self, path: &Path) -> usize {
        self.paths
            .iter()
            .filter(|p| pattern_covers(p, path))
            .map(|p| glob_base(p).as_os_str().len())
            .max()
            .unwrap_or(0)
    }
}

/// Does `pattern` cover `path`, either directly or through an ancestor directory?
pub fn pattern_covers(pattern: &str, path: &Path) -> bool {
    if !pattern.contains('/') {
        let Ok(p) = Pattern::new(pattern) else {
            return false;
        };
        return path.iter().any(|name| {
            name.to_str()
                .map(|n| p.matches_with(n, MATCH_OPTIONS))
                .unwrap_or(false)
        });
    }

    if !has_glob(pattern) {
        return path.starts_with(pattern);
    }

    let Ok(p) = Pattern::new(pattern) else {
        return false;
    };
    path.ancestors().any(|ancestor| {
        ancestor
            .to_str()
            .map(|a| p.matches_with(a, MATCH_OPTIONS))
            .unwrap_or(false)
    })
}

pub fn validate_name(name: &str) -> Result<()> {
    let valid = !name.is_empty()
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if valid {
        Ok(())
    } else {
        Err(ConfectError::InvalidCategoryName(name.to_string()))
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct CategoriesFile {
    #[serde(default)]
    categories: BTreeMap<String, Category>,
}

/// Categories of a repository (`.confect/categories.toml`), kept sorted on disk.
pub struct CategoryManager {
    categories: BTreeMap<String, Category>,
    file: PathBuf,
}

impl CategoryManager {
    pub fn file(repo_path: &Path) -> PathBuf {
        repo_path.join(".confect").join("categories.toml")
    }

    pub fn load(repo_path: &Path) -> Result<Self> {
        let file = Self::file(repo_path);
        let mut categories = if file.exists() {
            let content = fs::read_to_string(&file).at(&file)?;
            toml::from_str::<CategoriesFile>(&content)?.categories
        } else {
            BTreeMap::new()
        };
        for (name, category) in categories.iter_mut() {
            category.name = name.clone();
        }
        Ok(Self { categories, file })
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.file.parent() {
            fs::create_dir_all(parent).at(parent)?;
        }
        let content = toml::to_string_pretty(&CategoriesFile {
            categories: self.categories.clone(),
        })?;
        fs::write(&self.file, content).at(&self.file)?;
        Ok(())
    }

    pub fn list(&self) -> impl Iterator<Item = &Category> {
        self.categories.values()
    }

    pub fn names(&self) -> Vec<String> {
        self.categories.keys().cloned().collect()
    }

    pub fn exists(&self, name: &str) -> bool {
        self.categories.contains_key(name)
    }

    pub fn get(&self, name: &str) -> Result<&Category> {
        self.categories
            .get(name)
            .ok_or_else(|| ConfectError::CategoryNotFound(name.to_string()))
    }

    pub fn get_mut(&mut self, name: &str) -> Result<&mut Category> {
        self.categories
            .get_mut(name)
            .ok_or_else(|| ConfectError::CategoryNotFound(name.to_string()))
    }

    pub fn create(&mut self, name: &str, description: Option<String>) -> Result<&mut Category> {
        validate_name(name)?;
        if self.categories.contains_key(name) {
            return Err(ConfectError::CategoryAlreadyExists(name.to_string()));
        }
        let mut category = Category::new(name);
        category.description = description;
        Ok(self.categories.entry(name.to_string()).or_insert(category))
    }

    pub fn remove(&mut self, name: &str) -> Result<Category> {
        self.categories
            .remove(name)
            .ok_or_else(|| ConfectError::CategoryNotFound(name.to_string()))
    }

    /// The category a path belongs to; the most specific one wins for legacy overlaps.
    pub fn find_for_path(&self, path: &Path) -> Option<&Category> {
        self.categories
            .values()
            .filter(|c| c.matches(path))
            .max_by_key(|c| c.specificity(path))
    }

    /// The category whose paths cover `path`, even if it excludes it.
    pub fn find_covering(&self, path: &Path) -> Option<&Category> {
        self.categories
            .values()
            .filter(|c| c.covers(path))
            .max_by_key(|c| c.specificity(path))
    }

    /// Refuse a new path pattern that would put files into two categories or track
    /// them twice in one.
    pub fn check_overlap(&self, pattern: &str) -> Result<()> {
        let base = glob_base(pattern);
        for category in self.categories.values() {
            for existing in &category.paths {
                let overlaps = pattern_covers(existing, &base)
                    || pattern_covers(pattern, &glob_base(existing));
                if overlaps {
                    return Err(ConfectError::OverlappingPath {
                        path: PathBuf::from(pattern),
                        other: existing.clone(),
                        category: category.name.clone(),
                    });
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn category(paths: &[&str], exclude: &[&str]) -> Category {
        Category {
            name: "c".into(),
            paths: paths.iter().map(|s| s.to_string()).collect(),
            exclude: exclude.iter().map(|s| s.to_string()).collect(),
            ..Category::default()
        }
    }

    #[test]
    fn directory_path_covers_children_only() {
        let c = category(&["/etc/nginx"], &[]);
        assert!(c.matches(Path::new("/etc/nginx")));
        assert!(c.matches(Path::new("/etc/nginx/sites-enabled/default")));
        assert!(!c.matches(Path::new("/etc/nginx-old/nginx.conf")));
    }

    #[test]
    fn excluded_files_and_directories_are_not_matched() {
        let c = category(
            &["/etc/app"],
            &["/etc/app/cache", "/etc/app/secret.key", "*.pem"],
        );
        assert!(c.matches(Path::new("/etc/app/app.conf")));
        assert!(!c.matches(Path::new("/etc/app/cache/state.db")));
        assert!(!c.matches(Path::new("/etc/app/secret.key")));
        assert!(!c.matches(Path::new("/etc/app/tls/server.pem")));
    }

    #[test]
    fn globs_do_not_cross_directory_separators() {
        let c = category(&["/etc/nginx/*.conf"], &[]);
        assert!(c.matches(Path::new("/etc/nginx/nginx.conf")));
        assert!(!c.matches(Path::new("/etc/nginx/conf.d/site.conf")));
    }

    #[test]
    fn glob_matching_a_directory_covers_its_content() {
        let c = category(&["/etc/letsencrypt/renewal*"], &[]);
        assert!(c.matches(Path::new("/etc/letsencrypt/renewal/site.conf")));
        assert!(c.matches(Path::new("/etc/letsencrypt/renewal-hooks/deploy/x")));
        assert!(!c.matches(Path::new("/etc/letsencrypt/archive/site/privkey1.pem")));
    }

    #[test]
    fn overlapping_paths_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let mut manager = CategoryManager::load(dir.path()).unwrap();
        manager
            .create("web", None)
            .unwrap()
            .paths
            .push("/etc/nginx".into());
        assert!(manager.check_overlap("/etc/nginx/nginx.conf").is_err());
        assert!(manager.check_overlap("/etc").is_err());
        assert!(manager.check_overlap("/etc/nginx").is_err());
        assert!(manager.check_overlap("/etc/ssh").is_ok());
        assert!(manager.check_overlap("/etc/*.conf").is_ok());
    }

    #[test]
    fn category_names_are_validated() {
        assert!(validate_name("runtime").is_ok());
        assert!(validate_name("my_cat-1.x").is_ok());
        assert!(validate_name(".confect").is_err());
        assert!(validate_name("a/b").is_err());
        assert!(validate_name("").is_err());
    }
}
