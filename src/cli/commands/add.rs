use console::style;
use std::path::PathBuf;

use crate::core::{CategoryManager, Repository};
use crate::error::{ConfectError, Result};
use crate::fs::FileTracker;

pub fn run_add(
    path: PathBuf,
    category: Option<String>,
    create_category: bool,
    encrypt: bool,
) -> Result<()> {
    let repo = Repository::open_default()?;
    let mut categories = CategoryManager::load(&repo)?;

    // Validate path exists
    if !path.exists() {
        return Err(ConfectError::FileNotFound(path));
    }

    // Canonicalize path
    let path = path.canonicalize()?;

    // Validate path is safe to track
    validate_path(&path)?;

    // Determine category
    let category_name = match category {
        Some(name) => {
            if !categories.exists(&name) {
                if create_category {
                    categories.create(&name, None, vec![path.to_string_lossy().to_string()])?;
                    println!(
                        "{} Created category '{}'",
                        style("✓").green(),
                        style(&name).cyan()
                    );
                } else {
                    return Err(ConfectError::CategoryNotFound(name));
                }
            }
            name
        }
        None => {
            // Try to find matching category or use default
            categories
                .find_for_path(&path)
                .map(|c| c.name.clone())
                .unwrap_or_else(|| "default".to_string())
        }
    };

    // Track the file(s)
    let tracker = FileTracker::new(&repo);
    let added_files = tracker.add(&path, &category_name, encrypt)?;

    // Update categories if needed
    if !categories.contains_path(&category_name, &path) {
        categories.add_path(&category_name, path.to_string_lossy().to_string(), encrypt)?;
    }
    categories.save()?;

    // Report results
    println!();
    println!(
        "{} Added {} file(s) to category '{}':",
        style("✓").green().bold(),
        added_files.len(),
        style(&category_name).cyan()
    );
    for file in &added_files {
        let encrypted_mark = if encrypt { " (encrypted)" } else { "" };
        println!(
            "  {} {}{}",
            style("+").green(),
            file.display(),
            encrypted_mark
        );
    }
    println!();
    println!("Run {} to commit changes.", style("confect sync").cyan());

    Ok(())
}

fn validate_path(path: &PathBuf) -> Result<()> {
    let forbidden = [
        "/", "/root", "/home", "/boot", "/dev", "/proc", "/sys", "/run",
    ];

    for f in &forbidden {
        if path == &PathBuf::from(f) {
            return Err(ConfectError::ForbiddenPath(path.clone()));
        }
    }

    // Warn if not in /etc or /var
    if !path.starts_with("/etc") && !path.starts_with("/var") {
        eprintln!(
            "{} Adding file outside /etc or /var: {}",
            style("Warning:").yellow().bold(),
            path.display()
        );
    }

    Ok(())
}
