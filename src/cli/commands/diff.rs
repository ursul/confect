use console::style;
use similar::TextDiff;
use std::fs;
use std::path::{Path, PathBuf};

use crate::cli::context::Ctx;
use crate::core::paths::absolute;
use crate::error::Result;
use crate::track::plan::{Action, Change, Scope};
use crate::track::Kind;

/// Files larger than this are not diffed line by line.
const DIFF_LIMIT: u64 = 8 * 1024 * 1024;

pub fn run(
    explicit_repo: Option<&Path>,
    paths: Vec<PathBuf>,
    category: Option<String>,
) -> Result<()> {
    let ctx = Ctx::open(explicit_repo)?;
    let scope = Scope {
        category,
        paths: paths.iter().map(|p| absolute(p)).collect::<Result<_>>()?,
        ..Scope::default()
    };
    let plan = ctx.plan(&scope)?;
    crate::cli::ui::print_warnings(&plan.warnings);

    let mut shown = 0;
    for change in &plan.changes {
        if let Some(text) = render(&ctx, change) {
            print_colored(&text);
            shown += 1;
        }
    }
    if shown == 0 {
        println!("No differences.");
    }
    Ok(())
}

/// Unified diff from the stored copy (`---`) to the system (`+++`), plus attribute changes.
pub fn render(ctx: &Ctx, change: &Change) -> Option<String> {
    let path = &change.path;
    let label = path.display().to_string();
    let mut out = String::new();

    let kind = change
        .system
        .as_ref()
        .map(|s| s.kind)
        .or_else(|| change.stored.as_ref().map(|m| m.kind))?;
    let content_changed = matches!(
        change.action,
        Action::Add | Action::Delete | Action::Update { content: true, .. }
    );

    if content_changed && change.action != Action::Purge {
        match kind {
            Kind::Dir => {}
            Kind::Symlink => {
                let old = change
                    .stored
                    .as_ref()
                    .and_then(|m| ctx.store().read_link(path, m).ok());
                let new = change.system.as_ref().and_then(|s| s.target.clone());
                out.push_str(&format!(
                    "--- repo:{}\n+++ system:{}\n-> {}\n+> {}\n",
                    label,
                    label,
                    old.map(|p| p.display().to_string())
                        .unwrap_or_else(|| "(none)".into()),
                    new.map(|p| p.display().to_string())
                        .unwrap_or_else(|| "(none)".into()),
                ));
            }
            Kind::File => out.push_str(&file_diff(ctx, change, &label)),
        }
    }

    if let (Some(system), Some(stored)) = (&change.system, &change.stored) {
        if system.kind != Kind::Symlink {
            let new_mode = crate::track::entry::mode_string(system.mode);
            if stored.mode.as_deref() != Some(new_mode.as_str()) {
                out.push_str(&format!(
                    "mode {} → {}\n",
                    stored.mode.as_deref().unwrap_or("?"),
                    new_mode
                ));
            }
        }
        if stored.uid != system.uid || stored.gid != system.gid {
            out.push_str(&format!(
                "owner {}:{} → {}:{}\n",
                stored.owner,
                stored.group,
                crate::track::entry::user_name(system.uid),
                crate::track::entry::group_name(system.gid)
            ));
        }
    }

    if out.is_empty() {
        None
    } else {
        Some(format!("{} {}\n{}", style("diff").bold(), label, out))
    }
}

fn file_diff(ctx: &Ctx, change: &Change, label: &str) -> String {
    let too_big = |size: u64| size > DIFF_LIMIT;
    if change.system.as_ref().is_some_and(|s| too_big(s.size)) {
        return format!("{}: file too large to diff\n", label);
    }

    let old = match &change.stored {
        Some(meta) if change.action != Action::Add => match ctx.store().read(&change.path, meta) {
            Ok(content) => content,
            Err(err) => return format!("{}: stored copy unavailable: {}\n", label, err),
        },
        _ => Vec::new(),
    };
    let new = if change.action == Action::Delete {
        Vec::new()
    } else {
        match fs::read(&change.path) {
            Ok(content) => content,
            Err(err) => return format!("{}: cannot read: {}\n", label, err),
        }
    };

    if old.contains(&0) || new.contains(&0) {
        return format!("Binary files repo:{0} and system:{0} differ\n", label);
    }
    let old = String::from_utf8_lossy(&old);
    let new = String::from_utf8_lossy(&new);
    let diff = TextDiff::from_lines(old.as_ref(), new.as_ref());
    diff.unified_diff()
        .context_radius(3)
        .header(&format!("repo:{}", label), &format!("system:{}", label))
        .to_string()
}

pub fn print_colored(text: &str) {
    for line in colored_lines(text) {
        println!("{}", line);
    }
    println!();
}

pub fn colored_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(|line| {
            if line.starts_with("+++") || line.starts_with("---") {
                style(line).bold().to_string()
            } else if line.starts_with('+') {
                style(line).green().to_string()
            } else if line.starts_with('-') {
                style(line).red().to_string()
            } else if line.starts_with("@@") {
                style(line).cyan().to_string()
            } else {
                line.to_string()
            }
        })
        .collect()
}
