use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use roxmltree::Document;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use unica_coder::application::{ToolHandler, UnicaApplication};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum XmlImpactClass {
    None,
    CreateOrModify,
    RemoveOrModify,
}

#[derive(Debug, Clone, Copy)]
struct MutatorRegistryEntry {
    tool: &'static str,
    operation: &'static str,
    impact: XmlImpactClass,
    case_ids: &'static [&'static str],
    required_branches: &'static [&'static str],
}

#[derive(Debug, Clone, Copy)]
struct ExecutableCase {
    id: &'static str,
    tool: &'static str,
    branch: &'static str,
}

static MUTATOR_REGISTRY: &[MutatorRegistryEntry] = &[
    MutatorRegistryEntry {
        tool: "unica.cf.edit",
        operation: "cf-edit",
        impact: XmlImpactClass::CreateOrModify,
        case_ids: &[
            "cf-edit-root-property",
            "cf-edit-set-panels",
            "cf-edit-set-home-page",
        ],
        required_branches: &["root-property", "set-panels", "set-home-page"],
    },
    MutatorRegistryEntry {
        tool: "unica.cf.init",
        operation: "cf-init",
        impact: XmlImpactClass::CreateOrModify,
        case_ids: &["cf-init-default"],
        required_branches: &["default"],
    },
    MutatorRegistryEntry {
        tool: "unica.cfe.borrow",
        operation: "cfe-borrow",
        impact: XmlImpactClass::CreateOrModify,
        case_ids: &["cfe-borrow-object", "cfe-borrow-managed-form"],
        required_branches: &["metadata-object", "managed-form"],
    },
    MutatorRegistryEntry {
        tool: "unica.cfe.init",
        operation: "cfe-init",
        impact: XmlImpactClass::CreateOrModify,
        case_ids: &["cfe-init-default"],
        required_branches: &["default"],
    },
    MutatorRegistryEntry {
        tool: "unica.cfe.patch_method",
        operation: "cfe-patch-method",
        impact: XmlImpactClass::None,
        case_ids: &["cfe-patch-method-bsl-only"],
        required_branches: &["bsl-module-only"],
    },
    MutatorRegistryEntry {
        tool: "unica.code.patch",
        operation: "code-patch",
        impact: XmlImpactClass::None,
        case_ids: &["code-patch-bsl-only"],
        required_branches: &["bsl-only"],
    },
    MutatorRegistryEntry {
        tool: "unica.dcs.compile",
        operation: "dcs-compile",
        impact: XmlImpactClass::CreateOrModify,
        case_ids: &["dcs-compile-standalone"],
        required_branches: &["standalone"],
    },
    MutatorRegistryEntry {
        tool: "unica.dcs.edit",
        operation: "dcs-edit",
        impact: XmlImpactClass::CreateOrModify,
        case_ids: &["dcs-edit-owned-template"],
        required_branches: &["owned-template"],
    },
    MutatorRegistryEntry {
        tool: "unica.epf.init",
        operation: "epf-init",
        impact: XmlImpactClass::CreateOrModify,
        case_ids: &["epf-init-managed-form"],
        required_branches: &["managed-form"],
    },
    MutatorRegistryEntry {
        tool: "unica.erf.init",
        operation: "erf-init",
        impact: XmlImpactClass::CreateOrModify,
        case_ids: &["erf-init-managed-form"],
        required_branches: &["managed-form"],
    },
    MutatorRegistryEntry {
        tool: "unica.form.add",
        operation: "form-add",
        impact: XmlImpactClass::CreateOrModify,
        case_ids: &["form-add-managed"],
        required_branches: &["managed-form"],
    },
    MutatorRegistryEntry {
        tool: "unica.form.compile",
        operation: "form-compile",
        impact: XmlImpactClass::CreateOrModify,
        case_ids: &["form-compile-managed"],
        required_branches: &["managed-form"],
    },
    MutatorRegistryEntry {
        tool: "unica.form.edit",
        operation: "form-edit",
        impact: XmlImpactClass::CreateOrModify,
        case_ids: &["form-edit-managed"],
        required_branches: &["managed-form"],
    },
    MutatorRegistryEntry {
        tool: "unica.form.remove",
        operation: "form-remove",
        impact: XmlImpactClass::RemoveOrModify,
        case_ids: &["form-remove-managed"],
        required_branches: &["remove-managed-form"],
    },
    MutatorRegistryEntry {
        tool: "unica.help.add",
        operation: "help-add",
        impact: XmlImpactClass::CreateOrModify,
        case_ids: &["help-add-object"],
        required_branches: &["object-help"],
    },
    MutatorRegistryEntry {
        tool: "unica.interface.edit",
        operation: "interface-edit",
        impact: XmlImpactClass::CreateOrModify,
        case_ids: &["interface-edit-subsystem"],
        required_branches: &["subsystem-command-interface"],
    },
    MutatorRegistryEntry {
        tool: "unica.meta.compile",
        operation: "meta-compile",
        impact: XmlImpactClass::CreateOrModify,
        case_ids: &[
            "meta-compile-catalog",
            "meta-compile-document",
            "meta-compile-enum",
            "meta-compile-constant",
            "meta-compile-information-register",
            "meta-compile-accumulation-register",
            "meta-compile-accounting-register",
            "meta-compile-calculation-register",
            "meta-compile-chart-of-accounts",
            "meta-compile-chart-of-characteristic-types",
            "meta-compile-chart-of-calculation-types",
            "meta-compile-business-process",
            "meta-compile-task",
            "meta-compile-exchange-plan",
            "meta-compile-document-journal",
            "meta-compile-report",
            "meta-compile-data-processor",
            "meta-compile-common-module",
            "meta-compile-scheduled-job",
            "meta-compile-event-subscription",
            "meta-compile-http-service",
            "meta-compile-web-service",
            "meta-compile-defined-type",
        ],
        required_branches: &[
            "Catalog",
            "Document",
            "Enum",
            "Constant",
            "InformationRegister",
            "AccumulationRegister",
            "AccountingRegister",
            "CalculationRegister",
            "ChartOfAccounts",
            "ChartOfCharacteristicTypes",
            "ChartOfCalculationTypes",
            "BusinessProcess",
            "Task",
            "ExchangePlan",
            "DocumentJournal",
            "Report",
            "DataProcessor",
            "CommonModule",
            "ScheduledJob",
            "EventSubscription",
            "HTTPService",
            "WebService",
            "DefinedType",
        ],
    },
    MutatorRegistryEntry {
        tool: "unica.meta.edit",
        operation: "meta-edit",
        impact: XmlImpactClass::CreateOrModify,
        case_ids: &["meta-edit-property"],
        required_branches: &["modify-property"],
    },
    MutatorRegistryEntry {
        tool: "unica.meta.remove",
        operation: "meta-remove",
        impact: XmlImpactClass::RemoveOrModify,
        case_ids: &["meta-remove-object"],
        required_branches: &["remove-object"],
    },
    MutatorRegistryEntry {
        tool: "unica.mxl.compile",
        operation: "mxl-compile",
        impact: XmlImpactClass::CreateOrModify,
        case_ids: &["mxl-compile-standalone"],
        required_branches: &["standalone"],
    },
    MutatorRegistryEntry {
        tool: "unica.role.compile",
        operation: "role-compile",
        impact: XmlImpactClass::CreateOrModify,
        case_ids: &["role-compile-name-field"],
        required_branches: &["name-field"],
    },
    MutatorRegistryEntry {
        tool: "unica.subsystem.compile",
        operation: "subsystem-compile",
        impact: XmlImpactClass::CreateOrModify,
        case_ids: &["subsystem-compile-child"],
        required_branches: &["child-creation"],
    },
    MutatorRegistryEntry {
        tool: "unica.subsystem.edit",
        operation: "subsystem-edit",
        impact: XmlImpactClass::CreateOrModify,
        case_ids: &["subsystem-edit-add-child"],
        required_branches: &["add-child"],
    },
    MutatorRegistryEntry {
        tool: "unica.support.edit",
        operation: "support-edit",
        impact: XmlImpactClass::None,
        case_ids: &["support-edit-bin-only"],
        required_branches: &["parent-configurations-bin-only"],
    },
    MutatorRegistryEntry {
        tool: "unica.template.add",
        operation: "template-add",
        impact: XmlImpactClass::CreateOrModify,
        case_ids: &[
            "template-add-spreadsheet-document",
            "template-add-data-composition-schema",
            "template-add-text-document",
            "template-add-html-document",
            "template-add-binary-data",
        ],
        required_branches: &[
            "SpreadsheetDocument",
            "DataCompositionSchema",
            "TextDocument",
            "HTMLDocument",
            "BinaryData",
        ],
    },
    MutatorRegistryEntry {
        tool: "unica.template.remove",
        operation: "template-remove",
        impact: XmlImpactClass::RemoveOrModify,
        case_ids: &["template-remove-object-template"],
        required_branches: &["remove-object-template"],
    },
];

