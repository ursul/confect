//! Recognizes content that must never be stored in the repository as plaintext.
//!
//! The list is deliberately narrow — formats that are secrets by construction — so the
//! guard can block a sync without false alarms on ordinary configuration. Files are
//! scanned whole and binary content is not skipped: a single NUL byte must not be a way
//! around the guard.

use std::io::{self, Read};
use std::path::Path;

/// Fixed strings that mark private key material anywhere in a file.
const MARKERS: &[(&str, &str)] = &[
    ("-----BEGIN OPENSSH PRIVATE KEY-----", "OpenSSH private key"),
    ("-----BEGIN PGP PRIVATE KEY BLOCK-----", "PGP private key"),
    ("PRIVATE KEY-----", "PEM private key"),
    ("AGE-SECRET-KEY-1", "age identity"),
    ("PuTTY-User-Key-File-", "PuTTY private key"),
    ("(protected-private-key", "GnuPG private key"),
    ("(private-key", "GnuPG private key"),
];

/// File names that are key stores whatever their content looks like.
const SECRET_NAMES: &[(&str, &str)] = &[
    (".p12", "PKCS#12 key store"),
    (".pfx", "PKCS#12 key store"),
    (".jks", "Java key store"),
    (".keystore", "Java key store"),
    (".kdbx", "KeePass database"),
    ("secring.gpg", "GnuPG secret keyring"),
];

/// Substrings `git grep` can pre-filter history with before [`detect`] confirms a hit.
pub const HISTORY_NEEDLES: &[&str] = &[
    "PRIVATE KEY-----",
    "PRIVATE KEY BLOCK-----",
    "AGE-SECRET-KEY-1",
    "PuTTY-User-Key-File-",
    "private-key",
    "\"kty\"",
    "SCRAM-SHA-256$",
    "\"md5",
    ":$apr1$",
    ":$2y$",
    ":$2b$",
    ":$2a$",
    ":$5$",
    ":$6$",
    ":$y$",
    ":$7$",
    ":{SHA}",
];

/// Lines longer than this are not checked by the line rules (markers still are).
const MAX_LINE: usize = 64 * 1024;

/// What a file's name alone reveals, if anything.
pub fn detect_name(path: &Path) -> Option<&'static str> {
    if path
        .components()
        .any(|c| c.as_os_str() == "private-keys-v1.d")
    {
        return Some("GnuPG private key");
    }
    let name = path.file_name()?.to_str()?.to_ascii_lowercase();
    SECRET_NAMES
        .iter()
        .find(|(suffix, _)| name.ends_with(suffix))
        .map(|(_, what)| *what)
}

/// Return a description of the secret found in `content`, if any.
pub fn detect(content: &[u8]) -> Option<&'static str> {
    let mut scanner = Scanner::default();
    scanner.feed(content);
    scanner.finish()
}

/// Scan a whole stream without holding it in memory.
pub fn detect_reader(mut reader: impl Read) -> io::Result<Option<&'static str>> {
    let mut scanner = Scanner::default();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            return Ok(scanner.finish());
        }
        if let Some(found) = scanner.feed(&buf[..n]) {
            return Ok(Some(found));
        }
    }
}

/// Incremental detector: markers may span chunk boundaries, lines are reassembled.
#[derive(Default)]
struct Scanner {
    /// Unfinished last line of the previous chunk (also carries the marker overlap).
    line: Vec<u8>,
    found: Option<&'static str>,
}

impl Scanner {
    fn feed(&mut self, chunk: &[u8]) -> Option<&'static str> {
        if self.found.is_some() {
            return self.found;
        }
        let overlap = self.line.len().min(64);
        let mut window = self.line[self.line.len() - overlap..].to_vec();
        window.extend_from_slice(chunk);
        if let Some(what) = find_marker(&window) {
            self.found = Some(what);
            return self.found;
        }

        let mut rest = chunk;
        while let Some(end) = rest.iter().position(|&b| b == b'\n') {
            self.line.extend_from_slice(&rest[..end]);
            if let Some(what) = check_line(&self.line) {
                self.found = Some(what);
                return self.found;
            }
            self.line.clear();
            rest = &rest[end + 1..];
        }
        if self.line.len() + rest.len() <= MAX_LINE {
            self.line.extend_from_slice(rest);
        } else {
            // Keep only an overlap for markers; such a line is not a credential entry.
            let keep = rest.len().min(64);
            self.line.clear();
            self.line.extend_from_slice(&rest[rest.len() - keep..]);
        }
        None
    }

    fn finish(mut self) -> Option<&'static str> {
        if self.found.is_none() && !self.line.is_empty() {
            self.found = check_line(&self.line);
        }
        self.found
    }
}

fn find_marker(bytes: &[u8]) -> Option<&'static str> {
    MARKERS
        .iter()
        .find(|(marker, _)| contains(bytes, marker.as_bytes()))
        .map(|(_, what)| *what)
        .or_else(|| is_private_jwk(bytes).then_some("JSON Web Key private key"))
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn check_line(line: &[u8]) -> Option<&'static str> {
    let text = String::from_utf8_lossy(line);
    let line = text.trim();
    if line.starts_with('#') || line.starts_with(';') {
        return None;
    }
    if line.contains("SCRAM-SHA-256$") {
        return Some("SCRAM password verifier");
    }
    if is_userlist_md5(line) {
        return Some("MD5 password hash");
    }
    if is_password_hash_entry(line) {
        return Some("password hash (htpasswd/shadow format)");
    }
    None
}

