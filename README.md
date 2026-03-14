<p align="center">
  <h1 align="center">Clidex</h1>
  <p align="center">
    <strong>CLI tool discovery for AI agents</strong>
  </p>
  <p align="center">
    Search, compare, and install 440+ command-line tools with structured YAML/JSON output.
  </p>
  <p align="center">
    <a href="https://github.com/syshin0116/clidex/actions/workflows/ci.yml"><img src="https://github.com/syshin0116/clidex/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
    <a href="https://github.com/syshin0116/clidex/releases/latest"><img src="https://img.shields.io/github/v/release/syshin0116/clidex" alt="Release"></a>
    <a href="https://github.com/syshin0116/clidex/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License"></a>
  </p>
</p>

---

## Why Clidex?

AI agents like Claude Code, Codex, and Gemini CLI can run terminal commands — but they don't know which tools exist beyond the basics. An agent uses `grep` when `ripgrep` is 10x faster, or `find` when `fd` is simpler.

**Clidex bridges this gap.** It's a local CLI that returns structured metadata about CLI tools: what they do, how to install them, and where to find docs. No web search API calls, no HTML parsing, no cost — just a fast local lookup.

```
Agent: "I need to convert CSV to JSON"
  ↓
$ clidex "csv to json"
  ↓
Returns YAML: jq, miller, q — with `brew install` / `cargo install` commands
  ↓
Agent installs and uses the tool
```

### What makes it different

| | awesome-cli-apps / cli-anything | Clidex |
|--|------|------|
| Target user | Humans | AI agents (+ humans) |
| Output | Markdown / TUI | YAML / JSON / Pretty |
| Install info | Links only | `brew install jq` — ready to run |
| Docs access | Click a link | `llms.txt` URL for agents to read |
| Pipeline | No | `clidex ... \| next_tool` (YAML by default) |
| Compare | No | `clidex compare jq dasel yq` |

---

## Installation

### With Cargo (all platforms)

```bash
cargo install clidex
```

### Pre-built binaries

**Linux / macOS:**

```bash
curl -fsSL https://raw.githubusercontent.com/syshin0116/clidex/main/install.sh | sh
```

**Windows:**

```bash
curl -fsSL -o clidex.zip https://github.com/syshin0116/clidex/releases/latest/download/clidex-x86_64-pc-windows-msvc.zip
tar -xf clidex.zip
move clidex.exe %USERPROFILE%\.local\bin\
```

