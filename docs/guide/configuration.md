# Configuration

confect reads one global file per user and keeps everything else inside the repository.

## Global configuration

`~/.config/confect/config.toml` (or `$XDG_CONFIG_HOME/confect/config.toml`). Every key is
optional; these are the defaults:

```toml
[global]
# Repository directory. init writes it when you use --path or --system;
# without it confect uses ~/.local/share/confect.
repo_path = "/var/lib/confect"

# Remote that sync, push and pull talk to.
default_remote = "origin"

# Push after every sync that has a remote. Override per run with --push / --no-push.
auto_push = true

# Seconds a git network operation (clone, fetch, push, ls-remote) may take.
network_timeout = 300

[encryption]
# age identity (private key) used to read encrypted files.
identity_file = "/root/.config/confect/age-identity.txt"
# age recipients (public keys) that encrypted files are written for.
recipients_file = "/root/.config/confect/age-recipients.txt"
```

The two `[encryption]` paths default to `age-identity.txt` and `age-recipients.txt` next to
`config.toml`; [`key generate`](/commands/key) creates both.

Keys confect does not know are reported on every run and otherwise ignored:

```
Warning: /root/.config/confect/config.toml: unknown section [repository] is ignored (the repository path is [global] repo_path)
Warning: /root/.config/confect/config.toml: unknown section [sync] is ignored (auto_push belongs to [global])
```

::: warning
`confect init` rewrites this file: comments and unknown keys are not kept.
:::

## Choosing the repository

The repository is found in this order:

1. `--repo <PATH>`, accepted by every command
2. the `CONFECT_REPO` environment variable
3. `[global] repo_path`
4. `~/.local/share/confect`

```bash
confect --repo /srv/confect status
CONFECT_REPO=/srv/confect confect sync
```

## Host name

The host name decides the branch, `host/<name>`. `init` takes it from `--host` or the system
hostname and stores it in the repository (`.confect/config.toml`), so renaming the machine
later does not switch branches. Names may contain letters, digits, `.`, `_` and `-`.

## Repository layout

```
/var/lib/confect/                      # mode 0700
├── .confect/
│   ├── config.toml                    # format version and host name
│   ├── categories.toml                # categories and their patterns
│   └── metadata.toml                  # mode, owner, group and type of every stored path
├── nginx/
│   └── etc/nginx/
│       ├── nginx.conf
│       └── ssl/site.key.age           # stored encrypted
└── base/
    └── etc/ssh/sshd_config
```

Each stored copy lives under its category directory at its absolute system path. Encrypted
copies get an `.age` suffix. Symlinks are stored as symlinks. The copies themselves are
`0600` (`0700` if executable); the real permissions are in `metadata.toml`:

```toml
version = 2

[entries."/etc/nginx/nginx.conf"]
category = "nginx"
kind = "file"
mode = "0644"
owner = "root"
group = "root"
uid = 0
gid = 0

[entries."/etc/nginx/sites-enabled/default"]
category = "nginx"
kind = "symlink"
owner = "root"
group = "root"
uid = 0
gid = 0
target = "/etc/nginx/sites-available/default"
```

Directories have entries of their own (`kind = "dir"`), so empty directories and directory
permissions are restored too. The files in `.confect/` are written by confect and kept
sorted; edit `categories.toml` with the [`category`](/commands/category) commands.

Commits are made with your git identity. If git has no `user.name` / `user.email`,
confect uses `confect` / `confect@<host>`.

## Exit codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Error; nothing or only part of the work was done (see the message) |
| `3` | `status --exit-code` found differences, or `audit` found plaintext secrets |

Invalid command-line arguments exit with `2`, so monitoring can tell a typo from a finding.
