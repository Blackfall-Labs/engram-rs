# Contributing to Engram Core

Thanks for helping improve the Engram archive format! This repository contains the pure Rust implementation (`engram-core` and `engram-vfs`). JavaScript bindings now live in the sister repository [`engram-nodejs`](../engram-nodejs).

## Getting Started

1. Clone the repository
   ```bash
   git clone https://github.com/yourusername/engram-core.git
   cd engram-core
   ```
2. Install the Rust toolchain via [rustup](https://rustup.rs/).
3. Run the test suite to ensure the workspace builds on your machine:
   ```bash
   cargo test
   ```

If you also plan to work on the Node.js bindings, clone `engram-nodejs` alongside this repo and follow its own `README`.

## Development Workflow

```bash
cargo fmt        # Format code
cargo clippy     # Lint
cargo test       # Run tests
cargo bench      # (Optional) run benchmarks if they exist
cargo doc --open # Build documentation
```

Please include unit/integration tests for new functionality or bug fixes whenever practical, and keep the specification documents in `docs/` current with your changes.

## Coding Guidelines
- Target Rust 2021 edition and prefer standard library features over external crates where possible.
- Use `thiserror` for error enums; favor `Result<T, Error>` returns over panics.
- Keep public APIs documented with `///` doc comments.
- Follow existing module organization; open an issue before making sweeping structural refactors.

## Commit & PR Guidelines
- Use descriptive commit messages (`feat: add delta compression`, `fix: prevent CRC mismatch`).
- Reference GitHub issues in your PR description when applicable.
- Update relevant docs in `docs/` and `README.md`.
- Ensure `cargo fmt`, `cargo clippy`, and `cargo test` pass before opening a pull request.

## Reporting Issues

When filing a bug report, please include:
- Platform (Windows/macOS/Linux) and architecture.
- Rust version (`rustc --version`).
- Short reproduction steps or an archive that triggers the issue.

For feature ideas or large architectural questions, start a GitHub Discussion or issue so the team can align on direction.

## License

By contributing, you agree that your contributions will be licensed under the MIT License contained in this repository.
