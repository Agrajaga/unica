---
name: db-dump-xml
description: Выгрузка конфигурации 1С в XML-исходники через v8-runner dump. Используй когда нужно dump, DumpConfigToFiles, полная, инкрементальная или частичная выгрузка
argument-hint: "[full|incremental|partial]"
allowed-tools:
  - Bash
  - Read
  - Glob
  - AskUserQuestion
---

# /db-dump-xml — выгрузка XML через v8-runner

Используй `v8project.yaml` или `V8TR_CONFIG`. Source path и формат берутся из `source-set`.

Перед выгрузкой применяй `/db-auth-check`. Если обнаружена проблема лицензии 1С, остановись и попроси пользователя исправить окружение; не пытайся чинить лицензию.

## Команды

```sh
# полная выгрузка
../../scripts/run-v8-runner.sh dump --mode full

# инкрементальная выгрузка
../../scripts/run-v8-runner.sh dump --mode incremental

# частичная выгрузка
../../scripts/run-v8-runner.sh dump --mode partial --object Catalog:Номенклатура

# расширение или конкретный source-set
../../scripts/run-v8-runner.sh dump --mode full --extension MyExtension
../../scripts/run-v8-runner.sh dump --mode full --source-set main
```

Если пользователь просит каталог выгрузки, проверь `source-set[].path` в `v8project.yaml`; не передавай отдельный каталог в native script, пока v8-runner может выполнить операцию.