static EXECUTABLE_CASES: &[ExecutableCase] = &[
    ExecutableCase {
        id: "cf-edit-root-property",
        tool: "unica.cf.edit",
        branch: "root-property",
    },
    ExecutableCase {
        id: "cf-edit-set-panels",
        tool: "unica.cf.edit",
        branch: "set-panels",
    },
    ExecutableCase {
        id: "cf-edit-set-home-page",
        tool: "unica.cf.edit",
        branch: "set-home-page",
    },
    ExecutableCase {
        id: "cf-init-default",
        tool: "unica.cf.init",
        branch: "default",
    },
    ExecutableCase {
        id: "cfe-borrow-object",
        tool: "unica.cfe.borrow",
        branch: "metadata-object",
    },
    ExecutableCase {
        id: "cfe-borrow-managed-form",
        tool: "unica.cfe.borrow",
        branch: "managed-form",
    },
    ExecutableCase {
        id: "cfe-init-default",
        tool: "unica.cfe.init",
        branch: "default",
    },
    ExecutableCase {
        id: "cfe-patch-method-bsl-only",
        tool: "unica.cfe.patch_method",
        branch: "bsl-module-only",
    },
    ExecutableCase {
        id: "code-patch-bsl-only",
        tool: "unica.code.patch",
        branch: "bsl-only",
    },
    ExecutableCase {
        id: "dcs-compile-standalone",
        tool: "unica.dcs.compile",
        branch: "standalone",
    },
    ExecutableCase {
        id: "dcs-edit-owned-template",
        tool: "unica.dcs.edit",
        branch: "owned-template",
    },
    ExecutableCase {
        id: "epf-init-managed-form",
        tool: "unica.epf.init",
        branch: "managed-form",
    },
    ExecutableCase {
        id: "erf-init-managed-form",
        tool: "unica.erf.init",
        branch: "managed-form",
    },
    ExecutableCase {
        id: "form-add-managed",
        tool: "unica.form.add",
        branch: "managed-form",
    },
    ExecutableCase {
        id: "form-compile-managed",
        tool: "unica.form.compile",
        branch: "managed-form",
    },
    ExecutableCase {
        id: "form-edit-managed",
        tool: "unica.form.edit",
        branch: "managed-form",
    },
    ExecutableCase {
        id: "form-remove-managed",
        tool: "unica.form.remove",
        branch: "remove-managed-form",
    },
    ExecutableCase {
        id: "help-add-object",
        tool: "unica.help.add",
        branch: "object-help",
    },
    ExecutableCase {
        id: "interface-edit-subsystem",
        tool: "unica.interface.edit",
        branch: "subsystem-command-interface",
    },
    ExecutableCase {
        id: "meta-compile-catalog",
        tool: "unica.meta.compile",
        branch: "Catalog",
    },
    ExecutableCase {
        id: "meta-compile-document",
        tool: "unica.meta.compile",
        branch: "Document",
    },
    ExecutableCase {
        id: "meta-compile-enum",
        tool: "unica.meta.compile",
        branch: "Enum",
    },
    ExecutableCase {
        id: "meta-compile-constant",
        tool: "unica.meta.compile",
        branch: "Constant",
    },
    ExecutableCase {
        id: "meta-compile-information-register",
        tool: "unica.meta.compile",
        branch: "InformationRegister",
    },
    ExecutableCase {
        id: "meta-compile-accumulation-register",
        tool: "unica.meta.compile",
        branch: "AccumulationRegister",
    },
    ExecutableCase {
        id: "meta-compile-accounting-register",
        tool: "unica.meta.compile",
        branch: "AccountingRegister",
    },
    ExecutableCase {
        id: "meta-compile-calculation-register",
        tool: "unica.meta.compile",
        branch: "CalculationRegister",
    },
    ExecutableCase {
        id: "meta-compile-chart-of-accounts",
        tool: "unica.meta.compile",
        branch: "ChartOfAccounts",
    },
    ExecutableCase {
        id: "meta-compile-chart-of-characteristic-types",
        tool: "unica.meta.compile",
        branch: "ChartOfCharacteristicTypes",
    },
    ExecutableCase {
        id: "meta-compile-chart-of-calculation-types",
        tool: "unica.meta.compile",
        branch: "ChartOfCalculationTypes",
    },
    ExecutableCase {
        id: "meta-compile-business-process",
        tool: "unica.meta.compile",
        branch: "BusinessProcess",
    },
    ExecutableCase {
        id: "meta-compile-task",
        tool: "unica.meta.compile",
        branch: "Task",
    },
    ExecutableCase {
        id: "meta-compile-exchange-plan",
        tool: "unica.meta.compile",
        branch: "ExchangePlan",
    },
    ExecutableCase {
        id: "meta-compile-document-journal",
        tool: "unica.meta.compile",
        branch: "DocumentJournal",
    },
    ExecutableCase {
        id: "meta-compile-report",
        tool: "unica.meta.compile",
        branch: "Report",
    },
    ExecutableCase {
        id: "meta-compile-data-processor",
        tool: "unica.meta.compile",
        branch: "DataProcessor",
    },
    ExecutableCase {
        id: "meta-compile-common-module",
        tool: "unica.meta.compile",
        branch: "CommonModule",
    },
    ExecutableCase {
        id: "meta-compile-scheduled-job",
        tool: "unica.meta.compile",
        branch: "ScheduledJob",
    },
    ExecutableCase {
        id: "meta-compile-event-subscription",
        tool: "unica.meta.compile",
        branch: "EventSubscription",
    },
    ExecutableCase {
        id: "meta-compile-http-service",
        tool: "unica.meta.compile",
        branch: "HTTPService",
    },
    ExecutableCase {
        id: "meta-compile-web-service",
        tool: "unica.meta.compile",
        branch: "WebService",
    },
    ExecutableCase {
        id: "meta-compile-defined-type",
        tool: "unica.meta.compile",
        branch: "DefinedType",
    },
    ExecutableCase {
        id: "meta-edit-property",
        tool: "unica.meta.edit",
        branch: "modify-property",
    },
    ExecutableCase {
        id: "meta-remove-object",
        tool: "unica.meta.remove",
        branch: "remove-object",
    },
    ExecutableCase {
        id: "mxl-compile-standalone",
        tool: "unica.mxl.compile",
        branch: "standalone",
    },
    ExecutableCase {
        id: "role-compile-name-field",
        tool: "unica.role.compile",
        branch: "name-field",
    },
    ExecutableCase {
        id: "subsystem-compile-child",
        tool: "unica.subsystem.compile",
        branch: "child-creation",
    },
    ExecutableCase {
        id: "subsystem-edit-add-child",
        tool: "unica.subsystem.edit",
        branch: "add-child",
    },
    ExecutableCase {
        id: "support-edit-bin-only",
        tool: "unica.support.edit",
        branch: "parent-configurations-bin-only",
    },
    ExecutableCase {
        id: "template-add-spreadsheet-document",
        tool: "unica.template.add",
        branch: "SpreadsheetDocument",
    },
    ExecutableCase {
        id: "template-add-data-composition-schema",
        tool: "unica.template.add",
        branch: "DataCompositionSchema",
    },
    ExecutableCase {
        id: "template-add-text-document",
        tool: "unica.template.add",
        branch: "TextDocument",
    },
    ExecutableCase {
        id: "template-add-html-document",
        tool: "unica.template.add",
        branch: "HTMLDocument",
    },
    ExecutableCase {
        id: "template-add-binary-data",
        tool: "unica.template.add",
        branch: "BinaryData",
    },
    ExecutableCase {
        id: "template-remove-object-template",
        tool: "unica.template.remove",
        branch: "remove-object-template",
    },
];

type XmlSnapshot = BTreeMap<String, String>;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
struct XmlDelta {
    created: Vec<String>,
    modified: Vec<String>,
    removed: Vec<String>,
    unchanged: Vec<String>,
}

fn classify_xml_delta(before: &XmlSnapshot, after: &XmlSnapshot) -> XmlDelta {
    let mut delta = XmlDelta::default();
    for (path, before_hash) in before {
        match after.get(path) {
            None => delta.removed.push(path.clone()),
            Some(after_hash) if after_hash != before_hash => delta.modified.push(path.clone()),
            Some(_) => delta.unchanged.push(path.clone()),
        }
    }
    for path in after.keys() {
        if !before.contains_key(path) {
            delta.created.push(path.clone());
        }
    }
    delta
}

fn enforce_xml_impact(
    impact: XmlImpactClass,
    before: &XmlSnapshot,
    after: &XmlSnapshot,
) -> Result<XmlDelta, String> {
    let delta = classify_xml_delta(before, after);
    match impact {
        XmlImpactClass::None if before != after => {
            Err("XML impact None changed the complete XML map".to_string())
        }
        XmlImpactClass::CreateOrModify if delta.created.is_empty() && delta.modified.is_empty() => {
            Err("CreateOrModify case produced no XML create/modify delta".to_string())
        }
        XmlImpactClass::RemoveOrModify if delta.removed.is_empty() || delta.modified.is_empty() => {
            Err("RemoveOrModify case must remove XML and modify a surviving owner".to_string())
        }
        _ => Ok(delta),
    }
}

#[derive(Debug, Default)]
struct SequentialCallGate {
    target_call_active: bool,
    completed_target_calls: usize,
}

impl SequentialCallGate {
    fn begin(&mut self) -> Result<(), String> {
        if self.target_call_active {
            return Err("overlapping target calls are forbidden".to_string());
        }
        self.target_call_active = true;
        Ok(())
    }

    fn finish(&mut self) -> Result<(), String> {
        if !self.target_call_active {
            return Err("target call was not active".to_string());
        }
        self.target_call_active = false;
        self.completed_target_calls += 1;
        Ok(())
    }
}

fn common_args(workspace: &Path) -> Map<String, Value> {
    Map::from_iter([
        (
            "cwd".to_string(),
            Value::String(workspace.display().to_string()),
        ),
        ("dryRun".to_string(), Value::Bool(false)),
    ])
}

fn call_public_tool(tool: &str, args: &Map<String, Value>) -> Result<String, String> {
    assert_eq!(args.get("dryRun"), Some(&Value::Bool(false)));
    let app = UnicaApplication::new();
    let result = app.call_tool(tool, args)?;
    if !result.ok || !result.errors.is_empty() {
        return Err(format!(
            "{tool} failed: {}; errors={:?}",
            result.summary, result.errors
        ));
    }
    Ok(result.summary)
}

