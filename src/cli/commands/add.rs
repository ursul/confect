use console::style;
use std::fs;
use std::path::{Path, PathBuf};

use crate::cli::context::Ctx;
use crate::cli::ui;
use crate::core::paths::{absolute, ensure_trackable};
use crate::error::{ConfectError, Result};
use crate::track::plan::{Action, Scope};

pub fn run(
    explicit_repo: Option<&Path>,
    paths: Vec<PathBuf>,
    category: String,
    create_category: bool,
    encrypt: bool,
) -> Result<()> {
    let mut ctx = Ctx::open(explicit_repo)?;
    let _lock = ctx.repo.lock()?;

    let mut targets = Vec::new();
    for path in paths {
        let path = absolute(&path)?;
        if fs::symlink_metadata(&path).is_err() {
            return Err(ConfectError::FileNotFound(path));
        }
        ensure_trackable(&path, ctx.repo.path())?;
        if let Some(existing) = ctx.categories.find_covering(&path) {
            return Err(if existing.is_excluded(&path) {
                ConfectError::PathExcluded {
                    path,
                    category: existing.name.clone(),
                }
            } else {
                ConfectError::PathAlreadyTracked {
                    path,
                    category: existing.name.clone(),
                }
            });
        }
        let pattern = path.to_string_lossy().into_owned();
        ctx.categories.check_overlap(&pattern)?;
        targets.push((path, pattern));
    }

    if encrypt {
        ctx.crypto.recipients()?;
    }

    if !ctx.categories.exists(&category) {
        if !create_category {
            return Err(ConfectError::Other(format!(
                "category '{}' does not exist; add --create-category to create it (existing: {})",
                category,
                existing_list(&ctx)
            )));
        }
        ctx.categories.create(&category, None)?;
        ui::info(&format!("Creating category '{}'", style(&category).cyan()));
    }
    {
        let cat = ctx.categories.get_mut(&category)?;
        for (_, pattern) in &targets {
            cat.paths.push(pattern.clone());
            if encrypt {
                cat.encrypt.push(pattern.clone());
            }
        }
    }

    let scope = Scope {
        category: Some(category.clone()),
        paths: targets.iter().map(|(path, _)| path.clone()).collect(),
    };
    let plan = ctx.commit_category_change(&scope)?;

    let added: Vec<_> = plan
        .changes
        .iter()
        .filter(|c| c.action == Action::Add)
        .collect();
    ui::success(&format!(
        "Tracking {} path(s) in category '{}'",
        added.len(),
        style(&category).cyan()
    ));
    for change in added.iter().take(50) {
        println!("  {}", ui::change_line(change));
    }
    if added.len() > 50 {
        println!("  ... and {} more", added.len() - 50);
    }
    println!();
    println!("Run {} to commit.", style("confect sync").cyan());
    Ok(())
}

fn existing_list(ctx: &Ctx) -> String {
    let names = ctx.categories.names();
    if names.is_empty() {
        "none".into()
    } else {
        names.join(", ")
    }
}
