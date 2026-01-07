use console::style;
use std::path::PathBuf;

use crate::core::{CategoryManager, Repository};
use crate::error::Result;
use crate::fs::FileTracker;

pub fn run_remove(path: PathBuf, delete: bool) -> Result<()> {
    let repo = Repository::open_default()?;
    let mut categories = CategoryManager::load(&repo)?;
    let tracker = FileTracker::new(&repo);

    // Canonicalize path if it exists, otherwise use as-is
    let path = path.canonicalize().unwrap_or(path);

    // Find which category this file belongs to
    let category_name = categories.find_for_path(&path).map(|c| c.name.clone());

    // Remove from tracker
    let removed_files = tracker.remove(&path, delete)?;

    // Update category
    if let Some(name) = category_name {
        categories.remove_path(&name, path.to_string_lossy().as_ref())?;
        categories.save()?;
    }

    println!();
    println!(
        "{} Removed {} file(s) from tracking{}:",
        style("✓").green().bold(),
        removed_files.len(),
        if delete {
            " and deleted from repository"
        } else {
            ""
        }
    );
    for file in &removed_files {
        println!("  {} {}", style("-").red(), file.display());
    }
    println!();
    println!("Run {} to commit changes.", style("confect sync").cyan());

    Ok(())
}