fn call_target_tool(
    gate: &mut SequentialCallGate,
    tool: &str,
    args: &Map<String, Value>,
) -> Result<String, String> {
    gate.begin()?;
    let result = call_public_tool(tool, args);
    gate.finish()?;
    result
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn visit_xml_files(
    root: &Path,
    directory: &Path,
    snapshot: &mut XmlSnapshot,
) -> Result<(), String> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| format!("cannot read {}: {error}", directory.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("cannot enumerate {}: {error}", directory.display()))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
        if file_type.is_symlink() {
            return Err(format!(
                "workspace symlink is forbidden: {}",
                path.display()
            ));
        }
        if file_type.is_dir() {
            visit_xml_files(root, &path, snapshot)?;
        } else if file_type.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("xml"))
        {
            let relative = path
                .strip_prefix(root)
                .map_err(|error| format!("XML path escaped workspace: {error}"))?
                .to_string_lossy()
                .replace('\\', "/");
            snapshot.insert(relative, sha256_file(&path)?);
        }
    }
    Ok(())
}

fn snapshot_xml(workspace: &Path) -> Result<XmlSnapshot, String> {
    let mut snapshot = XmlSnapshot::new();
    visit_xml_files(workspace, workspace, &mut snapshot)?;
    Ok(snapshot)
}

fn registry_entry_for_case(case_id: &str) -> &'static MutatorRegistryEntry {
    MUTATOR_REGISTRY
        .iter()
        .find(|entry| entry.case_ids.contains(&case_id))
        .unwrap_or_else(|| panic!("case is absent from registry: {case_id}"))
}

fn cf_init_args(workspace: &Path, name: &str, output_dir: &str) -> Map<String, Value> {
    let mut args = common_args(workspace);
    args.insert("Name".to_string(), Value::String(name.to_string()));
    args.insert(
        "OutputDir".to_string(),
        Value::String(output_dir.to_string()),
    );
    args
}

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "unica-xml-corpus-{label}-{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

fn write_designer_project(
    workspace: &Path,
    source_sets: &[(&str, &str, &str)],
) -> Result<(), String> {
    let mut text = "format: DESIGNER\nsource-set:\n".to_string();
    for (name, kind, path) in source_sets {
        text.push_str(&format!(
            "  - name: {name}\n    type: {kind}\n    path: {path}\n"
        ));
    }
    fs::write(workspace.join("v8project.yaml"), text)
        .map_err(|error| format!("cannot write v8project.yaml: {error}"))
}

fn seed_configuration(workspace: &Path) -> Result<(), String> {
    call_public_tool(
        "unica.cf.init",
        &cf_init_args(workspace, "CorpusConfiguration", "src"),
    )?;
    write_designer_project(workspace, &[("main", "CONFIGURATION", "src")])
}

fn write_json_input(workspace: &Path, name: &str, value: &Value) -> Result<String, String> {
    let relative = format!("inputs/{name}.json");
    let path = workspace.join(&relative);
    fs::create_dir_all(path.parent().expect("input parent"))
        .map_err(|error| format!("cannot create inputs: {error}"))?;
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("cannot serialize input {name}: {error}"))?;
    fs::write(&path, bytes).map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    Ok(relative)
}

fn meta_compile_args(workspace: &Path, json_path: &str) -> Map<String, Value> {
    let mut args = common_args(workspace);
    args.insert("JsonPath".to_string(), Value::String(json_path.to_string()));
    args.insert("OutputDir".to_string(), Value::String("src".to_string()));
    args
}

fn seed_metadata(workspace: &Path, input_name: &str, definition: Value) -> Result<(), String> {
    let path = write_json_input(workspace, input_name, &definition)?;
    call_public_tool("unica.meta.compile", &meta_compile_args(workspace, &path))?;
    Ok(())
}

fn catalog_definition(name: &str) -> Value {
    json!({
        "type": "Catalog",
        "name": name,
        "synonym": "Corpus catalog",
        "codeLength": 9,
        "descriptionLength": 50,
        "attributes": [{"name": "Article", "type": "String(32)"}]
    })
}

fn seed_catalog(workspace: &Path) -> Result<(), String> {
    seed_metadata(
        workspace,
        "seed-catalog",
        catalog_definition("CorpusCatalog"),
    )
}

fn form_add_args(workspace: &Path) -> Map<String, Value> {
    let mut args = common_args(workspace);
    args.insert(
        "ObjectPath".to_string(),
        Value::String("src/Catalogs/CorpusCatalog.xml".to_string()),
    );
    args.insert(
        "FormName".to_string(),
        Value::String("CorpusForm".to_string()),
    );
    args.insert("Purpose".to_string(), Value::String("Object".to_string()));
    args.insert("SetDefault".to_string(), Value::Bool(true));
    args
}

fn seed_catalog_form(workspace: &Path) -> Result<(), String> {
    seed_catalog(workspace)?;
    call_public_tool("unica.form.add", &form_add_args(workspace))?;
    Ok(())
}

fn report_definition(name: &str) -> Value {
    json!({"type": "Report", "name": name, "synonym": "Corpus report"})
}

fn seed_report(workspace: &Path) -> Result<(), String> {
    seed_metadata(workspace, "seed-report", report_definition("CorpusReport"))
}

fn template_args(workspace: &Path, template_name: &str, template_type: &str) -> Map<String, Value> {
    let mut args = common_args(workspace);
    args.insert(
        "ObjectName".to_string(),
        Value::String("CorpusReport".to_string()),
    );
    args.insert(
        "TemplateName".to_string(),
        Value::String(template_name.to_string()),
    );
    args.insert(
        "TemplateType".to_string(),
        Value::String(template_type.to_string()),
    );
    args.insert(
        "SrcDir".to_string(),
        Value::String("src/Reports".to_string()),
    );
    args
}

fn seed_dcs_template(workspace: &Path) -> Result<(), String> {
    call_public_tool(
        "unica.template.add",
        &template_args(workspace, "CorpusDcs", "DataCompositionSchema"),
    )?;
    let mut compile = common_args(workspace);
    compile.insert(
        "Value".to_string(),
        Value::String(dcs_definition().to_string()),
    );
    compile.insert(
        "OutputPath".to_string(),
        Value::String("src/Reports/CorpusReport/Templates/CorpusDcs/Ext/Template.xml".to_string()),
    );
    call_public_tool("unica.dcs.compile", &compile)?;
    Ok(())
}

fn subsystem_compile_args(
    workspace: &Path,
    name: &str,
    parent: Option<&str>,
) -> Map<String, Value> {
    let mut args = common_args(workspace);
    args.insert(
        "Value".to_string(),
        Value::String(json!({"name": name}).to_string()),
    );
    args.insert("OutputDir".to_string(), Value::String("src".to_string()));
    if let Some(parent) = parent {
        args.insert("Parent".to_string(), Value::String(parent.to_string()));
    }
    args
}

fn seed_parent_subsystem(workspace: &Path) -> Result<(), String> {
    call_public_tool(
        "unica.subsystem.compile",
        &subsystem_compile_args(workspace, "CorpusParent", None),
    )?;
    Ok(())
}

fn cfe_init_args(workspace: &Path) -> Map<String, Value> {
    let mut args = common_args(workspace);
    args.insert(
        "Name".to_string(),
        Value::String("CorpusExtension".to_string()),
    );
    args.insert("OutputDir".to_string(), Value::String("ext".to_string()));
    args.insert(
        "ConfigPath".to_string(),
        Value::String("src/Configuration.xml".to_string()),
    );
    args
}

fn seed_extension(workspace: &Path) -> Result<(), String> {
    write_designer_project(
        workspace,
        &[
            ("main", "CONFIGURATION", "src"),
            ("extension", "EXTENSION", "ext"),
        ],
    )?;
    call_public_tool("unica.cfe.init", &cfe_init_args(workspace))?;
    Ok(())
}

fn dcs_definition() -> Value {
    json!({
        "dataSets": [{
            "name": "Main",
            "query": "SELECT 1 AS Value",
            "fields": ["Value:String"]
        }]
    })
}

