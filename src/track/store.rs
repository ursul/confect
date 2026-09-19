use std::fs::{self, File};
use std::io::{self, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::core::paths::repo_relative;
use crate::crypto::Crypto;
use crate::error::{IoContext, Result};
use crate::track::entry::Kind;
use crate::track::metadata::EntryMeta;

/// Stored copies inside the repository working tree.
pub struct Store<'a> {
    root: &'a Path,
    crypto: &'a Crypto,
}

impl<'a> Store<'a> {
    pub fn new(root: &'a Path, crypto: &'a Crypto) -> Self {
        Self { root, crypto }
    }

    pub fn crypto(&self) -> &Crypto {
        self.crypto
    }

    pub fn absolute(&self, relative: &Path) -> PathBuf {
        self.root.join(relative)
    }

    /// Plaintext of a stored file, decrypting it when needed.
    pub fn read(&self, system_path: &Path, meta: &EntryMeta) -> Result<Vec<u8>> {
        let path = self.absolute(&meta.repo_path(system_path));
        let content = fs::read(&path).at(&path)?;
        if meta.encrypted {
            self.crypto.decrypt(&content, system_path)
        } else {
            Ok(content)
        }
    }

    /// Stored symlink target.
    pub fn read_link(&self, system_path: &Path, meta: &EntryMeta) -> Result<PathBuf> {
        if let Some(target) = &meta.target {
            return Ok(PathBuf::from(target));
        }
        let path = self.absolute(&meta.repo_path(system_path));
        fs::read_link(&path).at(&path)
    }

    /// Does the stored plaintext file equal the system file? Streams both sides.
    pub fn same_plain_content(&self, system_path: &Path, meta: &EntryMeta) -> Result<bool> {
        let stored = self.absolute(&meta.repo_path(system_path));
        files_equal(system_path, &stored)
    }

    /// Store a system file under its category, encrypted if requested.
    pub fn write_file(
        &self,
        category: &str,
        system_path: &Path,
        content: &[u8],
        encrypted: bool,
        executable: bool,
    ) -> Result<()> {
        let relative = repo_relative(category, system_path, encrypted);
        let data;
        let bytes = if encrypted {
            data = self.crypto.encrypt(content)?;
            &data[..]
        } else {
            content
        };
        let mode = if executable { 0o700 } else { 0o600 };
        write_atomic(&self.absolute(&relative), bytes, mode)?;
        self.remove_variant(category, system_path, !encrypted)?;
        Ok(())
    }

    /// Copy a large plaintext file without holding it in memory.
    pub fn copy_file(&self, category: &str, system_path: &Path, executable: bool) -> Result<()> {
        let relative = repo_relative(category, system_path, false);
        let destination = self.absolute(&relative);
        let parent = destination
            .parent()
            .expect("repository paths have a parent");
        fs::create_dir_all(parent).at(parent)?;
        let mut temp = tempfile::NamedTempFile::new_in(parent).at(parent)?;
        let mut source = File::open(system_path).at(system_path)?;
        io::copy(&mut source, temp.as_file_mut()).at(system_path)?;
        let mode = if executable { 0o700 } else { 0o600 };
        temp.as_file()
            .set_permissions(fs::Permissions::from_mode(mode))
            .at(&destination)?;
        temp.persist(&destination)
            .map_err(|e| crate::error::ConfectError::io_at(&destination, e.error))?;
        self.remove_variant(category, system_path, true)?;
        Ok(())
    }

    pub fn write_symlink(&self, category: &str, system_path: &Path, target: &Path) -> Result<()> {
        let destination = self.absolute(&repo_relative(category, system_path, false));
        let parent = destination
            .parent()
            .expect("repository paths have a parent");
        fs::create_dir_all(parent).at(parent)?;
        let temp = parent.join(format!(".confect-link-{}", std::process::id()));
        let _ = fs::remove_file(&temp);
        std::os::unix::fs::symlink(target, &temp).at(&temp)?;
        fs::rename(&temp, &destination).at(&destination)?;
        self.remove_variant(category, system_path, true)?;
        Ok(())
    }

