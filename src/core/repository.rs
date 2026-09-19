use std::fs::{self, OpenOptions};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use nix::fcntl::{Flock, FlockArg};

use crate::core::category::CategoryManager;
use crate::core::config::{Config, RepoConfig, RepoMeta, REPO_FORMAT_VERSION};
use crate::core::git::Git;
use crate::error::{ConfectError, IoContext, Result};

/// An opened confect repository.
pub struct Repository {
    path: PathBuf,
    git: Git,
    config: Config,
    repo_config: RepoConfig,
    host: String,
}

/// Held for the duration of a command that changes the repository.
pub struct RepoLock {
    _lock: Flock<fs::File>,
}

impl Repository {
    /// Where the repository lives: `--repo`/`CONFECT_REPO`, then the global config.
    pub fn resolve_path(explicit: Option<&Path>, config: &Config) -> PathBuf {
        explicit
            .map(Path::to_path_buf)
            .unwrap_or_else(|| config.repo_path())
    }

    /// Open a repository in the current format.
    pub fn open(explicit: Option<&Path>) -> Result<Self> {
        let repo = Self::open_any_version(explicit)?;
        let found = repo.repo_config.repository.version;
        if found != REPO_FORMAT_VERSION {
            return Err(ConfectError::UnsupportedRepoVersion {
                path: repo.path.clone(),
                found,
                expected: REPO_FORMAT_VERSION,
            });
        }
        Ok(repo)
    }

    /// Open a repository of any format version (for `migrate` and `info`).
    pub fn open_any_version(explicit: Option<&Path>) -> Result<Self> {
        let config = Config::load()?;
        let path = Self::resolve_path(explicit, &config);
        if !path.join(".git").exists() || !path.join(".confect").exists() {
            return Err(ConfectError::NotInitialized(path));
        }
        let git = Git::new(&path, config.global.network_timeout);
        let repo_config = RepoConfig::load(&path)?;
        let host = resolve_host(&repo_config, &git, &config)?;
        Ok(Self {
            path,
            git,
            config,
            repo_config,
            host,
        })
    }

    /// Create a new repository with the host branch checked out.
    pub fn create(path: &Path, host: &str, config: Config) -> Result<Self> {
        validate_host(host)?;
        if path.join(".git").exists() {
            return Err(ConfectError::AlreadyInitialized(path.to_path_buf()));
        }
        create_private_dir(path)?;
        let git = Git::init(path, &branch_for(host))?;
        let git = Git::new(git.dir(), config.global.network_timeout);
        let repo_config = new_repo_config(host);
        write_skeleton(path, &repo_config)?;
        Ok(Self {
            path: path.to_path_buf(),
            git,
            config,
            repo_config,
            host: host.to_string(),
        })
    }

