# Quick Start

This guide will help you set up confect and start tracking your configuration files.

## Initialize repository

Create a new confect repository:

```bash
confect init
```

This creates a Git repository at `~/.local/share/confect` with the following structure:

```
~/.local/share/confect/
├── .confect/
│   ├── config.toml      # Repository config
│   ├── categories.toml  # Category definitions
│   └── metadata.toml    # File metadata (permissions, etc.)
└── .git/
```

### With remote

To set up a remote repository immediately:

```bash
confect init --remote git@github.com:username/configs.git
```

## Add your first files

Start tracking configuration files:

```bash
# Add a single file
confect add ~/.bashrc

# Add a directory
confect add ~/.config/nvim

# Add with encryption (for sensitive files)
confect add --encrypt ~/.ssh/config
```

## Check status

See what files are tracked and their status:

```bash
confect status
```

## Sync to remote

Commit changes and push to the remote repository:

```bash
confect sync
```

This will:
1. Check for changes in tracked files
2. Create a commit with all changes
3. Push to the remote repository

## Set up on another machine

On a new machine, clone your configs:

```bash
# Initialize from existing remote
confect init --remote git@github.com:username/configs.git

# Restore all files
confect restore
```

## Next steps

- [Configuration](/guide/configuration) — Customize confect behavior
- [Commands Reference](/commands/init) — All available commands
- [Encryption](/advanced/encryption) — Protect sensitive files
- [Multi-host Setup](/advanced/multi-host) — Manage configs across machines
