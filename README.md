# Engram Core

Rust implementation of the Engram archive format, including the core engine and SQLite virtual filesystem. This repository is meant to be published on its own and consumed by other language bindings such as `engram-nodejs`.

## Layout
- `crates/engram-core`: Archive reader/writer, compression pipeline, manifest handling.
- `crates/engram-vfs`: SQLite virtual filesystem backed by Engram archives.
- `docs/`: Specifications, architecture notes, and getting-started guides for the core.

## Getting Started
```bash
git clone https://github.com/yourusername/engram-core.git
cd engram-core
cargo test
```

### Building
```bash
cargo build --release
```

### Documentation & Specs
- High level overview: `docs/engram_architecture_clarification.md`
- Compression details: `docs/engram_compression_spec_v0.2.md`
- API reference and quick start: `docs/API.md`, `docs/GETTING_STARTED.md`

## Contributing
See `CONTRIBUTING.md` for guidelines covering code style, testing, and release process.

## License
MIT License – see `LICENSE`.
