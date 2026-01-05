use console::style;

use crate::core::Repository;
use crate::fs::{FileTracker, MetadataStore};
use crate::error::{Result, ConfectError};

pub fn run_sync(message: Option<String>, no_push: bool, _all_hosts: bool) -> Result<()> {
    let repo = Repository::open_default()?;
    let tracker = FileTracker::new(&repo);

    println!(
        "{} Checking for changes...",
        style("[1/4]").bold().dim()
    );

    // Refresh files from system to repository
    let updated = tracker.refresh_all()?;
    if updated.is_empty() {
        return Err(ConfectError::NoChanges);
    }

    println!(
        "{} Updated {} file(s) in repository",
        style("[2/4]").bold().dim(),
        updated.len()
    );

    // Update metadata
    let mut metadata = MetadataStore::load(&repo)?;
    for path in &updated {
        metadata.update_from_system(path)?;
    }
    metadata.save()?;

    // Generate commit message
    let commit_message = message.unwrap_or_else(|| {
        let file_count = updated.len();
        let categories: Vec<_> = updated
            .iter()
            .filter_map(|p| tracker.get_category(p).ok())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        if categories.len() == 1 {
            format!("Update {} ({} files)", categories[0], file_count)
        } else {
            format!("Update {} files across {} categories", file_count, categories.len())
        }
    });

    println!(
        "{} Creating commit...",
        style("[3/4]").bold().dim()
    );

    // Commit
    repo.commit_all(&commit_message)?;

    println!(
        "{} Committed: {}",
        style("✓").green(),
        style(&commit_message).italic()
    );

    // Push if enabled
    if !no_push && repo.has_remote("origin")? {
        println!(
            "{} Pushing to remote...",
            style("[4/4]").bold().dim()
        );
        repo.push("origin")?;
        println!("{} Pushed to origin", style("✓").green());
    } else {
        println!(
            "{} Skipping push (no remote or --no-push)",
            style("[4/4]").bold().dim()
        );
    }

    println!();
    println!(
        "{} Sync completed successfully!",
        style("✓").green().bold()
    );

    Ok(())
}
