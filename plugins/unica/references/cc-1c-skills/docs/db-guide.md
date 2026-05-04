# Базы данных и runtime workflows 1С

Старые `/db-*` навыки заменены единым packaged skill `v8-runner`.
Обычный путь выполнения: MCP `unica` -> tool `unica.runtime.execute` -> internal
v8-runner adapter. Отдельный запуск package launcher-а не является нормальным
workflow для пользовательских задач.

## Карта операций

| Задача | MCP arguments |
| --- | --- |
| Создать `v8project.yaml` | `operation=config-init`, `connection=<строка>` |
| Инициализировать базу/workspace | `operation=init` |
| Загрузить исходники | `operation=build` |
| Полная пересборка | `operation=build`, `fullRebuild=true` |
| Выгрузить исходники | `operation=dump`, `mode=full|incremental|partial` |
| Экспортировать CF/CFE/EPF/ERF | `operation=make`, `output=<file>` |
| Загрузить CF/CFE | `operation=load`, `path=<file>`, `mode=load|merge|update` |
| Запустить 1С | `operation=launch`, `clientMode=thin|thick|designer|ordinary` |
| Проверить синтаксис | `operation=syntax`, `mode=designer-config|designer-modules|edt` |
| Запустить тесты | `operation=test`, `testRunner=yaxunit|va` |

## Пример

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "unica.runtime.execute",
    "arguments": {
      "cwd": "<workspace>",
      "operation": "build",
      "sourceSet": "main",
      "dryRun": false
    }
  }
}
```

## Спецификации

- [v8project-guide.md](v8project-guide.md) — формат `v8project.yaml`
- [build-spec.md](build-spec.md) — пакетный режим платформы 1С
