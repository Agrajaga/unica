# 8. Сквозные концепции

## Single Public MCP

The LLM sees one server and does not coordinate multiple MCP caches or indexes.
This is the primary token and context saving mechanism.

## Dry Run Safety

Mutating tools default to dry-run. Skills pass `dryRun: false` only for explicit
user-requested mutations.

## Cache Ownership

The orchestrator owns cache state. Adapter calls must report through application
use cases so domain events and cache invalidation cannot be bypassed.

## Internal Adapter Pattern

Adapters are typed boundaries around existing engines. They may use CLI or MCP
protocol internally, but their names and cache lifecycle are not exposed to LLM.

Python/PowerShell/Bash operation files are not a runtime adapter class for
developer operations. Donor scripts can be kept only as fixture reference models
for native `unica.*` MCP handlers.

## Workspace-scoped Services

Some internal adapters may run behind hidden workspace services. These services
are owned by `unica`, scoped by workspace and source root, and coordinated
through volatile cache state. They are not public MCP registrations and must not
appear in skills as routing targets.

The lifecycle rule is lazy start, reuse while live, invalidate on domain events,
and natural exit after idle or max-age limits. Cheap read-only tools that do not
need warm analyzer/index state must not start the service.

## Source Of Truth Order

When documents disagree, use this order:

1. current code and tests;
2. package manifests and `.mcp.json`;
3. active `spec/`;
4. README and skill prose;
5. archived or research docs.
