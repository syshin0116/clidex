# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2026-04-05

### Added

- Levenshtein edit distance for typo correction (`ripgrpe` → `ripgrep`, `zoxdie` → `zoxide`)
- crates.io category-based discovery (command-line-utilities, development-tools, etc.)
- PyPI/pipx discovery with 39 seed Python CLI tools
- `pipx` install method in output schema
- Real index integration tests (5,277 tools): ranking, precision, typo, relevance, performance
- Index coverage tests: 67 must-have tools, category breadth, ecosystem presence, regression guard
- Adversarial test fixtures: 8 competing tools (ag, gitui, lsd, curlie, difftastic, dasel, yq, navi)
- `assert_ranks_above` helper for ranking order verification

### Fixed

- Fuzzy anchor too strict — high-confidence fuzzy matches now pass without substring anchor
- Synonym-only matches killed by `covered == 0` gate — now checks expanded terms
- `intent_coverage` substring matching — replaced with token-aware `word_boundary_match`
- `desc_bonus` aligned with `word_boundary_match` for scoring consistency
- Edit distance skipped for multi-word queries (performance: prevents O(n*m) on 5,000+ tools)

### Changed

- Recall test threshold raised from 85% to 95%
- Test fixture expanded from 20 to 28 tools (with adversarial competitors)
- Many `top 5` assertions tightened to `top 3`
- Synonym scoring reflected in ranking (0.5x weight), not just gate pass
- Edit distance threshold scaled by query length (4-5 chars: ≤1, 6+ chars: ≤2)
- README and llms.txt updated with current search algorithm and data sources

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

[0.3.0]: https://github.com/syshin0116/clidex/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/syshin0116/clidex/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/syshin0116/clidex/releases/tag/v0.1.0
