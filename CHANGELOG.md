# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [2.0.0] - 2026-09-19

A rewrite of the tracking engine. 1.x lost data and leaked secrets in several
ways (see Fixed); 2.0 changes the repository format, so existing repositories
need `confect migrate`. See "Upgrading from 1.x" in the guide.

### Security

- `--encrypt` and category `encrypt` patterns now really encrypt with age:
  the repository holds `<path>.age` and never the plaintext. In 1.x the flag
  printed "(encrypted)" and stored the file as is.
- `sync`, `add` and category changes refuse to store private keys (PEM,
  OpenSSH, PGP, GnuPG, age, PuTTY, JSON Web Keys), key stores (`.p12`, `.pfx`,
  `.jks`, `.keystore`, `.kdbx`), SCRAM verifiers, MD5 userlists and
  htpasswd/shadow hashes as plaintext, and write nothing when one is found.
  Whole files are scanned, binary ones included. `allow_plaintext` patterns
  make deliberate exceptions.
- New `confect audit [--history]` finds plaintext secrets already in the
  repository or anywhere in its history; files allowed with `allow_plaintext`
  are listed separately.
- The repository directory is created (and kept by `sync`) with mode 0700: git
  objects are world-readable and hold every past version of every file.
- `restore` resolves parent directories one component at a time and works
  through directory descriptors: a symlink on the way is followed only when
  root or the restoring user owns it, so a link planted by someone else cannot
  redirect a write made as root. Files are written to a temporary file and
  renamed into place; backups are created with `O_EXCL|O_NOFOLLOW`.
- Owners are restored by name; a user or group missing on the host is not
  replaced by the stored numeric ID, which could belong to someone else.
- `self-update` verifies the published SHA-256 before replacing the binary and
  no longer pulls in the vulnerable `tar` version of the `self_update` crate.

### Added

- Permissions, owner and group of files, symlinks and directories are recorded
  in `.confect/metadata.toml` on every `add` and `sync`, shown by `status` (`P`)
  and applied by `restore`.
- `init --from <url>` clones a configuration repository and continues this
  host's branch (or starts it empty): the way to rebuild a lost host.
- `category exclude|encrypt|allow-plaintext add|remove`, and `--exclude` /
  `--allow-plaintext` on `category create`.
- `confect key generate|show`, `confect push`, `confect migrate`.
- `status --exit-code` exits with 3 when the system differs from the
  repository, for monitoring; `audit` exits with 3 when it finds secrets.
  (2 stays reserved for invalid arguments.)
- `[global] network_timeout` bounds every git network operation.
- `sync --reencrypt` encrypts every encrypted file again, for example after
  replacing the age key. Without it, a stored copy that cannot be decrypted is
  kept and reported instead of being replaced.
- A lock on the repository serializes concurrent confect runs.

### Changed

- All git operations use the system `git` (2.28 or newer). `~/.ssh/config`,
  credential helpers and `core.sshCommand` now work as for git itself; ssh runs
  with `BatchMode` so a missing key fails instead of hanging.
- `sync` pushes only when `auto_push` is on (or with `--push`), pushes the host
  branch explicitly and treats a rejected push as an error.
- `pull` fetches and fast-forwards only `host/<name>`; divergence is an error,
  and it refuses to run over changes that `sync` has not committed yet.
- `init` creates `host/<name>` directly, keeps existing global settings and
  remembers `--path` and `--system` in `[global] repo_path`.
- `add`, `remove`, `status`, `diff` and `restore` take several paths;
  categories are selected with `-c`. `add` requires `-c` and
  `--create-category` for a new category.
- `remove` untracks for real: a category path is removed, a path inside a
  tracked directory is excluded, and stored copies are dropped.
- `restore` changes only paths that differ, asks for confirmation unless
  `--yes`, supports `--dry-run`, and fails when anything fails.
- `diff` is a real unified diff from the repository (`---`) to the system
  (`+++`) and reports binary files and permission changes.
- Category patterns: `*` no longer crosses `/`; a pattern without `/` matches a
  name at any depth. Categories may not overlap; for legacy overlaps the most
  specific category owns the file.
- `setup-timer` installs `confect-sync.service/.timer` (system or `--user`),
  with a start timeout and a configurable message; `--remove` also stops them.
- `categories.toml` and `metadata.toml` are written in sorted order.

### Fixed

- Excluded files inside tracked directories were committed anyway; `add`
  ignored exclusions; excluded copies were never removed from the repository.
- `restore` deleted the live file before copying and reported success when
  the copy failed.
- A push rejected by the server was reported as "Pushed".
- `pull` fast-forwarded to whichever branch came first in `FETCH_HEAD`.
- A `.gitignore` inside a tracked directory hid files from the backup, and a
  nested `.git` directory broke every later `sync`.
- `init` overwrote the global config; `--repo`/`CONFECT_REPO` were ignored.
- `restore -f` clashed with `--file` and panicked in debug builds.
- Dangling symlinks and non-UTF-8 names caused a commit on every `sync`.
- One unreadable directory aborted the whole `sync`; now it is reported and its
  stored copies are kept. A tracked path that disappears keeps its copies too.
- `sync` with nothing to do exited with an error.
- `add` without `-c` copied files into a category that did not exist.
- `setup-timer` rejects messages and schedules that would add lines to the
  unit files; git receives user-supplied URLs and names after
  `--end-of-options`.

### Removed

- Every `-r` short option: write `--remote`, `--restore` or `--repo`.
- `remove --delete`, `sync --all-hosts`, `restore <category> -f <file>` (use
  `restore <paths> -c <category>`), `restore --force` (use `--yes`) and
  `category delete --remove-files` (now the default).
- The `git2` dependency and its vendored OpenSSL.

## [1.4.1] - 2026-06-21

### Fixed

- Clippy warnings.

## [1.4.0] - 2026-06-21

### Added

- `sync` now automatically tracks new files added inside tracked directories.

### Changed

- Exact directory paths in categories now also match their nested files and
  subdirectories.
- `sync` now removes repository copies and stored metadata for files that were
  deleted from tracked directories.
- Symlinks are now consistently treated as trackable entries during status,
  refresh, and directory listing.

## [0.1.0] - 2024-XX-XX

### Added

- Initial release
- Core commands: `init`, `add`, `remove`, `sync`, `pull`, `restore`, `status`, `diff`, `info`
- Category management with `category` command
- File encryption with [age](https://github.com/FiloSottile/age)
- Multi-host support with per-host Git branches
- File metadata tracking (permissions, ownership)
- Systemd timer integration via `setup-timer`
- Support for `--system` flag for system-wide repository
- SSH and HTTPS authentication for Git remotes

[Unreleased]: https://github.com/ursul/confect/compare/v2.0.0...HEAD
[2.0.0]: https://github.com/ursul/confect/compare/v1.4.1...v2.0.0
[1.4.1]: https://github.com/ursul/confect/compare/v1.4.0...v1.4.1
[1.4.0]: https://github.com/ursul/confect/compare/v1.3.0...v1.4.0
[0.1.0]: https://github.com/ursul/confect/releases/tag/v0.1.0
