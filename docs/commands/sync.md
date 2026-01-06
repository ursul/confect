# sync

Sync tracked files, commit changes, and push to remote.

## Usage

```bash
confect sync [OPTIONS]
```

## Options

| Option | Description |
|--------|-------------|
| `--no-push` | Commit but don't push to remote |
| `--message <MSG>`, `-m` | Custom commit message |

## Examples

### Basic sync

```bash
confect sync
```

### Without pushing

```bash
confect sync --no-push
```

### Custom commit message

```bash
confect sync -m "Update nvim config"
```

## What it does

1. **Check for changes** — Compares tracked files with repository
2. **Update repository** — Copies changed files to repo
3. **Create commit** — Commits all changes with auto-generated message
4. **Push to remote** — Pushes to origin (unless `--no-push`)

## Auto-generated commit messages

confect generates descriptive commit messages:

```
Update zsh, nvim (3 files)
```

```
Add bashrc
```
