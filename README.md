# clidex

CLI tool discovery for AI agents. Search, compare, and install 440+ CLI tools with structured YAML/JSON output.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/syshin0116/clidex/main/install.sh | sh
```

Or with Cargo:

```bash
cargo install --git https://github.com/syshin0116/clidex.git
```

Then download the index:

```bash
clidex update
```

## Usage

### Search

```bash
clidex "csv to json"              # Pretty output
clidex "csv to json" --yaml       # YAML (recommended for agents)
clidex "csv to json" --json       # JSON
clidex "file manager" -n 3        # Top 3 results
```

### Tool info

```bash
clidex info jq
clidex info jq --yaml
```

### Compare tools

```bash
clidex compare jq dasel yq
```

### Trending

```bash
clidex trending -n 10
clidex trending --category git
```

### Categories

```bash
clidex --categories              # List all categories
clidex --category docker         # Filter by category
```

### Update index

```bash
clidex update
```

## Agent integration

clidex is designed for AI agents (Claude Code, Codex, Gemini CLI). Use `--yaml` or `--json` for structured output:

```yaml
- name: ripgrep
  desc: A line-oriented search tool that recursively searches...
  category: Files and Directories > Search
  tags: [files, directories, search, ripgrep, rg]
  install:
    brew: brew install ripgrep
    cargo: cargo install ripgrep
  links:
    repo: https://github.com/BurntSushi/ripgrep
```

Agents can extract `install.brew` or `install.cargo` and run it directly.

## Data sources

| Source | Purpose |
|--------|---------|
| [awesome-cli-apps](https://github.com/agarrharr/awesome-cli-apps) | Primary tool list (427 tools) |
| Homebrew | Install commands + 15 popular CLI tools |
| GitHub API | Stars, last updated |
| crates.io | `cargo install` commands (42 tools) |
| npm | `npm install -g` commands (12 tools) |

## Build index locally

```bash
cargo run --bin build_index -- index.yaml
```

Environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `GITHUB_TOKEN` | — | GitHub API token (increases rate limit) |
| `GITHUB_LIMIT` | 50 | Max GitHub API requests |
| `CRATES_LIMIT` | 100 | Max crates.io lookups |
| `NPM_LIMIT` | 50 | Max npm lookups |
| `LLMS_LIMIT` | 100 | Max llms.txt probes |

## License

MIT
