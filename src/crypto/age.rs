use std::cell::OnceCell;
use std::fs;
use std::io::{BufReader, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use age::armor::ArmoredReader;
use age::secrecy::ExposeSecret;

use crate::error::{ConfectError, IoContext, Result};

type Identities = Vec<Box<dyn age::Identity + Send + Sync>>;
type Recipients = Vec<Box<dyn age::Recipient + Send>>;

/// age encryption with keys loaded on first use.
///
/// Recipients are needed only to write encrypted files and the identity only to read
/// them, so a host with just the recipients file can still sync.
pub struct Crypto {
    identity_file: PathBuf,
    recipients_file: PathBuf,
    identities: OnceCell<std::result::Result<Option<Identities>, String>>,
    recipients: OnceCell<std::result::Result<Option<Recipients>, String>>,
}

impl Crypto {
    pub fn new(identity_file: PathBuf, recipients_file: PathBuf) -> Self {
        Self {
            identity_file,
            recipients_file,
            identities: OnceCell::new(),
            recipients: OnceCell::new(),
        }
    }

    pub fn identity_file(&self) -> &Path {
        &self.identity_file
    }

    pub fn recipients_file(&self) -> &Path {
        &self.recipients_file
    }

    pub fn has_identity(&self) -> bool {
        matches!(self.load_identities(), Ok(Some(_)))
    }

    /// `Ok(None)` when there is no identity file; an error when it exists but is unusable.
    fn load_identities(&self) -> std::result::Result<Option<&Identities>, String> {
        self.identities
            .get_or_init(|| {
                if !self.identity_file.exists() {
                    return Ok(None);
                }
                age::IdentityFile::from_file(self.identity_file.to_string_lossy().into_owned())
                    .map_err(|e| format!("{}: {}", self.identity_file.display(), e))?
                    .into_identities()
                    .map(Some)
                    .map_err(|e| format!("{}: {}", self.identity_file.display(), e))
            })
            .as_ref()
            .map(Option::as_ref)
            .map_err(Clone::clone)
    }

    fn load_recipients(&self) -> Result<&Recipients> {
        let loaded = self.recipients.get_or_init(|| {
            let content = match fs::read_to_string(&self.recipients_file) {
                Ok(content) => content,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(err) => return Err(format!("{}: {}", self.recipients_file.display(), err)),
            };
            let mut recipients: Recipients = Vec::new();
            for (number, line) in content.lines().enumerate() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let recipient = age::x25519::Recipient::from_str(line).map_err(|e| {
                    format!(
                        "{} line {}: not an age recipient (age1...): {}",
                        self.recipients_file.display(),
                        number + 1,
                        e
                    )
                })?;
                recipients.push(Box::new(recipient));
            }
            Ok((!recipients.is_empty()).then_some(recipients))
        });
        match loaded {
            Ok(Some(recipients)) => Ok(recipients),
            Ok(None) => Err(ConfectError::NoRecipients(self.recipients_file.clone())),
            Err(message) => Err(ConfectError::Encryption(message.clone())),
        }
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let recipients = self.load_recipients()?;
        let encryptor = age::Encryptor::with_recipients(recipients.iter().map(|r| r.as_ref() as _))
            .map_err(|e| ConfectError::Encryption(e.to_string()))?;
        let mut ciphertext = Vec::new();
        let mut writer = encryptor
            .wrap_output(&mut ciphertext)
            .map_err(|e| ConfectError::Encryption(e.to_string()))?;
        writer
            .write_all(plaintext)
            .map_err(|e| ConfectError::Encryption(e.to_string()))?;
        writer
            .finish()
            .map_err(|e| ConfectError::Encryption(e.to_string()))?;
        Ok(ciphertext)
    }

    /// Decrypt binary or ASCII-armored age data; `path` is only used in messages.
    pub fn decrypt(&self, ciphertext: &[u8], path: &Path) -> Result<Vec<u8>> {
        let identities = match self.load_identities() {
            Ok(Some(identities)) => identities,
            Ok(None) => return Err(ConfectError::NoIdentity(self.identity_file.clone())),
            Err(message) => return Err(ConfectError::Encryption(message)),
        };
        let fail = |message: String| ConfectError::Decryption {
            path: path.to_path_buf(),
            message,
        };
        let decryptor = age::Decryptor::new(ArmoredReader::new(BufReader::new(ciphertext)))
            .map_err(|e| fail(e.to_string()))?;
        let mut reader = decryptor
            .decrypt(identities.iter().map(|i| i.as_ref() as _))
            .map_err(|e| fail(e.to_string()))?;
        let mut plaintext = Vec::new();
        reader
            .read_to_end(&mut plaintext)
            .map_err(|e| fail(e.to_string()))?;
        Ok(plaintext)
    }

    /// Create a new key pair; refuses to overwrite an existing identity.
    pub fn generate(&self) -> Result<String> {
        if self.identity_file.exists() {
            return Err(ConfectError::Other(format!(
                "{} already exists; remove it first if you really want a new key",
                self.identity_file.display()
            )));
        }
        let identity = age::x25519::Identity::generate();
        let recipient = identity.to_public().to_string();

        if let Some(parent) = self.identity_file.parent() {
            fs::create_dir_all(parent).at(parent)?;
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&self.identity_file)
            .at(&self.identity_file)?;
        writeln!(
            file,
            "# created: {}\n# public key: {}\n{}",
            chrono::Utc::now().to_rfc3339(),
            recipient,
            identity.to_string().expose_secret()
        )
        .at(&self.identity_file)?;

        if let Some(parent) = self.recipients_file.parent() {
            fs::create_dir_all(parent).at(parent)?;
        }
        let mut recipients = String::new();
        if let Ok(existing) = fs::read_to_string(&self.recipients_file) {
            recipients.push_str(&existing);
            if !recipients.ends_with('\n') && !recipients.is_empty() {
                recipients.push('\n');
            }
        }
        recipients.push_str(&recipient);
        recipients.push('\n');
        fs::write(&self.recipients_file, recipients).at(&self.recipients_file)?;
        Ok(recipient)
    }

    /// Public keys encrypted files are written for.
    pub fn recipients(&self) -> Result<Vec<String>> {
        let content = fs::read_to_string(&self.recipients_file)
            .map_err(|_| ConfectError::NoRecipients(self.recipients_file.clone()))?;
        Ok(content
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(str::to_string)
            .collect())
    }
}

