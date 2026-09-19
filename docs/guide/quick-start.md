# Quick Start

This walk-through tracks the configuration of a web server, `web1`, as root. Replace the
remote URL with a repository you can push to.

## 1. Create the repository

```bash
sudo -i
confect init --system --remote git@git.example.com:ops/configs.git
```

```
✓ Created repository at /var/lib/confect on branch host/web1
Remote origin: git@git.example.com:ops/configs.git
```

`--system` puts the repository in `/var/lib/confect` and remembers that path in
`/root/.config/confect/config.toml`, so later commands find it without options. The
directory is created with mode `0700`. Each host works on its own branch,
`host/<hostname>`; see [Multiple Hosts](/advanced/multi-host).

## 2. Track files

Paths are grouped into [categories](/guide/categories). `add` needs one with `-c`; add
`--create-category` the first time:

```bash
confect add /etc/nginx -c nginx --create-category
confect add /etc/ssh/sshd_config /etc/fstab /etc/hosts -c base --create-category
```

```
i Creating category 'nginx'
✓ Tracking 12 path(s) in category 'nginx'
  A /etc/nginx/nginx.conf
  A /etc/nginx/sites-enabled/default
  ...
Run confect sync to commit.
```

A directory is tracked with everything below it, including files created there later.
`add` copies the files into the repository right away; the commit happens on `sync`.

If a directory holds private keys or password hashes, `add` stops and writes nothing:

```
Plaintext secrets would be stored in the repository:
  ! /etc/nginx/ssl/site.key (PEM private key, category 'nginx')
```

Store such files encrypted instead. Create an age key once, then create the category with
an `encrypt` pattern (and leave out what you don't want at all):

```bash
confect key generate
confect category create nginx --path /etc/nginx --encrypt '*.key' --exclude /etc/nginx/cache
```

Keep a copy of `/root/.config/confect/age-identity.txt` somewhere safe, away from this
machine: without it the encrypted files cannot be read. See
[Secrets and Encryption](/advanced/encryption).

## 3. Commit and push

```bash
confect sync -m "Initial configuration"
```

```
✓ Committed: Initial configuration
✓ Pushed host/web1 to origin
```

Without `-m`, the message summarizes the change, for example `Sync base (A1), nginx (M2)`.

## 4. Day to day

```bash
confect status            # what changed on the system since the last recording
confect diff              # the same as a unified diff
confect sync              # record it: commit and push
confect restore -n        # preview writing the stored files back
confect restore --backup  # undo local changes, keeping a copy of what gets overwritten
```

`status` lists changes with a letter: `A` added, `M` modified, `P` permissions or owner,
`D` deleted, `X` no longer tracked.

## 5. Automate

```bash
confect setup-timer
```

This installs and starts `confect-sync.timer`, which runs `confect sync` daily. See
[Systemd Timer](/advanced/systemd).

## 6. Rebuild a machine

On a reinstalled `web1`, put the age identity back first (if you use encryption), then clone
the host's branch and restore:

```bash
sudo -i
mkdir -p ~/.config/confect
cp /media/backup/age-identity.txt /media/backup/age-recipients.txt ~/.config/confect/
confect init --system --from git@git.example.com:ops/configs.git --host web1
confect restore --dry-run
confect restore --yes
```

## Next steps

- [Configuration](/guide/configuration) — global settings and repository layout
- [Categories](/guide/categories) — patterns, exclusions and name patterns
- [Commands](/commands/) — every command and option
