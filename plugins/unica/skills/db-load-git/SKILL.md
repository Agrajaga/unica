---
name: db-load-git
description: Загрузка изменений из Git/рабочего дерева в базу 1С через v8-runner build. Используй когда нужно применить изменения исходников к базе
argument-hint: "[--full-rebuild]"
allowed-tools:
  - Bash
  - Read
  - Glob
  - AskUserQuestion
---

# /db-load-git — загрузка изменений через v8-runner

Используй `v8project.yaml` или `V8TR_CONFIG`. Для обычного применения изменений из source-set запускай `v8-runner build`; change/cache логику должен вести v8-runner.

Перед операцией с базой применяй `/db-auth-check`. Если обнаружена проблема лицензии 1С, остановись и попроси пользователя исправить окружение; не пытайся чинить лицензию.

## Команды

```sh
# применить изменения source-set к базе
../../scripts/run-v8-runner.sh build

# принудительно пересобрать всё
../../scripts/run-v8-runner.sh build --full-rebuild
```

Если пользователь просит только посмотреть изменения, используй `git status` / `git diff --name-only` и не запускай build.

Native `scripts/db-load-git.*` используй только когда нужен режим, которого нет в v8-runner.
