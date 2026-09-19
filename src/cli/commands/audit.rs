use console::style;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use walkdir::WalkDir;

use crate::cli::ui;
use crate::core::{CategoryManager, Repository};
use crate::crypto::{is_age_file, secrets};
use crate::error::Result;

pub struct Finding {
    pub path: String,
    pub what: &'static str,
    pub commits: Vec<String>,
    /// Covered by an `allow_plaintext` pattern of its category.
    pub allowed: bool,
}

/// Whether a repository path ("category/etc/...") is allowed in plaintext on purpose.
fn allowed(categories: Option<&CategoryManager>, repo_path: &str) -> bool {
    let Some(categories) = categories else {
        return false;
    };
    let Some((category, rest)) = repo_path.split_once('/') else {
        return false;
    };
    categories
        .get(category)
        .is_ok_and(|c| c.allows_plaintext(&Path::new("/").join(rest)))
}

/// Returns the number of findings that are not allowed on purpose.
pub fn run(explicit_repo: Option<&Path>, history: bool) -> Result<usize> {
    let repo = Repository::open_any_version(explicit_repo)?;
    let categories = repo.categories().ok();
    let findings = if history {
        scan_history(&repo, categories.as_ref())?
    } else {
        scan_tree(repo.path(), categories.as_ref())
    };
    print(&findings, history);
    Ok(findings.iter().filter(|f| !f.allowed).count())
}

/// Plaintext secrets among the files currently in the working tree.
pub fn scan_tree(root: &Path, categories: Option<&CategoryManager>) -> Vec<Finding> {
    let mut findings = Vec::new();
    let walker = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| e.file_name() != ".git");
    for entry in walker.flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(mut file) = fs::File::open(entry.path()) else {
            continue;
        };
        let mut head = [0u8; 64];
        let read = std::io::Read::read(&mut file, &mut head).unwrap_or(0);
        if is_age_file(&head[..read]) {
            continue;
        }
        let found = secrets::detect_name(entry.path()).or_else(|| {
            fs::File::open(entry.path())
                .ok()
                .and_then(|f| secrets::detect_reader(f).ok().flatten())
        });
        if let Some(what) = found {
            let relative = entry
                .path()
                .strip_prefix(root)
                .unwrap_or(entry.path())
                .to_string_lossy()
                .into_owned();
            findings.push(Finding {
                allowed: allowed(categories, &relative),
                path: relative,
                what,
                commits: Vec::new(),
            });
        }
    }
    findings
}

fn scan_history(repo: &Repository, categories: Option<&CategoryManager>) -> Result<Vec<Finding>> {
    let git = repo.git();
    let mut found: BTreeMap<String, Finding> = BTreeMap::new();
    for path in git.all_paths_in_history()? {
        if let Some(what) = secrets::detect_name(Path::new(&path)) {
            let commits = git.commits_touching(&path)?;
            found.insert(
                path.clone(),
                Finding {
                    allowed: allowed(categories, &path),
                    path,
                    what,
                    commits,
                },
            );
        }
    }
    let named: std::collections::HashSet<String> = found.keys().cloned().collect();
    for commit in git.all_commits()? {
        for path in git.grep_fixed(&commit, secrets::HISTORY_NEEDLES)? {
            if named.contains(&path) {
                continue;
            }
            let content = git.show_blob(&commit, &path)?;
            if is_age_file(&content) {
                continue;
            }
            if let Some(what) = secrets::detect(&content) {
                found
                    .entry(path.clone())
                    .or_insert_with(|| Finding {
                        allowed: allowed(categories, &path),
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
    let (allowed, flagged): (Vec<&Finding>, Vec<&Finding>) =
        findings.iter().partition(|f| f.allowed);
    if !allowed.is_empty() {
        println!("Allowed in plaintext on purpose (allow_plaintext):");
        for finding in &allowed {
            println!("  {} ({})", finding.path, finding.what);
        }
        println!();
    }
    if flagged.is_empty() {
        ui::success(if history {
            "No other plaintext secrets in any commit."
        } else {
            "No other plaintext secrets in the current files."
        });
        return;
    }
    println!(
        "{}",
        style(format!(
            "{} file(s) with plaintext secrets{}:",
            flagged.len(),
            if history { " in history" } else { "" }
        ))
        .red()
        .bold()
    );
    for finding in flagged {
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