fn meta_definition(kind: &str) -> Option<Value> {
    Some(match kind {
        "Catalog" => catalog_definition("CorpusCatalog"),
        "Document" => json!({
            "type": "Document", "name": "CorpusDocument", "numberLength": 8,
            "attributes": ["Partner:CatalogRef.Partners|req,index"],
            "tabularSections": {"Lines": ["Quantity:Number(10,2)"]}
        }),
        "Enum" => json!({"type": "Enum", "name": "CorpusEnum", "values": ["New", "Closed"]}),
        "Constant" => json!({"type": "Constant", "name": "CorpusConstant", "valueType": "Boolean"}),
        "InformationRegister" => json!({
            "type": "InformationRegister", "name": "CorpusInformationRegister", "periodicity": "Month",
            "dimensions": ["Item:CatalogRef.Items|master,index"], "resources": ["Price:Number(15,2)"]
        }),
        "AccumulationRegister" => json!({
            "type": "AccumulationRegister", "name": "CorpusAccumulationRegister", "registerType": "Balances",
            "dimensions": ["Warehouse:CatalogRef.Warehouses|index"], "resources": ["Quantity:Number(15,3)"]
        }),
        "AccountingRegister" => json!({
            "type": "AccountingRegister", "name": "CorpusAccountingRegister",
            "chartOfAccounts": "ChartOfAccounts.CorpusAccounts",
            "dimensions": ["Department:CatalogRef.Departments"], "resources": ["Amount:Number(15,2)"]
        }),
        "CalculationRegister" => json!({
            "type": "CalculationRegister", "name": "CorpusCalculationRegister",
            "chartOfCalculationTypes": "ChartOfCalculationTypes.CorpusCalculationTypes", "periodicity": "Month",
            "dimensions": ["Employee:CatalogRef.Employees"], "resources": ["Result:Number(15,2)"]
        }),
        "ChartOfAccounts" => json!({
            "type": "ChartOfAccounts", "name": "CorpusAccounts",
            "extDimensionTypes": "ChartOfCharacteristicTypes.CorpusCharacteristics",
            "accountingFlags": ["Tax"], "extDimensionAccountingFlags": ["Department"]
        }),
        "ChartOfCharacteristicTypes" => json!({
            "type": "ChartOfCharacteristicTypes", "name": "CorpusCharacteristics",
            "valueTypes": ["String(50)", "Number(15,2)"]
        }),
        "ChartOfCalculationTypes" => json!({
            "type": "ChartOfCalculationTypes", "name": "CorpusCalculationTypes",
            "dependenceOnCalculationTypes": "OnActionPeriod",
            "baseCalculationTypes": ["ChartOfCalculationTypes.BaseSalary"]
        }),
        "BusinessProcess" => json!({
            "type": "BusinessProcess", "name": "CorpusBusinessProcess", "task": "Task.CorpusTask",
            "attributes": ["Subject:String(100)"]
        }),
        "Task" => json!({
            "type": "Task", "name": "CorpusTask", "addressing": "CatalogRef.Users",
            "mainAddressingAttribute": "Performer",
            "addressingAttributes": [{"name": "Performer", "type": "CatalogRef.Users", "addressingDimension": "Catalog.Users"}],
            "attributes": ["Priority:Number(3,0)"]
        }),
        "ExchangePlan" => json!({
            "type": "ExchangePlan", "name": "CorpusExchangePlan", "distributedInfoBase": true,
            "includeConfigurationExtensions": true, "attributes": ["NodeKind:String(20)"]
        }),
        "DocumentJournal" => json!({
            "type": "DocumentJournal", "name": "CorpusDocumentJournal",
            "registeredDocuments": ["Document.CorpusDocument"],
            "columns": [{"name": "Partner", "references": ["Document.CorpusDocument"]}]
        }),
        "Report" => report_definition("CorpusReport"),
        "DataProcessor" => json!({
            "type": "DataProcessor", "name": "CorpusDataProcessor", "attributes": ["FileName:String(260)"],
            "tabularSections": {"Rows": ["Value:String(100)"]}
        }),
        "CommonModule" => json!({
            "type": "CommonModule", "name": "CorpusCommonModule", "context": "server",
            "returnValuesReuse": "DuringRequest"
        }),
        "ScheduledJob" => json!({
            "type": "ScheduledJob", "name": "CorpusScheduledJob", "methodName": "EventHandlers.RunJob",
            "description": "Corpus job", "key": "corpus", "use": true, "predefined": true
        }),
        "EventSubscription" => json!({
            "type": "EventSubscription", "name": "CorpusEventSubscription",
            "source": ["DocumentObject.CorpusDocument"], "event": "BeforeWrite",
            "handler": "EventHandlers.OnBeforeWrite"
        }),
        "HTTPService" => json!({
            "type": "HTTPService", "name": "CorpusHTTPService", "rootURL": "corpus", "reuseSessions": "AutoUse",
            "urlTemplates": {"Items": {"template": "/items/{id}", "methods": {"Get": "GET"}}}
        }),
        "WebService" => json!({
            "type": "WebService", "name": "CorpusWebService", "namespace": "urn:corpus", "reuseSessions": "AutoUse",
            "operations": {"Ping": {"returnType": "xs:string", "parameters": {"Text": "xs:string"}}}
        }),
        "DefinedType" => json!({
            "type": "DefinedType", "name": "CorpusDefinedType", "valueTypes": ["String(100)", "CatalogRef.Products"]
        }),
        _ => return None,
    })
}

