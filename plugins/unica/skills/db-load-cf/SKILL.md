---
name: db-load-cf
description: Загрузка CF/CFE артефакта в базу 1С через v8-runner load. Используй когда нужно загрузить, восстановить, merge или update из .cf/.cfe
argument-hint: "<input.cf|input.cfe>"
allowed-tools:
  - Bash
  - Read
  - Glob
  - AskUserQuestion
---

# /db-load-cf — загрузка CF/CFE через v8-runner

Используй `v8project.yaml` или `V8TR_CONFIG`. Основная команда — `v8-runner load`.

Перед загрузкой применяй `/db-auth-check`. Если обнаружена проблема лицензии 1С, остановись и попроси пользователя исправить окружение; не пытайся чинить лицензию.

## Команды

```sh
# загрузить артефакт
../../scripts/run-v8-runner.sh load --path build/config.cf --mode load

# merge с настройками
../../scripts/run-v8-runner.sh load --path build/config.cf --mode merge --settings merge-settings.xml

# загрузить и применить обновление
../../scripts/run-v8-runner.sh load --path build/config.cf --mode update

# расширение
../../scripts/run-v8-runner.sh load --path build/MyExtension.cfe --extension MyExtension --mode update
```

Перед destructive `load`/`update` запроси подтверждение пользователя. Native `scripts/db-load-cf.*` оставлен только как fallback.
