# restore

Restore tracked files from the repository to the system.

## Usage

```bash
confect restore [OPTIONS] [PATH]...
```

## Arguments

| Argument | Description |
|----------|-------------|
| `[PATH]` | Specific files to restore (default: all) |

## Options

| Option | Description |
|--------|-------------|
| `--dry-run`, `-n` | Show what would be restored without making changes |
| `--backup`, `-b` | Create `.confect-backup` of existing files |
| `--force`, `-f` | Overwrite without confirmation |

## Examples

### Restore all files

```bash
confect restore
```

### Restore specific file

```bash
confect restore ~/.config/nvim
```

### Preview changes

```bash
confect restore --dry-run
```

### With backup

```bash
confect restore --backup
```

Creates `.confect-backup` files before overwriting.

## What it does

1. Reads file metadata from repository
2. Copies files to their original locations
3. Restores permissions (mode, owner, group)
4. Decrypts encrypted files if needed
