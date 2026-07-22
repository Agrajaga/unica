use crate::application::operation_descriptors::{native_operation_descriptor, FormatGuardPolicy};
use crate::application::ports::FormatGuardCheck;
use crate::application::{AdapterOutcome, ToolHandler, ToolSpec};
use crate::domain::format_profile::{
    classify_root_version, FormatCompatibility, ACTIVE_FORMAT_PROFILE,
};
use crate::domain::workspace::WorkspaceContext;
use roxmltree::Document;
use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn evaluate_format_guard(
    spec: ToolSpec,
    args: &Map<String, Value>,
    context: &WorkspaceContext,
) -> Result<FormatGuardCheck, String> {
    let ToolHandler::NativeOperation { operation, .. } = spec.handler else {
        return Ok(FormatGuardCheck::Allow);
    };
    let Some(descriptor) = native_operation_descriptor(operation) else {
        return Ok(FormatGuardCheck::Allow);
    };
    if descriptor.format_guard != FormatGuardPolicy::ExistingDump {
        return Ok(FormatGuardCheck::Allow);
    }
    for name in descriptor.source_path_args {
        let Some(raw_path) = args.get(*name).and_then(Value::as_str) else {
            continue;
        };
        let target = absolutize(raw_path, &context.cwd);
        let Some(root) = resolve_version_root(&target)? else {
            continue;
        };
        let root_info = read_root_info(&root)?;
        let compatibility = match classify_root_version(root_info.version.as_deref()) {
            Ok(compatibility) => compatibility,
            Err(error) => {
                let diagnostic = json!({
                    "code": error.code(),
                    "actualFormat": root_info.version,
                    "targetFormat": ACTIVE_FORMAT_PROFILE.export_format,
                    "targetPlatform": ACTIVE_FORMAT_PROFILE.platform_line,
                    "compatibility": "invalid",
                    "root": root.display().to_string(),
                });
                return Ok(format_check(
                    spec,
                    format!("Некорректная версия формата выгрузки в {}", root.display()),
                    diagnostic,
                ));
            }
        };
        if matches!(compatibility, FormatCompatibility::Supported { .. }) {
            continue;
        }
        let actual = compatibility.actual().to_string();
        let (code, warning) = match compatibility {
            FormatCompatibility::Older { .. } => {
                let migration_tool = if root_info.is_extension {
                    "unica.cfe.migrate_format"
                } else {
                    "unica.cf.migrate_format"
                };
                (
                    "formatMigrationAvailable",
                    format!(
                        "Формат выгрузки {actual} старше поддерживаемого {} для платформы 1С {}. Изменение отменено; предложите пользователю явную миграцию через {migration_tool}.",
                        ACTIVE_FORMAT_PROFILE.export_format, ACTIVE_FORMAT_PROFILE.platform_line
                    ),
                )
            }
            FormatCompatibility::Newer { .. } => (
                "platformVersionUnsupported",
                format!(
                    "Формат выгрузки {actual} новее поддерживаемого {} для платформы 1С {}. Unica пока не поддерживает работу с этой выгрузкой. Поддержка платформы 1С 8.5 планируется в ближайших версиях.",
                    ACTIVE_FORMAT_PROFILE.export_format, ACTIVE_FORMAT_PROFILE.platform_line
                ),
            ),
            FormatCompatibility::Supported { .. } => unreachable!(),
        };
        let diagnostic = json!({
            "code": code,
            "actualFormat": actual,
            "targetFormat": ACTIVE_FORMAT_PROFILE.export_format,
            "targetPlatform": ACTIVE_FORMAT_PROFILE.platform_line,
            "compatibility": compatibility.label(),
            "root": root.display().to_string(),
        });
        return Ok(format_check(spec, warning, diagnostic));
    }
    Ok(FormatGuardCheck::Allow)
}

fn format_check(spec: ToolSpec, warning: String, diagnostic: Value) -> FormatGuardCheck {
    if !spec.mutating {
        return FormatGuardCheck::Warn {
            warning,
            diagnostic,
        };
    }
    FormatGuardCheck::Block {
        outcome: AdapterOutcome {
            ok: false,
            summary: format!("{} blocked by export format guard", spec.name),
            changes: Vec::new(),
            warnings: vec![warning.clone()],
            errors: vec![warning.clone()],
            artifacts: Vec::new(),
            stdout: None,
            stderr: Some(format!("{warning}\n")),
            command: None,
        },
        diagnostic,
    }
}

struct RootInfo {
    version: Option<String>,
    is_extension: bool,
}

fn read_root_info(path: &Path) -> Result<RootInfo, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read format root {}: {error}", path.display()))?;
    let document = Document::parse(text.trim_start_matches('\u{feff}'))
        .map_err(|error| format!("failed to parse format root {}: {error}", path.display()))?;
    let root = document.root_element();
    Ok(RootInfo {
        version: root.attribute("version").map(str::to_string),
        is_extension: document.descendants().any(|node| {
            node.is_element() && node.tag_name().name() == "ConfigurationExtensionPurpose"
        }),
    })
}

