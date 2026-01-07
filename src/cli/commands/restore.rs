use console::style;
use dialoguer::Confirm;
use std::path::PathBuf;

use crate::core::{CategoryManager, Repository};
use crate::error::Result;
use crate::fs::{FileTracker, MetadataStore};

pub fn run_restore(
    category: Option<String>,
    file: Option<PathBuf>,
    dry_run: bool,
    force: bool,
    backup: bool,
) -> Result<()> {
    let repo = Repository::open_default()?;
    let categories = CategoryManager::load(&repo)?;
    let tracker = FileTracker::new(&repo);
    let metadata = MetadataStore::load(&repo)?;

    // Determine what to restore
    let files_to_restore = if let Some(path) = file {
        vec![path]
    } else if let Some(cat_name) = &category {
        let cat = categories.get(cat_name)?;
        tracker.list_files_in_category(&cat.name)?
    } else {
        tracker.list_all_tracked_files()?
    };

    if files_to_restore.is_empty() {
        println!("No files to restore.");
        return Ok(());
    }

    println!();
    println!(
        "{} {} file(s) to restore:",
        style("Found").bold(),
        files_to_restore.len()
    );
    for path in &files_to_restore {
        let exists = path.exists();
        let marker = if exists {
            style("→").yellow()
        } else {
            style("+").green()
        };
        println!("  {} {}", marker, path.display());
    }
    println!();

    if dry_run {
        println!("{}", style("Dry run - no changes made.").dim());
        return Ok(());
    }

    // Confirm unless forced
    if !force {
        let proceed = Confirm::new()
            .with_prompt("Proceed with restore?")
            .default(false)
            .interact()?;

        if !proceed {
            println!("Aborted.");
            return Ok(());
        }
    }

    // Create backups if requested
    if backup {
        println!("{} Creating backups...", style("[1/3]").bold().dim());
        for path in &files_to_restore {
            if path.exists() {
                let backup_path = PathBuf::from(format!("{}.confect-backup", path.display()));
                std::fs::copy(path, &backup_path)?;
            }
        }
    }

    // Restore files
    println!(
        "{} Restoring files...",
        style(if backup { "[2/3]" } else { "[1/2]" }).bold().dim()
    );

    let mut restored = 0;
    let mut errors = Vec::new();

    for path in &files_to_restore {
        match tracker.restore_file(path) {
            Ok(_) => {
                // Apply metadata (permissions, owner)
                if let Err(e) = metadata.apply_to(path) {
                    eprintln!(
                        "  {} Failed to restore permissions for {}: {}",
                        style("!").yellow(),
                        path.display(),
                        e
                    );
                }
                restored += 1;
            }
            Err(e) => {
                errors.push((path.clone(), e));
            }
        }
    }

    // Report results
    println!(
        "{} Finishing...",
        style(if backup { "[3/3]" } else { "[2/2]" }).bold().dim()
    );
    println!();

    if errors.is_empty() {
        println!(
            "{} Restored {} file(s) successfully!",
            style("✓").green().bold(),
            restored
        );
    } else {
        println!(
            "{} Restored {} file(s), {} error(s):",
            style("!").yellow().bold(),
            restored,
            errors.len()
        );
        for (path, err) in &errors {
            println!("  {} {}: {}", style("✗").red(), path.display(), err);
        }
    }

    if backup {
        println!();
        println!(
            "Backups saved with {} suffix.",
            style(".confect-backup").dim()
        );
    }

    Ok(())
}
