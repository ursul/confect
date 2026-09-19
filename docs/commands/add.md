# add

Start tracking files or directories.

## Usage

```bash
confect add [OPTIONS] --category <CATEGORY> <PATHS>...
```

## Options

| Option | Description |
|--------|-------------|
| `-c`, `--category <NAME>` | Category to add the paths to (required) |
| `--create-category` | Create the category if it does not exist |
| `-e`, `--encrypt` | Store the files encrypted with age |

## Examples

```bash
confect add /etc/nginx -c nginx --create-category
confect add /etc/ssh/sshd_config /etc/fstab /etc/hosts -c base
confect add /etc/wireguard -c vpn --create-category --encrypt
```

## What it does

Each path is added to the category's `paths`; with `--encrypt` also to its `encrypt`
patterns. Then confect copies the files into the repository right away and prints them:

```
✓ Tracking 3 path(s) in category 'base'
  A /etc/fstab
  A /etc/hosts
  A /etc/ssh/sshd_config

Run confect sync to commit.
```

The commit is made by the next [`sync`](/commands/sync).

- A directory is tracked with everything below it, including files created later.
- Symlinks are tracked as symlinks; their targets are not followed.
- Mode, owner and group are recorded for files, directories and symlinks.
- `--encrypt` needs the recipients file; run [`confect key generate`](/commands/key) first.

## Refusals

`add` changes nothing when:

- the path does not exist;
- the category does not exist and `--create-category` is missing;
- the path is already covered by a category, overlaps one, or is excluded in one;
- the path is one of the [locations that are never tracked](/guide/categories#what-is-never-tracked)
  as a whole, such as `/usr`, `/var` or `/home`, or lies inside the repository;
- a file would be stored as a plaintext secret. Add an `encrypt` or `exclude` pattern
  first, for example by creating the category yourself:

```bash
confect category create nginx --path /etc/nginx --encrypt '*.key'
```

For finer control over what a category covers, see [Categories](/guide/categories).
