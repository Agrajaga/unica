---
id: INV.SOURCE.PRIVATE-COMPILE-RECOVERY
status: active
governs: product
decision: DEC.2026-08-23.PRIVATE-COMPILE-RECOVERY
check: crates/unica-coder/src/infrastructure/native_operations/compile_transaction.rs::private_compile_recovery_contract_is_physical_and_rollback_safe
scope: [source]
---

# Recovery compile-транзакции не публикуется в source-set

Registration backup, removal backup и rollback quarantine для цели внутри
workspace резервируются под `<workspace>/.build/unica/recovery`, а не внутри
дерева исходников. Подготовка и резервирование не следуют через symbolic link
или reparse point, а rollback удерживает физический parent публикуемой цели
отдельно от parent приватного recovery. Публикация и восстановление перемещают
identity-bound child между этими удержанными parent, а совместимость файловых
систем проверяется до первой мутации.
