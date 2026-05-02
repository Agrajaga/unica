# v8project.yaml Contract

`v8project.yaml` is the only project configuration format used by Unica skills.
Use `V8TR_CONFIG` when the config file is not located at `./v8project.yaml`.

For a new repository with no workspace, use the `workspace-init` skill first. It
creates `v8project.yaml`, prepares the default `src` source-set, checks database
access, and stops on license problems instead of attempting environment repair.

Create or refresh the config with the bundled v8-runner:

```sh
plugins/unica/scripts/run-v8-runner.sh config init --connection '<connection-string>'
```

When running from a skill package, resolve the plugin launcher path relative to
the skill directory as `../../scripts/run-v8-runner.sh`.

## Minimal Shape

```yaml
basePath: '.'
workPath: 'build'
format: DESIGNER
builder: DESIGNER
connection: '/F/Users/me/1c-bases/dev'
source-set:
  - name: main
    type: CONFIGURATION
    path: 'src'
build:
  partialLoadThreshold: 20
```

Server infobase connections use the normal 1C connection string form, for
example `/Sserver/ref`.

## Command Mapping

Use v8-runner before older native Designer scripts when the operation is covered.

| Operation | v8-runner command |
| --- | --- |
| Create project config | `v8-runner config init --connection '<connection>'` |
| Initialize infobase/workspace | `v8-runner init` |
| Load XML sources and update DB | `v8-runner build` |
| Force full source load | `v8-runner build --full-rebuild` |
| Dump XML sources | `v8-runner dump --mode full|incremental|partial` |
| Dump selected objects | `v8-runner dump --mode partial --object TYPE:NAME` |
| Load `.cf` / `.cfe` artifact | `v8-runner load --path <file> --mode load|merge|update` |
| Export `.cf` / `.cfe` artifact | `v8-runner make --output <file>` |
| Launch 1C | `v8-runner launch thin|thick|designer|ordinary` |
| Run syntax checks | `v8-runner syntax designer-config` or `designer-modules` |
| Run tests | `v8-runner test yaxunit` or `test va` |

## Skill Rules

- Do not create or read any legacy JSON project registry.
- Resolve the active config as `V8TR_CONFIG` first, then `./v8project.yaml`.
- If the config is missing, use `v8-runner config init` or ask for the connection string.
- Prefer `source-set` names over ad hoc source directories.
- When credentials are absent, try only empty-password `Администратор`, then empty-password `Admin`; if both fail, ask the user.
- If a command reports a 1C license problem, stop and ask the user to fix licensing. Do not edit license services, HASP settings, registry, or license files.
- Use native skill scripts only for operations v8-runner does not expose directly, such as web Apache publication helpers and EPF/ERF dump/build fallback flows.
