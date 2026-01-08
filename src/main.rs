use anyhow::Result;
use clap::Parser;
use console::style;

use confect::cli::commands;
use confect::cli::{Cli, Commands};

fn main() {
    if let Err(err) = run() {
        eprintln!("{} {}", style("Error:").red().bold(), err);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init {
            path,
            system,
            remote,
            host,
        } => {
            commands::run_init(path, system, remote, host)?;
        }

        Commands::Add {
            path,
            category,
            create_category,
            encrypt,
        } => {
            commands::run_add(path, category, create_category, encrypt)?;
        }

        Commands::Remove { path, delete } => {
            commands::run_remove(path, delete)?;
        }

        Commands::Status { category, diff } => {
            commands::run_status(category, diff)?;
        }

        Commands::Sync {
            message,
            no_push,
            all_hosts,
        } => {
            commands::run_sync(message, no_push, all_hosts)?;
        }

        Commands::Restore {
            category,
            file,
            dry_run,
            force,
            backup,
        } => {
            commands::run_restore(category, file, dry_run, force, backup)?;
        }

        Commands::Category(cmd) => {
            commands::run_category(cmd)?;
        }

        Commands::Info => {
            commands::run_info()?;
        }

        Commands::SetupTimer { schedule, remove } => {
            setup_timer(&schedule, remove)?;
        }

        Commands::Pull { restore } => {
            pull_changes(restore)?;
        }

        Commands::Diff { category, file } => {
            commands::run_diff(category, file)?;
        }

        Commands::SelfUpdate { check } => {
            commands::run_self_update(check)?;
        }
    }

    Ok(())
}

fn setup_timer(schedule: &str, remove: bool) -> Result<()> {
    use std::fs;
    use std::path::PathBuf;

    let service_path = PathBuf::from("/etc/systemd/system/confect-backup.service");
    let timer_path = PathBuf::from("/etc/systemd/system/confect-backup.timer");

    if remove {
        // Remove timer
        if timer_path.exists() {
            fs::remove_file(&timer_path)?;
        }
        if service_path.exists() {
            fs::remove_file(&service_path)?;
        }
        println!("{} Removed confect-backup timer", style("✓").green());
        println!("Run: sudo systemctl daemon-reload");
        return Ok(());
    }

    // Get confect binary path
    let confect_path = std::env::current_exe()?;

    // Create service
    let service_content = format!(
        r#"[Unit]
Description=Confect configuration backup
After=network-online.target
Wants=network-online.target

[Service]
Type=oneshot
ExecStart={} sync -m "Automatic backup"
User=root

[Install]
WantedBy=multi-user.target
"#,
        confect_path.display()
    );

    // Create timer
    let timer_content = format!(
        r#"[Unit]
Description=Timer for confect configuration backup

[Timer]
OnCalendar={}
Persistent=true
RandomizedDelaySec=300

[Install]
WantedBy=timers.target
"#,
        schedule
    );

    fs::write(&service_path, service_content)?;
    fs::write(&timer_path, timer_content)?;

    println!("{} Created systemd timer", style("✓").green());
    println!();
    println!("To enable the timer, run:");
    println!("  sudo systemctl daemon-reload");
    println!("  sudo systemctl enable --now confect-backup.timer");
    println!();
    println!("To check timer status:");
    println!("  systemctl status confect-backup.timer");

    Ok(())
}

fn pull_changes(restore: bool) -> Result<()> {
    use confect::core::Repository;

    let repo = Repository::open_default()?;

    println!("{} Pulling from remote...", style("[1/2]").bold().dim());

    repo.pull("origin")?;

    println!("{} Pulled latest changes", style("✓").green());

    if restore {
        println!("{} Restoring files...", style("[2/2]").bold().dim());
        commands::run_restore(None, None, false, true, true)?;
    }

    Ok(())
}