fn prepare_target(case: &ExecutableCase, workspace: &Path) -> Result<Map<String, Value>, String> {
    fs::create_dir_all(workspace)
        .map_err(|error| format!("cannot create {}: {error}", workspace.display()))?;

    if case.id == "cf-init-default" {
        return Ok(cf_init_args(workspace, "CorpusConfiguration", "src"));
    }
    if matches!(case.id, "epf-init-managed-form" | "erf-init-managed-form") {
        let (kind, path, name, form) = if case.id.starts_with("epf") {
            (
                "EXTERNAL_DATA_PROCESSORS",
                "epf",
                "CorpusProcessor",
                "CorpusForm",
            )
        } else {
            ("EXTERNAL_REPORTS", "erf", "CorpusReport", "CorpusForm")
        };
        write_designer_project(workspace, &[("external", kind, path)])?;
        let mut args = common_args(workspace);
        args.insert("Name".to_string(), Value::String(name.to_string()));
        args.insert("OutputDir".to_string(), Value::String(path.to_string()));
        args.insert("FormName".to_string(), Value::String(form.to_string()));
        return Ok(args);
    }
    if case.id == "dcs-compile-standalone" {
        seed_configuration(workspace)?;
        seed_report(workspace)?;
        call_public_tool(
            "unica.template.add",
            &template_args(workspace, "CorpusTemplate", "DataCompositionSchema"),
        )?;
        let output = "src/Reports/CorpusReport/Templates/CorpusTemplate/Ext/Template.xml";
        fs::remove_file(workspace.join(output))
            .map_err(|error| format!("cannot remove preparatory DCS content: {error}"))?;
        let mut args = common_args(workspace);
        args.insert(
            "Value".to_string(),
            Value::String(dcs_definition().to_string()),
        );
        args.insert("OutputPath".to_string(), Value::String(output.to_string()));
        return Ok(args);
    }
    if case.id == "mxl-compile-standalone" {
        seed_configuration(workspace)?;
        seed_report(workspace)?;
        call_public_tool(
            "unica.template.add",
            &template_args(workspace, "CorpusTemplate", "SpreadsheetDocument"),
        )?;
        let output = "src/Reports/CorpusReport/Templates/CorpusTemplate/Ext/Template.xml";
        fs::remove_file(workspace.join(output))
            .map_err(|error| format!("cannot remove preparatory MXL content: {error}"))?;
        let path = write_json_input(
            workspace,
            "mxl",
            &json!({"columns": 1, "areas": [{"name": "A", "rows": [{"cells": [{"col": 1, "text": "x"}]}]}]}),
        )?;
        let mut args = common_args(workspace);
        args.insert("JsonPath".to_string(), Value::String(path));
        args.insert("OutputPath".to_string(), Value::String(output.to_string()));
        return Ok(args);
    }

    seed_configuration(workspace)?;

    if case.id.starts_with("cf-edit-") {
        let (operation, value) = match case.id {
            "cf-edit-root-property" => ("modify-property", "Version=1.0".to_string()),
            "cf-edit-set-panels" => ("set-panels", json!({"top": ["open"]}).to_string()),
            "cf-edit-set-home-page" => (
                "set-home-page",
                json!({"template": "OneColumn", "left": ["CommonForm.Demo"]}).to_string(),
            ),
            _ => unreachable!(),
        };
        let mut args = common_args(workspace);
        args.insert("ConfigPath".to_string(), Value::String("src".to_string()));
        args.insert(
            "Operation".to_string(),
            Value::String(operation.to_string()),
        );
        args.insert("Value".to_string(), Value::String(value));
        args.insert("NoValidate".to_string(), Value::Bool(true));
        return Ok(args);
    }

    if case.id == "cfe-init-default" {
        write_designer_project(
            workspace,
            &[
                ("main", "CONFIGURATION", "src"),
                ("extension", "EXTENSION", "ext"),
            ],
        )?;
        return Ok(cfe_init_args(workspace));
    }

    if case.id.starts_with("cfe-borrow-") {
        seed_catalog(workspace)?;
        if case.id.ends_with("managed-form") {
            call_public_tool("unica.form.add", &form_add_args(workspace))?;
        }
        seed_extension(workspace)?;
        let mut args = common_args(workspace);
        args.insert(
            "ExtensionPath".to_string(),
            Value::String("ext/Configuration.xml".to_string()),
        );
        args.insert(
            "ConfigPath".to_string(),
            Value::String("src/Configuration.xml".to_string()),
        );
        let object = if case.id.ends_with("managed-form") {
            "Catalog.CorpusCatalog.Form.CorpusForm"
        } else {
            "Catalog.CorpusCatalog"
        };
        args.insert("Object".to_string(), Value::String(object.to_string()));
        return Ok(args);
    }

    if case.id == "cfe-patch-method-bsl-only" {
        seed_extension(workspace)?;
        let mut args = common_args(workspace);
        args.insert(
            "ExtensionPath".to_string(),
            Value::String("ext".to_string()),
        );
        args.insert(
            "ModulePath".to_string(),
            Value::String("CommonModule.CorpusModule".to_string()),
        );
        args.insert("MethodName".to_string(), Value::String("Run".to_string()));
        args.insert(
            "InterceptorType".to_string(),
            Value::String("Before".to_string()),
        );
        return Ok(args);
    }

    if case.id == "code-patch-bsl-only" {
        seed_metadata(
            workspace,
            "code-module",
            json!({"type": "CommonModule", "name": "CorpusModule", "context": "server"}),
        )?;
        fs::write(
            workspace.join("src/CommonModules/CorpusModule/Ext/Module.bsl"),
            "Procedure Run()\n    Message(\"ok\");\nEndProcedure\n",
        )
        .map_err(|error| format!("cannot seed BSL module: {error}"))?;
        let mut args = common_args(workspace);
        args.insert("sourceDir".to_string(), Value::String("src".to_string()));
        args.insert(
            "path".to_string(),
            Value::String("src/CommonModules/CorpusModule/Ext/Module.bsl".to_string()),
        );
        args.insert("operation".to_string(), Value::String("insert".to_string()));
        args.insert("selector".to_string(), json!({"method": "Run"}));
        args.insert(
            "content".to_string(),
            Value::String("Procedure Added()\nEndProcedure".to_string()),
        );
        args.insert("position".to_string(), Value::String("after".to_string()));
        return Ok(args);
    }

    if case.id == "support-edit-bin-only" {
        let configuration = fs::read_to_string(workspace.join("src/Configuration.xml"))
            .map_err(|error| format!("cannot read seeded Configuration.xml: {error}"))?;
        let document = Document::parse(configuration.trim_start_matches('\u{feff}'))
            .map_err(|error| format!("cannot parse seeded Configuration.xml: {error}"))?;
        let uuid = document
            .descendants()
            .find(|node| node.has_tag_name("Configuration"))
            .and_then(|node| node.attribute("uuid"))
            .ok_or_else(|| "seeded Configuration has no uuid".to_string())?;
        let bin = format!(
            "\u{feff}{{6,1,1,dddddddd-dddd-dddd-dddd-dddddddddddd,0,eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee,\"1.0\",\"Vendor\",\"VendorConf\",3,1,0,{uuid},{uuid},0,0,bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb,bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb,2,0,cccccccc-cccc-cccc-cccc-cccccccccccc,cccccccc-cccc-cccc-cccc-cccccccccccc}}"
        );
        fs::write(workspace.join("src/Ext/ParentConfigurations.bin"), bin)
            .map_err(|error| format!("cannot seed ParentConfigurations.bin: {error}"))?;
        let mut args = common_args(workspace);
        args.insert("Path".to_string(), Value::String("src".to_string()));
        args.insert("Capability".to_string(), Value::String("off".to_string()));
        return Ok(args);
    }

    if case.id.starts_with("meta-compile-") {
        let definition = meta_definition(case.branch)
            .ok_or_else(|| format!("missing metadata definition for {}", case.branch))?;
        let path = write_json_input(workspace, case.id, &definition)?;
        return Ok(meta_compile_args(workspace, &path));
    }

    if matches!(case.id, "meta-edit-property" | "meta-remove-object") {
        seed_catalog(workspace)?;
        let mut args = common_args(workspace);
        if case.id == "meta-edit-property" {
            args.insert(
                "ObjectPath".to_string(),
                Value::String("src/Catalogs/CorpusCatalog.xml".to_string()),
            );
            args.insert(
                "Operation".to_string(),
                Value::String("modify-property".to_string()),
            );
            args.insert(
                "Value".to_string(),
                Value::String("Comment=Corpus edited".to_string()),
            );
        } else {
            args.insert("ConfigDir".to_string(), Value::String("src".to_string()));
            args.insert(
                "Object".to_string(),
                Value::String("Catalog.CorpusCatalog".to_string()),
            );
        }
        return Ok(args);
    }

    if case.id == "help-add-object" {
        seed_catalog(workspace)?;
        let mut args = common_args(workspace);
        args.insert(
            "ObjectName".to_string(),
            Value::String("Catalogs/CorpusCatalog".to_string()),
        );
        args.insert("SrcDir".to_string(), Value::String("src".to_string()));
        args.insert("Lang".to_string(), Value::String("ru".to_string()));
        return Ok(args);
    }

    if case.id == "form-add-managed" {
        seed_catalog(workspace)?;
        return Ok(form_add_args(workspace));
    }

    if matches!(
        case.id,
        "form-compile-managed" | "form-edit-managed" | "form-remove-managed"
    ) {
        seed_catalog_form(workspace)?;
        let mut args = common_args(workspace);
        if case.id == "form-compile-managed" {
            let path = write_json_input(
                workspace,
                "form-compile",
                &json!({
                    "title": "Corpus form",
                    "elements": [{"input": "Description", "path": "Object.Description"}]
                }),
            )?;
            args.insert("JsonPath".to_string(), Value::String(path));
            args.insert(
                "OutputPath".to_string(),
                Value::String(
                    "src/Catalogs/CorpusCatalog/Forms/CorpusForm/Ext/Form.xml".to_string(),
                ),
            );
        } else if case.id == "form-edit-managed" {
            args.insert(
                "FormPath".to_string(),
                Value::String(
                    "src/Catalogs/CorpusCatalog/Forms/CorpusForm/Ext/Form.xml".to_string(),
                ),
            );
            args.insert(
                "definition".to_string(),
                json!({"attributes": [{"name": "CorpusAdded", "type": "string"}]}),
            );
        } else {
            args.insert(
                "ObjectName".to_string(),
                Value::String("CorpusCatalog".to_string()),
            );
            args.insert(
                "FormName".to_string(),
                Value::String("CorpusForm".to_string()),
            );
            args.insert(
                "SrcDir".to_string(),
                Value::String("src/Catalogs".to_string()),
            );
        }
        return Ok(args);
    }

    if case.id == "interface-edit-subsystem" {
        seed_parent_subsystem(workspace)?;
        let mut args = common_args(workspace);
        args.insert(
            "CIPath".to_string(),
            Value::String("src/Subsystems/CorpusParent/Ext/CommandInterface.xml".to_string()),
        );
        args.insert("Operation".to_string(), Value::String("hide".to_string()));
        args.insert(
            "Value".to_string(),
            Value::String("Catalog.CorpusCatalog.StandardCommand.OpenList".to_string()),
        );
        args.insert("CreateIfMissing".to_string(), Value::Bool(true));
        return Ok(args);
    }

    if case.id == "subsystem-compile-child" {
        seed_parent_subsystem(workspace)?;
        return Ok(subsystem_compile_args(
            workspace,
            "CorpusChild",
            Some("src/Subsystems/CorpusParent.xml"),
        ));
    }

    if case.id == "subsystem-edit-add-child" {
        seed_parent_subsystem(workspace)?;
        let mut args = common_args(workspace);
        args.insert(
            "SubsystemPath".to_string(),
            Value::String("src/Subsystems/CorpusParent.xml".to_string()),
        );
        args.insert(
            "Operation".to_string(),
            Value::String("add-child".to_string()),
        );
        args.insert(
            "Value".to_string(),
            Value::String("CorpusEditedChild".to_string()),
        );
        return Ok(args);
    }

    if case.id.starts_with("template-add-") {
        seed_report(workspace)?;
        let template_type = match case.branch {
            "SpreadsheetDocument" => "SpreadsheetDocument",
            "DataCompositionSchema" => "DataCompositionSchema",
            "TextDocument" => "Text",
            "HTMLDocument" => "HTML",
            "BinaryData" => "BinaryData",
            other => return Err(format!("unknown template branch: {other}")),
        };
        return Ok(template_args(workspace, "CorpusTemplate", template_type));
    }

    if case.id == "template-remove-object-template" {
        seed_report(workspace)?;
        call_public_tool(
            "unica.template.add",
            &template_args(workspace, "CorpusTemplate", "SpreadsheetDocument"),
        )?;
        let mut args = common_args(workspace);
        args.insert(
            "ObjectName".to_string(),
            Value::String("CorpusReport".to_string()),
        );
        args.insert(
            "TemplateName".to_string(),
            Value::String("CorpusTemplate".to_string()),
        );
        args.insert(
            "SrcDir".to_string(),
            Value::String("src/Reports".to_string()),
        );
        return Ok(args);
    }

    if case.id == "dcs-edit-owned-template" {
        seed_report(workspace)?;
        seed_dcs_template(workspace)?;
        let mut args = common_args(workspace);
        args.insert(
            "TemplatePath".to_string(),
            Value::String(
                "src/Reports/CorpusReport/Templates/CorpusDcs/Ext/Template.xml".to_string(),
            ),
        );
        args.insert(
            "Operation".to_string(),
            Value::String("add-field".to_string()),
        );
        args.insert(
            "Value".to_string(),
            Value::String("Added:String".to_string()),
        );
        args.insert("DataSet".to_string(), Value::String("Main".to_string()));
        return Ok(args);
    }

    if case.id == "role-compile-name-field" {
        seed_catalog(workspace)?;
        let path = write_json_input(
            workspace,
            "role",
            &json!({
                "name": "CorpusReader",
                "synonym": "Corpus reader",
                "objects": ["Catalog.CorpusCatalog: @view"]
            }),
        )?;
        let mut args = common_args(workspace);
        args.insert("JsonPath".to_string(), Value::String(path));
        args.insert("OutputDir".to_string(), Value::String("src".to_string()));
        return Ok(args);
    }

    Err(format!("no executable preparation for case {}", case.id))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CorpusManifest {
    schema_version: u32,
    profile: String,
    cases: Vec<CorpusCase>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CorpusCase {
    id: String,
    workspace_path: String,
    platform_checkpoint: PlatformCheckpoint,
    checkpoint: String,
    tool_id: String,
    operation: String,
    branch: String,
    impact_class: String,
    xml_impact: String,
    files: Vec<CorpusFile>,
    removed_paths: Vec<String>,
    owner_versions: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlatformCheckpoint {
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    base_source_path: Option<String>,
    covered_case_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CorpusFile {
    path: String,
    sha256: String,
    family: String,
    seed: bool,
    delta: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    owner_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    new_standalone: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CaseReport {
    schema_version: u32,
    profile: String,
    id: String,
    workspace_path: String,
    platform_checkpoint: PlatformCheckpoint,
    tool_id: String,
    operation: String,
    branch: String,
    impact_class: String,
    public_arguments: Value,
    target_call: TargetCallReport,
    seed_outputs: Vec<String>,
    pre_xml_sha256: XmlSnapshot,
    post_xml_sha256: XmlSnapshot,
    delta: XmlDelta,
    remaining_xml: Vec<String>,
    removed_paths: Vec<RemovedPathReport>,
    owner_links: BTreeMap<String, String>,
    owner_versions: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TargetCallReport {
    sequence: usize,
    result_ok: bool,
    errors: Vec<String>,
    summary: String,
}

#[derive(Debug, Serialize)]
struct RemovedPathReport {
    path: String,
    sha256: String,
}

fn impact_class_name(impact: XmlImpactClass) -> &'static str {
    match impact {
        XmlImpactClass::None => "None",
        XmlImpactClass::CreateOrModify => "CreateOrModify",
        XmlImpactClass::RemoveOrModify => "RemoveOrModify",
    }
}

fn xml_impact_name(delta: &XmlDelta) -> &'static str {
    if !delta.removed.is_empty() {
        "removed"
    } else if !delta.created.is_empty() {
        "created"
    } else if !delta.modified.is_empty() {
        "modified"
    } else {
        "unchanged"
    }
}

fn xml_root_details(path: &Path) -> Result<(String, String, Option<String>), String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("cannot read XML {}: {error}", path.display()))?;
    let document = Document::parse(text.trim_start_matches('\u{feff}'))
        .map_err(|error| format!("cannot parse XML {}: {error}", path.display()))?;
    let root = document.root_element();
    let namespace = root.tag_name().namespace().unwrap_or("").to_string();
    let local_name = root.tag_name().name().to_string();
    let child_type = root
        .children()
        .find(|child| child.is_element())
        .map(|child| child.tag_name().name().to_string());
    Ok((namespace, local_name, child_type))
}

fn family_for_xml(path: &Path) -> Result<String, String> {
    let (namespace, local_name, _) = xml_root_details(path)?;
    let family = match (namespace.as_str(), local_name.as_str()) {
        ("http://v8.1c.ru/8.3/MDClasses", "MetaDataObject") => "metadata",
        ("http://v8.1c.ru/8.3/MDClasses", "Flowchart") => "flowchart",
        ("http://v8.1c.ru/8.3/xcf/logform", "Form") => "managed-form",
        ("http://v8.1c.ru/8.3/xcf/extrnprops", "CommandInterface") => "command-interface",
        ("http://v8.1c.ru/8.3/xcf/extrnprops", "Help") => "help",
        ("http://v8.1c.ru/8.3/xcf/extrnprops", "ExchangePlanContent") => "exchange-plan-content",
        ("http://v8.1c.ru/8.3/xcf/extrnprops", "HomePageWorkArea") => "home-page-work-area",
        ("http://v8.1c.ru/8.1/data-composition-system/schema", "DataCompositionSchema") => "dcs",
        ("http://v8.1c.ru/8.2/data/spreadsheet", "document") => "mxl",
        ("http://v8.1c.ru/8.2/managed-application/core", "ClientApplicationInterface") => {
            "client-application-interface"
        }
        ("http://v8.1c.ru/8.2/roles", "Rights") => "roles",
        _ => {
            return Err(format!(
                "unclassified XML root {{{namespace}}}{local_name}: {}",
                path.display()
            ));
        }
    };
    Ok(family.to_string())
}

fn source_set_owner_roots(
    workspace: &Path,
    post: &XmlSnapshot,
) -> Result<BTreeMap<String, String>, String> {
    let allowed = [
        "Configuration",
        "ConfigurationExtension",
        "ExternalDataProcessor",
        "ExternalReport",
    ];
    let mut owners = BTreeMap::new();
    for relative in post.keys() {
        let path = workspace.join(relative);
        let (namespace, local_name, child_type) = xml_root_details(&path)?;
        if namespace == "http://v8.1c.ru/8.3/MDClasses"
            && local_name == "MetaDataObject"
            && child_type
                .as_deref()
                .is_some_and(|kind| allowed.contains(&kind))
        {
            let root = Path::new(relative)
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .to_string_lossy()
                .replace('\\', "/");
            owners.insert(relative.clone(), root);
        }
    }
    Ok(owners)
}

fn owner_for_versionless_path<'a>(
    relative: &str,
    owners: &'a BTreeMap<String, String>,
) -> Option<&'a str> {
    owners
        .iter()
        .filter(|(_, root)| {
            root.is_empty()
                || relative == root.as_str()
                || relative.starts_with(&format!("{root}/"))
        })
        .max_by_key(|(_, root)| root.len())
        .map(|(owner, _)| owner.as_str())
}

