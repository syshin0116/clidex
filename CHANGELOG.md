# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-03-14

### Added

- crates.io enrichment (42 cargo install commands)
- npm enrichment (12 npm install -g commands)
- Published to crates.io
- Windows installation support
- `--pretty` flag for human-readable output

### Changed

- Default output format from Pretty to YAML (agent-first design)
- Replaced `--yaml` flag with `--pretty` flag
- Improved install.sh to detect ~/.cargo/bin and warn about duplicate binaries

## [0.1.0] - 2026-03-14

### Added

- BM25 search with synonym expansion, category boost, and popularity boost
- awesome-cli-apps parser (427 tools)
- Homebrew enrichment (brew install commands)
- GitHub API integration (stars, last_updated)
- Pretty-printed output format
- Subcommands: search, info, compare, trending, categories, stats, update
- Cross-platform binaries (Linux x86/arm, macOS x86/arm, Windows)
- install.sh for Linux/macOS
- MIT license

[0.2.0]: https://github.com/syshin0116/clidex/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/syshin0116/clidex/releases/tag/v0.1.0
