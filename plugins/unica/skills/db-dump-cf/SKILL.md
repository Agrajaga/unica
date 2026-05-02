---
name: db-dump-cf
description: Выгрузка конфигурации 1С в CF/CFE через v8-runner make. Используй когда нужно сохранить конфигурацию, расширение или релизный артефакт
argument-hint: "[output.cf]"
allowed-tools:
  - Bash
  - Read
  - Glob
  - AskUserQuestion
---

# /db-dump-cf — экспорт CF/CFE

Используй `v8project.yaml` или `V8TR_CONFIG`. Основная команда — `v8-runner make`.

Перед экспортом применяй `/db-auth-check`. Если обнаружена проблема лицензии 1С, остановись и попроси пользователя исправить окружение; не пытайся чинить лицензию.

## Команды

```sh
# основная конфигурация
../../scripts/run-v8-runner.sh make --output build/config.cf

# конкретный source-set
../../scripts/run-v8-runner.sh make --source-set main --output build/config.cf

# расширение
../../scripts/run-v8-runner.sh make --extension MyExtension --output build/MyExtension.cfe
```

Если `v8project.yaml` отсутствует, сначала выполни `/db-list init`.

Native `scripts/db-dump-cf.*` используй только как fallback для параметров Designer, которых нет в v8-runner.
