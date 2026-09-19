use std::path::Path;

use crate::cli::commands::restore::{self, Options};
use crate::cli::context::Ctx;
use crate::cli::ui;
use crate::core::Repository;
use crate::error::{ConfectError, Result};

pub fn pull(explicit_repo: Option<&Path>, then_restore: bool, yes: bool) -> Result<()> {
    let repo = Repository::open(explicit_repo)?;
    let _lock = repo.lock()?;
    let remote = repo.remote().to_string();
    let branch = repo.branch();
    if repo.git().remote_url(&remote)?.is_none() {
        return Err(ConfectError::Other(format!(
            "no remote '{}' configured",
            remote
        )));
    }
    if repo.git().is_dirty()? {
        return Err(ConfectError::Other(
            "the repository has changes that are not committed yet; run 'confect sync' first"
                .into(),
        ));
    }
    if !repo.git().fetch_branch(&remote, &branch)? {
        ui::info(&format!(
            "{} has no branch {} yet; nothing to pull",
            remote, branch
        ));
        return Ok(());
    }
    if repo.git().fast_forward(&remote, &branch)? {
        ui::success(&format!("Fast-forwarded {} from {}", branch, remote));
    } else {
        ui::success(&format!("{} is already up to date", branch));
    }
    drop(_lock);

    if then_restore {
        let ctx = Ctx::from_repo(repo)?;
        let _lock = ctx.repo.lock()?;
        restore::restore_with(
            &ctx,
            Options {
                paths: Vec::new(),
                category: None,
                dry_run: false,
                yes,
                backup: true,
            },
        )?;
    }
    Ok(())
}

pub fn push(explicit_repo: Option<&Path>) -> Result<()> {
    let repo = Repository::open(explicit_repo)?;
    let _lock = repo.lock()?;
    let remote = repo.remote().to_string();
    if repo.git().remote_url(&remote)?.is_none() {
        return Err(ConfectError::Other(format!(
            "no remote '{}' configured",
            remote
        )));
    }
    let outcome = repo.git().push(&remote, &repo.branch())?;
    if outcome.up_to_date {
        ui::success(&format!("{} is up to date", remote));
    } else {
        ui::success(&format!("Pushed {} to {}", repo.branch(), remote));
    }
    Ok(())
}
