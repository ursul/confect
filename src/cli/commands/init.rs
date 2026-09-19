use console::style;
use std::path::{Path, PathBuf};

use crate::cli::args::InitArgs;
use crate::cli::ui;
use crate::core::config::SYSTEM_REPO_PATH;
use crate::core::paths::absolute;
use crate::core::repository::default_host;
use crate::core::{Config, Repository};
use crate::error::{ConfectError, Result};

pub fn run(args: InitArgs, explicit_repo: Option<&Path>) -> Result<()> {
    let mut config = Config::load()?;
    let path = match (&args.path, args.system, explicit_repo) {
        (Some(path), _, _) => absolute(path)?,
        (None, true, _) => PathBuf::from(SYSTEM_REPO_PATH),
        (None, false, Some(path)) => absolute(path)?,
        (None, false, None) => config.repo_path(),
    };
    let host = args.host.clone().unwrap_or_else(default_host);
    for url in [&args.remote, &args.from].into_iter().flatten() {
        if url.starts_with('-') || url.is_empty() {
            return Err(ConfectError::Other(format!(
                "'{}' is not a repository URL",
                url
            )));
        }
    }
    if config.global.default_remote.starts_with('-') {
        return Err(ConfectError::Config(format!(
            "default_remote '{}' is not a remote name",
            config.global.default_remote
        )));
    }

    if path.join(".git").exists() {
        add_remote_to_existing(&path, args.remote.as_deref())?;
        if args.path.is_some() || args.system {
            config.global.repo_path = Some(path);
            config.save()?;
        }
        return Ok(());
    }

    let repo = if let Some(url) = &args.from {
        println!(
            "Cloning {} into {}",
            style(url).cyan(),
            style(path.display()).cyan()
        );
        let (repo, existing) = Repository::clone_from(url, &path, &host, config.clone())?;
        if existing {
            ui::success(&format!(
                "Checked out branch {} from the remote",
                style(repo.branch()).green()
            ));
        } else {
            repo.git().add_all()?;
            repo.git()
                .commit(&format!("Start configuration of {}", host), &host)?;
            ui::success(&format!(
                "The remote has no branch {} yet; started it empty",
                style(repo.branch()).green()
            ));
        }
        repo
    } else {
        let repo = Repository::create(&path, &host, config.clone())?;
        repo.git().add_all()?;
        repo.git().commit(
            &format!("Initialize confect repository for {}", host),
            &host,
        )?;
        if let Some(url) = &args.remote {
            repo.git().add_remote(repo.remote(), url)?;
        }
        ui::success(&format!(
            "Created repository at {} on branch {}",
            style(path.display()).cyan(),
            style(repo.branch()).green()
        ));
        repo
    };

    if path != Config::default_repo_path() {
        config.global.repo_path = Some(path.clone());
    }
    config.save()?;

    if let Some((name, url)) = repo.git().remotes()?.into_iter().next() {
        println!("Remote {}: {}", style(name).cyan(), url);
    }
    println!();
    println!("Next steps:");
    if args.from.is_some() {
        println!("  confect restore --dry-run    # see what would be written to this system");
        println!("  confect restore              # write it");
    } else {
        println!("  confect add /etc/nginx -c nginx --create-category");
        println!("  confect sync -m \"Initial configuration\"");
    }
    Ok(())
}

fn add_remote_to_existing(path: &Path, remote: Option<&str>) -> Result<()> {
    let Some(url) = remote else {
        return Err(ConfectError::AlreadyInitialized(path.to_path_buf()));
    };
    let repo = Repository::open_any_version(Some(path))?;
    let name = repo.remote().to_string();
    if let Some(existing) = repo.git().remote_url(&name)? {
        return Err(ConfectError::Other(format!(
            "remote {} already points to {}; change it with: git -C {} remote set-url {} <url>",
            name,
            existing,
            path.display(),
            name
        )));
    }
    repo.git().add_remote(&name, url)?;
    ui::success(&format!("Added remote {}: {}", name, url));
    Ok(())
}
