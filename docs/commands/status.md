# status

Show how the system differs from the repository: what the next `sync` would record, or what
`restore` would undo.

## Usage

```bash
confect status [OPTIONS] [PATHS]...
```

## Options

| Option | Description |
|--------|-------------|
| `[PATHS]...` | Only these paths and everything below them |
| `-c`, `--category <NAME>` | Only this category |
| `-d`, `--diff` | Show the content diff under each changed file |
| `--exit-code` | Exit with status `3` when anything differs |

## Output

```
Repository /var/lib/confect · branch host/web1
  X /etc/nginx/cache/state (no longer tracked)
  D /etc/nginx/conf.d/default.conf
  A /etc/nginx/conf.d/extra.conf
  M /etc/nginx/nginx.conf
  P /etc/nginx/sites-enabled/default mode 0644 → 0600
  M /etc/nginx/ssl/site.key (encrypted)

6 change(s). A added, M modified, P permissions/owner, D deleted, X no longer tracked.
Run confect sync to record them or confect restore to undo them.
```

| Letter | Meaning |
|--------|---------|
| `A` | New on the system, not stored yet |
| `M` | Content changed (or a file became a symlink, and so on); permission changes are shown too |
| `P` | Only mode or owner changed; the old and new values are shown |
| `D` | Stored, but gone from the system |
| `X` | Stored, but no longer tracked: now excluded, or no longer covered by the category |

Directories end with `/`. A new directory is not listed separately when files inside it are.
Warnings about missing or unreadable paths come before the list.

When nothing differs:

```
✓ The system matches the repository.
```

If a changed file looks like a plaintext secret, `status` shows the same report that would
stop `sync`. See [Secrets and Encryption](/advanced/encryption).

## Examples

```bash
confect status
confect status -c nginx
confect status /etc/nginx/nginx.conf --diff
```

For monitoring:

```bash
confect status --exit-code > /dev/null
echo $?   # 0 = matches, 3 = differences, 1 = error
```

## Notes

- `status` compares with the stored copies in the repository's working tree. `add`,
  `remove` and `category` commands update those copies immediately, so right after `add`
  the new files are not listed although they are not committed yet.
- A tracked path that is missing entirely is only warned about; its stored copies are kept
  and not listed as `D`, and `status` then says there are no differences in the paths that
  could be read. Use `confect restore --dry-run` to see what would be written back.
- Encrypted files are compared by decrypting the stored copy. Without the age identity,
  size and modification time are compared instead.
