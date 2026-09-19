# pull

Fast-forward this host's branch from the remote.

## Usage

```bash
confect pull [OPTIONS]
```

## Options

| Option | Description |
|--------|-------------|
| `--restore` | Restore the pulled files afterwards |
| `-y`, `--yes` | Do not ask for confirmation before restoring (only with `--restore`) |

## Examples

```bash
confect pull
confect pull --restore --yes
```

## What it does

`pull` fetches only `host/<name>` from `default_remote` and fast-forwards the local branch to
it. Other hosts' branches are never fetched or merged.

- If the remote does not have the branch yet: `origin has no branch host/web1 yet; nothing to pull`.
- If the local branch already contains the remote one: `host/web1 is already up to date`.
- If both have commits the other lacks, `pull` stops:

  ```
  Error: Local branch host/web1 and origin/host/web1 have diverged; resolve it with git in the repository
  ```

  Resolve it with git in the repository directory, for example by rebasing your local
  commits onto `origin/host/web1`, then run `confect push`.

`add`, `remove` and `category` change the repository before the next `sync` commits it.
`pull` refuses to run over such uncommitted changes
(`the repository has changes that are not committed yet; run 'confect sync' first`):
sync them first, then pull.

With `--restore`, confect then restores every tracked path like
[`restore --backup`](/commands/restore): it lists the changes, asks for confirmation
(`--yes` skips it) and keeps a `.confect-backup.<timestamp>` copy of every overwritten file.

You only need `pull` when the host's branch was changed from another clone, for example after
you rebuilt the machine and worked on it from a second checkout. See
[Multiple Hosts](/advanced/multi-host).
