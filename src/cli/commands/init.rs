use std::path::PathBuf;
use console::style;

use crate::core::{Config, Repository};
use crate::error::{ConfectError, Result};

pub fn run_init(
    path: Option<PathBuf>,
    system: bool,
    remote: Option<String>,
    host: Option<String>,
) -> Result<()> {
    let repo_path = if let Some(p) = path {
        p
    } else if system {
        Config::system_repo_path()
    } else {
        Config::default_repo_path()
    };
    let hostname = host.unwrap_or_else(|| {
        hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string())
    });

    let git_dir = repo_path.join(".git");
    let confect_dir = repo_path.join(".confect");

    if git_dir.exists() {
        if let Some(url) = remote.as_ref() {
            if !confect_dir.exists() {
                return Err(ConfectError::AlreadyInitialized(repo_path));
            }

            println!(
                "{} Repository already initialized at {}",
                style("[1/2]").bold().dim(),
                style(repo_path.display()).cyan()
            );

            let repo = Repository::open(&repo_path)?;
            if repo.has_remote("origin")? {
                println!(
                    "{} Remote origin already configured (use 'git remote set-url origin <url>' to change)",
                    style("[2/2]").bold().dim()
                );
            } else {
                repo.add_remote("origin", url)?;
                println!(
                    "{} Added remote origin: {}",
                    style("[2/2]").bold().dim(),
                    style(url).cyan()
                );
            }

            return Ok(());
        }

        return Err(ConfectError::AlreadyInitialized(repo_path));
    }

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
    if let Some(url) = remote.as_ref() {
        repo.add_remote("origin", url)?;
        println!(
            "{} Added remote origin: {}",
            style("[3/3]").bold().dim(),
            style(url).cyan()
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
