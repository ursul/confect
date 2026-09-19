//! Recognizes content that must never be stored in the repository as plaintext.
//!
//! The list is deliberately narrow — formats that are secrets by construction — so the
//! guard can block a sync without false alarms on ordinary configuration.

/// Fixed strings that mark private key material anywhere in a file.
const MARKERS: &[(&str, &str)] = &[
    ("-----BEGIN OPENSSH PRIVATE KEY-----", "OpenSSH private key"),
    ("-----BEGIN PGP PRIVATE KEY BLOCK-----", "PGP private key"),
    ("PRIVATE KEY-----", "PEM private key"),
    ("AGE-SECRET-KEY-1", "age identity"),
    ("PuTTY-User-Key-File-", "PuTTY private key"),
];

/// Substrings `git grep` can pre-filter history with before [`detect`] confirms a hit.
pub const HISTORY_NEEDLES: &[&str] = &[
    "PRIVATE KEY-----",
    "PRIVATE KEY BLOCK-----",
    "AGE-SECRET-KEY-1",
    "PuTTY-User-Key-File-",
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

/// Only this much of a file is inspected; key material sits in small files.
pub const SCAN_LIMIT: usize = 8 * 1024 * 1024;

/// Return a description of the secret found in `content`, if any.
pub fn detect(content: &[u8]) -> Option<&'static str> {
    let content = &content[..content.len().min(SCAN_LIMIT)];
    if content.contains(&0) {
        return None;
    }
    let text = String::from_utf8_lossy(content);

    for (marker, what) in MARKERS {
        if text.contains(marker) {
            return Some(what);
        }
    }

    if is_private_jwk(&text) {
        return Some("JSON Web Key private key");
    }

    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.starts_with(';') {
            continue;
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
    }
    None
}

/// A JSON Web Key with its private member `d` (certbot keeps ACME account keys so).
fn is_private_jwk(text: &str) -> bool {
    if !text.contains("\"kty\"") {
        return false;
    }
    let mut rest = text;
    while let Some(index) = rest.find("\"d\"") {
        rest = &rest[index + 3..];
        if rest.trim_start().starts_with(':') {
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
    use super::detect;

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
    fn private_json_web_keys_are_detected() {
        assert_eq!(
            detect(br#"{"n": "wTvj", "e": "AQAB", "d": "secret", "p": "x", "kty": "RSA"}"#),
            Some("JSON Web Key private key")
        );
        assert_eq!(detect(br#"{"kty": "RSA", "n": "wTvj", "e": "AQAB"}"#), None);
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
        assert_eq!(detect(b"\x00\x01binary PRIVATE KEY-----"), None);
    }
}
