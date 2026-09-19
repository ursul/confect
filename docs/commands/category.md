# category

Manage categories and their patterns. See [Categories](/guide/categories) for the pattern
syntax and the rules behind them.

## Usage

```bash
confect category <COMMAND>
```

| Command | Description |
|---------|-------------|
| `list` | List categories with the number of stored paths |
| `show <NAME>` | Show a category's patterns and stored paths |
| `create <NAME> --path <PATTERN>...` | Create a category |
| `delete <NAME>` | Delete a category and its stored copies |
| `add-path <NAME> <PATTERN>` | Add a path or glob to a category |
| `remove-path <NAME> <PATTERN>` | Remove a path or glob and drop its stored copies |
| `exclude add\|remove <NAME> <PATTERN>` | Manage exclusions |
| `encrypt add\|remove <NAME> <PATTERN>` | Manage patterns of files stored encrypted |
| `allow-plaintext add\|remove <NAME> <PATTERN>` | Manage patterns the secret guard lets through |

Every change is applied to the stored copies at once; the next [`sync`](/commands/sync)
commits it. If the change would store a plaintext secret, nothing is written, not even the
category change.

## list, show

```bash
confect category list
confect category show nginx
```

```
nginx
Web server
Paths:
  /etc/nginx
Excluded:
  /etc/nginx/cache
Encrypted:
  *.key
Stored: 9
  0755 /etc/nginx root:root
  0644 /etc/nginx/nginx.conf root:root
  0600 /etc/nginx/ssl/site.key root:root (encrypted)
  ...
```

## create

```bash
confect category create nginx --path /etc/nginx --description "Web server" \
    --exclude /etc/nginx/cache --exclude '*.bak' --encrypt '*.key'
```

| Option | Description |
|--------|-------------|
| `-p`, `--path <PATTERN>` | Path or glob to track (required, repeatable) |
| `-d`, `--description <TEXT>` | Description shown by `list` and `show` |
| `-e`, `--encrypt <PATTERN>` | Files to store encrypted (repeatable) |
| `-x`, `--exclude <PATTERN>` | Paths or names to leave out (repeatable) |
| `--allow-plaintext <PATTERN>` | Files the secret guard lets through (repeatable) |

The new paths must not overlap other categories. Paths that exist are stored right away;
paths that do not exist yet are only warned about. `--encrypt` needs the age recipients
file ([`key generate`](/commands/key)).

## delete

```bash
confect category delete nginx --yes
```

Removes the category and all its stored copies from the repository. System files are not
touched. Asks for confirmation unless `--yes` (`-y`) is given; without a terminal `--yes` is
required.

## add-path, remove-path

```bash
confect category add-path base /etc/sysctl.d
confect category add-path base '/etc/letsencrypt/renewal*'
confect category remove-path base '/etc/letsencrypt/renewal*'
```

`add-path` refuses overlaps with any category. `remove-path` takes the pattern exactly as it
is listed in the category and drops the copies it no longer covers.

## exclude, encrypt, allow-plaintext

```bash
confect category exclude add nginx /etc/nginx/cache
confect category exclude add nginx '*.bak'
confect category encrypt add nginx '*.key'
confect category encrypt remove nginx '*.key'
confect category allow-plaintext add base /etc/ssh/ssh_host_ed25519_key.pub
```

- `exclude add` drops stored copies that are now excluded; `exclude remove` stores what
  becomes covered again.
- `encrypt add` re-stores matching files encrypted; `encrypt remove` stores them in plaintext
  again, which the secret guard may refuse. `encrypt add` needs the recipients file.
- `allow-plaintext` lets the secret guard pass files you decided to keep readable. It does
  not hide them from [`audit`](/commands/audit).

A pattern with `/` is made absolute and only the paths below it are re-examined. A pattern
without `/` is a name pattern and re-examines the whole category, which also records any
other pending change in it.

If `encrypt add nginx '*.key'` is refused because another file in the category is a
plaintext secret, deal with that file first (by its path), then add the name pattern.
