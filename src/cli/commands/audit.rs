use console::style;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use walkdir::WalkDir;

use crate::cli::ui;
use crate::core::Repository;
use crate::crypto::{is_age_file, secrets};
use crate::error::Result;

pub struct Finding {
    pub path: String,
    pub what: &'static str,
    pub commits: Vec<String>,
}

/// Returns the number of findings.
pub fn run(explicit_repo: Option<&Path>, history: bool) -> Result<usize> {
    let repo = Repository::open_any_version(explicit_repo)?;
    let findings = if history {
        scan_history(&repo)?
    } else {
        scan_tree(repo.path())
    };
    print(&findings, history);
    Ok(findings.len())
}

/// Plaintext secrets among the files currently in the working tree.
pub fn scan_tree(root: &Path) -> Vec<Finding> {
    let mut findings = Vec::new();
    let walker = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| e.file_name() != ".git");
    for entry in walker.flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(content) = fs::read(entry.path()) else {
            continue;
        };
        if is_age_file(&content) {
            continue;
        }
        if let Some(what) = secrets::detect(&content) {
            let relative = entry
                .path()
                .strip_prefix(root)
                .unwrap_or(entry.path())
                .to_string_lossy()
                .into_owned();
            findings.push(Finding {
                path: relative,
                what,
                commits: Vec::new(),
            });
        }
    }
    findings
}

fn scan_history(repo: &Repository) -> Result<Vec<Finding>> {
    let git = repo.git();
    let mut found: BTreeMap<String, Finding> = BTreeMap::new();
    for commit in git.all_commits()? {
        for path in git.grep_fixed(&commit, secrets::HISTORY_NEEDLES)? {
            let content = git.show_blob(&commit, &path)?;
            if is_age_file(&content) {
                continue;
            }
            if let Some(what) = secrets::detect(&content) {
                found
                    .entry(path.clone())
                    .or_insert_with(|| Finding {
                        path,
                        what,
                        commits: Vec::new(),
                    })
                    .commits
                    .push(commit[..12.min(commit.len())].to_string());
            }
        }
    }
    Ok(found.into_values().collect())
}

fn print(findings: &[Finding], history: bool) {
    if findings.is_empty() {
        ui::success(if history {
            "No plaintext secrets in any commit."
        } else {
            "No plaintext secrets in the current files."
        });
        return;
    }
    println!(
        "{}",
        style(format!(
            "{} file(s) with plaintext secrets{}:",
            findings.len(),
            if history { " in history" } else { "" }
        ))
        .red()
        .bold()
    );
    for finding in findings {
        print!(
            "  {} {} ({})",
            style("!").red().bold(),
            finding.path,
            finding.what
        );
        if !finding.commits.is_empty() {
            let shown: Vec<&str> = finding.commits.iter().take(3).map(String::as_str).collect();
            print!(
                " in {} commit(s): {}{}",
                finding.commits.len(),
                shown.join(", "),
                if finding.commits.len() > 3 {
                    ", ..."
                } else {
                    ""
                }
            );
        }
        println!();
    }
    println!();
    println!("Treat these secrets as disclosed to everyone who can read the repository:");
    println!("rotate them, then encrypt or exclude the files. Removing them from history");
    println!("needs a history rewrite and a force-push, which confect does not do for you.");
}
