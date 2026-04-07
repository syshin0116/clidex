# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0] - 2026-04-07

### Added

- TTY auto-detection: pretty output in terminal, YAML in pipes (like `ls`, `grep`)
- `--score` flag: include relevance scores in search results
- `--yaml` flag: force YAML output (complement to `--json` and `--pretty`)
- `search` subcommand: explicit alternative to positional query (`clidex search "csv" --category data`)
- `categories` subcommand with name filter (`clidex categories git`)
- `trending --updated-since` filter: show popular tools updated after a given date
- Auto-download index on first search in interactive terminals
- Empty result suggestions: "Did you mean:", partial word re-search tips
- Install command shown in pretty search output (`$ brew install jq`)
- Description truncation based on terminal width
- Compare table adapts column width to terminal size
- 34 unit tests for core functions (edit_distance, word_boundary_match, intent_coverage, popularity_boost, etc.)
- `SearchIndex::hybrid_search()` method: semantic path now uses cached BM25 engine

### Fixed

- Category filter false positives: `contains()` → word-prefix + hierarchical + leaf segment matching
  - "File" no longer matches "Text Filters"
  - "docker" now matches "Development > Docker" (leaf segment)
- Exit code semantics: empty search/browse results → exit 0, lookup not-found → exit 1
- Non-interactive auto-download: CI/pipes get error message instead of silent network calls
- `to_lowercase()` cached outside per-tool loop (avoids redundant computation)
- Standalone `hybrid_search()` deprecated in favor of `SearchIndex::hybrid_search()`

### Changed

- Default output: TTY → pretty, pipe → YAML (previously always YAML)
- Score output uses `SearchResultOutput` wrapper instead of injecting into Tool schema
- `trending --since` renamed to `--updated-since` for clarity (filters by repo activity, not popularity growth)
- `trending` description changed to "Show popular tools (sorted by GitHub stars)"
- README rewritten: quickstart section, unified tool count (5,000+), new features documented
- SearchIndex used in CLI main entry point (both lexical and semantic paths)

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

[0.4.0]: https://github.com/syshin0116/clidex/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/syshin0116/clidex/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/syshin0116/clidex/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/syshin0116/clidex/releases/tag/v0.1.0
