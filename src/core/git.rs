use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Duration;

use wait_timeout::ChildExt;

use crate::error::{ConfectError, Result};

/// Thin wrapper over the system `git` binary.
///
/// Network operations go through git itself so that `~/.ssh/config`, credential helpers
/// and `core.sshCommand` behave exactly as they do for the administrator's own git.
pub struct Git {
    dir: PathBuf,
    network_timeout: Duration,
}

pub struct PushOutcome {
    pub up_to_date: bool,
}

impl Git {
    pub fn new(dir: &Path, network_timeout_secs: u64) -> Self {
        Self {
            dir: dir.to_path_buf(),
            network_timeout: Duration::from_secs(network_timeout_secs.max(1)),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// `git init` with the host branch as the initial branch.
    pub fn init(dir: &Path, branch: &str) -> Result<Self> {
        let output = base_command()
            .arg("init")
            .arg("-q")
            .arg("-b")
            .arg(branch)
            .arg(dir)
            .output()
            .map_err(spawn_error)?;
        check("init", &output)?;
        Ok(Self::new(dir, 300))
    }

    /// Clone without checking anything out; the caller picks the host branch.
    pub fn clone_bare_checkout(
        url: &str,
        dir: &Path,
        remote: &str,
        timeout_secs: u64,
    ) -> Result<Self> {
        let mut command = base_command();
        command
            .arg("clone")
            .arg("-q")
            .arg("--no-checkout")
            .arg("--origin")
            .arg(remote)
            .arg("--end-of-options")
            .arg(url)
            .arg(dir);
        let configured = global_config_get("core.sshCommand");
        apply_ssh_defaults(&mut command, configured.as_deref());
        let output = run_with_timeout(command, Duration::from_secs(timeout_secs), "clone")?;
        check("clone", &output)?;
        Ok(Self::new(dir, timeout_secs))
    }

    fn command(&self) -> Command {
        let mut command = base_command();
        command
            .arg("-C")
            .arg(&self.dir)
            .arg("-c")
            .arg(format!("safe.directory={}", self.dir.display()));
        command
    }

    fn run(&self, name: &str, args: &[&str]) -> Result<String> {
        let output = self.command().args(args).output().map_err(spawn_error)?;
        check(name, &output)?;
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    fn run_status(&self, args: &[&str]) -> Result<Output> {
        self.command().args(args).output().map_err(spawn_error)
    }

    fn run_network(&self, name: &str, args: &[&str]) -> Result<Output> {
        let mut command = self.command();
        command.args(args);
        let ssh_command = self.config_get("core.sshCommand")?;
        apply_ssh_defaults(&mut command, ssh_command.as_deref());
        run_with_timeout(command, self.network_timeout, name)
    }

    pub fn config_get(&self, key: &str) -> Result<Option<String>> {
        let output = self.run_status(&["config", "--get", key])?;
        if output.status.success() {
            let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok((!value.is_empty()).then_some(value))
        } else {
            Ok(None)
        }
    }

    pub fn current_branch(&self) -> Result<Option<String>> {
        let output = self.run_status(&["symbolic-ref", "--short", "-q", "HEAD"])?;
        if output.status.success() {
            Ok(Some(
                String::from_utf8_lossy(&output.stdout).trim().to_string(),
            ))
        } else {
            Ok(None)
        }
    }

    pub fn has_commits(&self) -> Result<bool> {
        Ok(self
            .run_status(&["rev-parse", "--verify", "-q", "HEAD"])?
            .status
            .success())
    }

    /// Stage everything, including files a tracked `.gitignore` would hide.
    pub fn add_all(&self) -> Result<()> {
        self.run("add", &["add", "--all", "--force", "--", "."])?;
        Ok(())
    }

    pub fn has_staged_changes(&self) -> Result<bool> {
        if !self.has_commits()? {
            let output = self.run("ls-files", &["ls-files", "--cached"])?;
            return Ok(!output.trim().is_empty());
        }
        let output = self.run_status(&["diff", "--cached", "--quiet"])?;
        match output.status.code() {
            Some(0) => Ok(false),
            Some(1) => Ok(true),
            _ => {
                check("diff --cached", &output)?;
                Ok(false)
            }
        }
    }

    /// Paths staged for the next commit with their status letter (A, M, D, ...).
    pub fn staged(&self) -> Result<Vec<(char, String)>> {
        let args: &[&str] = if self.has_commits()? {
            &["diff", "--cached", "--name-status", "-z", "--no-renames"]
        } else {
            &[
                "diff",
                "--cached",
                "--name-status",
                "-z",
                "--no-renames",
                "--root",
            ]
        };
        let output = self.run("diff --cached", args)?;
        let mut parts = output.split('\0').filter(|p| !p.is_empty());
        let mut result = Vec::new();
        while let (Some(status), Some(path)) = (parts.next(), parts.next()) {
            result.push((status.chars().next().unwrap_or('?'), path.to_string()));
        }
        Ok(result)
    }

    pub fn commit(&self, message: &str, fallback_email_host: &str) -> Result<()> {
        let mut command = self.command();
        if self.config_get("user.name")?.is_none() {
            command.arg("-c").arg("user.name=confect");
        }
        if self.config_get("user.email")?.is_none() {
            command
                .arg("-c")
                .arg(format!("user.email=confect@{}", fallback_email_host));
        }
        let output = command
            .args(["commit", "-q", "--no-verify", "-m", message])
            .output()
            .map_err(spawn_error)?;
        check("commit", &output)
    }

    pub fn remote_url(&self, remote: &str) -> Result<Option<String>> {
        let output = self.run_status(&["remote", "get-url", "--end-of-options", remote])?;
        if output.status.success() {
            Ok(Some(
                String::from_utf8_lossy(&output.stdout).trim().to_string(),
            ))
        } else {
            Ok(None)
        }
    }

    pub fn remotes(&self) -> Result<Vec<(String, String)>> {
        let names = self.run("remote", &["remote"])?;
        let mut result = Vec::new();
        for name in names.lines().filter(|l| !l.is_empty()) {
            if let Some(url) = self.remote_url(name)? {
                result.push((name.to_string(), url));
            }
        }
        Ok(result)
    }

    pub fn add_remote(&self, remote: &str, url: &str) -> Result<()> {
        self.run(
            "remote add",
            &["remote", "add", "--end-of-options", remote, url],
        )?;
        Ok(())
    }

    /// Push the current branch; a rejected update is an error, not a silent success.
    pub fn push(&self, remote: &str, branch: &str) -> Result<PushOutcome> {
        let refspec = format!("HEAD:refs/heads/{}", branch);
        let output = self.run_network(
            "push",
            &["push", "--porcelain", "--end-of-options", remote, &refspec],
        )?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let rejected: Vec<&str> = stdout
            .lines()
            .filter(|l| l.starts_with('!') && l.contains('\t'))
            .collect();
        if !output.status.success() || !rejected.is_empty() {
            let mut message = stderr_text(&output);
            for line in rejected {
                message.push('\n');
                message.push_str(line);
            }
            return Err(ConfectError::Git {
                command: "push".into(),
                message,
            });
        }
        let ref_lines: Vec<&str> = stdout.lines().filter(|l| l.contains('\t')).collect();
        let up_to_date = !ref_lines.is_empty() && ref_lines.iter().all(|l| l.starts_with('='));
        self.run_status(&[
            "branch",
            "--set-upstream-to",
            &format!("{}/{}", remote, branch),
        ])
        .ok();
        Ok(PushOutcome { up_to_date })
    }

    /// Fetch one branch; `Ok(false)` when the remote does not have it.
    pub fn fetch_branch(&self, remote: &str, branch: &str) -> Result<bool> {
        let refspec = format!("+refs/heads/{0}:refs/remotes/{1}/{0}", branch, remote);
        let output = self.run_network(
            "fetch",
            &[
                "fetch",
                "--no-tags",
                "-q",
                "--end-of-options",
                remote,
                &refspec,
            ],
        )?;
        if output.status.success() {
            return Ok(true);
        }
        let stderr = stderr_text(&output);
        if stderr.contains("couldn't find remote ref") {
            return Ok(false);
        }
        Err(ConfectError::Git {
            command: "fetch".into(),
            message: stderr,
        })
    }

    /// Whether the working tree or index differs from HEAD.
    pub fn is_dirty(&self) -> Result<bool> {
        let output = self.run(
            "status",
            &["status", "--porcelain", "--untracked-files=all"],
        )?;
        Ok(!output.trim().is_empty())
    }

    /// Fast-forward to the fetched branch; divergence is reported instead of merged.
    pub fn fast_forward(&self, remote: &str, branch: &str) -> Result<bool> {
        let upstream = format!("{}/{}", remote, branch);
        if !self.has_commits()? {
            self.run("checkout", &["checkout", "-q", "-B", branch, &upstream])?;
            return Ok(true);
        }
        let head = self.run("rev-parse", &["rev-parse", "HEAD"])?;
        let theirs = self.run("rev-parse", &["rev-parse", &upstream])?;
        if head.trim() == theirs.trim() {
            return Ok(false);
        }
        let output = self.run_status(&["merge", "--ff-only", "-q", &upstream])?;
        if output.status.success() {
            return Ok(true);
        }
        let base = self.run_status(&["merge-base", "--is-ancestor", &upstream, "HEAD"])?;
        if base.status.success() {
            return Ok(false);
        }
        Err(ConfectError::Diverged {
            remote: remote.into(),
            branch: branch.into(),
        })
    }

    pub fn remote_has_branch(&self, remote: &str, branch: &str) -> Result<bool> {
        let output = self.run_network(
            "ls-remote",
            &[
                "ls-remote",
                "--heads",
                "--end-of-options",
                remote,
                &format!("refs/heads/{}", branch),
            ],
        )?;
        check("ls-remote", &output)?;
        Ok(!output.stdout.is_empty())
    }

    pub fn checkout_tracking(&self, remote: &str, branch: &str) -> Result<()> {
        self.run(
            "checkout",
            &[
                "checkout",
                "-q",
                "-B",
                branch,
                "--track",
                &format!("{}/{}", remote, branch),
            ],
        )?;
        Ok(())
    }

    pub fn switch_orphan(&self, branch: &str) -> Result<()> {
        self.run("switch", &["switch", "-q", "--orphan", branch])?;
        Ok(())
    }

    /// Commits reachable from any ref, newest first.
    pub fn all_commits(&self) -> Result<Vec<String>> {
        let output = self.run("rev-list", &["rev-list", "--all"])?;
        Ok(output.lines().map(str::to_string).collect())
    }

    /// Every path that ever existed in any commit.
    pub fn all_paths_in_history(&self) -> Result<Vec<String>> {
        let output = self.run(
            "log",
            &[
                "log",
                "--all",
                "--format=",
                "--name-only",
                "-z",
                "--no-renames",
            ],
        )?;
        let mut paths: Vec<String> = output
            .split(['\0', '\n'])
            .filter(|p| !p.is_empty())
            .map(str::to_string)
            .collect();
        paths.sort();
        paths.dedup();
        Ok(paths)
    }

    /// Abbreviated commits that added or changed `path`, newest first.
    pub fn commits_touching(&self, path: &str) -> Result<Vec<String>> {
        let output = self.run(
            "log",
            &[
                "log",
                "--all",
                "--format=%h",
                "--end-of-options",
                "--",
                path,
            ],
        )?;
        Ok(output.lines().map(str::to_string).collect())
    }

    /// Files in `commit` containing any of the fixed strings, binary files included.
    pub fn grep_fixed(&self, commit: &str, needles: &[&str]) -> Result<Vec<String>> {
        let mut args = vec!["grep", "-a", "-l", "-F", "-z"];
        for needle in needles {
            args.push("-e");
            args.push(needle);
        }
        args.push(commit);
        let output = self.run_status(&args)?;
        match output.status.code() {
            Some(0) => {}
            Some(1) => return Ok(Vec::new()),
            _ => check("grep", &output).map(|_| ())?,
        }
        let prefix = format!("{}:", commit);
        Ok(String::from_utf8_lossy(&output.stdout)
            .split('\0')
            .filter(|p| !p.is_empty())
            .map(|p| p.strip_prefix(&prefix).unwrap_or(p).to_string())
            .collect())
    }

    pub fn show_blob(&self, commit: &str, path: &str) -> Result<Vec<u8>> {
        let output = self.run_status(&["show", &format!("{}:{}", commit, path)])?;
        check("show", &output)?;
        Ok(output.stdout)
    }
}

/// A setting from the user's or system git configuration (no repository yet).
fn global_config_get(key: &str) -> Option<String> {
    let output = base_command()
        .args(["config", "--get", key])
        .output()
        .ok()?;
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (output.status.success() && !value.is_empty()).then_some(value)
}

fn base_command() -> Command {
    let mut command = Command::new("git");
    command
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null());
    command
}

/// Never let ssh wait for a password or a host-key answer on a terminal nobody watches.
fn apply_ssh_defaults(command: &mut Command, configured: Option<&str>) {
    if std::env::var_os("GIT_SSH_COMMAND").is_none() && configured.is_none() {
        command.env(
            "GIT_SSH_COMMAND",
            "ssh -o BatchMode=yes -o ConnectTimeout=30 -o ServerAliveInterval=15 -o ServerAliveCountMax=4",
        );
    }
}

fn spawn_error(err: std::io::Error) -> ConfectError {
    if err.kind() == std::io::ErrorKind::NotFound {
        ConfectError::GitNotFound
    } else {
        ConfectError::Io(err)
    }
}

fn stderr_text(output: &Output) -> String {
    let text = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if text.is_empty() {
        format!("exit status {}", output.status)
    } else {
        text
    }
}

fn check(name: &str, output: &Output) -> Result<()> {
    if output.status.success() {
        Ok(())
    } else {
        Err(ConfectError::Git {
            command: name.to_string(),
            message: stderr_text(output),
        })
    }
}

fn run_with_timeout(mut command: Command, timeout: Duration, name: &str) -> Result<Output> {
    use std::os::unix::process::CommandExt;

    // Own process group, so a timeout also kills the ssh child git started.
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let mut child = command.spawn().map_err(spawn_error)?;

    let mut stdout = child.stdout.take().expect("piped stdout");
    let mut stderr = child.stderr.take().expect("piped stderr");
    let out_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });
    let err_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr.read_to_end(&mut buf);
        buf
    });

    let status = match child.wait_timeout(timeout)? {
        Some(status) => status,
        None => {
            let group = nix::unistd::Pid::from_raw(child.id() as i32);
            let _ = nix::sys::signal::killpg(group, nix::sys::signal::Signal::SIGKILL);
            let _ = child.kill();
            let _ = child.wait();
            return Err(ConfectError::GitTimeout {
                command: name.to_string(),
                seconds: timeout.as_secs(),
            });
        }
    };

    Ok(Output {
        status,
        stdout: out_reader.join().unwrap_or_default(),
        stderr: err_reader.join().unwrap_or_default(),
    })
}
