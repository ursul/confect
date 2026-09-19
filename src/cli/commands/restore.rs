use console::style;
use std::path::{Path, PathBuf};

use crate::cli::context::Ctx;
use crate::cli::ui;
use crate::core::paths::absolute;
use crate::error::{ConfectError, Result};
use crate::track::restore::{self, RestoreState};

pub struct Options {
    pub paths: Vec<PathBuf>,
    pub category: Option<String>,
    pub dry_run: bool,
    pub yes: bool,
    pub backup: bool,
}

pub fn run(explicit_repo: Option<&Path>, options: Options) -> Result<()> {
    let ctx = Ctx::open(explicit_repo)?;
    let _lock = ctx.repo.lock()?;
    restore_with(&ctx, options)
}

pub fn restore_with(ctx: &Ctx, options: Options) -> Result<()> {
    if let Some(name) = &options.category {
        ctx.categories.get(name)?;
    }
    let scope: Vec<PathBuf> = options
        .paths
        .iter()
        .map(|p| absolute(p))
        .collect::<Result<_>>()?;

    let selected: Vec<PathBuf> = ctx
        .metadata
        .iter()
        .filter(|(_, meta)| {
            options
                .category
                .as_deref()
                .map_or(true, |c| meta.category == c)
        })
        .filter(|(path, _)| scope.is_empty() || scope.iter().any(|s| path.starts_with(s)))
        .map(|(path, _)| path.clone())
        .collect();
    for requested in &scope {
        if !selected.iter().any(|p| p.starts_with(requested)) {
            return Err(ConfectError::PathNotTracked(requested.clone()));
        }
    }

    let store = ctx.store();
    let prepared = restore::prepare(&ctx.metadata, &store, &selected);
    let mut items = Vec::new();
    let mut unchanged = 0;
    for (item, problem) in prepared {
        if let Some(problem) = problem {
            ui::warn(&format!("{}: {}", item.path.display(), problem));
        }
        if item.state == RestoreState::Unchanged {
            unchanged += 1;
        } else {
            items.push(item);
        }
    }

    if items.is_empty() {
        ui::success(&format!(
            "Nothing to restore; {} tracked path(s) already match the repository.",
            unchanged
        ));
        return Ok(());
    }

    for item in &items {
        let (marker, note) = match item.state {
            RestoreState::Create => (style("+").green(), "create"),
            RestoreState::Overwrite => (style("~").yellow(), "overwrite"),
            RestoreState::Attributes => (style("p").cyan(), "permissions/owner"),
            RestoreState::Unchanged => (style("=").dim(), "unchanged"),
        };
        let encrypted = if item.meta.encrypted {
            " (encrypted)"
        } else {
            ""
        };
        println!(
            "  {} {} {}{}",
            marker,
            item.path.display(),
            style(note).dim(),
            style(encrypted).dim()
        );
    }
    println!(
        "{} path(s) to restore, {} unchanged.",
        items.len(),
        unchanged
    );

    if options.dry_run {
        println!("{}", style("Dry run: nothing was changed.").dim());
        return Ok(());
    }
    if !ui::confirm("Write these files to the system?", options.yes)? {
        return Err(ConfectError::Aborted);
    }

    let report = restore::restore(&items, &store, options.backup);
    ui::print_warnings(&report.warnings);
    for backup in &report.backups {
        println!("  backup: {}", backup.display());
    }
    if report.failures.is_empty() {
        ui::success(&format!("Restored {} path(s).", report.restored));
        Ok(())
    } else {
        for (path, err) in &report.failures {
            ui::error_line(&format!("{}: {}", path.display(), err));
        }
        Err(ConfectError::Other(format!(
            "restored {} path(s), {} failed",
            report.restored,
            report.failures.len()
        )))
    }
}
