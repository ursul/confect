# remove

Remove files from confect tracking.

## Usage

```bash
confect remove [OPTIONS] <PATH>...
```

## Options

| Option | Description |
|--------|-------------|
| `--delete` | Also delete the file from the system |

## Examples

```bash
# Stop tracking (keeps system file)
confect remove ~/.bashrc

# Stop tracking and delete
confect remove --delete ~/.old-config
```
