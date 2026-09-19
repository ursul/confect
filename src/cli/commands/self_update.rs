use flate2::read::GzDecoder;
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;

use crate::cli::ui;
use crate::error::{ConfectError, IoContext, Result};

const RELEASE_API: &str = "https://api.github.com/repos/ursul/confect/releases/latest";
const DOWNLOAD_LIMIT: u64 = 100 * 1024 * 1024;

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

fn asset_name() -> Result<String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Ok("confect-linux-x86_64.tar.gz".into()),
        ("linux", "aarch64") => Ok("confect-linux-aarch64.tar.gz".into()),
        (os, arch) => Err(ConfectError::Other(format!(
            "no release builds for {}-{}",
            os, arch
        ))),
    }
}

fn fetch(url: &str) -> Result<Vec<u8>> {
    let response = ureq::get(url)
        .header("User-Agent", concat!("confect/", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/octet-stream, application/json")
        .call()
        .map_err(|e| ConfectError::Other(format!("{}: {}", url, e)))?;
    response
        .into_body()
        .with_config()
        .limit(DOWNLOAD_LIMIT)
        .read_to_vec()
        .map_err(|e| ConfectError::Other(format!("{}: {}", url, e)))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn run(check: bool, yes: bool) -> Result<()> {
    let current = Version::parse(env!("CARGO_PKG_VERSION")).expect("crate version is semver");
    let release: Release = serde_json::from_slice(&fetch(RELEASE_API)?)
        .map_err(|e| ConfectError::Other(format!("unexpected GitHub response: {}", e)))?;
    let latest = Version::parse(release.tag_name.trim_start_matches('v')).map_err(|e| {
        ConfectError::Other(format!(
            "release tag {} is not a version: {}",
            release.tag_name, e
        ))
    })?;

    if latest <= current {
        ui::success(&format!("confect {} is the latest release", current));
        return Ok(());
    }
    ui::info(&format!(
        "confect {} is available (installed {})",
        latest, current
    ));
    if check {
        return Ok(());
    }

    let name = asset_name()?;
    let checksum_name = format!("{}.sha256", name);
    let find = |wanted: &str| {
        release
            .assets
            .iter()
            .find(|a| a.name == wanted)
            .ok_or_else(|| {
                ConfectError::Other(format!("release {} has no asset {}", latest, wanted))
            })
    };
    let archive_asset = find(&name)?;
    let checksum_asset = find(&checksum_name)?;

    let exe = std::env::current_exe()?;
    if !ui::confirm(&format!("Replace {} with {}?", exe.display(), latest), yes)? {
        return Err(ConfectError::Aborted);
    }

    let archive = fetch(&archive_asset.browser_download_url)?;
    let checksum_text =
        String::from_utf8_lossy(&fetch(&checksum_asset.browser_download_url)?).into_owned();
    let expected = checksum_text
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    let actual = hex(&Sha256::digest(&archive));
    if expected.len() != 64 || expected != actual {
        return Err(ConfectError::Other(format!(
            "checksum mismatch for {}: published {}, downloaded {}",
            name, expected, actual
        )));
    }
    ui::success(&format!("SHA-256 verified: {}", actual));

    let binary = extract_binary(&archive)?;
    let dir = exe
        .parent()
        .ok_or_else(|| ConfectError::Other("cannot locate the running binary".into()))?;
    let mut temp = tempfile::Builder::new()
        .prefix(".confect-update-")
        .tempfile_in(dir)
        .at(dir)?;
    std::io::Write::write_all(&mut temp, &binary).at(dir)?;
    temp.as_file()
        .set_permissions(fs::Permissions::from_mode(0o755))
        .at(dir)?;
    temp.persist(&exe)
        .map_err(|e| ConfectError::io_at(&exe, e.error))?;
    ui::success(&format!("Updated {} to {}", exe.display(), latest));
    Ok(())
}

/// The `confect` executable from a release archive; nothing else is unpacked.
fn extract_binary(archive: &[u8]) -> Result<Vec<u8>> {
    let mut tar = tar::Archive::new(GzDecoder::new(archive));
    for entry in tar.entries()? {
        let mut entry = entry?;
        let is_binary = entry.header().entry_type().is_file()
            && entry.path()?.file_name().is_some_and(|n| n == "confect");
        if is_binary {
            let mut binary = Vec::new();
            entry.read_to_end(&mut binary)?;
            if !binary.starts_with(b"\x7fELF") {
                return Err(ConfectError::Other(
                    "the archive does not contain an ELF binary".into(),
                ));
            }
            return Ok(binary);
        }
    }
    Err(ConfectError::Other(
        "the archive has no confect binary".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::{extract_binary, hex};
    use flate2::write::GzEncoder;
    use sha2::{Digest, Sha256};

    fn archive(name: &str, content: &[u8]) -> Vec<u8> {
        let mut builder =
            tar::Builder::new(GzEncoder::new(Vec::new(), flate2::Compression::fast()));
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        builder.append_data(&mut header, name, content).unwrap();
        builder.into_inner().unwrap().finish().unwrap()
    }

    #[test]
    fn only_the_confect_elf_binary_is_extracted() {
        let good = archive("confect", b"\x7fELF-binary");
        assert_eq!(extract_binary(&good).unwrap(), b"\x7fELF-binary");
        assert!(extract_binary(&archive("confect", b"#!/bin/sh")).is_err());
        assert!(extract_binary(&archive("other", b"\x7fELF")).is_err());
    }

    #[test]
    fn sha256_is_rendered_as_lowercase_hex() {
        assert_eq!(
            hex(&Sha256::digest(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
