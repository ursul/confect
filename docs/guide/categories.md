# Categories

Every tracked path belongs to exactly one category. A category is a named set of patterns:

| List | What it does |
|------|--------------|
| `paths` | What the category tracks |
| `exclude` | What it leaves out, even if `paths` covers it |
| `encrypt` | Which files are stored encrypted with age |
| `allow_plaintext` | Which files the secret guard lets through unencrypted |

Categories are stored in `.confect/categories.toml` in the repository:

```toml
[categories.nginx]
description = "Web server"
paths = ["/etc/nginx"]
encrypt = ["*.key"]
exclude = ["/etc/nginx/cache", "*.bak"]

[categories.base]
paths = [
    "/etc/ssh/sshd_config",
    "/etc/fstab",
    "/etc/letsencrypt/renewal*",
]
allow_plaintext = ["/etc/ssh/ssh_host_ed25519_key.pub"]
```

Category names use letters, digits, `.`, `_` and `-`. Each category is also a directory in
the repository, so the name appears in paths and commit messages.

## Pattern syntax

All four lists use the same rules:

| Pattern | Covers |
|---------|--------|
| `/etc/nginx` | the path itself and everything below it |
| `/etc/nginx/*.conf` | every match and everything below each match; `*` does not cross `/` |
| `/etc/letsencrypt/renewal*` | `renewal/` and `renewal-hooks/` with their contents |
| `*.pem`, `cache`, `id_*` | (no `/`) any file or directory with a matching name, at any depth |

- `*`, `?` and `[...]` are glob wildcards. `*` also matches names that start with a dot.
- `/etc/nginx` does not cover `/etc/nginx-old`: matching is by path components.
- A pattern without `/` matches a name, so `cache` excludes every directory called `cache`
  and all of its content.
- On the command line, `~` expands to your home directory and relative paths are made
  absolute. Patterns without `/` are kept as typed.
- `paths` entries should always be absolute paths or absolute globs; name patterns are
  meant for `exclude`, `encrypt` and `allow_plaintext`.

Quote globs so the shell does not expand them: `confect category encrypt add nginx '*.key'`.

## No overlaps

A path may be covered by only one category. `add`, `category create` and
`category add-path` refuse a path that overlaps an existing one:

```
Error: /etc/nginx/conf.d overlaps with '/etc/nginx' in category 'nginx'
```

To split a directory between categories, track the parts separately. To leave something
out, use `exclude` (or [`confect remove`](/commands/remove)), not a second category.

Repositories converted from 1.x may still contain overlapping categories. There the most
specific category (the longest matching path) owns the file, and the other category's copy
is dropped at the next sync. [`migrate`](/commands/migrate) warns about such pairs; remove the
redundant path.

## What is never tracked

- the exact locations `/`, `/bin`, `/boot`, `/dev`, `/home`, `/lib`, `/lib64`, `/proc`,
  `/root`, `/run`, `/sbin`, `/sys`, `/tmp`, `/usr`, `/var` (paths below them, such as
  `/var/spool/cron`, are fine)
- anything inside the confect repository
- `.git` directories inside tracked directories
- restore backups, `*.confect-backup` and `*.confect-backup.<timestamp>`
- sockets, FIFOs and device files
- names that are not valid UTF-8 (skipped with a warning)

## Missing and unreadable paths

When a path listed in `paths` does not exist, its stored copies are kept and every
`status` and `sync` warns:

```
warning: /etc/ssh is missing; its stored copies are kept (untrack it with 'confect remove')
```

This protects the repository from a mount that is not there or a directory removed by
mistake. Only literal paths are protected this way: when a glob such as
`/etc/letsencrypt/renewal*` stops matching something, that is recorded as a deletion. Files
and directories that cannot be read keep their stored copy, with a warning.

## Changing categories

```bash
confect category create cron --path /etc/crontab --path /etc/cron.d --description "Scheduled jobs"
confect category add-path cron /var/spool/cron/crontabs
confect category exclude add nginx '*.bak'
confect category encrypt add nginx /etc/nginx/htpasswd
confect category remove-path cron /etc/crontab
confect category delete cron --yes
```

Every change takes effect in the repository immediately: newly covered files are copied in,
excluded or uncovered ones are dropped, and files that now match `encrypt` are re-stored
encrypted. The commit is made by the next `sync`. See [category](/commands/category) for all
subcommands.

A change stores the current state of everything it touches. A pattern without `/` touches
the whole category, so it also records other pending changes of that category. If you want
to undo such changes with `restore`, do that first.

If the change would store a file that looks like a plaintext secret, nothing is written, not
even the category change. See [Secrets and Encryption](/advanced/encryption).

## Tips

- Group by what you restore together: one service, one category.
- Prefer directories over lists of files: new files in a tracked directory are picked up
  automatically.
- Exclude caches, runtime state and generated files (`cache`, `*.db`, `*.pid`).
- Put key material under `encrypt`, not `allow_plaintext`.
