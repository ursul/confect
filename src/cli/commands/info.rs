use console::style;
use std::path::Path;

use crate::cli::context::Ctx;
use crate::core::config::REPO_FORMAT_VERSION;
use crate::core::Repository;
use crate::error::Result;

pub fn run(explicit_repo: Option<&Path>) -> Result<()> {
    let repo = Repository::open_any_version(explicit_repo)?;
    let version = repo.repo_config().repository.version;

    println!("{}", style("Repository").bold());
    println!("  path     {}", style(repo.path().display()).cyan());
    println!("  host     {}", repo.host());
    println!("  branch   {}", repo.branch());
    println!(
        "  format   {}{}",
        version,
        if version == REPO_FORMAT_VERSION {
            String::new()
        } else {
            format!(
                " (run 'confect migrate' to convert to {})",
                REPO_FORMAT_VERSION
            )
        }
    );
    for (name, url) in repo.git().remotes()? {
        println!("  remote   {} {}", name, url);
    }
    println!(
        "  push     {}",
        if repo.config().global.auto_push {
            format!("after every sync to '{}'", repo.remote())
        } else {
            "manual (auto_push = false)".to_string()
        }
    );

    if version != REPO_FORMAT_VERSION {
        return Ok(());
    }
    let ctx = Ctx::from_repo(repo)?;
    println!();
    println!("{}", style("Encryption").bold());
    let recipients = ctx.crypto.recipients().map(|r| r.len()).unwrap_or(0);
    println!(
        "  recipients  {} ({})",
        ctx.crypto.recipients_file().display(),
        if recipients == 0 {
            "missing".to_string()
        } else {
            format!("{} key(s)", recipients)
        }
    );
    println!(
        "  identity    {} ({})",
        ctx.crypto.identity_file().display(),
        if ctx.crypto.has_identity() {
            "present"
        } else {
            "missing"
        }
    );

    println!();
    println!("{}", style("Categories").bold());
    for category in ctx.categories.list() {
        let total = ctx.metadata.in_category(&category.name).count();
        let encrypted = ctx
            .metadata
            .in_category(&category.name)
            .filter(|(_, m)| m.encrypted)
            .count();
        println!(
            "  {:<16} {} stored{}",
            category.name,
            total,
            if encrypted > 0 {
                format!(", {} encrypted", encrypted)
            } else {
                String::new()
            }
        );
    }
    println!("  total            {}", ctx.metadata.len());
    Ok(())
}