/// A JSON Web Key with its private member `d` (certbot keeps ACME account keys so).
fn is_private_jwk(bytes: &[u8]) -> bool {
    if !contains(bytes, b"\"kty\"") {
        return false;
    }
    let mut rest = bytes;
    while let Some(index) = rest.windows(3).position(|w| w == b"\"d\"") {
        rest = &rest[index + 3..];
        let next = rest.iter().find(|b| !b.is_ascii_whitespace());
        if next == Some(&b':') {
            return true;
        }
    }
    false
}

/// PgBouncer `userlist.txt` line: `"user" "md5<32 hex>"`.
fn is_userlist_md5(line: &str) -> bool {
    let Some(start) = line.find("\"md5") else {
        return false;
    };
    let hash = &line[start + 4..];
    hash.len() >= 33
        && hash[..32].chars().all(|c| c.is_ascii_hexdigit())
        && hash[32..].starts_with('"')
}

/// `name:<crypt hash>` as used by htpasswd, `/etc/shadow` and squid password files.
fn is_password_hash_entry(line: &str) -> bool {
    let Some((name, rest)) = line.split_once(':') else {
        return false;
    };
    if name.is_empty() || name.contains(char::is_whitespace) {
        return false;
    }
    let hash = rest.split(':').next().unwrap_or("");
    const PREFIXES: &[&str] = &["$apr1$", "$2y$", "$2b$", "$2a$", "$5$", "$6$", "$y$", "$7$"];
    let crypt = PREFIXES.iter().any(|p| hash.starts_with(p)) && hash.len() >= 20;
    let sha = hash.starts_with("{SHA}") && hash.len() >= 30;
    crypt || sha
}

#[cfg(test)]
mod tests {
    use super::{detect, detect_name, detect_reader};
    use std::path::Path;

    #[test]
    fn private_keys_are_detected() {
        assert_eq!(
            detect(b"-----BEGIN PRIVATE KEY-----\nMIIE...\n"),
            Some("PEM private key")
        );
        assert_eq!(
            detect(b"-----BEGIN OPENSSH PRIVATE KEY-----\nb3Blbn...\n"),
            Some("OpenSSH private key")
        );
        assert_eq!(
            detect(b"# comment\nAGE-SECRET-KEY-1QQQ\n"),
            Some("age identity")
        );
    }

    #[test]
    fn binary_content_and_large_files_do_not_hide_keys() {
        assert_eq!(
            detect(b"\x00\x01-----BEGIN OPENSSH PRIVATE KEY-----\n"),
            Some("OpenSSH private key")
        );
        let mut big = vec![b'x'; 9 * 1024 * 1024];
        big.extend_from_slice(b"\n-----BEGIN PRIVATE KEY-----\n");
        assert_eq!(detect_reader(&big[..]).unwrap(), Some("PEM private key"));
    }

    #[test]
    fn markers_split_across_chunks_are_found() {
        let mut data = vec![b'a'; 256 * 1024 - 10];
        data.extend_from_slice(b"-----BEGIN PRIVATE KEY-----");
        assert_eq!(detect_reader(&data[..]).unwrap(), Some("PEM private key"));
    }

    #[test]
    fn private_json_web_keys_are_detected() {
        assert_eq!(
            detect(br#"{"n": "wTvj", "e": "AQAB", "d": "secret", "p": "x", "kty": "RSA"}"#),
            Some("JSON Web Key private key")
        );
        assert_eq!(detect(br#"{"kty": "RSA", "n": "wTvj", "e": "AQAB"}"#), None);
    }

    #[test]
    fn key_stores_are_recognized_by_name() {
        assert_eq!(
            detect_name(Path::new("/etc/app/server.p12")),
            Some("PKCS#12 key store")
        );
        assert_eq!(
            detect_name(Path::new("/etc/app/trust.JKS")),
            Some("Java key store")
        );
        assert_eq!(
            detect_name(Path::new("/root/.gnupg/private-keys-v1.d/ABC.key")),
            Some("GnuPG private key")
        );
        assert_eq!(detect_name(Path::new("/etc/app/app.conf")), None);
    }

    #[test]
    fn password_files_are_detected() {
        assert_eq!(
            detect(b"\"pgbouncer\" \"SCRAM-SHA-256$4096:abc$def:ghi\"\n"),
            Some("SCRAM password verifier")
        );
        assert_eq!(
            detect(b"\"exporter\" \"md5d41d8cd98f00b204e9800998ecf8427e\"\n"),
            Some("MD5 password hash")
        );
        assert_eq!(
            detect(b"admin:$apr1$abcdefgh$0123456789abcdefghijkl\n"),
            Some("password hash (htpasswd/shadow format)")
        );
        assert_eq!(
            detect(b"root:$6$saltsalt$hashhashhashhashhash:19000:0:99999:7:::\n"),
            Some("password hash (htpasswd/shadow format)")
        );
    }

    #[test]
    fn ordinary_configuration_passes() {
        assert_eq!(
            detect(b"server {\n  listen 443 ssl;\n  ssl_certificate_key /etc/x/privkey.pem;\n}\n"),
            None
        );
        assert_eq!(
            detect(b"-----BEGIN CERTIFICATE-----\nMIID\n-----END CERTIFICATE-----\n"),
            None
        );
        assert_eq!(detect(b"echo \"$6\" \"$2\"\nPATH=$PATH:$HOME/bin\n"), None);
        assert_eq!(detect(b"# password = \"SCRAM-SHA-256$example\"\n"), None);
    }
}