fn manifest_case_prefix(case_id: &str) -> String {
    format!("cases/{case_id}/workspace")
}

fn manifest_path(case_id: &str, relative: &str) -> String {
    format!("{}/{relative}", manifest_case_prefix(case_id))
}

fn platform_checkpoint_for_case(case: &ExecutableCase) -> PlatformCheckpoint {
    let prefix = manifest_case_prefix(case.id);
    let (kind, source_path, base_source_path) =
        if registry_entry_for_case(case.id).impact == XmlImpactClass::None {
            ("none", None, None)
        } else if case.id.starts_with("cfe-") {
            (
                "extension",
                Some(format!("{prefix}/ext")),
                Some(format!("{prefix}/src")),
            )
        } else if case.id.starts_with("epf-") {
            ("epf", Some(format!("{prefix}/epf")), None)
        } else if case.id.starts_with("erf-") {
            ("erf", Some(format!("{prefix}/erf")), None)
        } else {
            ("configuration", Some(format!("{prefix}/src")), None)
        };
    PlatformCheckpoint {
        kind: kind.to_string(),
        source_path,
        base_source_path,
        covered_case_ids: vec![case.id.to_string()],
    }
}

fn owner_versions(
    case_id: &str,
    workspace: &Path,
    owners: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, String> {
    let mut versions = BTreeMap::new();
    for owner in owners.keys() {
        let text = fs::read_to_string(workspace.join(owner))
            .map_err(|error| format!("cannot read source-set owner {owner}: {error}"))?;
        let document = Document::parse(text.trim_start_matches('\u{feff}'))
            .map_err(|error| format!("cannot parse source-set owner {owner}: {error}"))?;
        let version = document
            .root_element()
            .attribute("version")
            .ok_or_else(|| format!("source-set owner has no version: {owner}"))?;
        if version != "2.20" {
            return Err(format!(
                "source-set owner is not format 2.20: {owner}: {version}"
            ));
        }
        versions.insert(manifest_path(case_id, owner), version.to_string());
    }
    Ok(versions)
}

fn ensure_all_xml_listed(actual: &XmlSnapshot, listed: &BTreeSet<String>) -> Result<(), String> {
    let actual_paths = actual.keys().cloned().collect::<BTreeSet<_>>();
    if actual_paths != *listed {
        let unlisted = actual_paths
            .difference(listed)
            .next()
            .cloned()
            .unwrap_or_default();
        let stale = listed
            .difference(&actual_paths)
            .next()
            .cloned()
            .unwrap_or_default();
        return Err(format!(
            "manifest XML listing mismatch: unlisted={unlisted:?}, stale={stale:?}"
        ));
    }
    Ok(())
}

fn delta_for_path<'a>(delta: &'a XmlDelta, path: &str) -> &'a str {
    if delta.created.iter().any(|item| item == path) {
        "created"
    } else if delta.modified.iter().any(|item| item == path) {
        "modified"
    } else if delta.unchanged.iter().any(|item| item == path) {
        "unchanged"
    } else {
        panic!("post-snapshot path absent from delta: {path}");
    }
}

