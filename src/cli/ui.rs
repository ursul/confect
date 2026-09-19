use console::style;
use std::io::IsTerminal;

use crate::error::{ConfectError, Result};
use crate::track::entry::mode_string;
use crate::track::plan::{Action, Change, SecretFinding};

pub fn success(message: &str) {
    println!("{} {}", style("✓").green().bold(), message);
}

pub fn info(message: &str) {
    println!("{} {}", style("i").cyan().bold(), message);
}

pub fn warn(message: &str) {
    eprintln!("{} {}", style("warning:").yellow().bold(), message);
}

pub fn error_line(message: &str) {
    eprintln!("{} {}", style("error:").red().bold(), message);
}

pub fn print_warnings(warnings: &[String]) {
    for warning in warnings {
        warn(warning);
    }
}

pub fn print_secrets(findings: &[SecretFinding]) {
    eprintln!(
        "{}",
        style("Plaintext secrets would be stored in the repository:")
            .red()
            .bold()
    );
    for finding in findings {
        eprintln!(
            "  {} {} ({}, category '{}')",
            style("!").red().bold(),
            finding.path.display(),
            finding.what,
            finding.category
        );
    }
    eprintln!();
    eprintln!("Store them encrypted:   confect category encrypt add <category> <path>");
    eprintln!("or do not track them:   confect category exclude add <category> <path>");
    eprintln!("or accept the risk:     confect category allow-plaintext add <category> <path>");
}

pub fn change_line(change: &Change) -> String {
    let letter = change.action.letter();
    let colored = match change.action {
        Action::Add => style(letter).green(),
        Action::Update { content: true, .. } => style(letter).yellow(),
        Action::Update { .. } => style(letter).cyan(),
        Action::Delete => style(letter).red(),
        Action::Purge => style(letter).magenta(),
    };
    let mut line = format!("{} {}", colored, change.path.display());
    let is_dir = change
        .system
        .as_ref()
        .map(|s| s.kind == crate::track::Kind::Dir)
        .or_else(|| {
            change
                .stored
                .as_ref()
                .map(|m| m.kind == crate::track::Kind::Dir)
        })
        .unwrap_or(false);
    if is_dir {
        line.push('/');
    }
    if change.encrypt {
        line.push_str(&format!(" {}", style("(encrypted)").dim()));
    }
    if let (Some(system), Some(stored)) = (&change.system, &change.stored) {
        let mut details = Vec::new();
        let stored_mode = stored.mode.clone();
        let system_mode = mode_string(system.mode);
        if system.kind != crate::track::Kind::Symlink
            && stored_mode.as_deref() != Some(system_mode.as_str())
        {
            details.push(format!(
                "mode {} → {}",
                stored_mode.unwrap_or_else(|| "?".into()),
                system_mode
            ));
        }
        if stored.uid != system.uid || stored.gid != system.gid {
            details.push(format!(
                "owner {}:{} → {}:{}",
                stored.owner,
                stored.group,
                crate::track::entry::user_name(system.uid),
                crate::track::entry::group_name(system.gid)
            ));
        }
        if matches!(change.action, Action::Update { .. }) && !details.is_empty() {
            line.push_str(&format!(" {}", style(details.join(", ")).dim()));
        }
    }
    if change.action == Action::Purge {
        line.push_str(&format!(" {}", style("(no longer tracked)").dim()));
    }
    line
}

/// Ask for confirmation; without a terminal the caller must pass --yes.
pub fn confirm(prompt: &str, yes: bool) -> Result<bool> {
    if yes {
        return Ok(true);
    }
    if !std::io::stdin().is_terminal() {
        return Err(ConfectError::Other(
            "confirmation needed but stdin is not a terminal; pass --yes".into(),
        ));
    }
    Ok(dialoguer::Confirm::new()
        .with_prompt(prompt)
        .default(false)
        .interact()?)
}
