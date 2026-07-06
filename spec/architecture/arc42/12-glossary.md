# 12. Глоссарий

- Adapter: internal boundary that calls a bundled tool or remote endpoint.
- Cache impact: the set of cache names invalidated or refreshed by domain events.
- Domain event: a typed fact emitted by an operation, for example `FormChanged`.
- Reference operation script: Python/PowerShell/Bash donor model kept under
  `tests/fixtures` for parity tests. It is not a runtime backend.
- MCP: Model Context Protocol.
- Orchestrator: the Rust `unica` server that owns public tool dispatch and
  cache/state coordination.
- Public MCP server: the only MCP server visible to LLM through `.mcp.json`.
- Skill: Codex operation instruction under `plugins/unica/skills`.
- Workspace epoch: lightweight fingerprint used to associate cache state with
  the current workspace.
