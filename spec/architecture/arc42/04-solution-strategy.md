# 4. Стратегия решения

## Strategy

Use a pragmatic DDD split:

- domain: workspace identity, cache impact, domain events;
- application: tool registry, use case dispatch, orchestration;
- infrastructure: internal adapters and filesystem state;
- interfaces: MCP JSON-RPC transport.

## Key Decisions

1. Hide all engines behind one MCP server.
2. Keep application logic transport-neutral.
3. Emit domain events for mutating operations.
4. Let cache invalidation happen inside `unica`.
5. Keep operation backend command semantics inside native Rust MCP handlers.

## Migration Shape

New operation backend behavior is ported into Rust first, with donor scripts
retained only as parity fixtures when they are needed as a source model.
