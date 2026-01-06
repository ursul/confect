use std::fs;
use std::path::{Path, PathBuf};
use git2::{Cred, CredentialType, FetchOptions, PushOptions, RemoteCallbacks, Repository as Git2Repo, Signature, StatusOptions};
use chrono::Utc;

use crate::core::config::{Config, RepoConfig, HostEntry};
use crate::error::{Result, ConfectError};

/// Wrapper around git2::Repository with confect-specific functionality
pub struct Repository {
    git: Git2Repo,
    path: PathBuf,
    hostname: String,
}

impl Repository {
    /// Initialize a new confect repository
    pub fn init(path: &Path, hostname: &str) -> Result<Self> {
        if path.join(".git").exists() {
            return Err(ConfectError::AlreadyInitialized(path.to_path_buf()));
        }

        // Create directory if needed
        fs::create_dir_all(path)?;

        // Initialize git repository
        let git = Git2Repo::init(path)?;

        // Create .confect directory structure
        let confect_dir = path.join(".confect");
        fs::create_dir_all(&confect_dir)?;

        // Create initial repo config
        let mut repo_config = RepoConfig::default();
        repo_config.repository.created = Some(Utc::now().to_rfc3339());
        repo_config.hosts.list.insert(
            hostname.to_string(),
            HostEntry {
                branch: format!("host/{}", hostname),
            },
        );
        repo_config.save(path)?;

        // Create empty categories file
        fs::write(confect_dir.join("categories.toml"), "[categories]\n")?;

        // Create empty metadata file
        fs::write(confect_dir.join("metadata.toml"), "[files]\n")?;

        // Create .gitignore
        fs::write(path.join(".gitignore"), "*.confect-backup\n")?;

        let repo = Self {
            git,
            path: path.to_path_buf(),
            hostname: hostname.to_string(),
        };

        // Create initial commit on main branch
        repo.commit_all("Initialize confect repository")?;

        // Create and switch to host branch
        repo.create_host_branch(hostname)?;

        Ok(repo)
    }

    /// Open an existing repository
    pub fn open(path: &Path) -> Result<Self> {
        if !path.join(".git").exists() {
            return Err(ConfectError::NotInitialized);
        }

        if !path.join(".confect").exists() {
            return Err(ConfectError::NotInitialized);
        }

        let git = Git2Repo::open(path)?;

        // Get hostname from global config
        let global_config = Config::load_global()?;
        let hostname = global_config.hosts.current.unwrap_or_else(|| {
            hostname::get()
                .map(|h| h.to_string_lossy().to_string())
                .unwrap_or_else(|_| "unknown".to_string())
        });

        Ok(Self {
            git,
            path: path.to_path_buf(),
            hostname,
        })
    }

    /// Open the default repository
    pub fn open_default() -> Result<Self> {
        let config = Config::load_global()?;
        let path = config.repo_path();
        Self::open(&path)
    }

    /// Get repository path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get current hostname
    pub fn current_host(&self) -> Result<&str> {
        Ok(&self.hostname)
    }

    /// Create a branch for a host
    fn create_host_branch(&self, hostname: &str) -> Result<()> {
        let branch_name = format!("host/{}", hostname);

        // Get HEAD commit
        let head = self.git.head()?;
        let commit = head.peel_to_commit()?;

        // Create branch
        self.git.branch(&branch_name, &commit, false)?;

        // Checkout the branch
        let refname = format!("refs/heads/{}", branch_name);
        let obj = self.git.revparse_single(&refname)?;
        self.git.checkout_tree(&obj, None)?;
        self.git.set_head(&refname)?;

        Ok(())
    }

    /// Add a remote
    pub fn add_remote(&self, name: &str, url: &str) -> Result<()> {
        self.git.remote(name, url)?;
        Ok(())
    }

    /// Check if remote exists
    pub fn has_remote(&self, name: &str) -> Result<bool> {
        Ok(self.git.find_remote(name).is_ok())
    }

    /// List remotes
    pub fn list_remotes(&self) -> Result<Vec<(String, String)>> {
        let remotes = self.git.remotes()?;
        let mut result = Vec::new();

        for remote_name in remotes.iter().flatten() {
            if let Ok(remote) = self.git.find_remote(remote_name) {
                let url = remote.url().unwrap_or("").to_string();
                result.push((remote_name.to_string(), url));
            }
        }

        Ok(result)
    }

