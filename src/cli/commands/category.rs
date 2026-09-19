use console::style;
use std::path::{Path, PathBuf};

use crate::cli::args::{CategoryCommands, PatternCommands};
use crate::cli::context::Ctx;
use crate::cli::ui;
use crate::core::category::validate_name;
use crate::core::paths::{absolute_pattern, ensure_trackable, glob_base};
use crate::error::{ConfectError, Result};
use crate::track::plan::Scope;

pub fn run(explicit_repo: Option<&Path>, command: CategoryCommands) -> Result<()> {
    let mut ctx = Ctx::open(explicit_repo)?;
    match command {
        CategoryCommands::List => list(&ctx),
        CategoryCommands::Show { name } => show(&ctx, &name),
        CategoryCommands::Create {
            name,
            path,
            description,
            encrypt,
            exclude,
            allow_plaintext,
        } => {
            let _lock = ctx.repo.lock()?;
            create(
                &mut ctx,
                &name,
                path,
                description,
                Patterns {
                    encrypt,
                    exclude,
                    allow_plaintext,
                },
            )
        }
        CategoryCommands::Delete { name, yes } => {
            let _lock = ctx.repo.lock()?;
            delete(&mut ctx, &name, yes)
        }
        CategoryCommands::AddPath { name, path } => {
            let _lock = ctx.repo.lock()?;
            let pattern = checked_path_pattern(&ctx, &path)?;
            ctx.categories.get(&name)?;
            ctx.categories.check_overlap(&pattern)?;
            ctx.categories.get_mut(&name)?.paths.push(pattern.clone());
            ctx.commit_category_change(&scope_for(&name, &pattern))?;
            ui::success(&format!("Added {} to '{}'", pattern, name));
            Ok(())
        }
        CategoryCommands::RemovePath { name, path } => {
            let _lock = ctx.repo.lock()?;
            let pattern = absolute_pattern(&path)?;
            let cat = ctx.categories.get_mut(&name)?;
            if !cat.paths.contains(&pattern) {
                return Err(ConfectError::Other(format!(
                    "'{}' has no path {}",
                    name, pattern
                )));
            }
            cat.paths.retain(|p| p != &pattern);
            let plan = ctx.commit_category_change(&scope_for(&name, &pattern))?;
            ui::success(&format!(
                "Removed {} from '{}' and dropped {} stored path(s)",
                pattern,
                name,
                plan.changes.len()
            ));
            Ok(())
        }
        CategoryCommands::Exclude(command) => {
            let _lock = ctx.repo.lock()?;
            edit_patterns(&mut ctx, command, PatternList::Exclude)
        }
        CategoryCommands::Encrypt(command) => {
            let _lock = ctx.repo.lock()?;
            if matches!(command, PatternCommands::Add { .. }) {
                ctx.crypto.recipients()?;
            }
            edit_patterns(&mut ctx, command, PatternList::Encrypt)
        }
        CategoryCommands::AllowPlaintext(command) => {
            let _lock = ctx.repo.lock()?;
            edit_patterns(&mut ctx, command, PatternList::AllowPlaintext)
        }
    }
}

fn scope_for(name: &str, pattern: &str) -> Scope {
    Scope {
        category: Some(name.to_string()),
        paths: vec![glob_base(pattern)],
    }
}

fn checked_path_pattern(ctx: &Ctx, input: &str) -> Result<String> {
    let pattern = absolute_pattern(input)?;
    ensure_trackable(&glob_base(&pattern), ctx.repo.path())?;
    Ok(pattern)
}

/// Patterns without `/` are name patterns and stay as typed; paths become absolute.
fn normalize_pattern(input: &str) -> Result<String> {
    if input.contains('/') || input.starts_with('~') {
        absolute_pattern(input)
    } else {
        Ok(input.to_string())
    }
}

#[derive(Clone, Copy)]
enum PatternList {
    Exclude,
    Encrypt,
    AllowPlaintext,
}

impl PatternList {
    fn label(self) -> &'static str {
        match self {
            PatternList::Exclude => "exclusions",
            PatternList::Encrypt => "encrypted patterns",
            PatternList::AllowPlaintext => "plaintext allowances",
        }
    }
}

