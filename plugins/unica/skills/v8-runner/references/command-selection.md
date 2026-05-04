# Command Selection

Use MCP `unica.runtime.execute` and choose `operation` by intent:

| Intent | Arguments |
|---|---|
| Missing project config | `operation=config-init`, optional `config`, `connection`, `format`, `builder` |
| Create runtime state | `operation=init` |
| Apply source changes to infobase | `operation=build`, optional `sourceSet`, `fullRebuild` |
| Bring infobase changes back to files | `operation=dump`, optional `mode`, `object`, `sourceSet`, `extension` |
| Convert Designer/EDT files | `operation=convert`, optional `sourceSet`, `output` |
| Export artifact | `operation=make`, required `output`, optional `sourceSet`, `extension` |
| Load artifact | `operation=load`, required `path`, optional `mode`, `settings`, `extension` |
| Syntax check | `operation=syntax`, required `mode` |
| Tests | `operation=test`, required `testRunner`, optional `testScope`, `module` |
| Client launch | `operation=launch`, required `clientMode` |
| Extension properties | `operation=extensions`, optional `sourceSet` |

For branch switches, rebases, large object moves, or suspicious incremental state, use `operation=build` with `fullRebuild=true`.

For dumps, inspect the worktree before execution and compare the resulting diff after execution.
