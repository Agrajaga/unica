---
id: INV.SOURCE.MODULE-COMMON-PROPERTIES
check:
  - crates/unica-coder/src/domain/module_projection.rs::common_module_flags_serialize_exactly_once_and_never_become_contexts
  - crates/unica-coder/src/infrastructure/bsl_module_projection.rs::common_module_requires_all_normalized_flags
  - crates/unica-coder/src/infrastructure/v13_read/tests.rs::borrowed_common_module_keeps_missing_privileged_unknown
  - crates/unica-coder/src/infrastructure/v13_read/tests.rs::borrowed_common_module_view_serializes_unknown_privileged_without_losing_contexts
  - crates/unica-coder/src/infrastructure/daemon/server.rs::borrowing_view_bounds_override_props_for_metadata_and_specialized_readers
  - crates/unica-coder/src/infrastructure/v13_read/tests.rs::common_module_return_values_reuse_requires_value_unless_borrowed
  - crates/unica-coder/tests/support/cfe_structure.rs::canonical_stdio_views_borrowed_common_module_without_return_values_reuse
gap: https://github.com/IngvarConsulting/unica/issues/973
---

# Свойства общего модуля не подменяют контексты компиляции

Описание общего модуля содержит по одному ключу `global`,
`clientManagedApplication`, `server`, `externalConnection`,
`clientOrdinaryApplication`, `serverCall`, `privileged` и `returnValuesReuse`.
Читатель требует значения всех свойств у обычного модуля. Для
заимствованного объектом расширения `CommonModule` отсутствие `Privileged`
или `ReturnValuesReuse` даёт `null` в соответствующем ключе `privileged`
или `returnValuesReuse`. Отсутствующее сведение не превращается в `false`
или `DontUse`. Некорректное значение `Privileged` и элемент
`ReturnValuesReuse` без текстового содержимого остаются ошибкой источника.
Построитель требует сам объект свойств целиком.

`serverCall`, `privileged` и `returnValuesReuse` не являются отдельными
контекстами компиляции. Проверки охватывают наличие данных на входе
построителя и форму сериализации сводки; расчёт контекстов по разным
сочетаниям флагов требует проверки из `gap`.
