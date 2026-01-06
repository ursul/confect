# Systemd Timer

Automate config backups with systemd user timers.

## Setup

```bash
confect setup-timer
```

This creates:
- `~/.config/systemd/user/confect.service`
- `~/.config/systemd/user/confect.timer`

## Timer schedule

By default, confect syncs every hour. Customize in the timer file:

```ini
# ~/.config/systemd/user/confect.timer
[Timer]
OnCalendar=hourly
Persistent=true
```

## Manual control

```bash
# Start timer
systemctl --user start confect.timer

# Stop timer
systemctl --user stop confect.timer

# Check status
systemctl --user status confect.timer

# View logs
journalctl --user -u confect.service
```

## Enable on boot

```bash
systemctl --user enable confect.timer
```
