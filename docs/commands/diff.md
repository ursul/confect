# diff

Show the differences between the repository and the system as a unified diff.

## Usage

```bash
confect diff [OPTIONS] [PATHS]...
```

## Options

| Option | Description |
|--------|-------------|
| `[PATHS]...` | Only these paths and everything below them |
| `-c`, `--category <NAME>` | Only this category |

## Examples

```bash
confect diff
confect diff /etc/nginx
confect diff -c base
```

## Output

The stored copy is the old side (`--- repo:`), the system the new side (`+++ system:`), so
the diff shows what `sync` would record:

```diff
diff /etc/nginx/nginx.conf
--- repo:/etc/nginx/nginx.conf
+++ system:/etc/nginx/nginx.conf
@@ -1,3 +1,3 @@
 user www-data;
-worker_processes auto;
+worker_processes 4;
 pid /run/nginx.pid;

diff /etc/nginx/sites-enabled/default
mode 0644 → 0600
owner root:root → www-data:www-data
```

- New files are diffed against nothing, deleted files against nothing on the system side.
- Symlinks show their old and new target (`-> target` / `+> target`).
- Files containing NUL bytes are reported as `Binary files repo:<path> and system:<path> differ`.
- Files over 8 MiB are not diffed.
- Encrypted files are decrypted for the diff, which needs the age identity.
- Paths that are no longer tracked (`X` in `status`) have no diff.

When nothing differs, `diff` prints `No differences.`
