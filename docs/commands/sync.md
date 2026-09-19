# sync

Record the current state of the system in the repository, commit it and push it.

## Usage

```bash
confect sync [OPTIONS]
```

## Options

| Option | Description |
|--------|-------------|
| `-m`, `--message <MSG>` | Commit message (default: a summary of the changes) |
| `--no-push` | Do not push, whatever `auto_push` says |
| `--push` | Push even if `auto_push = false`; fails when no remote is configured |

## Examples

```bash
confect sync
confect sync -m "Raise nginx worker count"
confect sync --no-push
```

## What it does

1. Makes sure the repository directory is private (`0700`) and says so if it had to fix it.
2. Compares every category with the system, as [`status`](/commands/status) does.
3. Runs the secret guard. If a new or changed file looks like a plaintext secret, `sync`
   stops before writing anything and exits with `1`:

   ```
   Plaintext secrets would be stored in the repository:
     ! /etc/nginx/ssl/site.key (PEM private key, category 'nginx')

   Store them encrypted:   confect category encrypt add <category> <path>
   or do not track them:   confect category exclude add <category> <path>
   or accept the risk:     confect category allow-plaintext add <category> <path>
   Error: 1 file(s) look like plaintext secrets; nothing was written (see above)
   ```

4. Writes new and changed files into the repository, encrypting those that match `encrypt`
   patterns, drops deleted and no longer tracked ones, and updates `.confect/metadata.toml`.
5. Commits everything, if anything changed.
6. Pushes the host branch when `auto_push` is on (or `--push` is given) and the remote
   exists.

## Commit messages

Without `-m`, the message lists the categories with the Git status letters of their files:

```
Sync base (A1 D1), nginx (M2)
```

When only files under `.confect/` changed (for example only permissions, owners or category
settings), the message is `Update confect configuration`. When nothing changed:

```
✓ Nothing to commit; the repository already matches the system.
```

This is a success (exit `0`). The push still runs, so a push that failed earlier is retried.

## Pushing

confect pushes `host/<name>` to `default_remote` with the system `git`, so your SSH config,
keys and credential helpers apply. A rejected push is an error (exit `1`); the commit stays in
the local repository. That happens when the branch on the remote has commits this
repository does not have; see [`pull`](/commands/pull). Without a remote, `sync` only commits.

## What is skipped

- A tracked path that is missing entirely keeps its stored copies, with a warning.
- Files and directories that cannot be read keep their stored copies, with a warning.
- `.git` directories, `*.confect-backup*` files, sockets, FIFOs and devices are skipped.
- Names that are not valid UTF-8 are skipped with a warning.

If some files cannot be written to the repository, `sync` reports them and exits with `1`
without committing.
