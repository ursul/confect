use console::style;
use std::path::{Path, PathBuf};

use crate::cli::commands::diff;
use crate::cli::context::Ctx;
use crate::cli::ui;
use crate::core::paths::absolute;
use crate::error::Result;
use crate::track::plan::Scope;

/// Returns whether anything differs, for `--exit-code`.
pub fn run(
    explicit_repo: Option<&Path>,
    paths: Vec<PathBuf>,
    category: Option<String>,
    show_diff: bool,
) -> Result<bool> {
    let ctx = Ctx::open(explicit_repo)?;
    let scope = Scope {
        category,
        paths: paths.iter().map(|p| absolute(p)).collect::<Result<_>>()?,
    };
    let plan = ctx.plan(&scope)?;

    println!(
        "Repository {} · branch {}",
        style(ctx.repo.path().display()).cyan(),
        style(ctx.repo.branch()).green()
    );
    ui::print_warnings(&plan.warnings);

    if plan.changes.is_empty() && plan.secrets.is_empty() {
        ui::success("The system matches the repository.");
        return Ok(false);
    }

    if !plan.secrets.is_empty() {
        ui::print_secrets(&plan.secrets);
        println!();
    }

    for change in &plan.changes {
        println!("  {}", ui::change_line(change));
        if show_diff {
            if let Some(text) = diff::render(&ctx, change) {
                for line in diff::colored_lines(&text).iter().skip(1) {
                    println!("      {}", line);
                }
            }
        }
    }
    println!();
    println!(
        "{} change(s). A added, M modified, P permissions/owner, D deleted, X no longer tracked.",
        plan.changes.len()
    );
    println!(
        "Run {} to record them or {} to undo them.",
        style("confect sync").cyan(),
        style("confect restore").cyan()
    );
    Ok(true)
}
