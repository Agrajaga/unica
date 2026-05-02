---
name: db-load-xml
description: Загрузка XML-исходников конфигурации в базу 1С через v8-runner build. Используй когда нужно LoadConfigFromFiles, применить source-set, обновить базу из исходников
argument-hint: "[--full-rebuild]"
allowed-tools:
  - Bash
  - Read
  - Glob
  - AskUserQuestion
---

# /db-load-xml — загрузка XML через v8-runner

Используй `v8project.yaml` или `V8TR_CONFIG`; каталог исходников берётся из `source-set`.

Перед загрузкой применяй `/db-auth-check`. Если обнаружена проблема лицензии 1С, остановись и попроси пользователя исправить окружение; не пытайся чинить лицензию.

## Команды

```sh
# обычная загрузка исходников
../../scripts/run-v8-runner.sh build

# полная пересборка без change cache
../../scripts/run-v8-runner.sh build --full-rebuild
```

Перед полной пересборкой запроси подтверждение пользователя. Если нужно изменить каталог исходников, обнови `source-set[].path` в `v8project.yaml`, а не передавай отдельный config dir в native script.

Native `scripts/db-load-xml.*` оставлен только как fallback для частичных сценариев, которых нет в v8-runner.