fn edit_patterns(ctx: &mut Ctx, command: PatternCommands, list: PatternList) -> Result<()> {
    let (name, pattern, add) = match command {
        PatternCommands::Add { name, pattern } => (name, pattern, true),
        PatternCommands::Remove { name, pattern } => (name, pattern, false),
    };
    let pattern = normalize_pattern(&pattern)?;
    let cat = ctx.categories.get_mut(&name)?;
    let patterns = match list {
        PatternList::Exclude => &mut cat.exclude,
        PatternList::Encrypt => &mut cat.encrypt,
        PatternList::AllowPlaintext => &mut cat.allow_plaintext,
    };
    if add {
        if patterns.contains(&pattern) {
            return Err(ConfectError::Other(format!(
                "{} is already in the {} of '{}'",
                pattern,
                list.label(),
                name
            )));
        }
        patterns.push(pattern.clone());
    } else {
        let before = patterns.len();
        patterns.retain(|p| p != &pattern);
        if patterns.len() == before {
            return Err(ConfectError::Other(format!(
                "{} is not in the {} of '{}'",
                pattern,
                list.label(),
                name
            )));
        }
    }

    let scope = if pattern.contains('/') {
        scope_for(&name, &pattern)
    } else {
        Scope {
            category: Some(name.clone()),
            paths: Vec::new(),
        }
    };
    let plan = ctx.commit_category_change(&scope)?;
    ui::success(&format!(
        "{} {} {} the {} of '{}'; {} stored path(s) updated",
        if add { "Added" } else { "Removed" },
        pattern,
        if add { "to" } else { "from" },
        list.label(),
        name,
        plan.changes.len()
    ));
    Ok(())
}

fn list(ctx: &Ctx) -> Result<()> {
    let mut any = false;
    for category in ctx.categories.list() {
        any = true;
        let files = ctx.metadata.in_category(&category.name).count();
        println!(
            "{} {} {}",
            style(&category.name).cyan().bold(),
            style(format!("({} stored)", files)).dim(),
            category.description.as_deref().unwrap_or("")
        );
    }
    if !any {
        println!(
            "No categories yet. Create one with 'confect add <path> -c <name> --create-category'."
        );
    }
    Ok(())
}

fn show(ctx: &Ctx, name: &str) -> Result<()> {
    let category = ctx.categories.get(name)?;
    println!("{}", style(&category.name).cyan().bold());
    if let Some(description) = &category.description {
        println!("{}", description);
    }
    let sections: [(&str, &Vec<String>); 4] = [
        ("Paths", &category.paths),
        ("Excluded", &category.exclude),
        ("Encrypted", &category.encrypt),
        ("Allowed in plaintext", &category.allow_plaintext),
    ];
    for (title, patterns) in sections {
        if !patterns.is_empty() {
            println!("{}:", style(title).bold());
            for pattern in patterns {
                println!("  {}", pattern);
            }
        }
    }
    let stored: Vec<(&PathBuf, _)> = ctx.metadata.in_category(name).collect();
    println!("{}: {}", style("Stored").bold(), stored.len());
    for (path, meta) in stored.iter().take(200) {
        let suffix = if meta.encrypted { " (encrypted)" } else { "" };
        println!(
            "  {} {} {}:{}{}",
            meta.mode.as_deref().unwrap_or("    "),
            path.display(),
            meta.owner,
            meta.group,
            suffix
        );
    }
    if stored.len() > 200 {
        println!("  ... and {} more", stored.len() - 200);
    }
    Ok(())
}

struct Patterns {
    encrypt: Vec<String>,
    exclude: Vec<String>,
    allow_plaintext: Vec<String>,
}

fn normalize_all(patterns: &[String]) -> Result<Vec<String>> {
    patterns.iter().map(|p| normalize_pattern(p)).collect()
}

fn create(
    ctx: &mut Ctx,
    name: &str,
    paths: Vec<String>,
    description: Option<String>,
    patterns: Patterns,
) -> Result<()> {
    validate_name(name)?;
    let mut checked = Vec::new();
    for path in &paths {
        let pattern = checked_path_pattern(ctx, path)?;
        ctx.categories.check_overlap(&pattern)?;
        checked.push(pattern);
    }
    let encrypt = normalize_all(&patterns.encrypt)?;
    if !encrypt.is_empty() {
        ctx.crypto.recipients()?;
    }
    {
        let category = ctx.categories.create(name, description)?;
        category.paths = checked;
        category.encrypt = encrypt;
        category.exclude = normalize_all(&patterns.exclude)?;
        category.allow_plaintext = normalize_all(&patterns.allow_plaintext)?;
    }
    ctx.commit_category_change(&Scope {
        category: Some(name.to_string()),
        paths: Vec::new(),
    })?;
    ui::success(&format!("Created category '{}'", name));
    println!("Run {} to commit.", style("confect sync").cyan());
    Ok(())
}

fn delete(ctx: &mut Ctx, name: &str, yes: bool) -> Result<()> {
    let stored: Vec<PathBuf> = ctx
        .metadata
        .in_category(name)
        .map(|(path, _)| path.clone())
        .collect();
    ctx.categories.get(name)?;
    if !ui::confirm(
        &format!(
            "Delete category '{}' and its {} stored path(s)? System files are not touched.",
            name,
            stored.len()
        ),
        yes,
    )? {
        return Err(ConfectError::Aborted);
    }
    ctx.categories.remove(name)?;
    ctx.categories.save()?;
    ctx.store().remove_category(name)?;
    for path in &stored {
        ctx.metadata.remove(path);
    }
    ctx.metadata.save()?;
    ui::success(&format!(
        "Deleted category '{}' ({} stored path(s) dropped)",
        name,
        stored.len()
    ));
    println!("Run {} to commit.", style("confect sync").cyan());
    Ok(())
}
