# Categories

Categories are a powerful way to organize your configuration files by purpose, application, or any other criteria you choose.

## Why use categories?

Instead of tracking files individually, categories let you:

- **Group related configs** — Keep shell configs together, editor configs together, etc.
- **Use glob patterns** — Track `~/.config/nvim/**/*` instead of adding each file
- **Apply encryption per-category** — Encrypt all files in a "secrets" category
- **Bulk operations** — Restore or sync entire categories at once

## Creating categories

```bash
# Basic category
confect category create shell

# With description
confect category create editor --description "Editor configurations"

# With initial path
confect category create nvim --path "~/.config/nvim"

# With encryption for all files
confect category create secrets --encrypt
```

## Adding paths to categories

Categories use glob patterns to match files:

```bash
# Single file
confect category add shell ~/.bashrc
confect category add shell ~/.zshrc

# Directory (all files recursively)
confect category add nvim ~/.config/nvim

# Glob pattern
confect category add shell "~/.config/fish/**/*"
```

## Example setup

Here's a typical category structure:

```bash
# Shell configurations
confect category create shell --description "Shell and terminal"
confect category add shell ~/.bashrc
confect category add shell ~/.zshrc
confect category add shell ~/.config/fish
confect category add shell ~/.config/starship.toml

# Editor
confect category create editor --description "Code editors"
confect category add editor ~/.config/nvim
confect category add editor ~/.config/helix
confect category add editor ~/.vimrc

# Desktop environment
confect category create desktop --description "Desktop and WM"
confect category add desktop ~/.config/sway
confect category add desktop ~/.config/waybar
confect category add desktop ~/.config/rofi

# Sensitive files (encrypted)
confect category create secrets --description "Private configs" --encrypt
confect category add secrets ~/.ssh/config
confect category add secrets ~/.config/gh/hosts.yml
```

## Viewing categories

```bash
# List all categories
confect category list

# Show details of a category
confect category show shell
```

Output:
```
Category: shell
Description: Shell and terminal

Paths:
  ~/.bashrc
  ~/.zshrc
  ~/.config/fish
  ~/.config/starship.toml

Encrypted patterns: none
```

## How categories work internally

Categories are stored in `.confect/categories.toml`:

```toml
[categories.shell]
description = "Shell and terminal"
paths = [
    "~/.bashrc",
    "~/.zshrc",
    "~/.config/fish",
]
encrypt = []

[categories.secrets]
description = "Private configs"
paths = [
    "~/.ssh/config",
]
encrypt = [
    "~/.ssh/config",
]
```

In the repository, files are organized by category:

```
~/.local/share/confect/
├── .confect/
│   ├── categories.toml
│   └── metadata.toml
├── shell/
│   ├── .bashrc
│   ├── .zshrc
│   └── .config/fish/...
├── editor/
│   └── .config/nvim/...
└── secrets/
    └── .ssh/config.age    # encrypted
```

## Managing categories

```bash
# Remove a path from category
confect category remove-path shell ~/.bashrc

# Delete entire category (keeps files in system)
confect category delete shell

# Delete category and remove files from repo
confect category delete shell --remove-files
```

## Tips

- **Use descriptive names** — `shell`, `editor`, `desktop` are clearer than `cat1`, `cat2`
- **Group by restore needs** — If you always restore nvim configs together, make them one category
- **Separate secrets** — Keep encrypted files in dedicated categories for clarity
- **Use globs wisely** — `~/.config/nvim` is usually better than `~/.config/nvim/**/*` (same result, cleaner)
