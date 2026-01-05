use std::path::PathBuf;
use console::style;

use crate::core::{Repository, CategoryManager};
use crate::fs::FileTracker;
use crate::error::Result;

pub fn run_diff(category: Option<String>, file: Option<PathBuf>) -> Result<()> {
    let repo = Repository::open_default()?;
    let categories = CategoryManager::load(&repo)?;
    let tracker = FileTracker::new(&repo);

    // Determine what to diff
    let files_to_diff = if let Some(path) = file {
        vec![path.canonicalize().unwrap_or(path)]
    } else if let Some(cat_name) = &category {
        let cat = categories.get(cat_name)?;
        tracker.list_files_in_category(&cat.name)?
    } else {
        tracker.list_all_tracked_files()?
    };

    if files_to_diff.is_empty() {
        println!("No files to compare.");
        return Ok(());
    }

    let mut has_diff = false;

    for path in &files_to_diff {
        if let Ok(diff) = tracker.diff_file(path) {
            if !diff.is_empty() {
                has_diff = true;
                println!();
                println!(
                    "{} {}",
                    style("diff").bold(),
                    style(path.display()).cyan()
                );
                println!("{}", style("─".repeat(60)).dim());

                for line in diff.lines() {
                    if line.starts_with('+') && !line.starts_with("+++") {
                        println!("{}", style(line).green());
                    } else if line.starts_with('-') && !line.starts_with("---") {
                        println!("{}", style(line).red());
                    } else if line.starts_with("@@") {
                        println!("{}", style(line).cyan());
                    } else {
                        println!("{}", line);
                    }
                }
            }
        }
    }

    if !has_diff {
        println!("No differences found.");
    }

    Ok(())
}
