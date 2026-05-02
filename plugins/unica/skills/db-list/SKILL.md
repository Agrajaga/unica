---
name: db-list
description: Управление v8project.yaml для 1С-проекта. Используй когда нужно создать, проверить, показать или обновить конфигурацию v8-runner, подключение к базе, source-set или путь V8TR_CONFIG
argument-hint: "[init|show|source-set]"
allowed-tools:
  - Bash
  - Read
  - Write
  - Glob
  - AskUserQuestion
---

# /db-list — конфигурация v8-runner

Единственный проектный конфиг Unica — `v8project.yaml` или файл из `V8TR_CONFIG`.
Не создавай и не читай отдельный JSON-реестр проекта.

Если репозиторий новый и workspace еще не создан, используй `/workspace-init`, а не ручную сборку нескольких разрозненных файлов.

## Быстрый выбор

| Задача | Действие |
| --- | --- |
| Конфига нет | `../../scripts/run-v8-runner.sh config init --connection '<строка>'` |
| Показать текущую базу | Прочитать `connection` из `v8project.yaml` |
| Показать исходники | Прочитать `source-set` |
| Сменить базу | Обновить `connection` в `v8project.yaml` или пересоздать через `config init --force` |
| Другой путь к конфигу | Использовать `V8TR_CONFIG=/path/to/v8project.yaml` |

## Формат

Минимальный файл:

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

Серверная база задаётся обычной строкой соединения 1С, например `/Sserver/ref`.

## Команды

```sh
# создать конфиг в текущем проекте
../../scripts/run-v8-runner.sh config init --connection '/F/Users/me/1c-bases/dev'

# создать в явном файле
../../scripts/run-v8-runner.sh config init --file ./v8project.yaml --connection '/Ssrv01/MyApp'

# перезаписать существующий конфиг
../../scripts/run-v8-runner.sh config init --force --connection '/F/Users/me/1c-bases/dev'
```

## Правила для остальных skills

1. Сначала используй `V8TR_CONFIG`, если переменная задана.
2. Иначе используй `./v8project.yaml` в корне текущего проекта.
3. Если конфига нет, спроси строку соединения и создай его через `v8-runner config init`.
4. Для операций загрузки, выгрузки, запуска, синтаксиса и тестов используй v8-runner команды поверх этого конфига.
5. Перед операциями с базой применяй `/db-auth-check`; при ошибке лицензии не пытайся чинить окружение.
6. Для операций, которых нет в v8-runner, используй native script, но параметры базы бери из `connection`, а исходники из `source-set`.

Подробная карта команд лежит в `../../references/v8project.md`.
