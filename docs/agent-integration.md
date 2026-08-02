# Agent integration examples

Clidex is designed to be called as a local subprocess. Agents should request JSON
output, inspect the structured install fields, and ask for approval before
installing software.

## Generic agent workflow

```shell
clidex update
clidex "convert csv to json" --json --score
clidex info jq --json
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
clidex "terminal json processor" --json \
  | jq '.[0] | {name, description: .desc, install, links}'
```

Structured output is the public integration boundary. Integrations should not
parse pretty terminal output.