    /// Stage all changes and commit
    pub fn commit_all(&self, message: &str) -> Result<()> {
        let mut index = self.git.index()?;

        // Add all files
        index.add_all(["."].iter(), git2::IndexAddOption::DEFAULT, None)?;
        index.write()?;

        let tree_id = index.write_tree()?;
        let tree = self.git.find_tree(tree_id)?;

        let sig = self.signature()?;

        // Get parent commit if exists
        let parent_commit = self.git.head().ok().and_then(|h| h.peel_to_commit().ok());

        match parent_commit {
            Some(parent) => {
                self.git
                    .commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent])?;
            }
            None => {
                self.git
                    .commit(Some("HEAD"), &sig, &sig, message, &tree, &[])?;
            }
        }

        Ok(())
    }

    /// Push to remote
    pub fn push(&self, remote_name: &str) -> Result<()> {
        let mut remote = self.git.find_remote(remote_name)?;

        let head = self.git.head()?;
        let branch_name = head
            .shorthand()
            .ok_or_else(|| ConfectError::Other("Could not get branch name".to_string()))?;

        let refspec = format!("refs/heads/{}:refs/heads/{}", branch_name, branch_name);

        let callbacks = create_credentials_callback();
        let mut push_options = PushOptions::new();
        push_options.remote_callbacks(callbacks);

        remote.push(&[&refspec], Some(&mut push_options))?;

        Ok(())
    }

    /// Pull from remote
    pub fn pull(&self, remote_name: &str) -> Result<()> {
        let mut remote = self.git.find_remote(remote_name)?;

        // Fetch with credentials
        let callbacks = create_credentials_callback();
        let mut fetch_options = FetchOptions::new();
        fetch_options.remote_callbacks(callbacks);

        remote.fetch(&[] as &[&str], Some(&mut fetch_options), None)?;

        // Get current branch
        let head = self.git.head()?;
        let branch_name = head.shorthand().unwrap_or("main");

        // Merge (fast-forward only)
        let fetch_head = self.git.find_reference("FETCH_HEAD")?;
        let fetch_commit = self.git.reference_to_annotated_commit(&fetch_head)?;

        let (analysis, _) = self.git.merge_analysis(&[&fetch_commit])?;

        if analysis.is_fast_forward() {
            let refname = format!("refs/heads/{}", branch_name);
            let mut reference = self.git.find_reference(&refname)?;
            reference.set_target(fetch_commit.id(), "Fast-forward")?;
            self.git.checkout_head(Some(
                git2::build::CheckoutBuilder::default().force(),
            ))?;
        }

        Ok(())
    }

    /// Get repository status
    pub fn status(&self) -> Result<Vec<(PathBuf, git2::Status)>> {
        let mut opts = StatusOptions::new();
        opts.include_untracked(true);

        let statuses = self.git.statuses(Some(&mut opts))?;

        let mut result = Vec::new();
        for entry in statuses.iter() {
            if let Some(path) = entry.path() {
                result.push((PathBuf::from(path), entry.status()));
            }
        }

        Ok(result)
    }

    /// Check if there are uncommitted changes
    pub fn has_changes(&self) -> Result<bool> {
        let status = self.status()?;
        Ok(!status.is_empty())
    }

    /// Get git signature for commits
    fn signature(&self) -> Result<Signature<'_>> {
        // Try to get from git config, fall back to defaults
        let config = self.git.config()?;

        let name = config
            .get_string("user.name")
            .unwrap_or_else(|_| "confect".to_string());
        let email = config
            .get_string("user.email")
            .unwrap_or_else(|_| "confect@localhost".to_string());

        Ok(Signature::now(&name, &email)?)
    }
}

/// Create RemoteCallbacks with authentication support
fn create_credentials_callback<'a>() -> RemoteCallbacks<'a> {
    let mut callbacks = RemoteCallbacks::new();

    callbacks.credentials(|url, username_from_url, allowed_types| {
        // Try SSH agent first
        if allowed_types.contains(CredentialType::SSH_KEY) {
            let username = username_from_url.unwrap_or("git");
            return Cred::ssh_key_from_agent(username);
        }

        // Try git credential helper for HTTPS
        if allowed_types.contains(CredentialType::USER_PASS_PLAINTEXT) {
            if let Ok(config) = git2::Config::open_default() {
                return Cred::credential_helper(&config, url, username_from_url);
            }
        }

        // Default (anonymous)
        if allowed_types.contains(CredentialType::DEFAULT) {
            return Cred::default();
        }

        Err(git2::Error::from_str("no authentication method available"))
    });

    callbacks
}
