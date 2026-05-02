---
name: db-run
description: Запуск 1С:Предприятия или Конфигуратора через v8-runner launch. Используй когда нужно открыть базу, Designer, thin/thick client, передать /C или /Execute
argument-hint: "[thin|thick|designer|ordinary]"
allowed-tools:
  - Bash
  - Read
  - Glob
  - AskUserQuestion
---

# /db-run — запуск 1С через v8-runner

Используй `v8project.yaml` или `V8TR_CONFIG`. Основная команда — `v8-runner launch`.

Перед запуском базы применяй `/db-auth-check`, если credentials не проверены в текущем сеансе. При ошибке лицензии 1С остановись и попроси пользователя исправить окружение.

## Команды

```sh
# пользовательский режим
../../scripts/run-v8-runner.sh launch thin

# конфигуратор
../../scripts/run-v8-runner.sh launch designer

# запуск внешней обработки
../../scripts/run-v8-runner.sh launch thin --execute build/Tool.epf

# параметр /C
../../scripts/run-v8-runner.sh launch thin --c 'ЗапуститьОбновление'
```

Для нестандартных ключей платформы используй `--raw-key`.
