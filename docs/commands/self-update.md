# self-update

Update confect from the latest GitHub release, verifying its checksum.

## Usage

```bash
confect self-update [OPTIONS]
```

## Options

| Option | Description |
|--------|-------------|
| `--check` | Only check whether a newer release exists |
| `-y`, `--yes` | Do not ask for confirmation (required without a terminal) |

## Examples

```bash
confect self-update --check
sudo confect self-update --yes
```

## What it does

1. Asks the GitHub API for the latest release of `ursul/confect` and compares its version
   with the running one. If it is not newer: `confect 2.0.0 is the latest release`.
2. With `--check`, stops after reporting the new version.
3. Asks for confirmation, then downloads `confect-linux-x86_64.tar.gz` or
   `confect-linux-aarch64.tar.gz` and its published `.sha256` file.
4. Refuses to continue if the SHA-256 of the archive does not match.
5. Extracts only the `confect` executable, checks that it is an ELF binary, and replaces the
   running binary atomically.

Replacing `/usr/local/bin/confect` needs write access to `/usr/local/bin`, hence `sudo`.
Release builds exist for Linux on x86_64 and aarch64 only.