    /// Clone an existing configuration repository and switch to this host's branch,
    /// starting the branch from scratch when the remote does not have it yet.
    pub fn clone_from(url: &str, path: &Path, host: &str, config: Config) -> Result<(Self, bool)> {
        validate_host(host)?;
        if path.join(".git").exists() {
            return Err(ConfectError::AlreadyInitialized(path.to_path_buf()));
        }
        if path.exists() && fs::read_dir(path).at(path)?.next().is_some() {
            return Err(ConfectError::InvalidPath {
                path: path.to_path_buf(),
                reason: "the directory is not empty".into(),
            });
        }
        let remote = config.global.default_remote.clone();
        create_private_dir(path)?;
        let git = Git::clone_bare_checkout(url, path, &remote, config.global.network_timeout)?;
        let branch = branch_for(host);
        let existing = git.remote_has_branch(&remote, &branch)?;
        let repo_config = if existing {
            git.checkout_tracking(&remote, &branch)?;
            RepoConfig::load(path)?
        } else {
            git.switch_orphan(&branch)?;
            let repo_config = new_repo_config(host);
            write_skeleton(path, &repo_config)?;
            repo_config
        };
        let repo = Self {
            path: path.to_path_buf(),
            git,
            config,
            repo_config,
            host: host.to_string(),
        };
        Ok((repo, existing))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn git(&self) -> &Git {
        &self.git
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn repo_config(&self) -> &RepoConfig {
        &self.repo_config
    }

    pub fn set_repo_config(&mut self, repo_config: RepoConfig) -> Result<()> {
        repo_config.save(&self.path)?;
        self.repo_config = repo_config;
        Ok(())
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn branch(&self) -> String {
        branch_for(&self.host)
    }

    pub fn remote(&self) -> &str {
        &self.config.global.default_remote
    }

    pub fn categories(&self) -> Result<CategoryManager> {
        CategoryManager::load(&self.path)
    }

    /// Exclusive lock against concurrent confect runs on this repository.
    pub fn lock(&self) -> Result<RepoLock> {
        let lock_path = self.path.join(".git").join("confect.lock");
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&lock_path)
            .at(&lock_path)?;
        match Flock::lock(file, FlockArg::LockExclusiveNonblock) {
            Ok(lock) => Ok(RepoLock { _lock: lock }),
            Err((file, nix::errno::Errno::EWOULDBLOCK)) => {
                eprintln!("Waiting for another confect process to finish...");
                Flock::lock(file, FlockArg::LockExclusive)
                    .map(|lock| RepoLock { _lock: lock })
                    .map_err(|(_, errno)| ConfectError::Unix(errno))
            }
            Err((_, errno)) => Err(ConfectError::Unix(errno)),
        }
    }

    /// Repository directories must not be readable by other users: git objects are
    /// world-readable by default and keep every past version of every file.
    pub fn ensure_private(&self) -> Result<bool> {
        let meta = fs::metadata(&self.path).at(&self.path)?;
        if meta.permissions().mode() & 0o077 != 0 {
            fs::set_permissions(&self.path, fs::Permissions::from_mode(0o700)).at(&self.path)?;
            return Ok(true);
        }
        Ok(false)
    }
}

pub fn branch_for(host: &str) -> String {
    format!("host/{}", host)
}

pub fn validate_host(host: &str) -> Result<()> {
    let valid = !host.is_empty()
        && !host.starts_with(['.', '-'])
        && !host.ends_with('.')
        && !host.contains("..")
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if valid {
        Ok(())
    } else {
        Err(ConfectError::InvalidHostName(host.to_string()))
    }
}

pub fn default_host() -> String {
    hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

fn resolve_host(repo_config: &RepoConfig, git: &Git, config: &Config) -> Result<String> {
    if let Some(host) = &repo_config.repository.host {
        return Ok(host.clone());
    }
    if let Some(branch) = git.current_branch()? {
        if let Some(host) = branch.strip_prefix("host/") {
            return Ok(host.to_string());
        }
    }
    Ok(config.hosts.current.clone().unwrap_or_else(default_host))
}

fn new_repo_config(host: &str) -> RepoConfig {
    RepoConfig {
        repository: RepoMeta {
            version: REPO_FORMAT_VERSION,
            created: Some(chrono::Utc::now().to_rfc3339()),
            host: Some(host.to_string()),
        },
    }
}

fn write_skeleton(path: &Path, repo_config: &RepoConfig) -> Result<()> {
    let confect_dir = path.join(".confect");
    fs::create_dir_all(&confect_dir).at(&confect_dir)?;
    repo_config.save(path)?;
    let categories = confect_dir.join("categories.toml");
    if !categories.exists() {
        fs::write(&categories, "[categories]\n").at(&categories)?;
    }
    let metadata = confect_dir.join("metadata.toml");
    if !metadata.exists() {
        fs::write(&metadata, "version = 2\n\n[entries]\n").at(&metadata)?;
    }
    Ok(())
}

fn create_private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).at(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).at(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_host;

    #[test]
    fn host_names_are_validated() {
        assert!(validate_host("s1.servers.ylkino.ru").is_ok());
        assert!(validate_host("TREMENDOUS").is_ok());
        assert!(validate_host("bad host").is_err());
        assert!(validate_host("a..b").is_err());
        assert!(validate_host("").is_err());
        assert!(validate_host("-x").is_err());
    }
}
