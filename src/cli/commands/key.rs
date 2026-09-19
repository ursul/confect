use std::path::Path;

use crate::cli::args::KeyCommands;
use crate::cli::ui;
use crate::core::Config;
use crate::crypto::Crypto;
use crate::error::Result;

pub fn run(_explicit_repo: Option<&Path>, command: KeyCommands) -> Result<()> {
    let config = Config::load()?;
    let crypto = Crypto::new(config.identity_file()?, config.recipients_file()?);
    match command {
        KeyCommands::Generate => {
            let recipient = crypto.generate()?;
            ui::success(&format!(
                "Created {} (keep a copy outside this machine: without it encrypted files are lost)",
                crypto.identity_file().display()
            ));
            ui::success(&format!(
                "Added {} to {}",
                recipient,
                crypto.recipients_file().display()
            ));
        }
        KeyCommands::Show => {
            for recipient in crypto.recipients()? {
                println!("{}", recipient);
            }
            let identity = if crypto.has_identity() {
                "present"
            } else {
                "missing (this host can encrypt but not decrypt)"
            };
            ui::info(&format!(
                "identity {}: {}",
                crypto.identity_file().display(),
                identity
            ));
        }
    }
    Ok(())
}
