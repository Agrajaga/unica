# Конфигурация проекта v8project.yaml

`v8project.yaml` — единый проектный конфиг Unica и v8-runner. Для локальных
секретов и путей используй `v8project.local.yaml`; для нестандартного
расположения основного конфига используй `V8TR_CONFIG`.

## Создание

Создавай конфиг через MCP `unica.runtime.execute`:

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "unica.runtime.execute",
    "arguments": {
      "operation": "config-init",
      "config": "./v8project.yaml",
      "connection": "File=build/ib",
      "dryRun": false
    }
  }
}
```

## Минимальный пример

```yaml
basePath: '.'
workPath: 'build'
format: DESIGNER
builder: DESIGNER
infobase:
  connection: 'File=build/ib'
source-set:
  - name: main
    type: CONFIGURATION
    path: 'src'
```

## Правила Unica

- `V8TR_CONFIG` имеет приоритет над `./v8project.yaml`.
- Skill `v8-runner` использует MCP `unica.runtime.execute`; cache refresh делает orchestrator.
- Для source path используй `source-set[].path`, а не отдельный project registry.
- Для web helpers используй connection из проекта, но Apache path задавай `UNICA_APACHE_PATH`, `-ApachePath` или `tools/apache24`.
- Credentials держи в локальном overlay или переменных окружения, а не в коммитимом конфиге.

## Операции

| Задача | MCP arguments |
| --- | --- |
| Инициализация базы/workspace | `operation=init` |
| Загрузка исходников | `operation=build` |
| Полная пересборка | `operation=build`, `fullRebuild=true` |
| Выгрузка XML | `operation=dump`, `mode=full|incremental|partial` |
| Экспорт CF/CFE | `operation=make`, `output=<file>` |
| Загрузка CF/CFE | `operation=load`, `path=<file>`, `mode=load|merge|update` |
| Запуск 1С | `operation=launch`, `clientMode=thin|thick|designer|ordinary` |
| Синтаксическая проверка | `operation=syntax`, `mode=designer-config|designer-modules|edt` |

Более короткая внутренняя памятка: `plugins/unica/references/v8project.md`.
