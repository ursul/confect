use std::path::PathBuf;
use console::style;

use crate::core::{Config, Repository};
use crate::error::Result;

pub fn run_init(
    path: Option<PathBuf>,
    remote: Option<String>,
    host: Option<String>,
) -> Result<()> {
    let repo_path = path.unwrap_or_else(|| PathBuf::from("/var/lib/confect"));
    let hostname = host.unwrap_or_else(|| {
        hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string())
    });

    println!(
        "{} Initializing confect repository at {}",
        style("[1/3]").bold().dim(),
        style(repo_path.display()).cyan()
    );

    // Create repository
    let repo = Repository::init(&repo_path, &hostname)?;

    println!(
        "{} Created branch {}",
        style("[2/3]").bold().dim(),
        style(format!("host/{}", hostname)).green()
    );

    // Set up remote if provided
    if let Some(url) = remote {
        repo.add_remote("origin", &url)?;
        println!(
            "{} Added remote origin: {}",
            style("[3/3]").bold().dim(),
            style(&url).cyan()
        );
    } else {
        println!(
            "{} No remote configured (use 'git remote add origin <url>' later)",
            style("[3/3]").bold().dim()
        );
    }

    // Create global config
    Config::init_global(&hostname)?;

    println!();
    println!(
        "{} Repository initialized successfully!",
        style("✓").green().bold()
    );
    println!();
    println!("Next steps:");
    println!("  1. Add files:     confect add /etc/nginx --category nginx");
    println!("  2. Sync changes:  confect sync -m \"Initial commit\"");
    println!("  3. View status:   confect status");

    Ok(())
}
