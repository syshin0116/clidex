# Agent integration examples

Clidex is designed to be called as a local subprocess. Agents should request JSON
output, inspect the structured install fields, and ask for approval before
installing software.

## Generic agent workflow

```shell
clidex update
clidex "json processor" --json --score
clidex info jq --json
clidex batch "json processor" "git diff" --json
```

Recommended behavior:

1. Search using a short task description.
2. Compare the top candidates and inspect repository and documentation links.
3. Select an install field matching the current platform.
4. Show the exact install command to the user before running it.
5. Verify that the installed binary resolves from `PATH`.

## Codex or Claude Code instruction

```text
When a task would benefit from a specialized CLI, run clidex with --json.
Inspect the top results and explain the choice. Ask before installing a new
package or executing an install command.
```

## Shell parsing with jq

```shell
clidex "terminal json processor" --json --fields name,desc,install,links \
  | jq '.[0]'
```

`--fields` keeps only the listed tool fields, in that order, which saves
context tokens. `score` stays when `--score` is set. It applies to search,
batch, info, compare, trending, and category listings in JSON or YAML.

Structured output is the public integration boundary. Integrations should not
parse pretty terminal output.

For repeated discovery, `batch` returns one `{query, results}` entry per query
while sharing index loading and engine construction. Empty results are `[]`.

## Output contract

`clidex stats --json` reports `schema_version`. Within a schema version,
fields are only added, never renamed or removed, so ignore unknown keys.

| Exit code | Meaning |
|-----------|---------|
| 0 | Success, including an empty result (`[]`) |
| 1 | Unexpected error |
| 2 | Invalid arguments, for example `-n 0` or a malformed `--updated-since` |
| 3 | Index missing or unreadable; run `clidex update` |
| 4 | `info` or `compare` could not find the named tool |
| 5 | `clidex update` failed (network, checksum, or invalid index) |

With `--json` or `--yaml`, the last line(s) of stderr are an error object:

```json
{"error":{"code":"index_unavailable","message":"Index not found at ...","exit_code":3}}
```

Earlier stderr lines are human hints such as "Did you mean" suggestions or the
stale-index warning, which appears when the index is more than 14 days old
(`CLIDEX_NO_STALE_WARNING=1` silences it).