Or download directly from [Releases](https://github.com/syshin0116/clidex/releases/latest).

### Setup

After installing, download the tool index:

```bash
clidex update    # Downloads ~/.clidex/index.yaml
```

---

## Usage

### Search

```bash
clidex "csv to json"              # YAML output (default, agent-friendly)
clidex "csv to json" --pretty     # Pretty-printed table (human-friendly)
clidex "csv to json" --json       # JSON output
clidex "file manager" -n 3        # Limit to top 3 results
```

Default output (YAML):

```yaml
- name: jq
  desc: JSON processor
  category: Data Manipulation > Processors
  tags: [data, manipulation, processors, json, jq]
  install:
    brew: brew install jq
  links:
    repo: https://github.com/stedolan/jq
    homepage: https://jqlang.github.io/jq/
```

### Tool info

```bash
clidex info ripgrep               # YAML output (default)
clidex info ripgrep --pretty      # Human-friendly output
```

### Compare tools

```bash
clidex compare jq dasel yq           # YAML output (default)
clidex compare jq dasel yq --pretty  # Side-by-side table
```

```
                jq                              dasel                           yq
                ──────────────────────────────  ──────────────────────────────  ──────────────────
Description     JSON processor                  JSON/YAML/TOML/XML processor…   YAML processor
Category        Processors                      Processors                      Processors
Stars           ★ 30.8k                         ★ 5.3k                          ★ 2.6k
Install         brew install jq                 brew install dasel              brew install yq
```

### Trending

```bash
clidex trending                    # Top tools by GitHub stars
clidex trending -n 10              # Top 10
clidex trending --category git     # Top Git tools
```

### Categories

```bash
clidex --categories                # List all categories with tool counts
clidex --category docker           # All tools in a category
clidex --category "file manager" -n 5
```

### Index management

```bash
clidex update                      # Download latest index
clidex stats                       # Show index statistics
```

### Common flags

| Flag | Description |
|------|-------------|
| `--pretty` | Human-friendly pretty-printed output |
| `--json` | JSON output |
| `-n <N>` | Max number of results (default: 10) |

Default output format is **YAML** — optimized for agent consumption.

---

## Agent integration

Clidex is built for AI agents to consume programmatically. The typical workflow:

1. Agent runs `clidex "task description"` (YAML by default)
2. Parses the structured result
3. Extracts `install.brew` or `install.cargo` command
4. Installs and uses the tool

### Output schema

Each tool in the result contains:

```yaml
name: string            # Tool name
binary: string?         # Binary name (if different from name)
desc: string            # One-line description
category: string        # Category path (e.g. "Files and Directories > Search")
tags: [string]          # Search tags
install:                # Install commands by package manager
  brew: string?
  cargo: string?
  npm: string?
stars: number?          # GitHub stars
links:
  repo: string?         # GitHub repository
  homepage: string?     # Project homepage
  docs: string?         # Documentation URL
  llms_txt: string?     # llms.txt URL (LLM-readable docs)
```

The `llms_txt` field is especially useful — it points to [llms.txt](https://llmstxt.org/) files that agents can fetch to learn how to use a tool.

---

## How search works

Clidex uses **BM25** text search with domain-specific optimizations:

- **Field weighting**: Tool name (3x) > tags + category (2x) > description (1x)
- **Synonym expansion**: `grep` → also matches `search`, `find`, `ripgrep`, `rg`
- **Category boost**: Query terms matching category names get +8 points each
- **Popularity boost**: GitHub stars add 0–10 bonus points
- **Fuzzy matching**: Catches typos and partial name matches
- **Alias mapping**: `rg` → ripgrep, `btm` → bottom, `z` → zoxide (24 pairs)

Search performance: **~3ms per query** on the full 440-tool index.

---

## Data sources

| Source | What it provides | Count |
|--------|-----------------|-------|
| [awesome-cli-apps](https://github.com/agarrharr/awesome-cli-apps) | Curated tool list with categories | 427 tools |
| [Homebrew](https://formulae.brew.sh/) | `brew install` commands + popular CLI tools | 186 matched + 15 added |
| [GitHub API](https://docs.github.com/en/rest) | Stars, last updated, homepage | Metadata |
| [crates.io](https://crates.io/) | `cargo install` commands | 42 tools |
| [npm](https://www.npmjs.com/) | `npm install -g` commands | 12 tools |

The index is rebuilt weekly via GitHub Actions and published as a [release asset](https://github.com/syshin0116/clidex/releases/tag/index).

---

## Build index locally

```bash
cargo run --bin build_index -- index.yaml
```

### Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `GITHUB_TOKEN` | — | GitHub API token (increases rate limit from 60 to 5000/hr) |
| `GITHUB_LIMIT` | `50` | Max GitHub API requests |
| `CRATES_LIMIT` | `100` | Max crates.io lookups |
| `NPM_LIMIT` | `50` | Max npm registry lookups |
| `LLMS_LIMIT` | `100` | Max llms.txt probes |

---

## Contributing

Contributions are welcome! Some areas that could use help:

- **Adding tools to the index**: Suggest popular CLI tools that are missing
- **Search quality**: Report queries that return unexpected results
- **New data sources**: Integrations with other package managers
- **Platform support**: Testing on different OS/architecture combinations

```bash
# Development
cargo build
cargo test
cargo clippy
cargo fmt
```

---

## License

[MIT](LICENSE)