pub fn is_age_file(content: &[u8]) -> bool {
    content.starts_with(b"age-encryption.org/")
        || content.starts_with(b"-----BEGIN AGE ENCRYPTED FILE-----")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_with_generated_keys() {
        let dir = tempfile::tempdir().unwrap();
        let crypto = Crypto::new(dir.path().join("id.txt"), dir.path().join("rcpt.txt"));
        assert!(!crypto.has_identity());
        crypto.generate().unwrap();

        let crypto = Crypto::new(dir.path().join("id.txt"), dir.path().join("rcpt.txt"));
        let ciphertext = crypto.encrypt(b"secret").unwrap();
        assert!(is_age_file(&ciphertext));
        assert_eq!(
            crypto.decrypt(&ciphertext, Path::new("x")).unwrap(),
            b"secret"
        );
        let mode = fs::metadata(dir.path().join("id.txt"))
            .unwrap()
            .permissions();
        assert_eq!(
            std::os::unix::fs::PermissionsExt::mode(&mode) & 0o777,
            0o600
        );
    }

    #[test]
    fn a_bad_recipient_line_is_reported_with_its_number() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("rcpt.txt"), "# keys\nssh-ed25519 AAAA\n").unwrap();
        let crypto = Crypto::new(dir.path().join("id.txt"), dir.path().join("rcpt.txt"));
        let err = crypto.encrypt(b"x").unwrap_err().to_string();
        assert!(err.contains("line 2"), "{}", err);
    }

    #[test]
    fn missing_recipients_is_a_clear_error() {
        let dir = tempfile::tempdir().unwrap();
        let crypto = Crypto::new(dir.path().join("id.txt"), dir.path().join("rcpt.txt"));
        assert!(matches!(
            crypto.encrypt(b"x"),
            Err(ConfectError::NoRecipients(_))
        ));
    }
}
