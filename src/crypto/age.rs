use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;

use age::secrecy::ExposeSecret;

use crate::error::{Result, ConfectError};

/// Age encryption wrapper
pub struct AgeEncryption {
    recipients: Vec<age::x25519::Recipient>,
}

impl AgeEncryption {
    /// Create new encryption instance with recipients
    pub fn new(recipient_strings: Vec<String>) -> Result<Self> {
        let recipients: std::result::Result<Vec<_>, _> = recipient_strings
            .iter()
            .filter(|s| !s.is_empty() && !s.starts_with('#'))
            .map(|s| s.parse::<age::x25519::Recipient>())
            .collect();

        let recipients = recipients.map_err(|e| {
            ConfectError::Encryption(format!("Invalid recipient: {}", e))
        })?;

        Ok(Self { recipients })
    }

    /// Load recipients from a file (one per line)
    pub fn from_recipients_file(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let recipient_strings: Vec<String> = content
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(|s| s.to_string())
            .collect();

        Self::new(recipient_strings)
    }

    /// Encrypt a file
    pub fn encrypt_file(&self, input_path: &Path, output_path: &Path) -> Result<()> {
        if self.recipients.is_empty() {
            return Err(ConfectError::Encryption(
                "No recipients configured".to_string(),
            ));
        }

        let mut input = File::open(input_path)?;
        let mut plaintext = Vec::new();
        input.read_to_end(&mut plaintext)?;

        let ciphertext = self.encrypt(&plaintext)?;

        let mut output = File::create(output_path)?;
        output.write_all(&ciphertext)?;

        Ok(())
    }

    /// Decrypt a file
    pub fn decrypt_file(
        &self,
        input_path: &Path,
        output_path: &Path,
        identity: &age::x25519::Identity,
    ) -> Result<()> {
        let mut input = File::open(input_path)?;
        let mut ciphertext = Vec::new();
        input.read_to_end(&mut ciphertext)?;

        let plaintext = self.decrypt(&ciphertext, identity)?;

        let mut output = File::create(output_path)?;
        output.write_all(&plaintext)?;

        Ok(())
    }

    /// Encrypt data
    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let recipients: Vec<Box<dyn age::Recipient + Send>> = self
            .recipients
            .iter()
            .map(|r| Box::new(r.clone()) as Box<dyn age::Recipient + Send>)
            .collect();

        if recipients.is_empty() {
            return Err(ConfectError::Encryption(
                "No valid recipients found".to_string(),
            ));
        }

        let encryptor = age::Encryptor::with_recipients(recipients)
            .ok_or_else(|| ConfectError::Encryption("Failed to create encryptor".to_string()))?;

        let mut encrypted = vec![];
        let mut writer = encryptor
            .wrap_output(&mut encrypted)
            .map_err(|e| ConfectError::Encryption(format!("Failed to wrap output: {}", e)))?;

        writer
            .write_all(plaintext)
            .map_err(|e| ConfectError::Encryption(format!("Failed to write: {}", e)))?;

        writer
            .finish()
            .map_err(|e| ConfectError::Encryption(format!("Failed to finish: {}", e)))?;

        Ok(encrypted)
    }

    /// Decrypt data
    fn decrypt(&self, ciphertext: &[u8], identity: &age::x25519::Identity) -> Result<Vec<u8>> {
        let decryptor = match age::Decryptor::new(ciphertext)
            .map_err(|e| ConfectError::Decryption(format!("Failed to create decryptor: {}", e)))?
        {
            age::Decryptor::Recipients(d) => d,
            _ => {
                return Err(ConfectError::Decryption(
                    "Passphrase-encrypted files not supported".to_string(),
                ))
            }
        };

        let mut decrypted = vec![];
        let mut reader = decryptor
            .decrypt(std::iter::once(identity as &dyn age::Identity))
            .map_err(|e| ConfectError::Decryption(format!("Failed to decrypt: {}", e)))?;

        reader
            .read_to_end(&mut decrypted)
            .map_err(|e| ConfectError::Decryption(format!("Failed to read: {}", e)))?;

        Ok(decrypted)
    }

    /// Check if a file is age-encrypted
    pub fn is_encrypted(path: &Path) -> bool {
        if let Ok(mut file) = File::open(path) {
            let mut header = [0u8; 16];
            if file.read_exact(&mut header).is_ok() {
                // Check for age header
                return header.starts_with(b"age-encryption.");
            }
        }
        false
    }

    /// Generate a new key pair
    pub fn generate_keypair() -> (String, String) {
        let identity = age::x25519::Identity::generate();
        let recipient = identity.to_public();
        (
            identity.to_string().expose_secret().clone(),
            recipient.to_string(),
        )
    }
}