fn build_corpus_case(
    case: &ExecutableCase,
    workspace: &Path,
    entry: &MutatorRegistryEntry,
    before: &XmlSnapshot,
    after: &XmlSnapshot,
    delta: &XmlDelta,
) -> Result<CorpusCase, String> {
    let owners = source_set_owner_roots(workspace, after)?;
    let owner_versions = owner_versions(case.id, workspace, &owners)?;
    let mut files = Vec::new();
    for (relative, hash) in after {
        let family = family_for_xml(&workspace.join(relative))?;
        let versionless = matches!(
            family.as_str(),
            "dcs" | "mxl" | "client-application-interface"
        );
        let owner = versionless
            .then(|| owner_for_versionless_path(relative, &owners))
            .flatten();
        let new_standalone = (versionless && owner.is_none())
            .then_some(delta.created.iter().any(|item| item == relative));
        if versionless && owner.is_none() && new_standalone != Some(true) {
            return Err(format!(
                "versionless XML has neither same-case owner nor new standalone evidence: {relative}"
            ));
        }
        files.push(CorpusFile {
            path: manifest_path(case.id, relative),
            sha256: hash.clone(),
            family,
            seed: before.contains_key(relative),
            delta: delta_for_path(delta, relative).to_string(),
            owner_path: owner.map(|owner| manifest_path(case.id, owner)),
            new_standalone,
        });
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    ensure_all_xml_listed(
        after,
        &files
            .iter()
            .map(|file| {
                file.path
                    .strip_prefix(&format!("{}/", manifest_case_prefix(case.id)))
                    .expect("case manifest prefix")
                    .to_string()
            })
            .collect(),
    )?;

    let mut removed_paths = delta
        .removed
        .iter()
        .map(|path| manifest_path(case.id, path))
        .collect::<Vec<_>>();
    removed_paths.sort();
    Ok(CorpusCase {
        id: case.id.to_string(),
        workspace_path: manifest_case_prefix(case.id),
        platform_checkpoint: platform_checkpoint_for_case(case),
        checkpoint: format!("cases/{}/case-report.json", case.id),
        tool_id: case.tool.to_string(),
        operation: entry.operation.to_string(),
        branch: case.branch.to_string(),
        impact_class: impact_class_name(entry.impact).to_string(),
        xml_impact: xml_impact_name(delta).to_string(),
        files,
        removed_paths,
        owner_versions,
    })
}

fn sanitize_value(value: &Value, workspace: &Path) -> Value {
    match value {
        Value::String(text) => {
            Value::String(text.replace(&workspace.display().to_string(), "$CASE_WORKSPACE"))
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| sanitize_value(item, workspace))
                .collect(),
        ),
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| (key.clone(), sanitize_value(value, workspace)))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn assert_case_postconditions(
    case: &ExecutableCase,
    workspace: &Path,
    before: &XmlSnapshot,
    after: &XmlSnapshot,
    delta: &XmlDelta,
) -> Result<(), String> {
    if matches!(
        registry_entry_for_case(case.id).impact,
        XmlImpactClass::None
    ) {
        if before.is_empty() {
            return Err(format!("None-impact case has no seed XML: {}", case.id));
        }
        if before != after {
            return Err(format!("None-impact case changed XML: {}", case.id));
        }
    }
    if case.id == "meta-compile-business-process"
        && !after
            .keys()
            .any(|path| path.ends_with("/Ext/Flowchart.xml"))
    {
        return Err("BusinessProcess case did not generate Flowchart.xml".to_string());
    }
    if case.id == "meta-compile-exchange-plan"
        && !after.keys().any(|path| path.ends_with("/Ext/Content.xml"))
    {
        return Err("ExchangePlan case did not generate Ext/Content.xml".to_string());
    }
    if matches!(case.branch, "SpreadsheetDocument" | "DataCompositionSchema")
        && case.id.starts_with("template-add-")
        && !after
            .keys()
            .any(|path| path.ends_with("/Templates/CorpusTemplate/Ext/Template.xml"))
    {
        return Err(format!("{} template content is not XML", case.branch));
    }
    if case.id.starts_with("template-add-")
        && matches!(case.branch, "TextDocument" | "HTMLDocument" | "BinaryData")
    {
        let extension = match case.branch {
            "TextDocument" => "txt",
            "HTMLDocument" => "html",
            "BinaryData" => "bin",
            _ => unreachable!(),
        };
        let relative =
            format!("src/Reports/CorpusReport/Templates/CorpusTemplate/Ext/Template.{extension}");
        if !workspace.join(&relative).is_file() || after.contains_key(&relative) {
            return Err(format!(
                "{} non-XML template content was missing or classified as XML",
                case.branch
            ));
        }
    }
    if matches!(
        registry_entry_for_case(case.id).impact,
        XmlImpactClass::RemoveOrModify
    ) && (delta.removed.is_empty() || delta.modified.is_empty())
    {
        return Err(format!(
            "removal case lacks removed path or surviving owner modification: {}",
            case.id
        ));
    }
    Ok(())
}

fn run_corpus_case(
    output: &Path,
    case: &ExecutableCase,
    gate: &mut SequentialCallGate,
) -> Result<CorpusCase, String> {
    let case_root = output.join("cases").join(case.id);
    let workspace = case_root.join("workspace");
    fs::create_dir_all(&workspace)
        .map_err(|error| format!("cannot create {}: {error}", workspace.display()))?;
    let args = prepare_target(case, &workspace)?;
    let before = snapshot_xml(&workspace)?;
    let sequence = gate.completed_target_calls + 1;
    let summary = call_target_tool(gate, case.tool, &args)?;
    let after = snapshot_xml(&workspace)?;
    let entry = registry_entry_for_case(case.id);
    let delta = enforce_xml_impact(entry.impact, &before, &after)?;
    assert_case_postconditions(case, &workspace, &before, &after, &delta)?;
    let manifest_case = build_corpus_case(case, &workspace, entry, &before, &after, &delta)?;
    let seed_outputs = before.keys().cloned().collect::<Vec<_>>();
    let remaining_xml = after.keys().cloned().collect::<Vec<_>>();
    let removed_paths = delta
        .removed
        .iter()
        .map(|path| RemovedPathReport {
            path: path.clone(),
            sha256: before
                .get(path)
                .expect("removed XML must have a pre-snapshot hash")
                .clone(),
        })
        .collect::<Vec<_>>();
    let owner_links = manifest_case
        .files
        .iter()
        .filter_map(|file| {
            file.owner_path
                .as_ref()
                .map(|owner| (file.path.clone(), owner.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    let report = CaseReport {
        schema_version: 1,
        profile: "1c-8.3.27-export-2.20".to_string(),
        id: case.id.to_string(),
        workspace_path: manifest_case.workspace_path.clone(),
        platform_checkpoint: manifest_case.platform_checkpoint.clone(),
        tool_id: case.tool.to_string(),
        operation: entry.operation.to_string(),
        branch: case.branch.to_string(),
        impact_class: impact_class_name(entry.impact).to_string(),
        public_arguments: sanitize_value(&Value::Object(args), &workspace),
        target_call: TargetCallReport {
            sequence,
            result_ok: true,
            errors: Vec::new(),
            summary: summary.replace(&workspace.display().to_string(), "$CASE_WORKSPACE"),
        },
        seed_outputs,
        pre_xml_sha256: before,
        post_xml_sha256: after,
        delta,
        remaining_xml,
        removed_paths,
        owner_links,
        owner_versions: manifest_case.owner_versions.clone(),
    };
    fs::write(
        case_root.join("case-report.json"),
        serde_json::to_vec_pretty(&report)
            .map_err(|error| format!("cannot serialize case report: {error}"))?,
    )
    .map_err(|error| format!("cannot write case report: {error}"))?;
    Ok(manifest_case)
}

fn sort_manifest(manifest: &mut CorpusManifest) {
    for case in &mut manifest.cases {
        case.files.sort_by(|left, right| left.path.cmp(&right.path));
        case.removed_paths.sort();
    }
    manifest.cases.sort_by(|left, right| left.id.cmp(&right.id));
}

fn generate_corpus(output: &Path) -> Result<CorpusManifest, String> {
    if !output.exists() {
        fs::create_dir(output).map_err(|error| {
            format!("cannot create corpus target {}: {error}", output.display())
        })?;
    }
    let mut cases = EXECUTABLE_CASES.iter().collect::<Vec<_>>();
    cases.sort_by_key(|case| case.id);
    let mut gate = SequentialCallGate::default();
    let mut generated = Vec::new();
    for case in cases {
        generated.push(run_corpus_case(output, case, &mut gate)?);
    }
    if gate.completed_target_calls != EXECUTABLE_CASES.len() {
        return Err(format!(
            "sequential target call count mismatch: {} != {}",
            gate.completed_target_calls,
            EXECUTABLE_CASES.len()
        ));
    }
    let mut manifest = CorpusManifest {
        schema_version: 1,
        profile: "1c-8.3.27-export-2.20".to_string(),
        cases: generated,
    };
    sort_manifest(&mut manifest);
    fs::write(
        output.join("corpus-manifest.json"),
        serde_json::to_vec_pretty(&manifest)
            .map_err(|error| format!("cannot serialize corpus manifest: {error}"))?,
    )
    .map_err(|error| format!("cannot write corpus manifest: {error}"))?;
    Ok(manifest)
}

fn normalize_absolute_path(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| format!("cannot resolve current directory: {error}"))?
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Path::new("/")),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err("output path escapes filesystem root".to_string());
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    Ok(normalized)
}

fn canonical_or_parent(path: &Path) -> Result<PathBuf, String> {
    if path.exists() {
        return path
            .canonicalize()
            .map_err(|error| format!("cannot canonicalize {}: {error}", path.display()));
    }
    let parent = path
        .parent()
        .ok_or_else(|| "output target has no parent directory".to_string())?;
    if !parent.is_dir() {
        return Err(format!(
            "output target parent does not exist: {}",
            parent.display()
        ));
    }
    let canonical_parent = parent
        .canonicalize()
        .map_err(|error| format!("cannot canonicalize {}: {error}", parent.display()))?;
    let name = path
        .file_name()
        .ok_or_else(|| "output target has no final path component".to_string())?;
    Ok(canonical_parent.join(name))
}

fn validate_output_directory(
    raw: &str,
    repo_root: &Path,
    home_root: &Path,
) -> Result<PathBuf, String> {
    if raw.trim().is_empty() {
        return Err("UNICA_XML_CORPUS_DIR is empty".to_string());
    }
    let normalized = normalize_absolute_path(Path::new(raw.trim()))?;
    if fs::symlink_metadata(&normalized).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err("corpus output target must not be a symlink".to_string());
    }
    let target = canonical_or_parent(&normalized)?;
    let filesystem_root = Path::new("/")
        .canonicalize()
        .map_err(|error| format!("cannot canonicalize filesystem root: {error}"))?;
    let home = home_root
        .canonicalize()
        .map_err(|error| format!("cannot canonicalize home root: {error}"))?;
    let repo = repo_root
        .canonicalize()
        .map_err(|error| format!("cannot canonicalize repository root: {error}"))?;
    if target == filesystem_root {
        return Err("refusing filesystem root as corpus output".to_string());
    }
    if target == home {
        return Err("refusing home root as corpus output".to_string());
    }
    if target == repo {
        return Err("refusing repository root as corpus output".to_string());
    }
    if target.exists() {
        if !target.is_dir() {
            return Err("corpus output target exists and is not a directory".to_string());
        }
        let mut entries = fs::read_dir(&target)
            .map_err(|error| format!("cannot inspect corpus output: {error}"))?;
        if entries.next().is_some() {
            return Err("corpus output directory must be empty".to_string());
        }
    }
    Ok(target)
}

fn configured_output_directory_from(
    raw: Option<&str>,
    repo_root: &Path,
    home_root: &Path,
) -> Result<PathBuf, String> {
    let raw = raw.ok_or_else(|| "UNICA_XML_CORPUS_DIR is not set".to_string())?;
    validate_output_directory(raw, repo_root, home_root)
}

fn configured_output_directory() -> Result<PathBuf, String> {
    let raw = std::env::var("UNICA_XML_CORPUS_DIR").ok();
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| "cannot resolve repository root".to_string())?;
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is not set".to_string())?;
    configured_output_directory_from(raw.as_deref(), repo_root, &home)
}

fn live_public_native_mutators() -> BTreeMap<&'static str, &'static str> {
    UnicaApplication::new()
        .tools()
        .into_iter()
        .filter_map(|tool| {
            if !tool.mutating {
                return None;
            }
            let ToolHandler::NativeOperation { operation, .. } = tool.handler else {
                return None;
            };
            Some((tool.name, operation))
        })
        .collect()
}

#[test]
fn every_public_native_mutator_has_xml_impact_and_case_coverage() {
    let live = live_public_native_mutators();
    let mut registry = BTreeMap::new();
    let mut seen_operations = BTreeSet::new();
    for entry in MUTATOR_REGISTRY {
        assert!(
            registry.insert(entry.tool, entry.operation).is_none(),
            "duplicate registry tool: {}",
            entry.tool
        );
        assert!(
            seen_operations.insert(entry.operation),
            "duplicate registry operation: {}",
            entry.operation
        );
    }

    assert_eq!(registry, live);

    let mut executable = BTreeMap::new();
    for case in EXECUTABLE_CASES {
        assert!(
            executable.insert(case.id, case).is_none(),
            "duplicate executable case: {}",
            case.id
        );
    }

    let mut referenced_cases = BTreeSet::new();
    for entry in MUTATOR_REGISTRY {
        assert!(
            !entry.case_ids.is_empty(),
            "registry row has no case: {}",
            entry.tool
        );
        let mut actual_branches = BTreeSet::new();
        for case_id in entry.case_ids {
            assert!(
                referenced_cases.insert(*case_id),
                "case referenced by multiple rows: {case_id}"
            );
            let case = executable
                .get(case_id)
                .unwrap_or_else(|| panic!("missing executable case: {case_id}"));
            assert_eq!(case.tool, entry.tool, "case tool mismatch: {case_id}");
            actual_branches.insert(case.branch);
        }
        let required_branches = entry
            .required_branches
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        assert!(
            !required_branches.is_empty(),
            "registry row has no branch contract: {}",
            entry.tool
        );
        assert_eq!(
            actual_branches, required_branches,
            "branch coverage mismatch"
        );
        let _ = entry.impact;
    }
    assert_eq!(
        referenced_cases,
        executable.keys().copied().collect(),
        "stale executable case"
    );
}

#[test]
fn xml_delta_classifies_exact_pre_post_impact() {
    let before = BTreeMap::from([
        ("modified.xml".to_string(), "old".to_string()),
        ("removed.xml".to_string(), "gone".to_string()),
        ("unchanged.xml".to_string(), "same".to_string()),
    ]);
    let after = BTreeMap::from([
        ("created.xml".to_string(), "new".to_string()),
        ("modified.xml".to_string(), "changed".to_string()),
        ("unchanged.xml".to_string(), "same".to_string()),
    ]);

    assert_eq!(
        classify_xml_delta(&before, &after),
        XmlDelta {
            created: vec!["created.xml".to_string()],
            modified: vec!["modified.xml".to_string()],
            removed: vec!["removed.xml".to_string()],
            unchanged: vec!["unchanged.xml".to_string()],
        }
    );
}

#[test]
fn remove_impact_requires_removal_and_surviving_owner_modification() {
    let before = BTreeMap::from([
        ("owner.xml".to_string(), "old".to_string()),
        ("removed.xml".to_string(), "gone".to_string()),
    ]);
    let after = BTreeMap::from([("owner.xml".to_string(), "new".to_string())]);

    let delta = enforce_xml_impact(XmlImpactClass::RemoveOrModify, &before, &after).unwrap();

    assert_eq!(delta.removed, ["removed.xml"]);
    assert_eq!(delta.modified, ["owner.xml"]);
}

#[test]
fn none_impact_rejects_any_xml_map_change() {
    let before = BTreeMap::from([("seed.xml".to_string(), "before".to_string())]);
    let after = BTreeMap::from([("seed.xml".to_string(), "after".to_string())]);

    let error = enforce_xml_impact(XmlImpactClass::None, &before, &after).unwrap_err();

    assert!(error.contains("complete XML map"), "{error}");
}

#[test]
fn sequential_execution_invariant_rejects_overlap() {
    let mut gate = SequentialCallGate::default();
    gate.begin().unwrap();

    let error = gate.begin().unwrap_err();

    assert!(error.contains("overlapping"), "{error}");
    gate.finish().unwrap();
    assert_eq!(gate.completed_target_calls, 1);
}

#[test]
fn cf_init_public_case_creates_real_xml() {
    let root = unique_temp_dir("cf-init-public-case");
    let workspace = root.join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    let before = snapshot_xml(&workspace).unwrap();
    let args = cf_init_args(&workspace, "CorpusConfiguration", "src");
    let mut gate = SequentialCallGate::default();

    call_target_tool(&mut gate, "unica.cf.init", &args).unwrap();

    let after = snapshot_xml(&workspace).unwrap();
    let entry = registry_entry_for_case("cf-init-default");
    let delta = enforce_xml_impact(entry.impact, &before, &after).unwrap();
    assert!(delta.created.contains(&"src/Configuration.xml".to_string()));
    assert_eq!(gate.completed_target_calls, 1);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn owner_assignment_requires_same_case_source_set_path() {
    let owners = BTreeMap::from([
        ("src/Configuration.xml".to_string(), "src".to_string()),
        ("ext/Configuration.xml".to_string(), "ext".to_string()),
    ]);

    assert_eq!(
        owner_for_versionless_path("src/Ext/ClientApplicationInterface.xml", &owners),
        Some("src/Configuration.xml")
    );
    assert_eq!(
        owner_for_versionless_path("ext/Ext/ClientApplicationInterface.xml", &owners),
        Some("ext/Configuration.xml")
    );
    assert_eq!(
        owner_for_versionless_path("standalone/Dcs.xml", &owners),
        None
    );
}

#[test]
fn unlisted_xml_prevention_rejects_missing_and_stale_paths() {
    let actual = BTreeMap::from([
        ("a.xml".to_string(), "a".to_string()),
        ("b.xml".to_string(), "b".to_string()),
    ]);
    let error = ensure_all_xml_listed(&actual, &BTreeSet::from(["a.xml".to_string()])).unwrap_err();
    assert!(error.contains("b.xml"), "{error}");

    let error = ensure_all_xml_listed(
        &actual,
        &BTreeSet::from([
            "a.xml".to_string(),
            "b.xml".to_string(),
            "stale.xml".to_string(),
        ]),
    )
    .unwrap_err();
    assert!(error.contains("stale.xml"), "{error}");
}

#[test]
fn output_directory_refusal_rules_are_fail_closed() {
    let root = unique_temp_dir("output-safety");
    let repo = root.join("repo");
    let home = root.join("home");
    let empty = root.join("empty");
    let non_empty = root.join("non-empty");
    fs::create_dir_all(&repo).unwrap();
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&empty).unwrap();
    fs::create_dir_all(&non_empty).unwrap();
    fs::write(non_empty.join("sentinel"), "keep").unwrap();

    assert!(configured_output_directory_from(None, &repo, &home).is_err());
    assert!(validate_output_directory("", &repo, &home).is_err());
    assert!(validate_output_directory("/", &repo, &home).is_err());
    assert!(validate_output_directory(home.to_str().unwrap(), &repo, &home).is_err());
    assert!(validate_output_directory(repo.to_str().unwrap(), &repo, &home).is_err());
    assert!(validate_output_directory(non_empty.to_str().unwrap(), &repo, &home).is_err());
    assert_eq!(
        validate_output_directory(empty.to_str().unwrap(), &repo, &home).unwrap(),
        empty.canonicalize().unwrap()
    );
    let absent = root.join("explicit-absent-target");
    assert_eq!(
        validate_output_directory(absent.to_str().unwrap(), &repo, &home).unwrap(),
        root.canonicalize().unwrap().join("explicit-absent-target")
    );
    assert!(!absent.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn manifest_sorting_and_task_7a_shape_round_trip() {
    let file = |path: &str| CorpusFile {
        path: path.to_string(),
        sha256: "a".repeat(64),
        family: "metadata".to_string(),
        seed: false,
        delta: "created".to_string(),
        owner_path: None,
        new_standalone: None,
    };
    let case = |id: &str, paths: &[&str]| CorpusCase {
        id: id.to_string(),
        workspace_path: format!("cases/{id}/workspace"),
        platform_checkpoint: PlatformCheckpoint {
            kind: "configuration".to_string(),
            source_path: Some(format!("cases/{id}/workspace/src")),
            base_source_path: None,
            covered_case_ids: vec![id.to_string()],
        },
        checkpoint: format!("cases/{id}/case-report.json"),
        tool_id: "unica.cf.init".to_string(),
        operation: "cf-init".to_string(),
        branch: "default".to_string(),
        impact_class: "CreateOrModify".to_string(),
        xml_impact: "created".to_string(),
        files: paths.iter().map(|path| file(path)).collect(),
        removed_paths: Vec::new(),
        owner_versions: BTreeMap::new(),
    };
    let mut manifest = CorpusManifest {
        schema_version: 1,
        profile: "1c-8.3.27-export-2.20".to_string(),
        cases: vec![case("z", &["z/b.xml", "z/a.xml"]), case("a", &["a/a.xml"])],
    };

    sort_manifest(&mut manifest);
    let bytes = serde_json::to_vec(&manifest).unwrap();
    let reparsed: CorpusManifest = serde_json::from_slice(&bytes).unwrap();
    let task_7a_view: Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(reparsed.cases[0].id, "a");
    assert_eq!(reparsed.cases[1].files[0].path, "z/a.xml");
    assert_eq!(task_7a_view["schemaVersion"], 1);
    assert_eq!(task_7a_view["profile"], "1c-8.3.27-export-2.20");
    for case in task_7a_view["cases"].as_array().unwrap() {
        assert!(case["id"].is_string());
        assert!(case["toolId"].is_string());
        assert!(case["xmlImpact"].is_string());
        for file in case["files"].as_array().unwrap() {
            assert!(file["path"].is_string());
            assert!(file["sha256"].is_string());
            assert!(file["family"].is_string());
            assert!(file["seed"].is_boolean());
        }
    }
}

#[test]
#[ignore = "writes an explicit developer-selected public-tool XML corpus"]
fn generate_platform_xml_corpus() {
    let output = configured_output_directory().expect("safe UNICA_XML_CORPUS_DIR");

    let manifest = generate_corpus(&output).expect("generate complete public-tool XML corpus");

    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.profile, "1c-8.3.27-export-2.20");
    assert_eq!(manifest.cases.len(), EXECUTABLE_CASES.len());
}
