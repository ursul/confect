# Installation

## From source

If you have Rust installed, you can build confect from source:

```bash
cargo install --git https://github.com/ursul/confect
```

## Pre-built binaries

Download the latest release from [GitHub Releases](https://github.com/ursul/confect/releases).

### Linux

```bash
# Download
curl -LO https://github.com/ursul/confect/releases/latest/download/confect-linux-x86_64.tar.gz

# Extract
tar xzf confect-linux-x86_64.tar.gz

# Install
sudo mv confect /usr/local/bin/
```

### macOS

```bash
# Download
curl -LO https://github.com/ursul/confect/releases/latest/download/confect-macos-x86_64.tar.gz

# Extract
tar xzf confect-macos-x86_64.tar.gz

# Install
sudo mv confect /usr/local/bin/
```

## Requirements

- Git (for remote sync)
- SSH key or credential helper configured for Git push/pull
