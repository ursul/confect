# remove

Stop tracking paths and drop their stored copies. Files on the system are never touched.

## Usage

```bash
confect remove <PATHS>...
```

## Examples

```bash
confect remove /etc/hosts
confect remove /etc/nginx/conf.d/debug.conf
```

```
  - /etc/hosts (removed from 'base')
  - /etc/nginx/conf.d/debug.conf (excluded in 'nginx')
✓ Dropped 2 stored path(s)
Run confect sync to commit.
```

## What it does

- A path that is listed in a category's `paths` is removed from the category (and from its
  `encrypt` and `allow_plaintext` lists).
- A path inside a tracked directory is added to the category's `exclude` list, so it stays
  out when the directory is synced again.
- The stored copies are removed from the repository. The next [`sync`](/commands/sync)
  commits the removal; the files remain in the Git history.

A path that no category covers is an error.

To untrack a glob pattern, use
[`category remove-path`](/commands/category#add-path-remove-path) with the pattern as written
in the category.
