# 11. Риски и технический долг

## Active Risks

- Standards adapter is not yet a full native HTTP MCP proxy.
- Native XML/DSL handlers can drift from donor behavior if parity fixtures are
  not updated together with Rust ports.
- Cache reporting exists before full lazy/eager rebuild implementation.
- The public tool list can grow too broad if every internal capability is
  mirrored one-to-one.
- Fresh Codex visibility can be affected by stale local plugin cache.

## Mitigations

- Keep gaps in the implementation task list.
- Add parity fixtures and MCP contract tests for donor behavior that must remain
  compatible.
- Keep `.mcp.json` single-server tests.
- Validate generated marketplace packages, not only the source checkout.
- Use clean `CODEX_HOME` for visibility proof.
