use console::style;
use std::collections::BTreeMap;
use std::path::Path;

use crate::cli::context::{guard_secrets, report_failures, Ctx};
use crate::cli::ui;
use crate::error::Result;
use crate::track::plan::{self, Scope};
use crate::track::Store;

pub fn run(
    explicit_repo: Option<&Path>,
    message: Option<String>,
    no_push: bool,
    force_push: bool,
) -> Result<()> {
    let mut ctx = Ctx::open(explicit_repo)?;
    let _lock = ctx.repo.lock()?;

    if ctx.repo.ensure_private()? {
        ui::warn(&format!(
            "{} was readable by other users; set it to 0700",
            ctx.repo.path().display()
        ));
    }

    let plan = ctx.plan(&Scope::default())?;
    ui::print_warnings(&plan.warnings);
    guard_secrets(&plan)?;

    let failures = {
        let store = Store::new(ctx.repo.path(), &ctx.crypto);
        plan::apply(&plan, &store, &mut ctx.metadata)?
    };
    ctx.metadata.save()?;
    report_failures(&failures)?;

    let git = ctx.repo.git();
    git.add_all()?;
    if git.has_staged_changes()? {
        let staged = git.staged()?;
        let message = message.unwrap_or_else(|| summary(&staged));
        git.commit(&message, ctx.repo.host())?;
        ui::success(&format!("Committed: {}", style(&message).italic()));
    } else {
        ui::success("Nothing to commit; the repository already matches the system.");
    }

    let push = force_push || (ctx.repo.config().global.auto_push && !no_push);
    if !push {
        return Ok(());
    }
    let remote = ctx.repo.remote().to_string();
    if git.remote_url(&remote)?.is_none() {
        if force_push {
            return Err(crate::error::ConfectError::Other(format!(
                "no remote '{}' configured",
                remote
            )));
        }
        return Ok(());
    }
    let outcome = git.push(&remote, &ctx.repo.branch())?;
    if outcome.up_to_date {
        ui::success(&format!("{} is up to date", remote));
    } else {
        ui::success(&format!("Pushed {} to {}", ctx.repo.branch(), remote));
    }
    Ok(())
}

/// Commit message such as "Sync nginx (M2), runtime (A1 D1)", from what git stages.
fn summary(staged: &[(char, String)]) -> String {
    let mut per_category: BTreeMap<&str, BTreeMap<char, usize>> = BTreeMap::new();
    for (letter, path) in staged {
        let category = path.split('/').next().unwrap_or(path);
        if category.starts_with('.') {
            continue;
        }
        *per_category
            .entry(category)
            .or_default()
            .entry(*letter)
            .or_default() += 1;
    }
    if per_category.is_empty() {
        return "Update confect configuration".to_string();
    }
    let parts: Vec<String> = per_category
        .into_iter()
        .map(|(category, counts)| {
            let counts: Vec<String> = counts
                .into_iter()
                .map(|(letter, count)| format!("{}{}", letter, count))
                .collect();
            format!("{} ({})", category, counts.join(" "))
        })
        .collect();
    format!("Sync {}", parts.join(", "))
}
