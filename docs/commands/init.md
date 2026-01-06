# init

Initialize a new confect repository.

## Usage

```bash
confect init [OPTIONS]
```

## Options

| Option | Description |
|--------|-------------|
| `--remote <URL>` | Add a remote repository and set as origin |
| `--system` | Initialize system-wide repository at `/var/lib/confect` |

## Examples

### Basic initialization

```bash
confect init
```

Creates a repository at `~/.local/share/confect`.

### With remote

```bash
confect init --remote git@github.com:user/configs.git
```

Creates a repository and sets up the remote.

### System-wide

```bash
sudo confect init --system
```

Creates a system-wide repository at `/var/lib/confect` for system configuration files.

## What it creates

```
~/.local/share/confect/
├── .confect/
│   ├── config.toml      # Repository configuration
│   ├── categories.toml  # Empty categories file
│   └── metadata.toml    # File metadata tracking
├── .gitignore
└── .git/
```

The repository is initialized with:
- A `main` branch with the initial commit
- A `host/<hostname>` branch for machine-specific configs
