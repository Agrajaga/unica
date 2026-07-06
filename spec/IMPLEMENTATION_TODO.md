# Active Tasks For `unica`

This file tracks open implementation work only.

## Current Tasks

- [ ] Resolve the remaining prompt-visible script-backed skills (`img-grid`,
  `web-test`): either move them behind an approved MCP/native boundary or
  document and test explicit ADR exceptions for non-XML/DSL utilities.

## Rules

- Keep this file short and active-only.
- If a task changes a public or architectural contract, update the ADR and active
  docs layer before implementation.
- Promote only immediately executable work here.

## Done Criteria

- The behavior is covered by a focused test.
- The relevant ADR or invariant is updated if the public contract changes.
- `python3.12 -m unittest discover -s tests/ci` and
  `cargo test --package unica-coder` pass.
- `cargo run --quiet --bin unica -- --help` still reports the public server as
  `unica`, and generated package metadata starts `./bin/<target>/unica`
  directly with `cwd` set to the plugin root.
