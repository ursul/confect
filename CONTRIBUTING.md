# Contributing to confect

Thank you for your interest in contributing to confect!

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/YOUR_USERNAME/confect`
3. Create a branch: `git checkout -b feature/your-feature`
4. Make your changes
5. Run tests: `cargo test`
6. Run lints: `cargo clippy && cargo fmt --check`
7. Commit your changes
8. Push and create a Pull Request

## Development

### Prerequisites

- Rust 1.75 or later
- Git

### Building

```bash
cargo build
```

### Testing

```bash
cargo test
```

### Linting

```bash
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

## Pull Request Guidelines

- Keep changes focused and atomic
- Update documentation if needed
- Add tests for new features
- Follow the existing code style
- Write clear commit messages

## Reporting Issues

When reporting bugs, please include:

- confect version (`confect --version`)
- Operating system
- Steps to reproduce
- Expected vs actual behavior

## Code of Conduct

Be respectful and constructive. We're all here to build something useful.
