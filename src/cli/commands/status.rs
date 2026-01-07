use console::style;

use crate::core::{CategoryManager, Repository};
use crate::error::Result;
use crate::fs::FileTracker;

pub fn run_status(category: Option<String>, show_diff: bool) -> Result<()> {
    let repo = Repository::open_default()?;
    let _categories = CategoryManager::load(&repo)?;
    let tracker = FileTracker::new(&repo);

    // Get status for all or specific category
    let status = tracker.status(category.as_deref())?;

    println!();
    println!(
        "{} confect repository: {}",
        style("Repository:").bold(),
        style(repo.path().display()).cyan()
    );
    println!(
        "{} {}",
        style("Host:").bold(),
        style(repo.current_host()?).green()
    );
    println!();

    if status.is_empty() {
        println!(
            "{} All tracked files are in sync with the repository.",
            style("✓").green()
        );
        return Ok(());
    }

    // Group by status type
    let mut modified = Vec::new();
    let mut added = Vec::new();
    let mut deleted = Vec::new();
    let mut missing = Vec::new();

    for (path, file_status) in &status {
        match file_status {
            FileStatus::Modified => modified.push(path),
            FileStatus::Added => added.push(path),
            FileStatus::Deleted => deleted.push(path),
            FileStatus::Missing => missing.push(path),
        }
    }

    if !modified.is_empty() {
        println!("{}", style("Modified files:").yellow().bold());
        for path in &modified {
            println!("  {} {}", style("M").yellow(), path.display());
            if show_diff {
                // Show inline diff (abbreviated)
                if let Ok(diff) = tracker.diff_file(path) {
                    for line in diff.lines().take(10) {
                        println!("    {}", line);
                    }
                    if diff.lines().count() > 10 {
                        println!("    ...");
                    }
                }
            }
        }
        println!();
    }

    if !added.is_empty() {
        println!(
            "{}",
            style("New files (in system, not in repo):").green().bold()
        );
        for path in &added {
            println!("  {} {}", style("A").green(), path.display());
        }
        println!();
    }

    if !deleted.is_empty() {
        println!(
            "{}",
            style("Deleted files (in repo, not in system):")
                .red()
                .bold()
        );
        for path in &deleted {
            println!("  {} {}", style("D").red(), path.display());
        }
        println!();
    }

    if !missing.is_empty() {
        println!("{}", style("Missing files (tracked but not found):").dim());
        for path in &missing {
            println!("  {} {}", style("?").dim(), path.display());
        }
        println!();
    }

    // Summary
    let total = modified.len() + added.len() + deleted.len();
    println!("{} {} file(s) changed", style("Summary:").bold(), total);
    println!();
    println!("Run {} to commit changes.", style("confect sync").cyan());
    println!("Run {} to see full diff.", style("confect diff").cyan());

    Ok(())
}

/// Status of a tracked file
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileStatus {
    /// File has been modified
    Modified,
    /// File exists in system but not in repo
    Added,
    /// File exists in repo but not in system
    Deleted,
    /// File is tracked but missing from both
    Missing,
}
