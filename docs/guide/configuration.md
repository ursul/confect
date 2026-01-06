# Configuration

confect uses two levels of configuration:

1. **Global config** — User preferences in `~/.config/confect/config.toml`
2. **Repository config** — Per-repository settings in `.confect/config.toml`

## Global configuration

Located at `~/.config/confect/config.toml`:

```toml
[repository]
# Default repository path
path = "~/.local/share/confect"

[sync]
# Automatically push after sync
auto_push = true

# Default remote name
default_remote = "origin"

[hosts]
# Override hostname detection
current = "my-laptop"
```

## Repository configuration

Located in `.confect/config.toml` inside your repository:

```toml
[repository]
created = "2024-01-01T00:00:00Z"

[hosts.list.my-laptop]
branch = "host/my-laptop"

[hosts.list.my-desktop]
branch = "host/my-desktop"
```

## Environment variables

| Variable | Description |
|----------|-------------|
| `CONFECT_REPO` | Override repository path |
| `CONFECT_HOST` | Override hostname |

## File metadata

confect tracks file metadata in `.confect/metadata.toml`:

```toml
[files.".bashrc"]
source = "/home/user/.bashrc"
mode = "644"
owner = "user"
group = "user"
encrypted = false

[files.".ssh/config"]
source = "/home/user/.ssh/config"
mode = "600"
owner = "user"
group = "user"
encrypted = true
```
