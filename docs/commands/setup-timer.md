# setup-timer

Install a systemd timer that runs `confect sync` on a schedule.

## Usage

```bash
confect setup-timer [OPTIONS]
```

## Options

| Option | Description |
|--------|-------------|
| `-s`, `--schedule <CALENDAR>` | systemd `OnCalendar` expression (default: `daily`) |
| `-m`, `--message <MSG>` | Commit message of the timer's syncs (default: `Automatic backup`) |
| `--user` | Install user units, even as root (always the case for other users) |
| `--remove` | Stop, disable and delete the timer |

## Examples

```bash
sudo confect setup-timer
sudo confect setup-timer --schedule hourly
sudo confect setup-timer --schedule '*-*-* 03:30' --message "Nightly snapshot"
sudo confect setup-timer --remove
```

## What it installs

Two units, `confect-sync.service` and `confect-sync.timer`:

- as root: in `/etc/systemd/system`;
- otherwise (or with `--user`): in `~/.config/systemd/user`, managed with `systemctl --user`.

The service runs the current confect binary with the current repository:

```ini
[Service]
Type=oneshot
ExecStart="/usr/local/bin/confect" --repo "/var/lib/confect" sync --message "Automatic backup"
TimeoutStartSec=600s
Nice=10
IOSchedulingClass=idle
```

`TimeoutStartSec` is `network_timeout` plus 300 seconds. The timer uses `Persistent=true`
(a run missed while the machine was off happens at the next boot) and a random delay of up
to five minutes. `setup-timer` reloads systemd and enables and starts the timer itself.
Running it again rewrites both units with the new options and reloads systemd.

As root, confect warns when its binary is not owned by root or is writable by other users:
the timer would run whatever is put there.

`--remove` does not need a repository. See [Systemd Timer](/advanced/systemd) for logs,
failures and monitoring.
