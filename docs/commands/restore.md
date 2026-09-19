# restore

Write the stored files back to the system.

## Usage

```bash
confect restore [OPTIONS] [PATHS]...
```

## Options

| Option | Description |
|--------|-------------|
| `[PATHS]...` | Only these paths and everything below them |
| `-c`, `--category <NAME>` | Only this category |
| `-n`, `--dry-run` | Only show what would change |
| `-y`, `--yes` | Do not ask for confirmation (required without a terminal) |
| `-b`, `--backup` | Keep a timestamped copy of every file that gets overwritten |

## Examples

```bash
confect restore --dry-run
confect restore /etc/nginx --backup
confect restore -c base --yes
```

## Output

```
  p /etc/nginx/conf.d permissions/owner
  ~ /etc/nginx/nginx.conf overwrite
  + /etc/nginx/sites-enabled/default create
  ~ /etc/nginx/ssl/site.key overwrite (encrypted)
4 path(s) to restore, 9 unchanged.
Write these files to the system? [y/N]
```

Paths that already match are skipped. With `--dry-run`, confect stops after the list.

## How files are written

- Only paths that differ are written.
- Files are written to a temporary file in the same directory, get their mode and owner,
  and are renamed into place, so readers never see a half-written file.
- A symlink at the target path is replaced, never written through. Symlinks are restored as
  symlinks.
- A directory where a file should be is not replaced; that path fails.
- Directories are created as needed; their mode and owner are applied last.
- Owners are looked up by name first, then by the stored uid/gid, so files keep the right
  owner on a machine where the IDs differ. As a non-root user, owners cannot be changed and
  confect warns.
- Encrypted files are decrypted with the age identity.

`restore` never deletes anything: files that exist on the system but not in the repository
are left alone.

## Backups

With `--backup`, a file whose content is about to be overwritten is first copied next to
itself:

```
/etc/nginx/nginx.conf.confect-backup.20260919T101500
```

The timestamp is local time. Backups are never picked up by `sync`. Remove them when you no
longer need them.

## Errors

If some paths fail, the others are still restored; the failures are listed and `restore`
exits with `1`. A path that is not tracked is an error before anything is written.
