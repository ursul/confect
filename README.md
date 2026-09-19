# confect

> Keep system configuration files in Git

[![CI](https://github.com/ursul/confect/actions/workflows/ci.yml/badge.svg)](https://github.com/ursul/confect/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**confect** records the configuration of a Linux machine (`/etc/nginx`, `/etc/ssh`, cron
jobs, ...) in a Git repository, shows what changed, and writes it back when you need it:
after a bad edit or on a rebuilt machine.

## Features

- **One branch per host.** `host/<name>` in one shared remote; hosts never merge.
- **Status, diff, sync, restore.** Real unified diffs, commits with a summary of what changed,
  atomic restores that never write through symlinks.
- **Permissions and owners.** Mode, owner and group of every file, directory and symlink are
  recorded and restored.
- **Categories.** Group paths by purpose, with exclusions and name patterns such as `*.pem`.
- **Secrets.** Files matching `encrypt` patterns are stored with [age](https://age-encryption.org);
  plaintext private keys and password hashes stop the sync; `audit` searches the history.
- **Systemd timer.** `setup-timer` records changes on a schedule.

## Installation

Requires Linux (x86_64 or aarch64) and Git 2.28 or newer.

```bash
ARCH=x86_64   # or aarch64
curl -LO https://github.com/ursul/confect/releases/latest/download/confect-linux-$ARCH.tar.gz
curl -LO https://github.com/ursul/confect/releases/latest/download/confect-linux-$ARCH.tar.gz.sha256
sha256sum -c confect-linux-$ARCH.tar.gz.sha256
tar xzf confect-linux-$ARCH.tar.gz
sudo install -m 0755 confect /usr/local/bin/confect
```

Or from source: `cargo install --git https://github.com/ursul/confect`. Later updates:
`sudo confect self-update`.

## Quick start

Run confect as root, so its configuration, keys and repository belong to one user:

```bash
sudo -i
confect init --system --remote git@git.example.com:ops/configs.git
confect add /etc/nginx -c nginx --create-category
confect add /etc/ssh/sshd_config /etc/fstab -c base --create-category
confect sync -m "Initial configuration"
confect setup-timer
```

Day to day:

```bash
confect status            # A added, M modified, P permissions/owner, D deleted, X no longer tracked
confect diff              # unified diff, repository → system
confect sync              # commit and push
confect restore -n        # preview writing the stored files back
confect restore --backup  # write them back, keeping a copy of what gets overwritten
```

Rebuild a machine from its branch:

```bash
confect init --system --from git@git.example.com:ops/configs.git --host web1
confect restore --dry-run
confect restore --yes
```

## Secrets

```bash
confect key generate                       # age key in ~/.config/confect/
confect category encrypt add nginx '*.key' # store matching files encrypted
confect audit --history                    # plaintext secrets in any commit?
```

Keep a copy of `~/.config/confect/age-identity.txt` outside the machine: without it the
encrypted files cannot be restored.

## Configuration

`~/.config/confect/config.toml`:

```toml
[global]
repo_path = "/var/lib/confect"   # written by init --path / --system
default_remote = "origin"
auto_push = true
network_timeout = 300            # seconds per git network operation

[encryption]
identity_file = "/root/.config/confect/age-identity.txt"
recipients_file = "/root/.config/confect/age-recipients.txt"
```

`--repo <PATH>` or `CONFECT_REPO` selects another repository for a single command.

## Upgrading from 1.x

confect 2.0 uses a new repository format. Back up the repository, install 2.0, then run
`confect migrate --yes` and follow the
[upgrade guide](https://ursul.github.io/confect/guide/migration). Several options changed:
`-r` is gone (use `--remote`, `--restore` or `--repo`), `restore --force` is `--yes`, the timer is `confect-sync.timer`.

## Documentation

Full documentation: [ursul.github.io/confect](https://ursul.github.io/confect)

## License

MIT
