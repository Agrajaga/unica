# Active Tasks For `unica`

This file tracks open implementation work only.

## Current Tasks

- None.

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
