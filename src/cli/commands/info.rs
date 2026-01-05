use console::style;

use crate::core::{Repository, CategoryManager, Config};
use crate::fs::FileTracker;
use crate::error::Result;

pub fn run_info() -> Result<()> {
    let repo = Repository::open_default()?;
    let categories = CategoryManager::load(&repo)?;
    let tracker = FileTracker::new(&repo);
    let config = Config::load_global()?;

    println!();
    println!("{}", style("confect repository info").bold().underlined());
    println!();

    // Repository info
    println!("{}", style("Repository:").bold());
    println!("  Path:     {}", style(repo.path().display()).cyan());
    println!("  Host:     {}", style(repo.current_host()?).green());

    // Remote info
    if let Ok(remotes) = repo.list_remotes() {
        if !remotes.is_empty() {
            println!("  Remotes:");
            for (name, url) in remotes {
                println!("    {}: {}", style(name).cyan(), url);
            }
        }
    }

    println!();

    // Categories
    let cats = categories.list();
    println!("{}", style("Categories:").bold());
    if cats.is_empty() {
        println!("  (none)");
    } else {
        for cat in &cats {
            let file_count = tracker.count_files_in_category(&cat.name).unwrap_or(0);
            println!(
                "  {} ({} files)",
                style(&cat.name).cyan(),
                file_count
            );
        }
    }
    println!();

    // Stats
    let total_files = tracker.count_all_files().unwrap_or(0);
    let status = tracker.status(None).unwrap_or_default();
    let modified = status.len();

    println!("{}", style("Statistics:").bold());
    println!("  Total tracked files:  {}", total_files);
    println!("  Modified files:       {}", modified);
    println!();

    // Config info
    println!("{}", style("Configuration:").bold());
    println!("  Auto-push:   {}", if config.global.auto_push { "yes" } else { "no" });
    println!("  Encryption:  {}", if config.encryption.enabled { "enabled" } else { "disabled" });
    println!();

    Ok(())
}
