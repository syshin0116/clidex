---
name: clidex
description: Find, compare, and install command-line tools with the clidex index of thousands of CLIs. Use when a task needs a CLI the machine may not have (format conversion, search, benchmarking, data wrangling, cloud or git tooling), when choosing between similar tools, or when the user asks "is there a CLI for X".
---

# clidex

Requires the `clidex` binary (`cargo install clidex` or the install script in the repository README).

1. Search with a short task description and keep only what you need:
   `clidex "convert csv to json" --json --fields name,desc,install,links -n 5`
2. For a shortlist, `clidex compare jq yq dasel --json` or `clidex info <name> --json`.
3. Prefer an install command for a package manager already on the machine (`brew`, `cargo`, `npm`, `pipx`, `pacman`, `apk`, `scoop`).
4. Show the user the exact install command and ask before running it. After installing, confirm the binary resolves on `PATH`.

For several needs at once, `clidex batch "query one" "query two" --json` loads the index once.

## Gotchas

- Exit code 3 means the index is missing or unreadable: run `clidex update`. Exit 4 means `info`/`compare` found no such tool. An empty result is `[]` with exit 0.
- A stderr warning that the index is N days old means install commands may be outdated; `clidex update` is cheap when nothing changed.
- Install commands come from package registries, not from the tool authors. Check the repository link before installing anything unfamiliar.
