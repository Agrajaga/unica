---
name: db-create
description: Создание или инициализация информационной базы 1С через v8-runner. Используй когда нужно создать базу, подготовить v8project.yaml, выполнить v8-runner init
argument-hint: "<connection>"
allowed-tools:
  - Bash
  - Read
  - Write
  - Glob
  - AskUserQuestion
---

# /db-create — создание базы через v8-runner

Используй `v8project.yaml` как единственный проектный конфиг. Если разработчик начинает в пустом репозитории без workspace, сначала используй `/workspace-init`.

## Workflow

1. Определи строку соединения 1С: файловая база `/F/path/to/ib`, серверная база `/Sserver/ref`.
2. Создай или обнови конфиг:

```sh
../../scripts/run-v8-runner.sh config init --connection '/F/Users/me/1c-bases/dev'
```

3. Инициализируй базу и workspace:

```sh
../../scripts/run-v8-runner.sh init
```

4. Если конфиг лежит не в корне проекта, передавай `V8TR_CONFIG=/path/to/v8project.yaml`.

## Важно

- Перед первой операцией с базой применяй `/db-auth-check`: без логина пробуй только `Администратор` без пароля, затем `Admin` без пароля; при проблеме лицензии остановись и попроси пользователя исправить окружение.
- Для существующего `v8project.yaml` не перезаписывай файл без явного согласия; используй `config init --force` только после подтверждения.
- Для загрузки исходников после создания базы используй `/db-load-xml`, который вызывает `v8-runner build`.
- Native `scripts/db-create.*` оставлены только как fallback, если v8-runner не покрывает конкретную платформенную ситуацию.
