use std::path::{Component, Path, PathBuf};

use crate::error::{ConfectError, Result};

/// Locations that are never tracked as a whole: too large, volatile or dangerous to restore.
const FORBIDDEN: &[&str] = &[
    "/", "/bin", "/boot", "/dev", "/home", "/lib", "/lib64", "/proc", "/root", "/run", "/sbin",
    "/sys", "/tmp", "/usr", "/var",
];

/// Turn user input into an absolute, lexically normalized path.
///
/// `~` expands to the home directory and relative paths resolve against the current
/// directory. Symlinks are deliberately not resolved: a tracked symlink stays a symlink.
pub fn absolute(input: &Path) -> Result<PathBuf> {
    let expanded = expand_tilde(input)?;
    let joined = if expanded.is_absolute() {
        expanded
    } else {
        std::env::current_dir()?.join(expanded)
    };
    Ok(normalize(&joined))
}

/// Same as [`absolute`] for category patterns, which may contain glob characters.
pub fn absolute_pattern(input: &str) -> Result<String> {
    Ok(absolute(Path::new(input))?.to_string_lossy().into_owned())
}

fn expand_tilde(input: &Path) -> Result<PathBuf> {
    let mut components = input.components();
    match components.next() {
        Some(Component::Normal(first)) if first == "~" => {
            let home = dirs::home_dir().ok_or_else(|| ConfectError::InvalidPath {
                path: input.to_path_buf(),
                reason: "home directory is unknown".to_string(),
            })?;
            Ok(home.join(components.as_path()))
        }
        _ => Ok(input.to_path_buf()),
    }
}

/// Remove `.` and resolve `..` without touching the filesystem.
pub fn normalize(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            other => result.push(other.as_os_str()),
        }
    }
    if result.as_os_str().is_empty() {
        PathBuf::from("/")
    } else {
        result
    }
}

/// Reject locations that must not be tracked or restored wholesale.
pub fn ensure_trackable(path: &Path, repo_path: &Path) -> Result<()> {
    if FORBIDDEN.iter().any(|f| path == Path::new(f)) {
        return Err(ConfectError::ForbiddenPath(path.to_path_buf()));
    }
    if path.starts_with(repo_path) {
        return Err(ConfectError::InvalidPath {
            path: path.to_path_buf(),
            reason: "it is inside the confect repository".to_string(),
        });
    }
    if path.to_str().is_none() {
        return Err(ConfectError::InvalidPath {
            path: path.to_path_buf(),
            reason: "the name is not valid UTF-8".to_string(),
        });
    }
    Ok(())
}

pub fn has_glob(pattern: &str) -> bool {
    pattern
        .chars()
        .any(|ch| matches!(ch, '*' | '?' | '[' | ']' | '{' | '}'))
}

/// The directory part of a glob pattern before its first wildcard component.
pub fn glob_base(pattern: &str) -> PathBuf {
    let mut base = PathBuf::new();
    for component in Path::new(pattern).components() {
        let text = component.as_os_str().to_string_lossy();
        if has_glob(&text) {
            break;
        }
        base.push(component.as_os_str());
    }
    base
}

/// Path of a system file inside a category directory of the repository.
pub fn repo_relative(category: &str, system_path: &Path, encrypted: bool) -> PathBuf {
    let relative = system_path.strip_prefix("/").unwrap_or(system_path);
    let mut path = PathBuf::from(category).join(relative);
    if encrypted {
        let mut name = path.file_name().unwrap_or_default().to_os_string();
        name.push(".age");
        path.set_file_name(name);
    }
    path
}

pub fn is_backup_name(name: &str) -> bool {
    name.ends_with(".confect-backup") || name.contains(".confect-backup.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_resolves_dots_lexically() {
        assert_eq!(
            normalize(Path::new("/etc/./nginx/../ssh/")),
            PathBuf::from("/etc/ssh")
        );
        assert_eq!(normalize(Path::new("/..")), PathBuf::from("/"));
    }

    #[test]
    fn glob_base_stops_at_first_wildcard() {
        assert_eq!(
            glob_base("/etc/nginx/*/site.conf"),
            PathBuf::from("/etc/nginx")
        );
        assert_eq!(glob_base("/etc/hosts"), PathBuf::from("/etc/hosts"));
    }

    #[test]
    fn repo_relative_appends_age_suffix_for_encrypted_files() {
        assert_eq!(
            repo_relative("secrets", Path::new("/etc/app/key.pem"), true),
            PathBuf::from("secrets/etc/app/key.pem.age")
        );
        assert_eq!(
            repo_relative("net", Path::new("/etc/hosts"), false),
            PathBuf::from("net/etc/hosts")
        );
    }

    #[test]
    fn forbidden_and_repo_paths_are_rejected() {
        let repo = Path::new("/var/lib/confect");
        assert!(ensure_trackable(Path::new("/etc"), repo).is_ok());
        assert!(ensure_trackable(Path::new("/usr"), repo).is_err());
        assert!(ensure_trackable(Path::new("/var/lib/confect/x"), repo).is_err());
    }

    #[test]
    fn backup_names_are_recognized() {
        assert!(is_backup_name("nginx.conf.confect-backup"));
        assert!(is_backup_name("nginx.conf.confect-backup.20260919T101500"));
        assert!(!is_backup_name("nginx.conf"));
    }
}
