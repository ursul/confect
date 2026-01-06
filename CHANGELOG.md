# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/ursul/confect/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/ursul/confect/releases/tag/v0.1.0
