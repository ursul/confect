# Upgrading from 1.x

confect 2.0 uses a new repository format: a real metadata index, private repository
permissions, non-overlapping categories and a guard against plaintext secrets. A 1.x
repository has to be converted once with `confect migrate`. Until then the commands that
work with the repository, apart from `info`, `audit` and `migrate`, stop with:

```
Error: Repository at /var/lib/confect uses format version 1, this confect needs 2. Run 'confect migrate' first.
```

Run the steps below on every host, as the user that ran confect 1.x (usually root). Each host
converts its own branch.

::: danger Files marked --encrypt in 1.x are not encrypted
confect 1.x accepted `--encrypt` and `encrypt` patterns but stored those files in
plaintext. They are in the repository history and on your remote. Rotate those secrets after
the upgrade (step 6).
:::

## 1. Stop the old timer and back up the repository

```bash
systemctl disable --now confect-backup.timer
tar -C /var/lib -czf /root/confect-1x-backup.tar.gz confect
```

Use your repository path if it is not `/var/lib/confect`. The archive is your way back if
anything goes wrong.

## 2. Install 2.0 and check the configuration

Install the new binary ([Installation](/guide/installation)), then:

```bash
confect info
```

`info` works on 1.x repositories and shows `format 1 (run 'confect migrate' to convert to 2)`.
It also prints a warning for every setting 2.0 does not understand. Move them to the
[2.0 keys](/guide/configuration):

| 1.x setting | 2.0 |
|-------------|-----|
| `[repository] path` | `[global] repo_path` |
| `[sync] auto_push` | `[global] auto_push` |
| `[sync] default_remote` | `[global] default_remote` |
| `[hosts] current` | fallback host name until `migrate` records the host in the repository |
| `[encryption] enabled`, `public_key`; `[hosts] strategy` | accepted, not used |

## 3. Migrate

```bash
confect migrate --yes
```

`migrate`:

- rewrites `.confect/categories.toml`, sorted by category name;
- rebuilds `.confect/metadata.toml` from the system: mode, owner, group and type of every
  stored copy (for a copy whose system file is gone, the copy's own attributes are used);
- records the tracked directories, so their permissions are restored too;
- sets the repository format to version 2 and stores the host name in `.confect/config.toml`;
- restricts the repository directory to its owner (`0700`);
- commits `Migrate repository to confect 2 format` (it does not push);
- warns about overlapping categories and relative paths;
- lists stored files that are plaintext secrets.

Without a terminal, `migrate` needs `--yes`. Running it again on a converted repository does
nothing.

## 4. Fix what migrate reported

**Overlapping categories.** 2.0 allows a path in one category only. Until you fix it, the more
specific category keeps the file and the other copy is dropped at the next sync. Remove the
redundant path:

```bash
confect category remove-path base /etc/nginx/nginx.conf
```

**Relative paths** (for example `~/.bashrc` written by hand) match nothing, and the next
`sync` drops the copies stored for them. Before syncing, replace them in
`.confect/categories.toml` with absolute paths (`/root/.bashrc`). Edit the file directly:
`category remove-path` makes its argument absolute and so cannot find the relative entry.

**Missing paths.** `confect status` warns about every tracked path that no longer exists. If it
is gone for good, untrack it:

```bash
confect status
confect remove /etc/old-service
```

## 5. Move secrets to encrypt patterns

The secret guard checks files whose content changes. Copies that 1.x already stored in
plaintext are not blocked, so handle every file `migrate` listed:

```bash
confect key generate
confect category encrypt add nginx '*.key'
confect category exclude add base /etc/ssh/ssh_host_ed25519_key
```

Create the key before the first `sync` if any category has `encrypt` patterns: files matching
them are now stored encrypted, and `sync` fails without recipients. Keep a copy of
`~/.config/confect/age-identity.txt` off the machine. See
[Secrets and Encryption](/advanced/encryption).

## 6. Check the history and rotate

```bash
confect audit
confect audit --history
```

`audit` checks the current files, `audit --history` every commit; both exit with `3` while
something is found. A secret that was ever committed is readable by everyone with access to
the repository or the remote, even after you encrypt or exclude the file. Rotate it: new keys,
new passwords, new certificates. Removing it from history needs a history rewrite and a
force-push of every affected branch, which confect does not do for you.

## 7. Sync and schedule

```bash
confect status
confect sync
rm /etc/systemd/system/confect-backup.service /etc/systemd/system/confect-backup.timer
systemctl daemon-reload
confect setup-timer --schedule hourly
```

The first `sync` after the migration records directory permissions and anything that changed.
The new timer is called `confect-sync.timer` and runs daily by default; `--schedule hourly`
keeps the 1.x interval.

## Changed commands and options

| 1.x | 2.0 |
|-----|-----|
| `init -r <url>` | `init --remote <url>` |
| `pull -r` | `pull --restore` |
| `add <path>` | `add <paths>... -c <category>`: the category is required, `--create-category` creates it |
| `remove --delete` | removed; `remove` untracks and drops the stored copies, never touches the system |
| `restore <category> -f <file>` | `restore [paths]... -c <category>` |
| `restore --force` / `-f` | `restore --yes` / `-y` |
| `diff <category> -f <file>` | `diff [paths]... -c <category>` |
| `category delete --force` / `-f` | `category delete --yes` / `-y` |
| `category delete --remove-files` | removed; deleting a category always drops its stored copies |
| `category add-path --encrypt` | `category encrypt add <name> <pattern>` |
| `sync --all-hosts` | removed; every host syncs its own branch |
| `--verbose` / `-v` | removed |
| `setup-timer`: `confect-backup.*`, hourly, enabled by hand | `confect-sync.*`, daily, enabled by `setup-timer` |

::: warning -r is gone
2.0 has no `-r` short option at all: write `--remote`, `--restore` or `--repo` in full, so a 1.x
habit fails loudly instead of doing something else.
:::

New in 2.0: [`push`](/commands/push), [`audit`](/commands/audit), [`key`](/commands/key),
[`migrate`](/commands/migrate); `status` accepts paths and `--exit-code`; `restore` has `-n`;
`init` has `--from` for [rebuilding a host](/advanced/multi-host#rebuild-a-host);
`category create` accepts `--exclude` and `--allow-plaintext`; `setup-timer` has `--message`
and `--user`; `self-update` verifies the published SHA-256 and has `--yes`.

Other behaviour that changed:

- Categories must not overlap.
- Directories are tracked with their permissions, including empty ones.
- `sync` refuses to store plaintext private keys and password hashes.
- Nothing is merged between host branches; there is no shared `main` branch workflow.
