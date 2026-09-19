# Installation

confect is a single Linux binary. It keeps its data in a Git repository and runs the
system `git` for everything Git does.

## Requirements

- Linux on x86_64 or aarch64
- Git 2.28 or newer, in `PATH`
- For a remote: SSH keys or a credential helper that work for `git push` without a prompt

confect runs `git` non-interactively. SSH is started with `BatchMode=yes` and
`ConnectTimeout=30`, so a missing key or an unknown host key fails fast instead of
waiting for input: add the remote's host key to `known_hosts` before the first push. If
`GIT_SSH_COMMAND` is set, or `core.sshCommand` is set in git config, confect leaves the ssh
options to you (`init --from` only looks at `GIT_SSH_COMMAND`). Every network operation is
killed after `network_timeout` seconds (300 by default, see
[Configuration](/guide/configuration)).

## Pre-built binaries

Download the archive and its checksum from
[GitHub Releases](https://github.com/ursul/confect/releases):

```bash
ARCH=x86_64   # or aarch64
curl -LO https://github.com/ursul/confect/releases/latest/download/confect-linux-$ARCH.tar.gz
curl -LO https://github.com/ursul/confect/releases/latest/download/confect-linux-$ARCH.tar.gz.sha256
sha256sum -c confect-linux-$ARCH.tar.gz.sha256
tar xzf confect-linux-$ARCH.tar.gz
sudo install -m 0755 confect /usr/local/bin/confect
```

The binaries are statically linked (musl) and have no runtime dependencies besides `git`.

## From source

With a recent stable Rust toolchain:

```bash
cargo install --git https://github.com/ursul/confect
```

## Updating

```bash
confect self-update --check   # only report whether a newer release exists
sudo confect self-update      # download, verify the published SHA-256, replace the binary
```

See [self-update](/commands/self-update). Coming from confect 1.x? Read
[Upgrading from 1.x](/guide/migration) before you run the new binary on an old repository.

## Which user?

confect's own settings and keys live in the invoking user's `~/.config/confect/`. To manage
files in `/etc`, run every confect command as root (for example from `sudo -i`), so the
configuration, the age keys and the repository all belong to the same user.
