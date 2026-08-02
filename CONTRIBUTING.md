# Contributing to clidex

Thanks for helping improve clidex. Small, focused pull requests are easiest to
review.

## Development

Install a Rust toolchain compatible with the `rust-version` in `Cargo.toml`, then
run:

```shell
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

Tests that use the current full index expect an index at
`~/.clidex/index.yaml`. Run `cargo run -- update` before those tests if needed.

## Search quality changes

Add a regression test for the query and expected ranking. Prefer an assertion at
the public search boundary. A useful report includes the exact query and output
from `clidex --json --score`.

## Pull requests

- Explain the user-visible behavior and compatibility impact.
- Keep structured YAML and JSON output backward compatible where possible.
- Update `CHANGELOG.md` for notable changes.
- Do not commit generated indexes, build output, credentials, or local caches.

By contributing, you agree that your contribution is licensed under the MIT
License.
