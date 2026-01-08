use anyhow::{bail, Result};
use console::style;
use self_update::backends::github::{ReleaseList, Update};
use self_update::cargo_crate_version;

const REPO_OWNER: &str = "ursul";
const REPO_NAME: &str = "confect";

fn get_target() -> Result<&'static str> {
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;

    match (os, arch) {
        ("linux", "x86_64") => Ok("linux-x86_64"),
        ("linux", "aarch64") => Ok("linux-aarch64"),
        _ => bail!("Unsupported platform: {}-{}", os, arch),
    }
}

pub fn run_self_update(check: bool) -> Result<()> {
    let current_version = cargo_crate_version!();
    let target = get_target()?;

    println!(
        "{} Current version: {}",
        style("i").cyan().bold(),
        current_version
    );
    println!("{} Checking for updates...", style("i").cyan().bold());

    let releases = ReleaseList::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .build()?
        .fetch()?;

    let latest = releases.first();
    let latest_version = match latest {
        Some(release) => release.version.trim_start_matches('v'),
        None => {
            println!("{} No releases found", style("!").yellow().bold());
            return Ok(());
        }
    };

    if latest_version == current_version {
        println!(
            "{} Already up to date ({})",
            style("✓").green().bold(),
            current_version
        );
        return Ok(());
    }

    println!(
        "{} New version available: {} -> {}",
        style("i").cyan().bold(),
        current_version,
        latest_version
    );

    if check {
        return Ok(());
    }

    println!("{} Downloading update...", style("i").cyan().bold());

    let status = Update::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name("confect")
        .target(target)
        .current_version(current_version)
        .build()?
        .update()?;

    if status.updated() {
        println!(
            "{} Updated to version {}",
            style("✓").green().bold(),
            status.version()
        );
    } else {
        println!("{} Update failed", style("✗").red().bold());
    }

    Ok(())
}
