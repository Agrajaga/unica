---
name: db-update
description: Применение изменений конфигурации к базе 1С через v8-runner build/load update. Используй когда нужно обновить БД после изменения исходников или CF/CFE
argument-hint: "[sources|artifact]"
allowed-tools:
  - Bash
  - Read
  - Glob
  - AskUserQuestion
---

# /db-update — обновление БД через v8-runner

Используй `v8project.yaml` или `V8TR_CONFIG`. Не запускай отдельный native `/UpdateDBCfg`, если v8-runner может выполнить обновление как часть build/load.

Перед обновлением применяй `/db-auth-check`. Если обнаружена проблема лицензии 1С, остановись и попроси пользователя исправить окружение; не пытайся чинить лицензию.

## Выбор команды

| Сценарий | Команда |
| --- | --- |
| Изменились XML-исходники | `../../scripts/run-v8-runner.sh build` |
| Нужна полная пересборка | `../../scripts/run-v8-runner.sh build --full-rebuild` |
| Нужно применить CF/CFE | `../../scripts/run-v8-runner.sh load --path <file> --mode update` |

Перед destructive update запроси подтверждение пользователя и уточни, нужна ли монопольная блокировка базы.

Native `scripts/db-update.*` используй только как fallback для специальных Designer-режимов, которых нет в v8-runner.