    /// Remove both the plain and the encrypted copy of a path.
    pub fn remove(&self, category: &str, system_path: &Path) -> Result<()> {
        self.remove_variant(category, system_path, false)?;
        self.remove_variant(category, system_path, true)?;
        Ok(())
    }

    fn remove_variant(&self, category: &str, system_path: &Path, encrypted: bool) -> Result<()> {
        let path = self.absolute(&repo_relative(category, system_path, encrypted));
        match fs::symlink_metadata(&path) {
            Ok(meta) if !meta.is_dir() => {
                fs::remove_file(&path).at(&path)?;
                self.prune_empty_dirs(&path, category);
            }
            _ => {}
        }
        Ok(())
    }

    fn prune_empty_dirs(&self, removed: &Path, category: &str) {
        let stop = self.root.join(category);
        let mut dir = removed.parent();
        while let Some(current) = dir {
            if !current.starts_with(&stop) || current == stop.parent().unwrap_or(self.root) {
                break;
            }
            if fs::remove_dir(current).is_err() {
                break;
            }
            dir = current.parent();
        }
    }

    /// Remove a whole category directory from the repository.
    pub fn remove_category(&self, category: &str) -> Result<()> {
        let dir = self.root.join(category);
        if dir.exists() {
            fs::remove_dir_all(&dir).at(&dir)?;
        }
        Ok(())
    }

    /// Files under a category directory, as system paths (an encrypted copy keeps its
    /// `.age` suffix). Used to find copies the metadata index does not know about.
    pub fn list_category(&self, category: &str) -> Vec<(PathBuf, Kind)> {
        let dir = self.root.join(category);
        let mut result = Vec::new();
        if !dir.is_dir() {
            return result;
        }
        for entry in WalkDir::new(&dir).follow_links(false).into_iter().flatten() {
            let file_type = entry.file_type();
            if file_type.is_dir() {
                continue;
            }
            let Ok(relative) = entry.path().strip_prefix(&dir) else {
                continue;
            };
            let system = Path::new("/").join(relative);
            let kind = if file_type.is_symlink() {
                Kind::Symlink
            } else {
                Kind::File
            };
            result.push((system, kind));
        }
        result
    }
}

/// Write via a temporary file in the same directory and rename it into place.
pub fn write_atomic(path: &Path, content: &[u8], mode: u32) -> Result<()> {
    let parent = path.parent().expect("absolute paths have a parent");
    fs::create_dir_all(parent).at(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).at(parent)?;
    temp.write_all(content).at(path)?;
    temp.as_file()
        .set_permissions(fs::Permissions::from_mode(mode))
        .at(path)?;
    temp.persist(path)
        .map_err(|e| crate::error::ConfectError::io_at(path, e.error))?;
    Ok(())
}

/// Byte-for-byte comparison without reading whole files into memory.
pub fn files_equal(a: &Path, b: &Path) -> Result<bool> {
    let meta_a = fs::metadata(a).at(a)?;
    let meta_b = match fs::symlink_metadata(b) {
        Ok(meta) if meta.is_file() => meta,
        Ok(_) => return Ok(false),
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(crate::error::ConfectError::io_at(b, err)),
    };
    if meta_a.len() != meta_b.len() {
        return Ok(false);
    }
    let mut reader_a = BufReader::new(File::open(a).at(a)?);
    let mut reader_b = BufReader::new(File::open(b).at(b)?);
    let mut buf_a = [0u8; 64 * 1024];
    let mut buf_b = [0u8; 64 * 1024];
    loop {
        let n = read_full(&mut reader_a, &mut buf_a).at(a)?;
        let m = read_full(&mut reader_b, &mut buf_b).at(b)?;
        if n != m || buf_a[..n] != buf_b[..m] {
            return Ok(false);
        }
        if n == 0 {
            return Ok(true);
        }
    }
}

fn read_full(reader: &mut impl Read, buf: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match reader.read(&mut buf[filled..])? {
            0 => break,
            n => filled += n,
        }
    }
    Ok(filled)
}
