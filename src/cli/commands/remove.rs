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
    for raw in paths {
        // A relative 1.x category path can only be matched as typed.
        let typed = raw.to_string_lossy().into_owned();
        let relative_owner = ctx
            .categories
            .list()
            .find(|c| !typed.starts_with('/') && c.paths.contains(&typed))
            .map(|c| c.name.clone());
        if let Some(name) = relative_owner {
            ctx.categories.get_mut(&name)?.paths.retain(|p| p != &typed);
            ctx.categories.save()?;
            println!("  {} {} (removed from '{}')", style("-").red(), typed, name);
            continue;
        }
        let path = absolute(&raw)?;
        let pattern = path.to_string_lossy().into_owned();

        let owner = ctx
            .categories
            .list()
            .find(|c| c.paths.contains(&pattern))
            .map(|c| c.name.clone());
        if let Some(name) = owner {
            let cat = ctx.categories.get_mut(&name)?;
            cat.paths.retain(|p| p != &pattern);
            let below = |p: &String| p.starts_with('/') && Path::new(p).starts_with(&path);
            cat.encrypt.retain(|p| !below(p));
            cat.allow_plaintext.retain(|p| !below(p));
            cat.exclude.retain(|p| !below(p));
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
            ..Scope::default()
        })?;
        dropped += plan.changes.len();
    }
    ui::success(&format!("Dropped {} stored path(s)", dropped));
    println!("Run {} to commit.", style("confect sync").cyan());
    Ok(())
}
