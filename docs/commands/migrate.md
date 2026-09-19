# migrate

Convert a confect 1.x repository to the 2.0 format.

## Usage

```bash
confect migrate [--yes]
```

## Options

| Option | Description |
|--------|-------------|
| `-y`, `--yes` | Do not ask for confirmation (required without a terminal) |

## What it does

```
Migrating /var/lib/confect (format 1 → 2, branch host/web1)
warning: base:/etc/nginx/nginx.conf and nginx:/etc/nginx overlap; the more specific category keeps the files and the other copy is dropped at the next sync. Remove the redundant path.
i Restricted the repository directory to its owner (0700)
✓ Indexed 42 stored path(s) and committed the migration

warning: 1 stored file(s) are plaintext secrets. Encrypt or exclude them, rotate them, and check the history with 'confect audit --history':
  ! nginx/etc/nginx/ssl/site.key (PEM private key)

Run confect sync next: it records directory permissions and anything that changed.
```

1. Warns about overlapping categories and about paths that are not absolute.
2. Rewrites `.confect/categories.toml` sorted by category.
3. Rebuilds `.confect/metadata.toml`: for every stored copy it records type, mode, owner and
   group from the system file, or from the copy itself when the system file is gone.
4. Records the tracked directories with their permissions.
5. Sets the format version to 2 and stores the host name in `.confect/config.toml`.
6. Restricts the repository directory to `0700`.
7. Commits `Migrate repository to confect 2 format`. Nothing is pushed.
8. Lists stored files that are plaintext secrets.

On a repository that is already in the 2.0 format, `migrate` only says so.

The other commands that work with the repository refuse a 1.x repository and point here. The full procedure, including configuration changes, secrets and the timer, is
in [Upgrading from 1.x](/guide/migration).
