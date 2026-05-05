# ADR-0003: Cache и workspace state принадлежат orchestrator

- Статус: `accepted`
- Дата: `2026-05-03`

## Контекст

Internal engines can maintain their own indexes, but LLM-visible coordination
between those engines is the core problem this architecture fixes. After a
mutation, Unica must know which workspace caches are stale without asking the
model to call another tool.

## Решение

The `unica` orchestrator owns workspace state and cache coordination.

1. Mutating use cases emit typed domain events.
2. `CacheImpact` maps events to invalidated and refreshed cache names.
3. `WorkspaceStateRepository` stores volatile cache state under `.build/unica`
   or `UNICA_CACHE_DIR`.
4. Dry-run calls report cache impact without writing state.
5. Applied successful mutations may persist cache state changes.

## Неграницы

1. This ADR does not require immediate eager rebuild of every heavy cache.
2. Internal engines may keep implementation-specific caches.
3. Internal caches must not become public coordination responsibility for LLM.

## Последствия

1. Every mutating tool needs an event mapping.
2. Cache behavior must be tested at the orchestrator boundary.
3. Future lazy rebuild should use the same repository and event model.

## План реализации

1. Keep domain event enums explicit.
2. Add cache impact tests for each new mutating tool family.
3. Expand state repository only after adapter contract tests exist.

## Верификация

- [x] ADR defines orchestrator ownership.
- [x] ADR covers dry-run behavior.
- [x] ADR separates internal engine caches from public coordination.
