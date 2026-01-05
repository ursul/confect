use console::style;
use dialoguer::Confirm;

use crate::cli::CategoryCommands;
use crate::core::{Repository, CategoryManager, Category};
use crate::error::Result;

pub fn run_category(cmd: CategoryCommands) -> Result<()> {
    let repo = Repository::open_default()?;
    let mut categories = CategoryManager::load(&repo)?;

    match cmd {
        CategoryCommands::List => {
            list_categories(&categories)?;
        }
        CategoryCommands::Show { name } => {
            show_category(&categories, &name)?;
        }
        CategoryCommands::Create { name, description, path, encrypt } => {
            create_category(&mut categories, &name, description, path, encrypt)?;
        }
        CategoryCommands::Delete { name, force, remove_files } => {
            delete_category(&mut categories, &name, force, remove_files)?;
        }
        CategoryCommands::AddPath { name, path, encrypt } => {
            add_path(&mut categories, &name, &path, encrypt)?;
        }
        CategoryCommands::RemovePath { name, path } => {
            remove_path(&mut categories, &name, &path)?;
        }
    }

    Ok(())
}

fn list_categories(categories: &CategoryManager) -> Result<()> {
    let cats = categories.list();

    if cats.is_empty() {
        println!("No categories defined.");
        println!();
        println!(
            "Create one with: {}",
            style("confect category create <name> --path <path>").cyan()
        );
        return Ok(());
    }

    println!();
    println!("{}", style("Categories:").bold());
    println!();

    for cat in cats {
        let desc = cat.description.as_deref().unwrap_or("");
        println!(
            "  {} {}",
            style(&cat.name).cyan().bold(),
            style(desc).dim()
        );
        println!(
            "    {} path(s), {} encrypted pattern(s)",
            cat.paths.len(),
            cat.encrypt.len()
        );
    }
    println!();

    Ok(())
}

fn show_category(categories: &CategoryManager, name: &str) -> Result<()> {
    let cat = categories.get(name)?;

    println!();
    println!("{} {}", style("Category:").bold(), style(&cat.name).cyan().bold());

    if let Some(desc) = &cat.description {
        println!("{} {}", style("Description:").bold(), desc);
    }

    println!();
    println!("{}", style("Paths:").bold());
    for path in &cat.paths {
        println!("  {}", path);
    }

    if !cat.encrypt.is_empty() {
        println!();
        println!("{}", style("Encrypted patterns:").bold());
        for pattern in &cat.encrypt {
            println!("  {} {}", style("🔒").dim(), pattern);
        }
    }

    if !cat.exclude.is_empty() {
        println!();
        println!("{}", style("Excluded patterns:").bold());
        for pattern in &cat.exclude {
            println!("  {} {}", style("✗").dim(), pattern);
        }
    }

    println!();

    Ok(())
}

fn create_category(
    categories: &mut CategoryManager,
    name: &str,
    description: Option<String>,
    paths: Vec<String>,
    encrypt: Vec<String>,
) -> Result<()> {
    let cat = Category {
        name: name.to_string(),
        description,
        paths,
        encrypt,
        exclude: Vec::new(),
    };

    categories.add(cat)?;
    categories.save()?;

    println!(
        "{} Created category '{}'",
        style("✓").green().bold(),
        style(name).cyan()
    );

    Ok(())
}

fn delete_category(
    categories: &mut CategoryManager,
    name: &str,
    force: bool,
    _remove_files: bool,
) -> Result<()> {
    // Check if exists
    let _cat = categories.get(name)?;

    if !force {
        let proceed = Confirm::new()
            .with_prompt(format!("Delete category '{}'?", name))
            .default(false)
            .interact()?;

        if !proceed {
            println!("Aborted.");
            return Ok(());
        }
    }

    categories.remove(name)?;
    categories.save()?;

    println!(
        "{} Deleted category '{}'",
        style("✓").green().bold(),
        style(name).cyan()
    );

    Ok(())
}

fn add_path(
    categories: &mut CategoryManager,
    name: &str,
    path: &str,
    encrypt: bool,
) -> Result<()> {
    categories.add_path(name, path.to_string(), encrypt)?;
    categories.save()?;

    let encrypted_note = if encrypt { " (encrypted)" } else { "" };
    println!(
        "{} Added path '{}' to category '{}'{}",
        style("✓").green().bold(),
        style(path).cyan(),
        style(name).cyan(),
        encrypted_note
    );

    Ok(())
}

fn remove_path(
    categories: &mut CategoryManager,
    name: &str,
    path: &str,
) -> Result<()> {
    categories.remove_path(name, path)?;
    categories.save()?;

    println!(
        "{} Removed path '{}' from category '{}'",
        style("✓").green().bold(),
        style(path).cyan(),
        style(name).cyan()
    );

    Ok(())
}
