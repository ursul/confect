use console::style;
use std::path::{Path, PathBuf};

use crate::cli::context::Ctx;
use crate::cli::ui;
use crate::core::paths::absolute;
use crate::error::{ConfectError, Result};
use crate::track::plan::Scope;

/// Untrack paths: a category path is removed from the category, a path inside a tracked
/// directory is excluded. Stored copies are dropped either way.
pub fn run(explicit_repo: Option<&Path>, paths: Vec<PathBuf>) -> Result<()> {
    let mut ctx = Ctx::open(explicit_repo)?;
    let _lock = ctx.repo.lock()?;

    let mut scope_paths = Vec::new();
    let mut touched = Vec::new();
    for path in paths {
        let path = absolute(&path)?;
        let pattern = path.to_string_lossy().into_owned();

        let owner = ctx
            .categories
            .list()
            .find(|c| c.paths.contains(&pattern))
            .map(|c| c.name.clone());
        if let Some(name) = owner {
            let cat = ctx.categories.get_mut(&name)?;
            cat.paths.retain(|p| p != &pattern);
            cat.encrypt.retain(|p| p != &pattern);
            cat.allow_plaintext.retain(|p| p != &pattern);
            println!(
                "  {} {} (removed from '{}')",
                style("-").red(),
                path.display(),
                name
            );
            touched.push(name);
        } else if let Some(name) = ctx.categories.find_for_path(&path).map(|c| c.name.clone()) {
            ctx.categories.get_mut(&name)?.exclude.push(pattern);
            println!(
                "  {} {} (excluded in '{}')",
                style("-").red(),
                path.display(),
                name
            );
            touched.push(name);
        } else {
            return Err(ConfectError::PathNotTracked(path));
        }
        scope_paths.push(path);
    }

    let mut dropped = 0;
    touched.sort();
    touched.dedup();
    for name in touched {
        let plan = ctx.commit_category_change(&Scope {
            category: Some(name),
            paths: scope_paths.clone(),
        })?;
        dropped += plan.changes.len();
    }
    ui::success(&format!("Dropped {} stored path(s)", dropped));
    println!("Run {} to commit.", style("confect sync").cyan());
    Ok(())
}
