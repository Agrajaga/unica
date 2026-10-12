---
id: INV.SOURCE.MODULE-COMMON-PROPERTIES
check:
  - crates/unica-coder/src/domain/module_projection.rs::common_module_flags_serialize_exactly_once_and_never_become_contexts
  - crates/unica-coder/src/infrastructure/bsl_module_projection.rs::common_module_requires_all_normalized_flags
  - crates/unica-coder/src/infrastructure/v13_read/tests.rs::borrowed_common_module_keeps_missing_privileged_unknown
  - crates/unica-coder/src/infrastructure/v13_read/tests.rs::borrowed_common_module_view_serializes_unknown_privileged_without_losing_contexts
  - crates/unica-coder/src/infrastructure/daemon/server.rs::borrowing_view_bounds_override_props_for_metadata_and_specialized_readers
  - crates/unica-coder/src/infrastructure/v13_read/tests.rs::platform_borrowed_common_module_preserves_unknown_contexts_and_independent_facts
  - crates/unica-coder/src/infrastructure/v13_read/tests.rs::common_module_absence_policy_keeps_ordinary_properties_required
  - crates/unica-coder/src/infrastructure/bsl_module_projection.rs::unknown_execution_flags_do_not_become_empty_or_universal_contexts
gap: https://github.com/IngvarConsulting/unica/issues/973
---

# Свойства общего модуля не подменяют контексты компиляции

Описание общего модуля содержит по одному ключу `global`,
`clientManagedApplication`, `server`, `externalConnection`,
`clientOrdinaryApplication`, `serverCall`, `privileged` и `returnValuesReuse`.
Читатель требует значения всех свойств у обычного модуля. В заимствованном
общем модуле расширения отсутствие любого из этих свойств возвращается как
`null`. Допустимость отсутствия подтверждена двумя независимыми циклами
импорта и выгрузки 8.3.27.2130 в
`tests/fixtures/platform_8_3_27/incomplete_borrowed/`.
Явно указанное некорректное значение остаётся ошибкой источника.
Построитель требует сам объект свойств целиком.

Без `ClientManagedApplication`, `ClientOrdinaryApplication`, `Server` или
`ExternalConnection` список контекстов компиляции неизвестен. Методы и
диапазоны сохраняют сигнатуры, директивы и условия, а `contexts` равен `null`.
Ответ сообщает ограничение; фильтр по контексту отказывает с его причиной.
Нефильтрованные методы и BSL доступны. Отсутствие остальных свойств не
мешает вычислению контекстов по присутствующим флагам исполнения.

`serverCall`, `privileged` и `returnValuesReuse` не являются отдельными
контекстами компиляции. Проверки охватывают наличие данных на входе
построителя и форму сериализации сводки; расчёт контекстов по разным
сочетаниям флагов требует проверки из `gap`.
