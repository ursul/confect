#![allow(dead_code)]

use assert_cmd::Command;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Output;
use tempfile::TempDir;

pub const HOST: &str = "test-host";

/// An isolated home, a fake system tree and helpers to drive the confect binary.
pub struct Env {
    pub root: TempDir,
    pub home: PathBuf,
    pub sys: PathBuf,
}

impl Env {
    pub fn new() -> Self {
        let root = tempfile::tempdir().expect("tempdir");
        let home = root.path().join("home");
        let sys = root.path().join("sys");
        fs::create_dir_all(home.join(".config")).unwrap();
        fs::create_dir_all(&sys).unwrap();
        fs::write(home.join(".gitconfig"), "").unwrap();
        Self { root, home, sys }
    }

    /// Environment with `confect init` already run.
    pub fn initialized() -> Self {
        let env = Self::new();
        env.ok(&["init", "--host", HOST]);
        env
    }

    pub fn repo(&self) -> PathBuf {
        self.home.join(".local/share/confect")
    }

    pub fn config_dir(&self) -> PathBuf {
        self.home.join(".config/confect")
    }

    pub fn cmd(&self) -> Command {
        let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("confect");
        self.apply_env(&mut cmd);
        cmd
    }

    fn apply_env(&self, cmd: &mut Command) {
        cmd.env("HOME", &self.home)
            .env("XDG_CONFIG_HOME", self.home.join(".config"))
            .env("XDG_DATA_HOME", self.home.join(".local/share"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", self.home.join(".gitconfig"))
            .env("NO_COLOR", "1")
            .env_remove("CONFECT_REPO")
            .env_remove("GIT_SSH_COMMAND")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE");
    }

    pub fn run(&self, args: &[&str]) -> Output {
        self.cmd().args(args).output().expect("run confect")
    }

    /// Run and require success; returns stdout + stderr.
    pub fn ok(&self, args: &[&str]) -> String {
        let output = self.run(args);
        let text = text(&output);
        assert!(
            output.status.success(),
            "confect {:?} failed ({}):\n{}",
            args,
            output.status,
            text
        );
        text
    }

    /// Run and require the given exit code; returns stdout + stderr.
    pub fn code(&self, args: &[&str], expected: i32) -> String {
        let output = self.run(args);
        let text = text(&output);
        assert_eq!(
            output.status.code(),
            Some(expected),
            "confect {:?} exit status:\n{}",
            args,
            text
        );
        text
    }

    pub fn sys_path(&self, relative: &str) -> PathBuf {
        self.sys.join(relative)
    }

    pub fn write(&self, relative: &str, content: &str) -> PathBuf {
        let path = self.sys.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, content).unwrap();
        path
    }

    pub fn chmod(&self, path: &Path, mode: u32) {
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }

    /// Absolute system path as a string argument.
    pub fn arg(&self, relative: &str) -> String {
        self.sys.join(relative).to_string_lossy().into_owned()
    }

    /// Where a system file is stored in the repository.
    pub fn stored(&self, category: &str, system: &Path) -> PathBuf {
        self.repo()
            .join(category)
            .join(system.strip_prefix("/").unwrap())
    }

    pub fn git(&self, dir: &Path, args: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", self.home.join(".gitconfig"))
            .env("GIT_AUTHOR_NAME", "test")
            .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
            .env("GIT_COMMITTER_NAME", "test")
            .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    pub fn head_count(&self) -> usize {
        self.git(&self.repo(), &["rev-list", "--count", "HEAD"])
            .trim()
            .parse()
            .unwrap()
    }

    /// A bare repository to use as the remote.
    pub fn bare_remote(&self, name: &str) -> PathBuf {
        let path = self.root.path().join(name);
        let output = std::process::Command::new("git")
            .args(["init", "-q", "--bare"])
            .arg(&path)
            .output()
            .unwrap();
        assert!(output.status.success());
        path
    }

    pub fn write_config(&self, content: &str) {
        fs::create_dir_all(self.config_dir()).unwrap();
        fs::write(self.config_dir().join("config.toml"), content).unwrap();
    }
}

pub fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

pub fn mode(path: &Path) -> u32 {
    fs::symlink_metadata(path).unwrap().permissions().mode() & 0o7777
}

pub fn is_root() -> bool {
    nix::unistd::Uid::effective().is_root()
}

/// A syntactically plausible PEM private key (not a real key).
pub const FAKE_KEY: &str =
    "-----BEGIN PRIVATE KEY-----\nMIIEvQIBADANBgkqhkiG9w0BAQEFAASC\n-----END PRIVATE KEY-----\n";
