use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cli::ui;
use crate::core::{Config, Repository};
use crate::error::{ConfectError, IoContext, Result};

const SERVICE: &str = "confect-sync.service";
const TIMER: &str = "confect-sync.timer";

pub struct TimerOptions {
    pub schedule: String,
    pub message: String,
    pub user: bool,
    pub remove: bool,
}

/// Unit files are line-based: a value with a newline would add directives of its own.
fn validate(options: &TimerOptions) -> Result<()> {
    if options.message.chars().any(char::is_control) {
        return Err(ConfectError::Other(
            "the timer message must be a single line without control characters".into(),
        ));
    }
    let schedule_ok = !options.schedule.is_empty()
        && options
            .schedule
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || " *:/,.~-".contains(c));
    if !schedule_ok {
        return Err(ConfectError::Other(format!(
            "'{}' is not an OnCalendar expression",
            options.schedule
        )));
    }
    Ok(())
}

pub fn run(explicit_repo: Option<&Path>, options: TimerOptions) -> Result<()> {
    validate(&options)?;
    let root = nix::unistd::Uid::effective().is_root();
    let user = options.user || !root;
    let dir = if user {
        dirs::config_dir()
            .ok_or_else(|| ConfectError::Config("cannot determine the config directory".into()))?
            .join("systemd/user")
    } else {
        PathBuf::from("/etc/systemd/system")
    };

    if options.remove {
        let _ = systemctl(user, &["disable", "--now", TIMER]);
        for name in [TIMER, SERVICE] {
            let path = dir.join(name);
            if path.exists() {
                fs::remove_file(&path).at(&path)?;
            }
        }
        systemctl(user, &["daemon-reload"])?;
        ui::success("Removed the confect sync timer");
        return Ok(());
    }

    let config = Config::load()?;
    let repo = Repository::open(explicit_repo)?;
    let exe = std::env::current_exe()?;
    if !user {
        let meta = fs::metadata(&exe).at(&exe)?;
        if meta.uid() != 0 || meta.mode() & 0o022 != 0 {
            ui::warn(&format!(
                "{} is not owned by root or is writable by others; a root timer would run whatever is put there",
                exe.display()
            ));
        }
    }

    let service = format!(
        "[Unit]\n\
         Description=Record configuration changes with confect\n\
         After=network-online.target\n\
         Wants=network-online.target\n\
         \n\
         [Service]\n\
         Type=oneshot\n\
         ExecStart={} --repo {} sync --message {}\n\
         TimeoutStartSec={}s\n\
         Nice=10\n\
         IOSchedulingClass=idle\n",
        quote(&exe.to_string_lossy()),
        quote(&repo.path().to_string_lossy()),
        quote(&options.message),
        config.global.network_timeout + 300
    );
    let timer = format!(
        "[Unit]\n\
         Description=Run confect sync on a schedule\n\
         \n\
         [Timer]\n\
         OnCalendar={}\n\
         Persistent=true\n\
         RandomizedDelaySec=300\n\
         \n\
         [Install]\n\
         WantedBy=timers.target\n",
        options.schedule
    );

    fs::create_dir_all(&dir).at(&dir)?;
    fs::write(dir.join(SERVICE), service).at(dir.join(SERVICE))?;
    fs::write(dir.join(TIMER), timer).at(dir.join(TIMER))?;
    systemctl(user, &["daemon-reload"])?;
    systemctl(user, &["enable", "--now", TIMER])?;
    ui::success(&format!(
        "Installed and started {} in {} ({})",
        TIMER,
        dir.display(),
        options.schedule
    ));
    Ok(())
}

/// Quote a value for a systemd ExecStart line.
fn quote(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%")
        .replace('$', "$$");
    format!("\"{}\"", escaped)
}

fn systemctl(user: bool, args: &[&str]) -> Result<()> {
    let mut command = Command::new("systemctl");
    if user {
        command.arg("--user");
    }
    let output = command
        .args(args)
        .output()
        .map_err(|e| ConfectError::Other(format!("systemctl: {}", e)))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(ConfectError::Other(format!(
            "systemctl {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::{quote, validate, TimerOptions};

    fn options(schedule: &str, message: &str) -> TimerOptions {
        TimerOptions {
            schedule: schedule.into(),
            message: message.into(),
            user: true,
            remove: false,
        }
    }

    #[test]
    fn injected_directives_are_rejected() {
        assert!(validate(&options("daily", "Automatic backup")).is_ok());
        assert!(validate(&options("Mon..Fri *-*-* 03:00:00", "x")).is_ok());
        assert!(validate(&options("daily", "sync\n[Service]\nExecStart=/bin/id")).is_err());
        assert!(validate(&options("daily\nExecStart=/bin/id", "x")).is_err());
    }

    #[test]
    fn exec_start_values_are_quoted() {
        assert_eq!(quote("/usr/bin/confect"), "\"/usr/bin/confect\"");
        assert_eq!(quote("say \"hi\" 100%"), "\"say \\\"hi\\\" 100%%\"");
    }
}
