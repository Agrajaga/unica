# ADR-0004: Legacy skill scripts are migration debt

- Статус: `accepted`
- Дата: `2026-05-03`

## Контекст

Unica currently contains useful Python/PowerShell operation implementations
inside skill directories. They helped bootstrap XML/JSON DSL workflows, but they
split execution behavior between prompt-visible skill prose and the Rust MCP
orchestrator.

The target architecture requires one execution surface: MCP `unica`. If skills
continue to rely on local operation files, cache invalidation and command
semantics can drift away from the orchestrator.

## Решение

Skill-local Python/PowerShell operation files are migration debt, not accepted
target architecture.

1. All developer operations must be implemented as `unica.*` MCP tools.
2. Existing operation-file command semantics must be ported into native Rust MCP
   handlers with fixture parity.
3. Migrated skills must reference MCP `unica` tools only.
4. After parity, migrated skill-local operation files must be removed from
   `plugins/unica/skills`.
5. Package launchers and CI scripts remain infrastructure and are not covered by
   this removal rule.

## Неграницы

1. This ADR does not delete existing files immediately.
2. This ADR does not ban package wrapper scripts such as `run-unica.sh`.
3. This ADR does not require replacing bundled external engines that remain
   behind internal adapters.

## Последствия

1. The active task list must track the parity and removal work.
2. Skill tests should reject operation-file workflow guidance for migrated skills.
3. Native MCP handlers become the target home for XML/JSON DSL behavior.
4. Documentation must call current skill-local operation files migration debt.

## План реализации

1. Add parity fixtures around current operation behavior.
2. Port read-only `info` and `validate` operations into native Rust MCP handlers.
3. Port generators/removers, then editors and complex CFE/UI operations.
4. Rewrite each migrated skill to route only through MCP `unica`.
5. Remove the corresponding skill-local operation files after tests pass.

## Верификация

- [x] ADR states that skill-local operation files are not target architecture.
- [x] ADR distinguishes package launchers from skill-local operation files.
- [x] ADR requires MCP implementation and parity tests before deletion.
