# Внешние обработки и отчеты (EPF / ERF)

Runtime-сценарии EPF/ERF выполняются через skill `v8-runner` и публичный MCP tool `unica.runtime.execute`. Старые отдельные EPF/ERF runtime skills заменены external source-set workflow.

`epf-bsp-init` и `epf-bsp-add-command` остаются помощниками для редактирования кода регистрации БСП. Они не собирают и не выгружают артефакты.

## Source Sets

| Артефакт | v8-runner source-set type | Типовое имя source-set |
| --- | --- | --- |
| `.epf` external processing | `EXTERNAL_DATA_PROCESSORS` | `external-processors` |
| `.erf` external report | `EXTERNAL_REPORTS` | `external-reports` |

External source sets настраиваются в `v8project.yaml`. Если нужный source-set еще не объявлен, используй `v8-runner` `operation=config-init` или явно отредактируй конфиг.

## MCP Workflows

### Загрузка XML-исходников в базу

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "unica.runtime.execute",
    "arguments": {
      "operation": "build",
      "cwd": "<workspace>",
      "sourceSet": "external-processors",
      "mode": "full"
    }
  }
}
```

Для внешних отчетов используй `sourceSet: "external-reports"`.

### Выгрузка внешних исходников из базы

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "unica.runtime.execute",
    "arguments": {
      "operation": "dump",
      "cwd": "<workspace>",
      "sourceSet": "external-reports",
      "mode": "full"
    }
  }
}
```

### Публикация `.epf` / `.erf` артефактов

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "unica.runtime.execute",
    "arguments": {
      "operation": "make",
      "cwd": "<workspace>",
      "sourceSet": "external-processors",
      "output": "build/external"
    }
  }
}
```

Для external source-set операция `make` пишет артефакты в каталог. Для публикации `.erf` используй `sourceSet: "external-reports"`.

## Related Skills

Работа с XML на уровне объектов остается за native metadata skills:

- `form-add`, `form-compile`, `form-edit`, `form-validate`, `form-info`;
- `template-add`, `template-remove`;
- `mxl-compile`, `mxl-decompile`, `mxl-info`, `mxl-validate`;
- `skd-compile`, `skd-edit`, `skd-info`, `skd-validate`;
- `epf-bsp-init`, `epf-bsp-add-command` для кода регистрации БСП.

## Important Limits

- `operation=load` предназначен только для `.cf` и `.cfe`; он не загружает `.epf` или `.erf`.
- Для `build` и `dump` нужна настроенная база с целевой платформой и типами метаданных. Пустые базы могут потерять ссылочные типы при выгрузке.
- Инвалидацией cache после успешных non-dry-run runtime операций управляет MCP orchestrator `unica`; skill-инструкции не должны просить ассистента запускать отдельное обновление cache.

## Specifications

- [XML-формат внешних обработок](1c-epf-spec.md)
- [XML-формат внешних отчетов](1c-erf-spec.md)
- [Встроенная справка](1c-help-spec.md)
- [Runtime-сборка и выгрузка](build-spec.md)
