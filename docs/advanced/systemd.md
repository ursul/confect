# Systemd Timer

Record configuration changes automatically with a systemd timer.

## Set up

```bash
sudo confect setup-timer
```

```
✓ Installed and started confect-sync.timer in /etc/systemd/system (daily)
```

As root this installs `confect-sync.service` and `confect-sync.timer` in `/etc/systemd/system`.
The service runs `confect --repo <repository> sync --message "Automatic backup"`, so every
recorded change becomes a commit and, with `auto_push`, is pushed. Options:

```bash
sudo confect setup-timer --schedule hourly
sudo confect setup-timer --schedule '*-*-* 03:30' --message "Nightly snapshot"
```

`--schedule` takes any systemd `OnCalendar` expression; check one with
`systemd-analyze calendar '*-*-* 03:30'`. See [setup-timer](/commands/setup-timer) for all
options.

## Check it

```bash
systemctl list-timers confect-sync.timer
systemctl status confect-sync.service
journalctl -u confect-sync.service
sudo systemctl start confect-sync.service   # run it now
```

## When a run fails

The service fails (and shows up in `systemctl --failed`) when `sync` exits with an error,
for example:

- the secret guard found a plaintext secret: nothing was recorded, not even the other
  changes, until you encrypt, exclude or allow the file;
- the push was rejected or timed out: the commit exists locally and the next run pushes
  it again.

Hook your usual alerting to failed units, or check the journal. A run that starts while you
use confect by hand waits for your command to finish instead of failing.

## Monitoring drift

The timer records changes; it does not tell you about them. To be alerted when a system
differs from its recorded state, run `status` from your monitoring system:

```bash
confect status --exit-code > /dev/null
```

Exit code `0` means no differences, `3` means differences, `1` an error. `confect audit`
uses the same codes for plaintext secrets in the repository.

## User timers

Run as a non-root user (or with `--user`), `setup-timer` installs the units in
`~/.config/systemd/user` and manages them with `systemctl --user`. User timers only run
while the user has a session, unless lingering is enabled:

```bash
sudo loginctl enable-linger "$USER"
systemctl --user list-timers confect-sync.timer
```

## Remove

```bash
sudo confect setup-timer --remove
```

This stops and disables the timer and deletes both unit files.

## Upgrading from 1.x

confect 1.x installed `confect-backup.service` and `confect-backup.timer`. Remove them and
install the new timer; see [Upgrading from 1.x](/guide/migration#_7-sync-and-schedule).