fn resolve_version_root(target: &Path) -> Result<Option<PathBuf>, String> {
    let start = if target.is_dir() {
        target
    } else {
        target.parent().unwrap_or(target)
    };
    for ancestor in start.ancestors() {
        let candidate = ancestor.join("Configuration.xml");
        if candidate.is_file() {
            return Ok(Some(candidate));
        }
    }
    if target.is_file() && target.extension().and_then(|value| value.to_str()) == Some("xml") {
        let info = read_root_info(target)?;
        if info.version.is_some() {
            return Ok(Some(target.to_path_buf()));
        }
    }
    Ok(None)
}

fn absolutize(raw: &str, cwd: &Path) -> PathBuf {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::evaluate_format_guard;
    use crate::application::ports::FormatGuardCheck;
    use crate::application::tools;
    use crate::domain::workspace::WorkspaceContext;
    use serde_json::{Map, Value};

    fn context(root: &std::path::Path) -> WorkspaceContext {
        WorkspaceContext {
            cwd: root.to_path_buf(),
            workspace_root: root.to_path_buf(),
            cache_root: root.join(".build/unica"),
            workspace_epoch: 1,
        }
    }

    fn config(root: &std::path::Path, version: Option<&str>) -> std::path::PathBuf {
        let src = root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        let version = version
            .map(|value| format!(r#" version="{value}""#))
            .unwrap_or_default();
        std::fs::write(
            src.join("Configuration.xml"),
            format!(r#"<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses"{version}/>"#),
        )
        .unwrap();
        src.join("Configuration.xml")
    }

    fn spec(name: &str) -> crate::application::ToolSpec {
        tools().into_iter().find(|tool| tool.name == name).unwrap()
    }

    #[test]
    fn older_dump_blocks_mutation_and_offers_explicit_migration() {
        let root = std::env::temp_dir().join(format!(
            "unica-format-guard-old-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let path = config(&root, Some("2.19"));
        let before = std::fs::read(&path).unwrap();
        let mut args = Map::new();
        args.insert(
            "ConfigPath".into(),
            Value::String(path.display().to_string()),
        );

        let check = evaluate_format_guard(spec("unica.cf.edit"), &args, &context(&root)).unwrap();
        let FormatGuardCheck::Block {
            outcome,
            diagnostic,
        } = check
        else {
            panic!("older mutation must be blocked");
        };
        assert!(!outcome.ok);
        assert_eq!(diagnostic["code"], "formatMigrationAvailable");
        assert_eq!(diagnostic["actualFormat"], "2.19");
        assert!(outcome
            .warnings
            .join("\n")
            .contains("unica.cf.migrate_format"));
        assert_eq!(std::fs::read(path).unwrap(), before);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn older_extension_dump_offers_extension_migration() {
        let root =
            std::env::temp_dir().join(format!("unica-format-guard-old-cfe-{}", std::process::id()));
        let src = root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        let path = src.join("Configuration.xml");
        std::fs::write(
            &path,
            r#"<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.19"><Configuration><Properties><ConfigurationExtensionPurpose>Customization</ConfigurationExtensionPurpose></Properties></Configuration></MetaDataObject>"#,
        )
        .unwrap();
        let mut args = Map::new();
        args.insert(
            "ExtensionPath".into(),
            Value::String(path.display().to_string()),
        );

        let check =
            evaluate_format_guard(spec("unica.cfe.patch_method"), &args, &context(&root)).unwrap();
        let FormatGuardCheck::Block { outcome, .. } = check else {
            panic!("older extension mutation must be blocked");
        };
        assert!(outcome
            .warnings
            .join("\n")
            .contains("unica.cfe.migrate_format"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn supported_dump_allows_mutation_preflight() {
        let root =
            std::env::temp_dir().join(format!("unica-format-guard-ok-{}", std::process::id()));
        let path = config(&root, Some("2.20"));
        let mut args = Map::new();
        args.insert(
            "ConfigPath".into(),
            Value::String(path.display().to_string()),
        );
        assert!(matches!(
            evaluate_format_guard(spec("unica.cf.edit"), &args, &context(&root)).unwrap(),
            FormatGuardCheck::Allow
        ));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn newer_dump_warns_for_read_only_with_roadmap_copy() {
        let root =
            std::env::temp_dir().join(format!("unica-format-guard-new-{}", std::process::id()));
        let path = config(&root, Some("2.21"));
        let mut args = Map::new();
        args.insert(
            "ConfigPath".into(),
            Value::String(path.display().to_string()),
        );
        let check = evaluate_format_guard(spec("unica.cf.info"), &args, &context(&root)).unwrap();
        let FormatGuardCheck::Warn {
            warning,
            diagnostic,
        } = check
        else {
            panic!("newer read-only input must warn and continue");
        };
        assert_eq!(diagnostic["code"], "platformVersionUnsupported");
        assert!(warning.contains("Поддержка платформы 1С 8.5 планируется"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn missing_root_version_is_classified_as_1_0() {
        let root =
            std::env::temp_dir().join(format!("unica-format-guard-v1-{}", std::process::id()));
        let path = config(&root, None);
        let mut args = Map::new();
        args.insert(
            "ConfigPath".into(),
            Value::String(path.display().to_string()),
        );
        let check =
            evaluate_format_guard(spec("unica.cf.validate"), &args, &context(&root)).unwrap();
        let FormatGuardCheck::Warn { diagnostic, .. } = check else {
            panic!("missing root version must be old-format warning");
        };
        assert_eq!(diagnostic["actualFormat"], "1.0");
        let _ = std::fs::remove_dir_all(root);
    }
}
