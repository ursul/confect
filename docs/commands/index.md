# Commands

```bash
confect [--repo <PATH>] <COMMAND> [OPTIONS]
```

| Command | Purpose |
|---------|---------|
| [init](/commands/init) | Create a repository, or clone one with `--from` |
| [add](/commands/add) | Start tracking files or directories |
| [remove](/commands/remove) | Stop tracking paths and drop their stored copies |
| [status](/commands/status) | Show how the system differs from the repository |
| [diff](/commands/diff) | Show those differences as a unified diff |
| [sync](/commands/sync) | Record the system state, commit and push |
| [restore](/commands/restore) | Write stored files back to the system |
| [pull](/commands/pull) | Fast-forward this host's branch from the remote |
| [push](/commands/push) | Push this host's branch to the remote |
| [category](/commands/category) | Manage categories and their patterns |
| [audit](/commands/audit) | Look for plaintext secrets in the repository |
| [key](/commands/key) | Create or show the age key for encrypted files |
| [migrate](/commands/migrate) | Convert a 1.x repository |
| [info](/commands/info) | Show repository information |
| [setup-timer](/commands/setup-timer) | Install a systemd timer that runs `sync` |
| [self-update](/commands/self-update) | Update confect from the latest GitHub release |

`confect <command> --help` shows the options of every command.

## Global option

| Option | Description |
|--------|-------------|
| `--repo <PATH>` | Repository to use instead of the configured one. Also read from `CONFECT_REPO`. |

See [Configuration](/guide/configuration#choosing-the-repository) for how the repository is
found otherwise.

## Locking

Commands that change the repository take a lock on it. A second confect process on the same
repository prints `Waiting for another confect process to finish...` and continues when the
first one is done.

## Exit codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Error |
| `3` | [`status --exit-code`](/commands/status) found differences, or [`audit`](/commands/audit) found plaintext secrets |

Invalid command-line arguments exit with `2`, so monitoring can tell a typo from a finding.
