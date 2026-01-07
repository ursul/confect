use glob::Pattern;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::core::Repository;
use crate::error::{ConfectError, Result};

/// A category groups related configuration files together
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Glob patterns for files in this category
    #[serde(default)]
    pub paths: Vec<String>,
    /// Glob patterns for files that should be encrypted
    #[serde(default)]
    pub encrypt: Vec<String>,
    /// Exclusion patterns
    #[serde(default)]
    pub exclude: Vec<String>,
}

impl Category {
    /// Create a new category
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            description: None,
            paths: Vec::new(),
            encrypt: Vec::new(),
            exclude: Vec::new(),
        }
    }

    /// Check if a path matches this category
    pub fn matches(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        // Check exclusions first
        for pattern in &self.exclude {
            if let Ok(p) = Pattern::new(pattern) {
                if p.matches(&path_str) {
                    return false;
                }
            }
        }

        // Check if path matches any pattern
        for pattern in &self.paths {
            if let Ok(p) = Pattern::new(pattern) {
                if p.matches(&path_str) {
                    return true;
                }
            }
            // Also check exact match
            if pattern == &path_str.to_string() {
                return true;
            }
        }

        false
    }

    /// Check if a file should be encrypted
    pub fn should_encrypt(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        for pattern in &self.encrypt {
            if let Ok(p) = Pattern::new(pattern) {
                if p.matches(&path_str) {
                    return true;
                }
            }
        }

        false
    }

    /// Get the repository path for a system path
    pub fn repo_path_for(&self, system_path: &Path) -> PathBuf {
        // category_name/etc/nginx/nginx.conf
        let path_str = system_path.to_string_lossy();
        let relative = path_str.trim_start_matches('/');
        PathBuf::from(&self.name).join(relative)
    }

    /// Get the system path from a repository path
    pub fn system_path_for(&self, repo_path: &Path) -> Option<PathBuf> {
        // Strip category prefix and add leading /
        let components: Vec<_> = repo_path.components().collect();
        if components.is_empty() {
            return None;
        }

        // Skip the category name (first component)
        let rest: PathBuf = components.into_iter().skip(1).collect();
        if rest.as_os_str().is_empty() {
            return None;
        }

        Some(PathBuf::from("/").join(rest))
    }
}

/// Categories file structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CategoriesFile {
    #[serde(default)]
    categories: HashMap<String, CategoryData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CategoryData {
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    paths: Vec<String>,
    #[serde(default)]
    encrypt: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
}

impl From<&Category> for CategoryData {
    fn from(cat: &Category) -> Self {
        Self {
            description: cat.description.clone(),
            paths: cat.paths.clone(),
            encrypt: cat.encrypt.clone(),
            exclude: cat.exclude.clone(),
        }
    }
}

/// Manages categories for a repository
pub struct CategoryManager {
    categories: HashMap<String, Category>,
    repo_path: PathBuf,
}

impl CategoryManager {
    /// Load categories from repository
    pub fn load(repo: &Repository) -> Result<Self> {
        let repo_path = repo.path().to_path_buf();
        let categories_file = repo_path.join(".confect").join("categories.toml");

        let categories = if categories_file.exists() {
            let content = fs::read_to_string(&categories_file)?;
            let file: CategoriesFile = toml::from_str(&content)?;

            file.categories
                .into_iter()
                .map(|(name, data)| {
                    let cat = Category {
                        name: name.clone(),
                        description: data.description,
                        paths: data.paths,
                        encrypt: data.encrypt,
                        exclude: data.exclude,
                    };
                    (name, cat)
                })
                .collect()
        } else {
            HashMap::new()
        };

        Ok(Self {
            categories,
            repo_path,
        })
    }

    /// Save categories to repository
    pub fn save(&self) -> Result<()> {
        let confect_dir = self.repo_path.join(".confect");
        fs::create_dir_all(&confect_dir)?;

        let categories_file = confect_dir.join("categories.toml");

        let file = CategoriesFile {
            categories: self
                .categories
                .iter()
                .map(|(name, cat)| (name.clone(), CategoryData::from(cat)))
                .collect(),
        };

        let content = toml::to_string_pretty(&file)?;
        fs::write(&categories_file, content)?;
        Ok(())
    }

    /// List all categories
    pub fn list(&self) -> Vec<&Category> {
        self.categories.values().collect()
    }

    /// Check if category exists
    pub fn exists(&self, name: &str) -> bool {
        self.categories.contains_key(name)
    }

    /// Get category by name
    pub fn get(&self, name: &str) -> Result<&Category> {
        self.categories
            .get(name)
            .ok_or_else(|| ConfectError::CategoryNotFound(name.to_string()))
    }

    /// Get mutable category by name
    pub fn get_mut(&mut self, name: &str) -> Result<&mut Category> {
        self.categories
            .get_mut(name)
            .ok_or_else(|| ConfectError::CategoryNotFound(name.to_string()))
    }

    /// Add a new category
    pub fn add(&mut self, category: Category) -> Result<()> {
        if self.categories.contains_key(&category.name) {
            return Err(ConfectError::CategoryAlreadyExists(category.name.clone()));
        }
        self.categories.insert(category.name.clone(), category);
        Ok(())
    }

    /// Create a new category with paths
    pub fn create(
        &mut self,
        name: &str,
        description: Option<String>,
        paths: Vec<String>,
    ) -> Result<()> {
        let cat = Category {
            name: name.to_string(),
            description,
            paths,
            encrypt: Vec::new(),
            exclude: Vec::new(),
        };
        self.add(cat)
    }

    /// Remove a category
    pub fn remove(&mut self, name: &str) -> Result<()> {
        if self.categories.remove(name).is_none() {
            return Err(ConfectError::CategoryNotFound(name.to_string()));
        }
        Ok(())
    }

    /// Find category for a given path
    pub fn find_for_path(&self, path: &Path) -> Option<&Category> {
        self.categories.values().find(|cat| cat.matches(path))
    }

    /// Check if any category contains a path
    pub fn contains_path(&self, category_name: &str, path: &Path) -> bool {
        if let Some(cat) = self.categories.get(category_name) {
            cat.matches(path)
        } else {
            false
        }
    }

    /// Add a path to an existing category
    pub fn add_path(&mut self, name: &str, path: String, encrypt: bool) -> Result<()> {
        let cat = self.get_mut(name)?;

        if !cat.paths.contains(&path) {
            cat.paths.push(path.clone());
        }

        if encrypt && !cat.encrypt.contains(&path) {
            cat.encrypt.push(path);
        }

        Ok(())
    }

    /// Remove a path from a category
    pub fn remove_path(&mut self, name: &str, path: &str) -> Result<()> {
        let cat = self.get_mut(name)?;

        cat.paths.retain(|p| p != path);
        cat.encrypt.retain(|p| p != path);

        Ok(())
    }
}
