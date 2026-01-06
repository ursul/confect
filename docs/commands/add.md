# add

Add files or directories to confect tracking.

## Usage

```bash
confect add [OPTIONS] <PATH>...
```

## Arguments

| Argument | Description |
|----------|-------------|
| `<PATH>` | File or directory to track |

## Options

| Option | Description |
|--------|-------------|
| `--encrypt`, `-e` | Encrypt the file before storing |
| `--category <NAME>`, `-c` | Add to a specific category |
| `--force`, `-f` | Overwrite if already tracked |

## Examples

### Add a single file

```bash
confect add ~/.bashrc
```

### Add a directory

```bash
confect add ~/.config/nvim
```

### Add with encryption

```bash
confect add --encrypt ~/.ssh/config
```

### Add to a category

```bash
confect add --category shell ~/.zshrc
```

## How it works

1. Copies the file to the repository
2. Records metadata (permissions, owner, group)
3. If encrypted, encrypts with your age key
4. Updates `.confect/metadata.toml`

## Notes

- Symlinks are followed and the target file is copied
- Empty directories are ignored
- Binary files work but may not diff well
