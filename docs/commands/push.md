# push

Push this host's branch to the remote.

## Usage

```bash
confect push
```

## What it does

Pushes the current commit to `host/<name>` on `default_remote` and sets it as the upstream.
`sync` does this on its own when `auto_push` is on; use `push` after `sync --no-push`, with
`auto_push = false`, or after fixing a rejected push.

```
✓ Pushed host/web1 to origin
```

or, when there is nothing new:

```
✓ origin is up to date
```

A rejected push (the remote branch has commits this repository lacks) is an error and exits
with `1`; see [`pull`](/commands/pull). Without a configured remote, `push` fails with
`no remote 'origin' configured`; add one with
[`init --remote`](/commands/init#add-a-remote-later).
