use console::style;

use crate::core::Repository;
use crate::error::{ConfectError, Result};
use crate::fs::{FileTracker, MetadataStore};

pub fn run_sync(message: Option<String>, no_push: bool, _all_hosts: bool) -> Result<()> {
    let repo = Repository::open_default()?;
    let tracker = FileTracker::new(&repo);

    println!("{} Checking for changes...", style("[1/4]").bold().dim());

    // Refresh files from system to repository
    let updated = tracker.refresh_all()?;

    // Check if there are any git changes (including untracked files from `add`)
    let has_git_changes = repo.has_changes()?;

    if updated.is_empty() && !has_git_changes {
        if !no_push && repo.has_remote("origin")? {
            println!("{} No local changes to commit", style("[2/4]").bold().dim());
            println!("{} Pushing to remote...", style("[3/4]").bold().dim());
            repo.push("origin")?;
            println!("{} Pushed to origin", style("✓").green());
            println!();
            println!("{} Sync completed successfully!", style("✓").green().bold());
            return Ok(());
        }

        return Err(ConfectError::NoChanges);
    }

    // Count changes for message
    let change_count = if updated.is_empty() {
        // Count from git status
        repo.status()?.len()
    } else {
        updated.len()
    };

    println!(
        "{} Staging {} file(s)",
        style("[2/4]").bold().dim(),
        change_count
    );

    // Update metadata for refreshed files
    if !updated.is_empty() {
        let mut metadata = MetadataStore::load(&repo)?;
        for path in &updated {
            metadata.update_from_system(path)?;
        }
        metadata.save()?;
    }

    // Generate commit message
    let commit_message = message.unwrap_or_else(|| {
        if !updated.is_empty() {
            let categories: Vec<_> = updated
                .iter()
                .filter_map(|p| tracker.get_category(p).ok())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();

            if categories.len() == 1 {
                format!("Update {} ({} files)", categories[0], updated.len())
            } else {
                format!(
                    "Update {} files across {} categories",
                    updated.len(),
                    categories.len()
                )
            }
        } else {
            format!("Add {} files", change_count)
        }
    });

    println!("{} Creating commit...", style("[3/4]").bold().dim());

    // Commit
    repo.commit_all(&commit_message)?;

    println!(
        "{} Committed: {}",
        style("✓").green(),
        style(&commit_message).italic()
    );

    // Push if enabled
    if !no_push && repo.has_remote("origin")? {
        println!("{} Pushing to remote...", style("[4/4]").bold().dim());
        repo.push("origin")?;
        println!("{} Pushed to origin", style("✓").green());
    } else {
        println!(
            "{} Skipping push (no remote or --no-push)",
            style("[4/4]").bold().dim()
        );
    }

    println!();
    println!("{} Sync completed successfully!", style("✓").green().bold());

    Ok(())
}
