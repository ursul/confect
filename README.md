# confect

> Manage your system configuration files with Git

[![CI](https://github.com/ursul/confect/actions/workflows/ci.yml/badge.svg)](https://github.com/ursul/confect/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**confect** is a CLI tool that tracks and syncs your configuration files across multiple machines using Git as the backend. It handles file permissions, supports encryption for sensitive configs, and organizes everything into categories.

## Features

- **Git-backed** — Version control for all your configs
- **Multi-host sync** — Per-host branches for machine-specific settings
- **Encryption** — Protect sensitive files with [age](https://github.com/FiloSottile/age)
- **Categories** — Organize configs by purpose (shell, editor, desktop, etc.)
- **Permissions preserved** — Tracks ownership and file modes
- **Systemd integration** — Automatic backups with timers

## Quick Start

```bash
# Initialize a new confect repository
confect init

# Add configuration files
confect add ~/.config/nvim
confect add ~/.zshrc

# Sync changes to remote
confect sync
```

## Installation

### From source

```bash
cargo install --git https://github.com/ursul/confect
```

### Pre-built binaries

Download from [Releases](https://github.com/ursul/confect/releases).

## Usage

### Adding files

```bash
# Add a single file
confect add ~/.bashrc

# Add a directory
confect add ~/.config/nvim

# Add with encryption
confect add --encrypt ~/.ssh/config

# Add to a category
confect add --category shell ~/.zshrc
```

### Syncing

```bash
# Commit and push changes
confect sync

# Pull changes from remote
confect pull

# Check status
confect status
```

### Restoring

```bash
# Restore all tracked files
confect restore

# Restore specific file
confect restore ~/.config/nvim

# Dry run (preview changes)
confect restore --dry-run
```

### Categories

```bash
# List categories
confect category list

# Create a category
confect category create shell

# Add file to category
confect category add shell ~/.zshrc
```

## Configuration

Config file: `~/.config/confect/config.toml`

```toml
[repository]
path = "~/.local/share/confect"

[sync]
auto_push = true
default_remote = "origin"
```

## Documentation

Full documentation: [ursul.github.io/confect](https://ursul.github.io/confect)

## License

MIT
