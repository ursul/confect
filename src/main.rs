use clap::Parser;
use console::style;

use confect::cli::commands::{
    add, audit, category, diff, info, init, key, migrate, remote, remove, restore, self_update,
    status, sync, timer,
};
use confect::cli::{Cli, Commands};
use confect::Result;

/// Exit status when `status --exit-code` or `audit` found something.
const EXIT_FOUND: i32 = 2;

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            eprintln!("{} {}", style("Error:").red().bold(), err);
            std::process::exit(1);
        }
    }
}

fn run() -> Result<i32> {
    let cli = Cli::parse();
    let repo = cli.repo.as_deref();

    match cli.command {
        Commands::Init(args) => init::run(args, repo)?,
        Commands::Add {
            paths,
            category,
            create_category,
            encrypt,
        } => add::run(repo, paths, category, create_category, encrypt)?,
        Commands::Remove { paths } => remove::run(repo, paths)?,
        Commands::Status {
            paths,
            category,
            diff,
            exit_code,
        } => {
            let differs = status::run(repo, paths, category, diff)?;
            if exit_code && differs {
                return Ok(EXIT_FOUND);
            }
        }
        Commands::Diff { paths, category } => diff::run(repo, paths, category)?,
        Commands::Sync {
            message,
            no_push,
            push,
        } => sync::run(repo, message, no_push, push)?,
        Commands::Restore {
            paths,
            category,
            dry_run,
            yes,
            backup,
        } => restore::run(
            repo,
            restore::Options {
                paths,
                category,
                dry_run,
                yes,
                backup,
            },
        )?,
        Commands::Pull { restore, yes } => remote::pull(repo, restore, yes)?,
        Commands::Push => remote::push(repo)?,
        Commands::Category(command) => category::run(repo, command)?,
        Commands::Audit { history } => {
            if audit::run(repo, history)? > 0 {
                return Ok(EXIT_FOUND);
            }
        }
        Commands::Key(command) => key::run(repo, command)?,
        Commands::Migrate { yes } => migrate::run(repo, yes)?,
        Commands::Info => info::run(repo)?,
        Commands::SetupTimer {
            schedule,
            message,
            user,
            remove,
        } => timer::run(
            repo,
            timer::TimerOptions {
                schedule,
                message,
                user,
                remove,
            },
        )?,
        Commands::SelfUpdate { check, yes } => self_update::run(check, yes)?,
    }
    Ok(0)
}
