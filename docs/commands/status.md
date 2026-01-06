# status

Show the status of tracked files.

## Usage

```bash
confect status
```

## Output

```
Tracked files:
  M ~/.bashrc           (modified)
  ✓ ~/.config/nvim      (synced)
  + ~/.zshrc            (new)
  ! ~/.ssh/config       (encrypted, modified)
```

## Status indicators

| Symbol | Meaning |
|--------|---------|
| `✓` | File is synced (no changes) |
| `M` | File has been modified |
| `+` | File is new (not in repo) |
| `!` | File is missing from system |
| `E` | Encrypted file |
