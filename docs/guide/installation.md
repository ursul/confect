# Installation

confect is a Linux utility for managing system configuration files.

## From source

If you have Rust installed, you can build confect from source:

```bash
cargo install --git https://github.com/ursul/confect
```

## Pre-built binaries

Download the latest release from [GitHub Releases](https://github.com/ursul/confect/releases).

### x86_64

```bash
curl -LO https://github.com/ursul/confect/releases/latest/download/confect-linux-x86_64.tar.gz
tar xzf confect-linux-x86_64.tar.gz
sudo mv confect /usr/local/bin/
```

### aarch64 (ARM64)

```bash
curl -LO https://github.com/ursul/confect/releases/latest/download/confect-linux-aarch64.tar.gz
tar xzf confect-linux-aarch64.tar.gz
sudo mv confect /usr/local/bin/
```

## Requirements

- Linux (x86_64 or aarch64)
- Git (for remote sync)
- SSH key or credential helper configured for Git push/pull
