# 7. Представление развертывания

## Source Checkout

In the repository checkout, `run-unica.sh` detects `Cargo.toml` and runs the Rust
binary through cargo. This keeps local development fast and avoids generated
binary commits.

## Generated Marketplace Package

The release pipeline builds target-specific bundled binaries, writes
`third-party/manifest.json`, and packages `plugins/unica`.

Packaged execution:

1. `.mcp.json` starts `scripts/run-unica.sh`.
2. `run-unica.sh` delegates to `scripts/run-tool.sh unica`.
3. `run-tool.sh` verifies host target and SHA-256 from generated manifest.
4. The bundled `unica` binary starts as stdio MCP server.

## Local Install

`scripts/dev/install-local-unica.sh` builds a local package, installs it as a
local Codex marketplace, validates launchers, and can verify fresh Codex prompt
visibility.

## Runtime State

Volatile state defaults to `.build/unica` under the workspace root and can be
overridden by `UNICA_CACHE_DIR`.
