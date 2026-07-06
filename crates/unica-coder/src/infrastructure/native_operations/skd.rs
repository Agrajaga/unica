#![allow(dead_code, unused_imports)]

use crate::domain::workspace::WorkspaceContext;
use crate::infrastructure::AdapterOutcome;
use roxmltree::Document;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::common::*;
use super::{
    cf::*, cfe::*, form::*, interface::*, meta::*, mxl::*, role::*, subsystem::*, template::*,
};
pub(crate) struct SkdValidationReporter {
    pub(crate) errors: usize,
    pub(crate) warnings: usize,
    pub(crate) ok_count: usize,
    pub(crate) stopped: bool,
    pub(crate) max_errors: usize,
    pub(crate) detailed: bool,
    pub(crate) lines: Vec<String>,
}

pub(crate) struct SkdValidationRun {
    pub(crate) ok: bool,
    pub(crate) stdout: String,
    pub(crate) out_file: Option<PathBuf>,
    pub(crate) out_file_label: Option<String>,
    pub(crate) artifact: PathBuf,
    pub(crate) errors: Vec<String>,
}

impl SkdValidationReporter {
    pub(crate) fn new(max_errors: usize, detailed: bool, file_name: &str) -> Self {
        Self {
            errors: 0,
            warnings: 0,
            ok_count: 0,
            stopped: false,
            max_errors,
            detailed,
            lines: vec![format!("=== Validation: {file_name} ==="), String::new()],
        }
    }

    pub(crate) fn ok(&mut self, message: impl Into<String>) {
        self.ok_count += 1;
        if self.detailed {
            self.lines.push(format!("[OK]    {}", message.into()));
        }
    }

    pub(crate) fn error(&mut self, message: impl Into<String>) {
        self.errors += 1;
        self.lines.push(format!("[ERROR] {}", message.into()));
        if self.errors >= self.max_errors {
            self.stopped = true;
        }
    }

    pub(crate) fn warn(&mut self, message: impl Into<String>) {
        self.warnings += 1;
        self.lines.push(format!("[WARN]  {}", message.into()));
    }

    pub(crate) fn finalize(mut self, file_name: &str) -> (bool, String, Vec<String>) {
        let checks = self.ok_count + self.errors + self.warnings;
        let ok = self.errors == 0;
        if ok && self.warnings == 0 && !self.detailed {
            return (
                true,
                format!("=== Validation OK: {file_name} ({checks} checks) ===\n"),
                Vec::new(),
            );
        }
        self.lines.push(String::new());
        self.lines.push(format!(
            "=== Result: {} errors, {} warnings ({checks} checks) ===",
            self.errors, self.warnings
        ));
        let errors = self
            .lines
            .iter()
            .filter(|line| line.starts_with("[ERROR] "))
            .cloned()
            .collect::<Vec<_>>();
        (ok, format!("{}\n", self.lines.join("\n")), errors)
    }
}

pub(crate) fn analyze_skd_info(
    args: &Map<String, Value>,
    context: &WorkspaceContext,
) -> AdapterOutcome {
    const NS_SCHEMA: &str = "http://v8.1c.ru/8.1/data-composition-system/schema";
    const NS_SETTINGS: &str = "http://v8.1c.ru/8.1/data-composition-system/settings";

    let result = (|| -> Result<(String, Option<PathBuf>, PathBuf), String> {
        let template_path = resolve_skd_info_path_for_script(args, context)?;
        let resolved_path = template_path
            .canonicalize()
            .unwrap_or_else(|_| template_path.clone());
        let text = read_utf8_sig(&resolved_path)?;
        let doc = Document::parse(text.trim_start_matches('\u{feff}'))
            .map_err(|err| format!("XML parse error in {}: {err}", resolved_path.display()))?;
        let root = doc.root_element();
        let mode = string_arg(args, &["mode", "Mode"]).unwrap_or("overview");
        let out_file_label = string_arg(args, &["outFile", "OutFile"]).map(ToOwned::to_owned);
        let out_file = out_file_label
            .as_ref()
            .filter(|value| !value.is_empty())
            .map(|value| absolutize(PathBuf::from(value), &context.cwd));
        let limit = int_arg(args, &["limit", "Limit"]).unwrap_or(150).max(0) as usize;
        let offset = int_arg(args, &["offset", "Offset"]).unwrap_or(0).max(0) as usize;
        let mut lines = Vec::<String>::new();

        match mode {
            "overview" => {
                skd_info_overview(
                    root,
                    &resolved_path,
                    &text,
                    &mut lines,
                    NS_SCHEMA,
                    NS_SETTINGS,
                );
                skd_info_overview_hints(root, &mut lines, NS_SCHEMA, NS_SETTINGS);
            }
            "query" => skd_info_query(root, &mut lines, NS_SCHEMA)?,
            "fields" => skd_info_fields(root, &mut lines, NS_SCHEMA),
            "links" => skd_info_links(root, &mut lines, NS_SCHEMA),
            "calculated" => {
                let count = skd_children(root, "calculatedField", NS_SCHEMA).len();
                if count == 0 {
                    lines.push("(no calculated fields)".to_string());
                } else {
                    lines.push(format!("=== Calculated fields ({count}) ==="));
                }
            }
            "resources" => {
                let count = skd_children(root, "totalField", NS_SCHEMA).len();
                if count == 0 {
                    lines.push("(no resources)".to_string());
                } else {
                    lines.push(format!("=== Resources ({count}) ==="));
                }
            }
            "params" => {
                let count = skd_children(root, "parameter", NS_SCHEMA).len();
                lines.push(format!("=== Parameters ({count}) ==="));
                lines.push(
                    "  Name                            Type                   Default          Visible  Expression"
                        .to_string(),
                );
            }
            "variant" => skd_info_variant(root, &mut lines, NS_SCHEMA, NS_SETTINGS),
            "templates" => skd_info_templates(root, &mut lines, NS_SCHEMA),
            "trace" => {
                let name = string_arg(args, &["name", "Name"]).unwrap_or("");
                if name.is_empty() {
                    return Err("Trace mode requires -Name <field_name_or_title>".to_string());
                }
                return Err(format!("Field '{name}' not found by dataPath or title"));
            }
            "full" => {
                skd_info_overview(
                    root,
                    &resolved_path,
                    &text,
                    &mut lines,
                    NS_SCHEMA,
                    NS_SETTINGS,
                );
            }
            other => {
                return Err(format!(
                    "argument -Mode: invalid choice: '{other}' (choose from 'overview', 'query', 'fields', 'links', 'calculated', 'resources', 'params', 'variant', 'trace', 'templates', 'full')"
                ));
            }
        }

        let total_lines = lines.len();
        if let Some(out_file) = &out_file {
            if let Some(parent) = out_file.parent() {
                fs::create_dir_all(parent)
                    .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
            }
            write_utf8_bom(out_file, &lines.join("\n"))?;
            let label = out_file_label.as_deref().unwrap_or("");
            return Ok((
                format!("Written {total_lines} lines to {label}\n"),
                Some(out_file.clone()),
                resolved_path,
            ));
        }

        let mut result = if offset > 0 {
            if offset >= total_lines {
                return Ok((
                    format!(
                        "[INFO] Offset {offset} exceeds total lines ({total_lines}). Nothing to show.\n"
                    ),
                    None,
                    resolved_path,
                ));
            }
            lines[offset..].to_vec()
        } else {
            lines
        };
        let stdout = if result.len() > limit {
            result.truncate(limit);
            format!(
                "{}\n\n[TRUNCATED] Shown {limit} of {total_lines} lines. Use -Offset {} to continue.\n",
                result.join("\n"),
                offset + limit
            )
        } else {
            format!("{}\n", result.join("\n"))
        };
        Ok((stdout, None, resolved_path))
    })();

    match result {
        Ok((stdout, out_file, artifact)) => {
            let mut artifacts = vec![artifact.display().to_string()];
            if let Some(out_file) = &out_file {
                artifacts.push(out_file.display().to_string());
            }
            AdapterOutcome {
                ok: true,
                summary: "unica.skd.info completed with native DCS inspector".to_string(),
                changes: Vec::new(),
                warnings: Vec::new(),
                errors: Vec::new(),
                artifacts,
                stdout: Some(stdout),
                stderr: None,
                command: None,
            }
        }
        Err(error) => AdapterOutcome {
            ok: false,
            summary: "unica.skd.info failed in native DCS inspector".to_string(),
            changes: Vec::new(),
            warnings: Vec::new(),
            errors: vec![error.clone()],
            artifacts: Vec::new(),
            stdout: None,
            stderr: Some(format!("{error}\n")),
            command: None,
        },
    }
}

pub(crate) fn skd_info_overview(
    root: roxmltree::Node<'_, '_>,
    resolved_path: &Path,
    text: &str,
    lines: &mut Vec<String>,
    ns_schema: &str,
    ns_settings: &str,
) {
    let template_name = skd_info_template_name(resolved_path);
    let total_xml_lines = text.lines().count();
    lines.push(format!(
        "=== DCS: {template_name} ({total_xml_lines} lines) ==="
    ));
    lines.push(format!(
        "Поддержка: {}",
        support_status_for_path(resolved_path)
    ));
    lines.push(String::new());

    let sources = skd_children(root, "dataSource", ns_schema)
        .into_iter()
        .map(|source| {
            format!(
                "{} ({})",
                skd_child(source, "name", ns_schema)
                    .map(skd_text_of)
                    .unwrap_or_default(),
                skd_child(source, "dataSourceType", ns_schema)
                    .map(skd_text_of)
                    .unwrap_or_default()
            )
        })
        .collect::<Vec<_>>();
    lines.push(format!("Sources: {}", sources.join(", ")));
    lines.push(String::new());

    lines.push("Datasets:".to_string());
    for data_set in skd_children(root, "dataSet", ns_schema) {
        skd_info_dataset_overview(data_set, lines, ns_schema, "  ");
    }

    let links = skd_children(root, "dataSetLink", ns_schema);
    if !links.is_empty() {
        let mut link_pairs = BTreeMap::<String, usize>::new();
        let mut ordered = Vec::<String>::new();
        for link in links {
            let key = format!(
                "{} -> {}",
                skd_child(link, "sourceDataSet", ns_schema)
                    .map(skd_text_of)
                    .unwrap_or_default(),
                skd_child(link, "destinationDataSet", ns_schema)
                    .map(skd_text_of)
                    .unwrap_or_default()
            );
            if !link_pairs.contains_key(&key) {
                ordered.push(key.clone());
            }
            *link_pairs.entry(key).or_insert(0) += 1;
        }
        let link_strs = ordered
            .into_iter()
            .map(|key| {
                let count = link_pairs.get(&key).copied().unwrap_or(0);
                if count > 1 {
                    format!("{key} ({count} fields)")
                } else {
                    key
                }
            })
            .collect::<Vec<_>>();
        lines.push(format!("Links: {}", link_strs.join(", ")));
    }

    let calculated = skd_children(root, "calculatedField", ns_schema);
    if !calculated.is_empty() {
        lines.push(format!("Calculated: {}", calculated.len()));
    }

    let totals = skd_children(root, "totalField", ns_schema);
    if !totals.is_empty() {
        let mut unique = HashSet::<String>::new();
        let mut has_grouped = false;
        for total in &totals {
            unique.insert(
                skd_child(*total, "dataPath", ns_schema)
                    .map(skd_text_of)
                    .unwrap_or_default(),
            );
            if skd_child(*total, "group", ns_schema).is_some() {
                has_grouped = true;
            }
        }
        let group_note = if has_grouped {
            ", with group formulas"
        } else {
            ""
        };
        if unique.len() == totals.len() {
            lines.push(format!("Resources: {}{group_note}", totals.len()));
        } else {
            lines.push(format!(
                "Resources: {} ({} fields{group_note})",
                totals.len(),
                unique.len()
            ));
        }
    }

    let templates = skd_children(root, "template", ns_schema);
    if !templates.is_empty() {
        let field_templates = skd_children(root, "fieldTemplate", ns_schema);
        let group_count = skd_children(root, "groupTemplate", ns_schema).len()
            + skd_children(root, "groupHeaderTemplate", ns_schema).len()
            + skd_children(root, "groupFooterTemplate", ns_schema).len();
        let mut parts = Vec::new();
        if !field_templates.is_empty() {
            parts.push(format!("{} field", field_templates.len()));
        }
        if group_count > 0 {
            parts.push(format!("{group_count} group"));
        }
        if parts.is_empty() {
            lines.push(format!("Templates: {} defined", templates.len()));
        } else {
            lines.push(format!(
                "Templates: {} defined ({} bindings)",
                templates.len(),
                parts.join(", ")
            ));
        }
    }

    let params = skd_children(root, "parameter", ns_schema);
    if params.is_empty() {
        lines.push("Params: (none)".to_string());
    } else {
        let mut visible_names = Vec::new();
        let mut hidden_count = 0usize;
        for param in &params {
            let name = skd_child(*param, "name", ns_schema)
                .map(skd_text_of)
                .unwrap_or_default();
            let hidden = skd_child(*param, "useRestriction", ns_schema)
                .map(skd_text_of)
                .is_some_and(|value| value == "true");
            if hidden {
                hidden_count += 1;
            } else {
                visible_names.push(name);
            }
        }
        let mut line = format!("Params: {}", params.len());
        if hidden_count > 0 && !visible_names.is_empty() {
            line.push_str(&format!(
                " ({} visible, {hidden_count} hidden)",
                visible_names.len()
            ));
        } else if hidden_count == params.len() {
            line.push_str(" (all hidden)");
        }
        if !visible_names.is_empty() && visible_names.len() <= 8 {
            line.push_str(": ");
            line.push_str(&visible_names.join(", "));
        }
        lines.push(line);
    }

    lines.push(String::new());
    let variants = skd_children(root, "settingsVariant", ns_schema);
    if !variants.is_empty() {
        lines.push("Variants:".to_string());
        for (index, variant) in variants.iter().enumerate() {
            let name = skd_child(*variant, "name", ns_settings)
                .map(skd_text_of)
                .unwrap_or_default();
            let presentation = skd_child(*variant, "presentation", ns_settings)
                .map(skd_info_multilang_or_inner_text)
                .unwrap_or_default();
            let presentation_str = if presentation.is_empty() {
                String::new()
            } else {
                format!("  \"{presentation}\"")
            };
            let settings = skd_child(*variant, "settings", ns_settings);
            let mut struct_items = Vec::new();
            let mut filter_count = 0usize;
            if let Some(settings) = settings {
                for item in skd_children(settings, "item", ns_settings) {
                    let item_type = skd_info_structure_item_type(item);
                    let group_fields = skd_info_group_fields(item, ns_settings);
                    let group = if group_fields.is_empty() {
                        "(detail)".to_string()
                    } else {
                        format!("({})", group_fields.join(","))
                    };
                    struct_items.push(format!("{item_type}{group}"));
                }
                if let Some(filter) = skd_child(settings, "filter", ns_settings) {
                    filter_count = skd_children(filter, "item", ns_settings).len();
                }
            }
            let struct_str = if struct_items.is_empty() {
                String::new()
            } else {
                format!("  {}", struct_items.join(", "))
            };
            let filter_str = if filter_count > 0 {
                format!("  {filter_count} filters")
            } else {
                String::new()
            };
            lines.push(format!(
                "  [{}] {name}{presentation_str}{struct_str}{filter_str}",
                index + 1
            ));
        }
    }
}

pub(crate) fn skd_info_dataset_overview(
    data_set: roxmltree::Node<'_, '_>,
    lines: &mut Vec<String>,
    ns_schema: &str,
    indent: &str,
) {
    let ds_type = skd_info_dataset_type(data_set);
    let name = skd_child(data_set, "name", ns_schema)
        .map(skd_text_of)
        .unwrap_or_default();
    let field_count = skd_children(data_set, "field", ns_schema).len();
    match ds_type.as_str() {
        "Query" => {
            let query_lines = skd_child(data_set, "query", ns_schema)
                .map(|node| skd_inner_text(node).split('\n').count())
                .unwrap_or(0);
            lines.push(format!(
                "{indent}[Query]  {name}   {field_count} fields, query {query_lines} lines"
            ));
        }
        "Object" => {
            let obj_str = skd_child(data_set, "objectName", ns_schema)
                .map(skd_text_of)
                .filter(|value| !value.is_empty())
                .map(|value| format!("  objectName={value}"))
                .unwrap_or_default();
            lines.push(format!(
                "{indent}[Object] {name}{obj_str}  {field_count} fields"
            ));
        }
        "Union" => {
            lines.push(format!("{indent}[Union]  {name}  {field_count} fields"));
            for sub_ds in skd_children(data_set, "item", ns_schema) {
                let sub_type = skd_info_dataset_type(sub_ds);
                let sub_name = skd_child(sub_ds, "name", ns_schema)
                    .map(skd_text_of)
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| "?".to_string());
                let sub_fields = skd_children(sub_ds, "field", ns_schema).len();
                if sub_type == "Query" {
                    let query_lines = skd_child(sub_ds, "query", ns_schema)
                        .map(|node| skd_inner_text(node).split('\n').count())
                        .unwrap_or(0);
                    lines.push(format!(
                        "    ├─ [Query] {sub_name}   {sub_fields} fields, query {query_lines} lines"
                    ));
                } else if sub_type == "Object" {
                    let obj_str = skd_child(sub_ds, "objectName", ns_schema)
                        .map(skd_text_of)
                        .filter(|value| !value.is_empty())
                        .map(|value| format!("  objectName={value}"))
                        .unwrap_or_default();
                    lines.push(format!(
                        "    ├─ [Object] {sub_name}{obj_str}  {sub_fields} fields"
                    ));
                } else {
                    lines.push(format!(
                        "    ├─ [{sub_type}] {sub_name}  {sub_fields} fields"
                    ));
                }
            }
        }
        _ => lines.push(format!("{indent}[{ds_type}] {name}  {field_count} fields")),
    }
}

pub(crate) fn skd_info_overview_hints(
    root: roxmltree::Node<'_, '_>,
    lines: &mut Vec<String>,
    ns_schema: &str,
    ns_settings: &str,
) {
    lines.push(String::new());
    let mut hints = Vec::<String>::new();
    let mut query_names = Vec::<String>::new();
    for data_set in skd_children(root, "dataSet", ns_schema) {
        if skd_info_dataset_type(data_set) == "Query" {
            query_names.push(
                skd_child(data_set, "name", ns_schema)
                    .map(skd_text_of)
                    .unwrap_or_default(),
            );
        } else if skd_info_dataset_type(data_set) == "Union" {
            for sub_ds in skd_children(data_set, "item", ns_schema) {
                if skd_info_dataset_type(sub_ds) == "Query" {
                    query_names.push(
                        skd_child(sub_ds, "name", ns_schema)
                            .map(skd_text_of)
                            .unwrap_or_default(),
                    );
                }
            }
        }
    }
    if query_names.len() == 1 {
        hints.push("-Mode query             query text".to_string());
    } else if query_names.len() > 1 {
        hints.push(format!(
            "-Mode query -Name <ds>  query text ({})",
            query_names.join(", ")
        ));
    }
    hints.push("-Mode fields            field tables by dataset".to_string());
    let links = skd_children(root, "dataSetLink", ns_schema);
    if !links.is_empty() {
        hints.push(format!(
            "-Mode links             dataset connections ({})",
            links.len()
        ));
    }
    let calculated = skd_children(root, "calculatedField", ns_schema);
    if !calculated.is_empty() {
        hints.push(format!(
            "-Mode calculated        calculated field expressions ({})",
            calculated.len()
        ));
    }
    let totals = skd_children(root, "totalField", ns_schema);
    if !totals.is_empty() {
        hints.push(format!(
            "-Mode resources         resource aggregation ({})",
            totals.len()
        ));
    }
    if !skd_children(root, "parameter", ns_schema).is_empty() {
        hints.push("-Mode params            parameter details".to_string());
    }
    let variants = skd_children(root, "settingsVariant", ns_schema);
    if variants.len() == 1 {
        hints.push("-Mode variant           variant structure".to_string());
    } else if variants.len() > 1 {
        hints.push(format!(
            "-Mode variant -Name <N> variant structure (1..{})",
            variants.len()
        ));
    }
    if !skd_children(root, "template", ns_schema).is_empty() {
        hints.push("-Mode templates         template bindings and expressions".to_string());
    }
    let _ = ns_settings;
    hints.push("-Mode trace -Name <f>   trace field origin (by name or title)".to_string());
    hints.push("-Mode full              all sections at once".to_string());
    lines.push("Next:".to_string());
    for hint in hints {
        lines.push(format!("  {hint}"));
    }
}

pub(crate) fn skd_info_query(
    root: roxmltree::Node<'_, '_>,
    lines: &mut Vec<String>,
    ns_schema: &str,
) -> Result<(), String> {
    let mut target = None;
    for data_set in skd_children(root, "dataSet", ns_schema) {
        if skd_info_dataset_type(data_set) == "Query" {
            target = Some(data_set);
            break;
        }
        if skd_info_dataset_type(data_set) == "Union" {
            for sub_ds in skd_children(data_set, "item", ns_schema) {
                if skd_info_dataset_type(sub_ds) == "Query" {
                    target = Some(sub_ds);
                    break;
                }
            }
        }
        if target.is_some() {
            break;
        }
    }
    let Some(target) = target else {
        return Err("No Query dataset found".to_string());
    };
    let query = skd_child(target, "query", ns_schema)
        .map(skd_inner_text)
        .unwrap_or_default();
    let name = skd_child(target, "name", ns_schema)
        .map(skd_text_of)
        .unwrap_or_default();
    lines.push(format!(
        "=== Query: {name} ({} lines) ===",
        query.split('\n').count()
    ));
    lines.push(String::new());
    for line in query.trim().split('\n') {
        lines.push(line.trim_end().to_string());
    }
    Ok(())
}

pub(crate) fn skd_info_fields(
    root: roxmltree::Node<'_, '_>,
    lines: &mut Vec<String>,
    ns_schema: &str,
) {
    lines.push("=== Fields map ===".to_string());
    for data_set in skd_children(root, "dataSet", ns_schema) {
        skd_info_field_map(data_set, lines, ns_schema, "");
        if skd_info_dataset_type(data_set) == "Union" {
            for sub_ds in skd_children(data_set, "item", ns_schema) {
                skd_info_field_map(sub_ds, lines, ns_schema, "  ");
            }
        }
    }
    lines.push(String::new());
    lines.push("Use -Name <field> for details.".to_string());
}

pub(crate) fn skd_info_field_map(
    data_set: roxmltree::Node<'_, '_>,
    lines: &mut Vec<String>,
    ns_schema: &str,
    indent: &str,
) {
    let fields = skd_children(data_set, "field", ns_schema)
        .into_iter()
        .filter_map(|field| skd_child(field, "dataPath", ns_schema).map(skd_text_of))
        .collect::<Vec<_>>();
    let name = skd_child(data_set, "name", ns_schema)
        .map(skd_text_of)
        .unwrap_or_default();
    let mut name_list = fields.join(", ");
    if name_list.len() > 100 {
        name_list.truncate(97);
        name_list.push_str("...");
    }
    lines.push(format!(
        "{indent}{name} [{}] ({}): {name_list}",
        skd_info_dataset_type(data_set),
        fields.len()
    ));
}

pub(crate) fn skd_info_links(
    root: roxmltree::Node<'_, '_>,
    lines: &mut Vec<String>,
    ns_schema: &str,
) {
    let links = skd_children(root, "dataSetLink", ns_schema);
    if links.is_empty() {
        lines.push("(no links)".to_string());
    } else {
        lines.push(format!("=== Links ({}) ===", links.len()));
    }
}

pub(crate) fn skd_info_variant(
    root: roxmltree::Node<'_, '_>,
    lines: &mut Vec<String>,
    ns_schema: &str,
    ns_settings: &str,
) {
    let variants = skd_children(root, "settingsVariant", ns_schema);
    if variants.is_empty() {
        lines.push("=== Variants: (none) ===".to_string());
        return;
    }
    lines.push(format!("=== Variants ({}) ===", variants.len()));
    for (index, variant) in variants.iter().enumerate() {
        let name = skd_child(*variant, "name", ns_settings)
            .map(skd_text_of)
            .unwrap_or_default();
        let presentation = skd_child(*variant, "presentation", ns_settings)
            .map(skd_info_multilang_or_inner_text)
            .unwrap_or_default();
        let presentation_str = if presentation.is_empty() {
            String::new()
        } else {
            format!("  \"{presentation}\"")
        };
        lines.push(format!("  [{}] {name}{presentation_str}", index + 1));
    }
}

pub(crate) fn skd_info_templates(
    root: roxmltree::Node<'_, '_>,
    lines: &mut Vec<String>,
    ns_schema: &str,
) {
    let templates = skd_children(root, "template", ns_schema);
    let field_count = skd_children(root, "fieldTemplate", ns_schema).len();
    let group_count = skd_children(root, "groupTemplate", ns_schema).len()
        + skd_children(root, "groupHeaderTemplate", ns_schema).len()
        + skd_children(root, "groupFooterTemplate", ns_schema).len();
    lines.push(format!(
        "=== Templates ({} defined: {field_count} field, {group_count} group) ===",
        templates.len()
    ));
}

pub(crate) fn skd_info_dataset_type(data_set: roxmltree::Node<'_, '_>) -> String {
    let xsi_type = attribute_by_local_name(data_set, "type").unwrap_or("");
    if xsi_type.contains("DataSetQuery") {
        "Query".to_string()
    } else if xsi_type.contains("DataSetObject") {
        "Object".to_string()
    } else if xsi_type.contains("DataSetUnion") {
        "Union".to_string()
    } else {
        "Unknown".to_string()
    }
}

pub(crate) fn skd_info_structure_item_type(item: roxmltree::Node<'_, '_>) -> &'static str {
    let xsi_type = attribute_by_local_name(item, "type").unwrap_or("");
    if xsi_type.contains("StructureItemGroup") {
        "Group"
    } else if xsi_type.contains("StructureItemTable") {
        "Table"
    } else if xsi_type.contains("StructureItemChart") {
        "Chart"
    } else {
        "Unknown"
    }
}

pub(crate) fn skd_info_multilang_or_inner_text(node: roxmltree::Node<'_, '_>) -> String {
    let value = multilang_text(node);
    if value.is_empty() {
        node.descendants()
            .filter_map(|child| child.text())
            .collect::<String>()
            .trim()
            .to_string()
    } else {
        value
    }
}

pub(crate) fn skd_info_group_fields(
    item: roxmltree::Node<'_, '_>,
    ns_settings: &str,
) -> Vec<String> {
    let mut fields = Vec::new();
    for group_item in skd_find_all_path(item, &[("groupItems", ns_settings), ("item", ns_settings)])
    {
        if let Some(field) = skd_child(group_item, "field", ns_settings) {
            let mut value = skd_text_of(field);
            let group_type = skd_child(group_item, "groupType", ns_settings)
                .map(skd_text_of)
                .unwrap_or_default();
            if !group_type.is_empty() && group_type != "Items" {
                value.push_str(&format!("({group_type})"));
            }
            fields.push(value);
        }
    }
    fields
}

pub(crate) fn skd_info_template_name(path: &Path) -> String {
    let parts = path
        .components()
        .map(|part| part.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    for index in (0..parts.len()).rev() {
        if parts[index] == "Ext" && index >= 1 {
            return parts[index - 1].clone();
        }
    }
    path.display().to_string()
}

pub(crate) fn resolve_skd_info_path_for_script(
    args: &Map<String, Value>,
    context: &WorkspaceContext,
) -> Result<PathBuf, String> {
    let raw_path = required_path(
        args,
        &["templatePath", "TemplatePath", "path", "Path"],
        "TemplatePath",
    )?;
    let original_path = raw_path.clone();
    let mut template_path = raw_path.clone();
    if template_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| !value.eq_ignore_ascii_case("xml"))
        .unwrap_or(true)
    {
        let candidate = template_path.join("Ext").join("Template.xml");
        if absolutize(candidate.clone(), &context.cwd).is_file() {
            template_path = candidate;
        }
    }

    let abs_template = absolutize(template_path.clone(), &context.cwd);
    if !abs_template.is_file()
        && template_path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| !value.eq_ignore_ascii_case("xml"))
            .unwrap_or(true)
    {
        let templates_dir = absolutize(original_path.join("Templates"), &context.cwd);
        if templates_dir.is_dir() {
            let mut dcs_templates = Vec::<PathBuf>::new();
            for entry in fs::read_dir(&templates_dir)
                .map_err(|err| format!("failed to read {}: {err}", templates_dir.display()))?
            {
                let entry = entry
                    .map_err(|err| format!("failed to read {}: {err}", templates_dir.display()))?;
                let path = entry.path();
                if path.extension().and_then(|value| value.to_str()) != Some("xml") {
                    continue;
                }
                let Ok(text) = fs::read_to_string(&path) else {
                    continue;
                };
                let Ok(doc) = Document::parse(text.trim_start_matches('\u{feff}')) else {
                    continue;
                };
                let template_type = doc
                    .descendants()
                    .find(|node| node.is_element() && node.tag_name().name() == "TemplateType")
                    .and_then(|node| node.text())
                    .unwrap_or("")
                    .trim();
                if template_type == "DataCompositionSchema" {
                    if let Some(stem) = path.file_stem().and_then(|value| value.to_str()) {
                        let template = templates_dir.join(stem).join("Ext").join("Template.xml");
                        if template.is_file() {
                            dcs_templates.push(template);
                        }
                    }
                }
            }
            if dcs_templates.len() == 1 {
                return Ok(dcs_templates.remove(0));
            }
            if dcs_templates.len() > 1 {
                return Err(format!(
                    "Multiple DCS templates found in: {}",
                    original_path.display()
                ));
            }
            return Err(format!(
                "No DCS templates found in: {}",
                original_path.display()
            ));
        }
    }

    let abs_template = absolutize(template_path, &context.cwd);
    if !abs_template.is_file() {
        return Err(format!("File not found: {}", abs_template.display()));
    }
    Ok(abs_template)
}

pub(crate) fn validate_skd(
    args: &Map<String, Value>,
    context: &WorkspaceContext,
) -> AdapterOutcome {
    const NS_SCHEMA: &str = "http://v8.1c.ru/8.1/data-composition-system/schema";

    let result = (|| -> Result<SkdValidationRun, String> {
        let template_path = resolve_skd_validate_path(args, context)?;
        let resolved_path = template_path
            .canonicalize()
            .unwrap_or_else(|_| template_path.clone());
        let file_name = resolved_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_string();
        let out_file_label = string_arg(args, &["outFile", "OutFile"]).map(ToOwned::to_owned);
        let out_file = out_file_label
            .as_ref()
            .filter(|value| !value.is_empty())
            .map(|value| absolutize(PathBuf::from(value), &context.cwd));
        let detailed = bool_arg(args, &["detailed", "Detailed"]);
        let max_errors = int_arg(args, &["maxErrors", "MaxErrors"])
            .unwrap_or(20)
            .max(0) as usize;

        let text = read_utf8_sig(&resolved_path)?;
        let mut report = SkdValidationReporter::new(max_errors, detailed, &file_name);
        let doc = match Document::parse(text.trim_start_matches('\u{feff}')) {
            Ok(doc) => {
                report.ok("XML parsed successfully");
                doc
            }
            Err(err) => {
                report.error(format!("XML parse failed: {err}"));
                let errors = report
                    .lines
                    .iter()
                    .filter(|line| line.starts_with("[ERROR] "))
                    .cloned()
                    .collect::<Vec<_>>();
                return Ok(SkdValidationRun {
                    ok: false,
                    stdout: format!("{}\n", report.lines.join("\n")),
                    out_file,
                    out_file_label,
                    artifact: resolved_path,
                    errors,
                });
            }
        };

        let root = doc.root_element();
        let root_local = root.tag_name().name();
        if root_local != "DataCompositionSchema" {
            report.error(format!(
                "Root element is '{root_local}', expected 'DataCompositionSchema'"
            ));
        } else {
            report.ok("Root element: DataCompositionSchema");
        }
        let root_ns = root.tag_name().namespace().unwrap_or("");
        if root_ns != NS_SCHEMA {
            report.error(format!(
                "Default namespace is '{root_ns}', expected '{NS_SCHEMA}'"
            ));
        } else {
            report.ok("Default namespace correct");
        }
        if report.stopped {
            return skd_validation_finish(
                report,
                &file_name,
                out_file,
                out_file_label,
                resolved_path,
            );
        }

        let data_source_nodes = skd_children(root, "dataSource", NS_SCHEMA);
        let mut data_source_names = HashSet::<String>::new();
        for dsn in &data_source_nodes {
            if let Some(name) = skd_child(*dsn, "name", NS_SCHEMA) {
                data_source_names.insert(skd_inner_text(name));
            }
        }

        let data_set_nodes = skd_children(root, "dataSet", NS_SCHEMA);
        let mut data_set_names = HashSet::<String>::new();
        let mut all_field_paths = HashMap::<String, String>::new();
        for ds in &data_set_nodes {
            if let Some(name_node) = skd_child(*ds, "name", NS_SCHEMA) {
                let ds_name = skd_inner_text(name_node);
                data_set_names.insert(ds_name.clone());
                skd_collect_data_set_fields(*ds, &ds_name, &mut all_field_paths);
            }
        }

        let calc_field_nodes = skd_children(root, "calculatedField", NS_SCHEMA);
        let mut calc_field_paths = HashSet::<String>::new();
        for cf in &calc_field_nodes {
            if let Some(dp) = skd_child(*cf, "dataPath", NS_SCHEMA) {
                calc_field_paths.insert(skd_inner_text(dp));
            }
        }
        let total_field_nodes = skd_children(root, "totalField", NS_SCHEMA);
        let param_nodes = skd_children(root, "parameter", NS_SCHEMA);
        let template_nodes = skd_children(root, "template", NS_SCHEMA);
        let mut template_names = HashSet::<String>::new();
        for template in &template_nodes {
            if let Some(name_node) = skd_child(*template, "name", NS_SCHEMA) {
                template_names.insert(skd_inner_text(name_node));
            }
        }
        let group_template_nodes = skd_children(root, "groupTemplate", NS_SCHEMA);
        let variant_nodes = skd_children(root, "settingsVariant", NS_SCHEMA);
        let mut known_fields = all_field_paths.keys().cloned().collect::<HashSet<String>>();
        known_fields.extend(calc_field_paths.iter().cloned());

        skd_validate_data_sources(&mut report, &data_source_nodes);
        if report.stopped {
            return skd_validation_finish(
                report,
                &file_name,
                out_file,
                out_file_label,
                resolved_path,
            );
        }
        skd_validate_data_sets(&mut report, &data_set_nodes, &data_source_names);
        if report.stopped {
            return skd_validation_finish(
                report,
                &file_name,
                out_file,
                out_file_label,
                resolved_path,
            );
        }
        for ds in &data_set_nodes {
            let ds_name = skd_child(*ds, "name", NS_SCHEMA)
                .map(skd_inner_text)
                .unwrap_or_else(|| "(unnamed)".to_string());
            skd_validate_data_set_fields(&mut report, *ds, &ds_name);
            if report.stopped {
                return skd_validation_finish(
                    report,
                    &file_name,
                    out_file.clone(),
                    out_file_label.clone(),
                    resolved_path.clone(),
                );
            }
        }
        if report.stopped {
            return skd_validation_finish(
                report,
                &file_name,
                out_file,
                out_file_label,
                resolved_path,
            );
        }
        skd_validate_data_set_links(&mut report, root, &data_set_names);
        if report.stopped {
            return skd_validation_finish(
                report,
                &file_name,
                out_file,
                out_file_label,
                resolved_path,
            );
        }
        skd_validate_calculated_fields(&mut report, &calc_field_nodes, &all_field_paths);
        if report.stopped {
            return skd_validation_finish(
                report,
                &file_name,
                out_file,
                out_file_label,
                resolved_path,
            );
        }
        skd_validate_total_fields(&mut report, &total_field_nodes);
        if report.stopped {
            return skd_validation_finish(
                report,
                &file_name,
                out_file,
                out_file_label,
                resolved_path,
            );
        }
        skd_validate_parameters(&mut report, &param_nodes);
        if report.stopped {
            return skd_validation_finish(
                report,
                &file_name,
                out_file,
                out_file_label,
                resolved_path,
            );
        }
        skd_validate_templates(&mut report, &template_nodes);
        if report.stopped {
            return skd_validation_finish(
                report,
                &file_name,
                out_file,
                out_file_label,
                resolved_path,
            );
        }
        skd_validate_group_templates(&mut report, &group_template_nodes, &template_names);
        if report.stopped {
            return skd_validation_finish(
                report,
                &file_name,
                out_file,
                out_file_label,
                resolved_path,
            );
        }
        skd_validate_settings_variants(&mut report, &variant_nodes, &known_fields);
        if report.stopped {
            return skd_validation_finish(
                report,
                &file_name,
                out_file,
                out_file_label,
                resolved_path,
            );
        }
        skd_validate_value_types(&mut report, root);
        skd_validation_finish(report, &file_name, out_file, out_file_label, resolved_path)
    })();

    match result {
        Ok(run) => {
            let mut stdout = run.stdout.clone();
            let mut artifacts = vec![run.artifact.display().to_string()];
            if let Some(out_file) = &run.out_file {
                if let Err(error) = write_utf8_bom(out_file, run.stdout.trim_end_matches('\n')) {
                    return AdapterOutcome {
                        ok: false,
                        summary: "unica.skd.validate failed in native DCS validator".to_string(),
                        changes: Vec::new(),
                        warnings: Vec::new(),
                        errors: vec![error.clone()],
                        artifacts,
                        stdout: None,
                        stderr: Some(format!("{error}\n")),
                        command: None,
                    };
                }
                if let Some(label) = &run.out_file_label {
                    stdout.push_str(&format!("Written to: {label}\n"));
                }
                artifacts.push(out_file.display().to_string());
            }
            AdapterOutcome {
                ok: run.ok,
                summary: if run.ok {
                    "unica.skd.validate completed with native DCS validator".to_string()
                } else {
                    "unica.skd.validate failed in native DCS validator".to_string()
                },
                changes: Vec::new(),
                warnings: Vec::new(),
                errors: run.errors,
                artifacts,
                stdout: Some(stdout),
                stderr: Some(String::new()),
                command: None,
            }
        }
        Err(error) => AdapterOutcome {
            ok: false,
            summary: "unica.skd.validate failed in native DCS validator".to_string(),
            changes: Vec::new(),
            warnings: Vec::new(),
            errors: vec![error.clone()],
            artifacts: Vec::new(),
            stdout: None,
            stderr: Some(format!("{error}\n")),
            command: None,
        },
    }
}

pub(crate) fn skd_validation_finish(
    report: SkdValidationReporter,
    file_name: &str,
    out_file: Option<PathBuf>,
    out_file_label: Option<String>,
    artifact: PathBuf,
) -> Result<SkdValidationRun, String> {
    let (ok, stdout, errors) = report.finalize(file_name);
    Ok(SkdValidationRun {
        ok,
        stdout,
        out_file,
        out_file_label,
        artifact,
        errors,
    })
}

pub(crate) fn skd_validate_data_sources(
    report: &mut SkdValidationReporter,
    data_source_nodes: &[roxmltree::Node<'_, '_>],
) {
    const NS_SCHEMA: &str = "http://v8.1c.ru/8.1/data-composition-system/schema";
    if data_source_nodes.is_empty() {
        report.warn("No dataSource elements found (settings-only DCS?)");
        return;
    }
    let mut names_seen = HashSet::<String>::new();
    let mut ds_ok = true;
    for dsn in data_source_nodes {
        let name = skd_child(*dsn, "name", NS_SCHEMA);
        let typ = skd_child(*dsn, "dataSourceType", NS_SCHEMA);
        let name_text = name.map(skd_inner_text).unwrap_or_default();
        if name_text.is_empty() {
            report.error("DataSource has empty name");
            ds_ok = false;
        } else if !names_seen.insert(name_text.clone()) {
            report.error(format!("Duplicate dataSource name: {name_text}"));
            ds_ok = false;
        }
        if let Some(typ) = typ {
            let type_text = skd_inner_text(typ);
            if !matches!(type_text.as_str(), "Local" | "External") {
                report.warn(format!(
                    "DataSource '{name_text}' has unusual type: {type_text}"
                ));
            }
        }
    }
    if ds_ok {
        report.ok(format!(
            "{} dataSource(s) found, names unique",
            data_source_nodes.len()
        ));
    }
}

pub(crate) fn skd_validate_value_types(
    report: &mut SkdValidationReporter,
    root: roxmltree::Node<'_, '_>,
) {
    const NS_SCHEMA: &str = "http://v8.1c.ru/8.1/data-composition-system/schema";
    const NS_V8: &str = "http://v8.1c.ru/8.1/data/core";
    const NS_CONFIG: &str = "http://v8.1c.ru/8.1/data/enterprise/current-config";
    const NS_ENTERPRISE: &str = "http://v8.1c.ru/8.1/data/enterprise";
    let valid_types = [
        "xs:decimal",
        "xs:string",
        "xs:dateTime",
        "xs:boolean",
        "v8:StandardPeriod",
        "v8:UUID",
        "v8:Null",
        "v8:Type",
        "v8:ValueStorage",
    ];
    let valid_sign = ["Any", "Nonnegative", "Negative"];
    let valid_length = ["Variable", "Fixed"];
    let valid_fractions = ["Date", "DateTime", "Time"];
    let value_types = root
        .descendants()
        .filter(|node| role_info_element(*node, "valueType", Some(NS_SCHEMA)))
        .collect::<Vec<_>>();
    if value_types.is_empty() {
        return;
    }

    let mut all_ok = true;
    for value_type in &value_types {
        let mut types = HashSet::<String>::new();
        let mut qualifiers = Vec::<String>::new();
        for child in value_type.children().filter(|child| child.is_element()) {
            if child.tag_name().namespace().unwrap_or("") != NS_V8 {
                continue;
            }
            let local = child.tag_name().name();
            if local == "Type" {
                let type_text = skd_text_of(child);
                if type_text.is_empty() {
                    report.error("valueType: <v8:Type> is empty");
                    all_ok = false;
                    if report.stopped {
                        return;
                    }
                    continue;
                }
                let Some((prefix, local_type)) = type_text.split_once(':') else {
                    report.error(format!(
                        "valueType: type '{type_text}' has no namespace prefix (expected xs:/v8:/d5p1: — e.g. xs:decimal not decimal)"
                    ));
                    all_ok = false;
                    if report.stopped {
                        return;
                    }
                    continue;
                };
                if matches!(prefix, "xs" | "v8") {
                    if !valid_types.contains(&type_text.as_str()) {
                        report.error(format!(
                            "valueType: unknown type '{type_text}' (allowed: xs:decimal/xs:string/xs:dateTime/xs:boolean/v8:StandardPeriod or <prefix>:*Ref.X)"
                        ));
                        all_ok = false;
                    } else {
                        types.insert(type_text);
                    }
                } else {
                    let prefix_ns = child.lookup_namespace_uri(Some(prefix));
                    if prefix_ns == Some(NS_CONFIG) {
                        if !skd_validate_config_ref_type_shape(local_type) {
                            report.error(format!(
                                "valueType: ref type '{type_text}' must look like '<prefix>:<Kind>.<Name>' (e.g. d5p1:CatalogRef.X)"
                            ));
                            all_ok = false;
                        } else {
                            types.insert(String::new());
                        }
                    } else if prefix_ns == Some(NS_ENTERPRISE) {
                        if !skd_validate_system_type_shape(local_type) {
                            report.error(format!(
                                "valueType: system type '{type_text}' has unexpected local-name shape"
                            ));
                            all_ok = false;
                        } else {
                            types.insert(String::new());
                        }
                    } else {
                        report.error(format!(
                            "valueType: type '{type_text}' uses prefix '{prefix}' bound to unexpected namespace '{}'",
                            prefix_ns.unwrap_or("None")
                        ));
                        all_ok = false;
                    }
                }
                if report.stopped {
                    return;
                }
            } else if local.ends_with("Qualifiers") {
                let q_name = format!("v8:{local}");
                qualifiers.push(q_name.clone());
                match q_name.as_str() {
                    "v8:NumberQualifiers" => {
                        let digits = skd_child(child, "Digits", NS_V8).map(skd_text_of);
                        let fraction = skd_child(child, "FractionDigits", NS_V8).map(skd_text_of);
                        let sign = skd_child(child, "AllowedSign", NS_V8).map(skd_text_of);
                        if digits
                            .as_deref()
                            .filter(|value| {
                                !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit())
                            })
                            .is_none()
                        {
                            report.error(
                                "v8:NumberQualifiers: <v8:Digits> missing or not a non-negative integer",
                            );
                            all_ok = false;
                        }
                        if fraction
                            .as_deref()
                            .filter(|value| {
                                !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit())
                            })
                            .is_none()
                        {
                            report.error(
                                "v8:NumberQualifiers: <v8:FractionDigits> missing or not a non-negative integer",
                            );
                            all_ok = false;
                        }
                        if let Some(sign) = sign.as_deref().filter(|value| !value.is_empty()) {
                            if !valid_sign.contains(&sign) {
                                report.error(format!(
                                    "v8:NumberQualifiers: <v8:AllowedSign>{sign}</v8:AllowedSign> — must be one of: {}",
                                    valid_sign.join(", ")
                                ));
                                all_ok = false;
                            }
                        }
                    }
                    "v8:StringQualifiers" => {
                        let length = skd_child(child, "Length", NS_V8).map(skd_text_of);
                        let allowed_length =
                            skd_child(child, "AllowedLength", NS_V8).map(skd_text_of);
                        if length
                            .as_deref()
                            .filter(|value| {
                                !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit())
                            })
                            .is_none()
                        {
                            report.error(
                                "v8:StringQualifiers: <v8:Length> missing or not a non-negative integer",
                            );
                            all_ok = false;
                        }
                        if let Some(allowed_length) =
                            allowed_length.as_deref().filter(|value| !value.is_empty())
                        {
                            if !valid_length.contains(&allowed_length) {
                                report.error(format!(
                                    "v8:StringQualifiers: <v8:AllowedLength>{allowed_length}</v8:AllowedLength> — must be one of: {}",
                                    valid_length.join(", ")
                                ));
                                all_ok = false;
                            }
                        }
                    }
                    "v8:DateQualifiers" => {
                        let fractions = skd_child(child, "DateFractions", NS_V8).map(skd_text_of);
                        if let Some(fractions) =
                            fractions.as_deref().filter(|value| !value.is_empty())
                        {
                            if !valid_fractions.contains(&fractions) {
                                report.error(format!(
                                    "v8:DateQualifiers: <v8:DateFractions>{fractions}</v8:DateFractions> — must be one of: {}",
                                    valid_fractions.join(", ")
                                ));
                                all_ok = false;
                            }
                        }
                    }
                    _ => {}
                }
                if report.stopped {
                    return;
                }
            }
        }

        for qualifier in qualifiers {
            let producer = match qualifier.as_str() {
                "v8:NumberQualifiers" => Some("xs:decimal"),
                "v8:StringQualifiers" => Some("xs:string"),
                "v8:DateQualifiers" => Some("xs:dateTime"),
                _ => None,
            };
            if let Some(producer) = producer {
                if !types.contains(producer) {
                    report.error(format!(
                        "valueType: <{qualifier}> has no matching <v8:Type>{producer}</v8:Type> in this valueType"
                    ));
                    all_ok = false;
                    if report.stopped {
                        return;
                    }
                }
            }
        }
    }

    if all_ok {
        report.ok(format!(
            "{} valueType block(s): structure and qualifiers OK",
            value_types.len()
        ));
    }
}

pub(crate) fn skd_validate_config_ref_type_shape(local_type: &str) -> bool {
    let Some((kind, name)) = local_type.split_once('.') else {
        return false;
    };
    !kind.is_empty() && !name.is_empty() && kind.chars().all(|ch| ch.is_ascii_alphabetic())
}

pub(crate) fn skd_validate_system_type_shape(local_type: &str) -> bool {
    let mut chars = local_type.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_alphabetic() && chars.all(|ch| ch.is_ascii_alphanumeric())
}

pub(crate) fn skd_validate_data_sets(
    report: &mut SkdValidationReporter,
    data_set_nodes: &[roxmltree::Node<'_, '_>],
    data_source_names: &HashSet<String>,
) {
    const NS_SCHEMA: &str = "http://v8.1c.ru/8.1/data-composition-system/schema";
    let valid_ds_types = ["DataSetQuery", "DataSetObject", "DataSetUnion"];
    if data_set_nodes.is_empty() {
        report.warn("No dataSet elements found (settings-only DCS?)");
        return;
    }
    let mut names_seen = HashSet::<String>::new();
    let mut ds_ok = true;
    for ds in data_set_nodes {
        let xsi_type = attribute_by_local_name(*ds, "type").unwrap_or("");
        let name_node = skd_child(*ds, "name", NS_SCHEMA);
        let ds_name = name_node
            .map(skd_inner_text)
            .unwrap_or_else(|| "(unnamed)".to_string());
        if name_node.is_none() || ds_name.is_empty() {
            report.error("DataSet has empty name");
            ds_ok = false;
        } else if !names_seen.insert(ds_name.clone()) {
            report.error(format!("Duplicate dataSet name: {ds_name}"));
            ds_ok = false;
        }
        if xsi_type.is_empty() {
            report.error(format!("DataSet '{ds_name}' missing xsi:type"));
            ds_ok = false;
        } else if !valid_ds_types.contains(&xsi_type) {
            report.warn(format!(
                "DataSet '{ds_name}' has unusual xsi:type: {xsi_type}"
            ));
        }
        if xsi_type != "DataSetUnion" {
            if let Some(src_node) = skd_child(*ds, "dataSource", NS_SCHEMA) {
                let source = skd_inner_text(src_node);
                if !source.is_empty() && !data_source_names.contains(&source) {
                    report.error(format!(
                        "DataSet '{ds_name}' references unknown dataSource: {source}"
                    ));
                    ds_ok = false;
                }
            }
        }
        if xsi_type == "DataSetQuery" {
            let query_node = skd_child(*ds, "query", NS_SCHEMA);
            if query_node.map(skd_text_of).unwrap_or_default().is_empty() {
                report.warn(format!("DataSet '{ds_name}' (Query) has empty query"));
            }
        }
        if xsi_type == "DataSetObject" {
            let obj_node = skd_child(*ds, "objectName", NS_SCHEMA);
            if obj_node.map(skd_text_of).unwrap_or_default().is_empty() {
                report.error(format!("DataSet '{ds_name}' (Object) has empty objectName"));
                ds_ok = false;
            }
        }
    }
    if ds_ok {
        report.ok(format!(
            "{} dataSet(s) found, names unique",
            data_set_nodes.len()
        ));
    }
}

pub(crate) fn skd_validate_data_set_fields(
    report: &mut SkdValidationReporter,
    ds_node: roxmltree::Node<'_, '_>,
    ds_name: &str,
) {
    const NS_SCHEMA: &str = "http://v8.1c.ru/8.1/data-composition-system/schema";
    let fields = skd_children(ds_node, "field", NS_SCHEMA);
    if fields.is_empty() {
        return;
    }
    let mut paths_seen = HashSet::<String>::new();
    let mut field_ok = true;
    for field in &fields {
        let dp = skd_child(*field, "dataPath", NS_SCHEMA);
        let field_ref = skd_child(*field, "field", NS_SCHEMA);
        let path = dp.map(skd_inner_text).unwrap_or_default();
        if path.is_empty() {
            report.error(format!("DataSet '{ds_name}': field has empty dataPath"));
            field_ok = false;
            continue;
        }
        if !paths_seen.insert(path.clone()) {
            report.warn(format!("DataSet '{ds_name}': duplicate dataPath '{path}'"));
        }
        if field_ref.map(skd_inner_text).unwrap_or_default().is_empty() {
            report.warn(format!(
                "DataSet '{ds_name}': field '{path}' has empty <field> element"
            ));
        }
    }
    if field_ok {
        report.ok(format!(
            "DataSet \"{ds_name}\": {} fields, dataPath unique",
            fields.len()
        ));
    }
    for item in skd_children(ds_node, "item", NS_SCHEMA) {
        let item_name = skd_child(item, "name", NS_SCHEMA)
            .map(skd_inner_text)
            .unwrap_or_else(|| "(unnamed item)".to_string());
        skd_validate_data_set_fields(report, item, &item_name);
    }
}

pub(crate) fn skd_validate_data_set_links(
    report: &mut SkdValidationReporter,
    root: roxmltree::Node<'_, '_>,
    data_set_names: &HashSet<String>,
) {
    const NS_SCHEMA: &str = "http://v8.1c.ru/8.1/data-composition-system/schema";
    let link_nodes = skd_children(root, "dataSetLink", NS_SCHEMA);
    if link_nodes.is_empty() {
        return;
    }
    let mut link_ok = true;
    for link in &link_nodes {
        let src = skd_child(*link, "sourceDataSet", NS_SCHEMA);
        let dst = skd_child(*link, "destinationDataSet", NS_SCHEMA);
        let src_expr = skd_child(*link, "sourceExpression", NS_SCHEMA);
        let dst_expr = skd_child(*link, "destinationExpression", NS_SCHEMA);
        let src_text = src.map(skd_inner_text).unwrap_or_default();
        if !src_text.is_empty() && !data_set_names.contains(&src_text) {
            report.error(format!("DataSetLink: sourceDataSet '{src_text}' not found"));
            link_ok = false;
        }
        let dst_text = dst.map(skd_inner_text).unwrap_or_default();
        if !dst_text.is_empty() && !data_set_names.contains(&dst_text) {
            report.error(format!(
                "DataSetLink: destinationDataSet '{dst_text}' not found"
            ));
            link_ok = false;
        }
        if src_expr.map(skd_text_of).unwrap_or_default().is_empty() {
            report.error("DataSetLink: empty sourceExpression");
            link_ok = false;
        }
        if dst_expr.map(skd_text_of).unwrap_or_default().is_empty() {
            report.error("DataSetLink: empty destinationExpression");
            link_ok = false;
        }
    }
    if link_ok {
        report.ok(format!(
            "{} dataSetLink(s): references valid",
            link_nodes.len()
        ));
    }
}

pub(crate) fn skd_validate_calculated_fields(
    report: &mut SkdValidationReporter,
    calc_field_nodes: &[roxmltree::Node<'_, '_>],
    all_field_paths: &HashMap<String, String>,
) {
    const NS_SCHEMA: &str = "http://v8.1c.ru/8.1/data-composition-system/schema";
    if calc_field_nodes.is_empty() {
        return;
    }
    let mut cf_ok = true;
    let mut cf_seen = HashSet::<String>::new();
    for calc in calc_field_nodes {
        let dp = skd_child(*calc, "dataPath", NS_SCHEMA);
        let expr = skd_child(*calc, "expression", NS_SCHEMA);
        let path = dp.map(skd_inner_text).unwrap_or_default();
        if path.is_empty() {
            report.error("CalculatedField has empty dataPath");
            cf_ok = false;
            continue;
        }
        if !cf_seen.insert(path.clone()) {
            report.error(format!("Duplicate calculatedField dataPath: {path}"));
            cf_ok = false;
        }
        if expr.map(skd_text_of).unwrap_or_default().is_empty() {
            report.error(format!("CalculatedField '{path}' has empty expression"));
            cf_ok = false;
        }
        if let Some(ds_name) = all_field_paths.get(&path) {
            report.warn(format!(
                "CalculatedField '{path}' shadows dataSet field in '{ds_name}'"
            ));
        }
    }
    if cf_ok {
        report.ok(format!(
            "{} calculatedField(s): dataPath and expression valid",
            calc_field_nodes.len()
        ));
    }
}

pub(crate) fn skd_validate_total_fields(
    report: &mut SkdValidationReporter,
    total_field_nodes: &[roxmltree::Node<'_, '_>],
) {
    const NS_SCHEMA: &str = "http://v8.1c.ru/8.1/data-composition-system/schema";
    if total_field_nodes.is_empty() {
        return;
    }
    let mut tf_ok = true;
    for total in total_field_nodes {
        let dp = skd_child(*total, "dataPath", NS_SCHEMA);
        let expr = skd_child(*total, "expression", NS_SCHEMA);
        let path = dp.map(skd_inner_text).unwrap_or_default();
        if path.is_empty() {
            report.error("TotalField has empty dataPath");
            tf_ok = false;
            continue;
        }
        if expr.map(skd_text_of).unwrap_or_default().is_empty() {
            report.error(format!("TotalField '{path}' has empty expression"));
            tf_ok = false;
        }
    }
    if tf_ok {
        report.ok(format!(
            "{} totalField(s): dataPath and expression present",
            total_field_nodes.len()
        ));
    }
}

pub(crate) fn skd_validate_parameters(
    report: &mut SkdValidationReporter,
    param_nodes: &[roxmltree::Node<'_, '_>],
) {
    const NS_SCHEMA: &str = "http://v8.1c.ru/8.1/data-composition-system/schema";
    if param_nodes.is_empty() {
        return;
    }
    let mut param_ok = true;
    let mut param_seen = HashSet::<String>::new();
    for param in param_nodes {
        let name = skd_child(*param, "name", NS_SCHEMA)
            .map(skd_inner_text)
            .unwrap_or_default();
        if name.is_empty() {
            report.error("Parameter has empty name");
            param_ok = false;
            continue;
        }
        if !param_seen.insert(name.clone()) {
            report.error(format!("Duplicate parameter name: {name}"));
            param_ok = false;
        }
    }
    if param_ok {
        report.ok(format!("{} parameter(s): names unique", param_nodes.len()));
    }
}

pub(crate) fn skd_validate_templates(
    report: &mut SkdValidationReporter,
    template_nodes: &[roxmltree::Node<'_, '_>],
) {
    const NS_SCHEMA: &str = "http://v8.1c.ru/8.1/data-composition-system/schema";
    if template_nodes.is_empty() {
        return;
    }
    let mut tpl_ok = true;
    let mut tpl_seen = HashSet::<String>::new();
    for template in template_nodes {
        let name = skd_child(*template, "name", NS_SCHEMA)
            .map(skd_inner_text)
            .unwrap_or_default();
        if name.is_empty() {
            report.error("Template has empty name");
            tpl_ok = false;
            continue;
        }
        if !tpl_seen.insert(name.clone()) {
            report.error(format!("Duplicate template name: {name}"));
            tpl_ok = false;
        }
    }
    if tpl_ok {
        report.ok(format!(
            "{} template(s): names unique",
            template_nodes.len()
        ));
    }
}

pub(crate) fn skd_validate_group_templates(
    report: &mut SkdValidationReporter,
    group_template_nodes: &[roxmltree::Node<'_, '_>],
    template_names: &HashSet<String>,
) {
    const NS_SCHEMA: &str = "http://v8.1c.ru/8.1/data-composition-system/schema";
    if group_template_nodes.is_empty() {
        return;
    }
    let valid_tpl_types = [
        "Header",
        "Footer",
        "Overall",
        "OverallHeader",
        "OverallFooter",
    ];
    let mut gt_ok = true;
    for group_template in group_template_nodes {
        let tpl_ref = skd_child(*group_template, "template", NS_SCHEMA)
            .map(skd_inner_text)
            .unwrap_or_default();
        let tpl_type = skd_child(*group_template, "templateType", NS_SCHEMA)
            .map(skd_inner_text)
            .unwrap_or_default();
        if !tpl_ref.is_empty() && !template_names.contains(&tpl_ref) {
            report.error(format!(
                "GroupTemplate references unknown template: {tpl_ref}"
            ));
            gt_ok = false;
        }
        if !tpl_type.is_empty() && !valid_tpl_types.contains(&tpl_type.as_str()) {
            report.warn(format!(
                "GroupTemplate has unusual templateType: {tpl_type}"
            ));
        }
    }
    if gt_ok {
        report.ok(format!(
            "{} groupTemplate(s): references valid",
            group_template_nodes.len()
        ));
    }
}

pub(crate) fn skd_validate_settings_variants(
    report: &mut SkdValidationReporter,
    variant_nodes: &[roxmltree::Node<'_, '_>],
    known_fields: &HashSet<String>,
) {
    const NS_SETTINGS: &str = "http://v8.1c.ru/8.1/data-composition-system/settings";
    if variant_nodes.is_empty() {
        report.warn("No settingsVariant elements found");
        return;
    }
    let mut v_ok = true;
    for (idx, variant) in variant_nodes.iter().enumerate() {
        let v_name = skd_child(*variant, "name", NS_SETTINGS);
        let variant_name = v_name.map(skd_inner_text).unwrap_or_default();
        if variant_name.is_empty() {
            report.error(format!("SettingsVariant #{} has empty name", idx + 1));
            v_ok = false;
        }
        let settings = skd_child(*variant, "settings", NS_SETTINGS);
        let Some(settings) = settings else {
            report.error(format!(
                "SettingsVariant '{variant_name}' has no settings element"
            ));
            v_ok = false;
            continue;
        };
        skd_check_settings(report, settings, &variant_name, known_fields);
    }
    if v_ok {
        report.ok(format!("{} settingsVariant(s) found", variant_nodes.len()));
    }
}

pub(crate) fn skd_check_settings(
    report: &mut SkdValidationReporter,
    settings_node: roxmltree::Node<'_, '_>,
    variant_name: &str,
    known_fields: &HashSet<String>,
) {
    const NS_SETTINGS: &str = "http://v8.1c.ru/8.1/data-composition-system/settings";
    if report.stopped {
        return;
    }
    for selected_item in skd_find_all_path(
        settings_node,
        &[("selection", NS_SETTINGS), ("item", NS_SETTINGS)],
    ) {
        let xsi_type = attribute_by_local_name(selected_item, "type").unwrap_or("");
        if xsi_type == "dcsset:SelectedItemField" {
            let field = skd_child(selected_item, "field", NS_SETTINGS)
                .map(skd_inner_text)
                .unwrap_or_default();
            if !field.is_empty() && field != "SystemFields.Number" {
                let base_path = field.split('.').next().unwrap_or("");
                if !known_fields.contains(&field) && !known_fields.contains(base_path) {
                    // Soft check in the reference script: autoFillFields may add implicit fields.
                }
            }
        }
    }
    skd_check_filter_items(report, settings_node, variant_name);
    for order_item in skd_find_all_path(
        settings_node,
        &[("order", NS_SETTINGS), ("item", NS_SETTINGS)],
    ) {
        let xsi_type = attribute_by_local_name(order_item, "type").unwrap_or("");
        if xsi_type == "dcsset:OrderItemField" {
            let order_type = skd_child(order_item, "orderType", NS_SETTINGS)
                .map(skd_inner_text)
                .unwrap_or_default();
            if !order_type.is_empty() && !matches!(order_type.as_str(), "Asc" | "Desc") {
                report.warn(format!(
                    "Variant '{variant_name}' order: invalid orderType '{order_type}'"
                ));
            }
        }
    }
    for structure_item in skd_children(settings_node, "item", NS_SETTINGS) {
        skd_check_structure_item(report, structure_item, variant_name);
    }
}

pub(crate) fn skd_check_filter_items(
    report: &mut SkdValidationReporter,
    parent_node: roxmltree::Node<'_, '_>,
    variant_name: &str,
) {
    const NS_SETTINGS: &str = "http://v8.1c.ru/8.1/data-composition-system/settings";
    let valid_comparison_types = [
        "Equal",
        "NotEqual",
        "Greater",
        "GreaterOrEqual",
        "Less",
        "LessOrEqual",
        "InList",
        "NotInList",
        "InHierarchy",
        "InListByHierarchy",
        "Contains",
        "NotContains",
        "BeginsWith",
        "NotBeginsWith",
        "Filled",
        "NotFilled",
    ];
    for filter_item in skd_find_all_path(
        parent_node,
        &[("filter", NS_SETTINGS), ("item", NS_SETTINGS)],
    ) {
        if report.stopped {
            return;
        }
        let xsi_type = attribute_by_local_name(filter_item, "type").unwrap_or("");
        if xsi_type == "dcsset:FilterItemComparison" {
            let comp_type = skd_child(filter_item, "comparisonType", NS_SETTINGS)
                .map(skd_inner_text)
                .unwrap_or_default();
            if !comp_type.is_empty() && !valid_comparison_types.contains(&comp_type.as_str()) {
                report.error(format!(
                    "Variant '{variant_name}' filter: invalid comparisonType '{comp_type}'"
                ));
            }
        } else if xsi_type == "dcsset:FilterItemGroup" {
            let group_type = skd_child(filter_item, "groupType", NS_SETTINGS)
                .map(skd_inner_text)
                .unwrap_or_default();
            if !group_type.is_empty()
                && !matches!(group_type.as_str(), "AndGroup" | "OrGroup" | "NotGroup")
            {
                report.warn(format!(
                    "Variant '{variant_name}' filter group: unusual groupType '{group_type}'"
                ));
            }
            for nested in skd_children(filter_item, "item", NS_SETTINGS) {
                let nested_type = attribute_by_local_name(nested, "type").unwrap_or("");
                if nested_type == "dcsset:FilterItemComparison" {
                    let comp_type = skd_child(nested, "comparisonType", NS_SETTINGS)
                        .map(skd_inner_text)
                        .unwrap_or_default();
                    if !comp_type.is_empty()
                        && !valid_comparison_types.contains(&comp_type.as_str())
                    {
                        report.error(format!(
                            "Variant '{variant_name}' filter: invalid comparisonType '{comp_type}'"
                        ));
                    }
                }
            }
        }
    }
}

pub(crate) fn skd_check_structure_item(
    report: &mut SkdValidationReporter,
    item_node: roxmltree::Node<'_, '_>,
    variant_name: &str,
) {
    const NS_SETTINGS: &str = "http://v8.1c.ru/8.1/data-composition-system/settings";
    if report.stopped {
        return;
    }
    let valid_structure_types = [
        "dcsset:StructureItemGroup",
        "dcsset:StructureItemTable",
        "dcsset:StructureItemChart",
        "dcsset:StructureItemNestedObject",
    ];
    let xsi_type = attribute_by_local_name(item_node, "type").unwrap_or("");
    if xsi_type.is_empty() {
        report.error(format!(
            "Variant '{variant_name}': structure item missing xsi:type"
        ));
        return;
    }
    if !valid_structure_types.contains(&xsi_type) {
        report.warn(format!(
            "Variant '{variant_name}': unusual structure item type '{xsi_type}'"
        ));
    }
    for nested in skd_children(item_node, "item", NS_SETTINGS) {
        skd_check_structure_item(report, nested, variant_name);
    }
    if xsi_type == "dcsset:StructureItemTable" {
        let columns = skd_children(item_node, "column", NS_SETTINGS);
        let rows = skd_children(item_node, "row", NS_SETTINGS);
        if columns.is_empty() {
            report.warn(format!("Variant '{variant_name}': table has no columns"));
        }
        if rows.is_empty() {
            report.warn(format!("Variant '{variant_name}': table has no rows"));
        }
    }
}

pub(crate) fn skd_collect_data_set_fields(
    ds_node: roxmltree::Node<'_, '_>,
    ds_name: &str,
    all_field_paths: &mut HashMap<String, String>,
) -> HashSet<String> {
    const NS_SCHEMA: &str = "http://v8.1c.ru/8.1/data-composition-system/schema";
    let mut local_paths = HashSet::<String>::new();
    for field in skd_children(ds_node, "field", NS_SCHEMA) {
        if let Some(dp) = skd_child(field, "dataPath", NS_SCHEMA) {
            let path = skd_inner_text(dp);
            local_paths.insert(path.clone());
            all_field_paths.insert(path, ds_name.to_string());
        }
    }
    for item in skd_children(ds_node, "item", NS_SCHEMA) {
        if let Some(item_name) = skd_child(item, "name", NS_SCHEMA) {
            skd_collect_data_set_fields(item, &skd_inner_text(item_name), all_field_paths);
        }
    }
    local_paths
}

pub(crate) fn skd_children<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
    local_name: &str,
    namespace: &str,
) -> Vec<roxmltree::Node<'a, 'input>> {
    node.children()
        .filter(|child| role_info_element(*child, local_name, Some(namespace)))
        .collect()
}

pub(crate) fn skd_child<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
    local_name: &str,
    namespace: &str,
) -> Option<roxmltree::Node<'a, 'input>> {
    node.children()
        .find(|child| role_info_element(*child, local_name, Some(namespace)))
}

pub(crate) fn skd_find_all_path<'a, 'input>(
    parent: roxmltree::Node<'a, 'input>,
    path: &[(&str, &str)],
) -> Vec<roxmltree::Node<'a, 'input>> {
    let mut current = vec![parent];
    for (local_name, namespace) in path {
        let mut next = Vec::<roxmltree::Node<'a, 'input>>::new();
        for node in current {
            next.extend(skd_children(node, local_name, namespace));
        }
        current = next;
    }
    current
}

pub(crate) fn skd_inner_text(node: roxmltree::Node<'_, '_>) -> String {
    node.text().unwrap_or("").to_string()
}

pub(crate) fn skd_text_of(node: roxmltree::Node<'_, '_>) -> String {
    node.text().unwrap_or("").trim().to_string()
}

pub(crate) fn resolve_skd_validate_path(
    args: &Map<String, Value>,
    context: &WorkspaceContext,
) -> Result<PathBuf, String> {
    let raw_path = required_path(
        args,
        &["templatePath", "TemplatePath", "path", "Path"],
        "TemplatePath",
    )?;
    let mut display_path = raw_path.clone();
    let mut template_path = absolutize(raw_path, &context.cwd);

    if template_path.is_dir() {
        display_path = display_path.join("Ext").join("Template.xml");
        template_path = template_path.join("Ext").join("Template.xml");
    }
    if !template_path.exists()
        && display_path.file_name().and_then(|value| value.to_str()) == Some("Template.xml")
    {
        let display_candidate = display_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join("Ext")
            .join("Template.xml");
        let candidate = template_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join("Ext")
            .join("Template.xml");
        if candidate.exists() {
            display_path = display_candidate;
            template_path = candidate;
        }
    }
    if !template_path.exists()
        && display_path
            .extension()
            .and_then(|value| value.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("xml"))
            .unwrap_or(false)
    {
        if let Some(stem) = display_path.file_stem().and_then(|value| value.to_str()) {
            let display_candidate = display_path
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .join(stem)
                .join("Ext")
                .join("Template.xml");
            let candidate = template_path
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .join(stem)
                .join("Ext")
                .join("Template.xml");
            if candidate.exists() {
                display_path = display_candidate;
                template_path = candidate;
            }
        }
    }
    if !template_path.exists() {
        return Err(format!("File not found: {}", display_path.display()));
    }
    Ok(template_path)
}

pub(crate) fn compile_skd(args: &Map<String, Value>, context: &WorkspaceContext) -> AdapterOutcome {
    let write_result = (|| -> Result<(String, PathBuf), String> {
        let definition_file = path_arg(args, &["definitionFile", "DefinitionFile"]);
        let value = string_arg(args, &["value", "Value"]);
        if definition_file.is_some() && value.is_some() {
            return Err("Cannot use both -DefinitionFile and -Value".to_string());
        }
        if definition_file.is_none() && value.is_none() {
            return Err("Either -DefinitionFile or -Value is required".to_string());
        }

        let output_path_label = string_arg(args, &["outputPath", "OutputPath"])
            .ok_or_else(|| "missing required OutputPath argument".to_string())?
            .to_string();
        let output_path = absolutize(PathBuf::from(&output_path_label), &context.cwd);

        let (json_text, query_base_dir) = if let Some(definition_file) = definition_file {
            let definition_file = absolutize(definition_file, &context.cwd);
            if !definition_file.exists() {
                return Err(format!(
                    "Definition file not found: {}",
                    definition_file.display()
                ));
            }
            let text = fs::read_to_string(&definition_file)
                .map_err(|err| format!("failed to read {}: {err}", definition_file.display()))?;
            let base_dir = definition_file
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| context.cwd.clone());
            (text, base_dir)
        } else {
            (value.unwrap_or("").to_string(), context.cwd.clone())
        };

        let mut defn: Value = serde_json::from_str(json_text.trim_start_matches('\u{feff}'))
            .map_err(|err| format!("failed to parse SKD JSON: {err}"))?;
        {
            let Some(data_sets) = defn.get_mut("dataSets").and_then(Value::as_array_mut) else {
                return Err("JSON must have at least one entry in 'dataSets'".to_string());
            };
            if data_sets.is_empty() {
                return Err("JSON must have at least one entry in 'dataSets'".to_string());
            }
            for (index, data_set) in data_sets.iter_mut().enumerate() {
                if data_set
                    .get("name")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .is_none()
                {
                    if let Some(object) = data_set.as_object_mut() {
                        object.insert(
                            "name".to_string(),
                            Value::String(format!("НаборДанных{}", index + 1)),
                        );
                    }
                }
            }
        }

        let content = skd_compile_xml(&defn, &query_base_dir, &context.cwd)?;
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
        }
        write_utf8_bom(&output_path, &content)?;

        let empty_data_sets = Vec::new();
        let data_sets = defn
            .get("dataSets")
            .and_then(Value::as_array)
            .unwrap_or(&empty_data_sets);
        let ds_count = data_sets.len();
        let field_count = data_sets
            .iter()
            .map(|data_set| {
                data_set
                    .get("fields")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0)
            })
            .sum::<usize>();
        let calc_count = defn
            .get("calculatedFields")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0);
        let total_count = defn
            .get("totalFields")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0);
        let param_count = defn
            .get("parameters")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0);
        let variant_count = defn
            .get("settingsVariants")
            .and_then(Value::as_array)
            .filter(|items| !items.is_empty())
            .map(Vec::len)
            .unwrap_or(1);
        let file_size = fs::metadata(&output_path)
            .map_err(|err| format!("failed to stat {}: {err}", output_path.display()))?
            .len();
        let stdout = format!(
            "OK  {output_path_label}\n    DataSets: {ds_count}  Fields: {field_count}  Calculated: {calc_count}  Totals: {total_count}  Params: {param_count}  Variants: {variant_count}\n    Size: {file_size} bytes\n"
        );
        Ok((stdout, output_path))
    })();

    match write_result {
        Ok((stdout, output_path)) => AdapterOutcome {
            ok: true,
            summary: "unica.skd.compile completed with native DCS compiler".to_string(),
            changes: vec![format!("created {}", output_path.display())],
            warnings: Vec::new(),
            errors: Vec::new(),
            artifacts: vec![output_path.display().to_string()],
            stdout: Some(stdout),
            stderr: None,
            command: None,
        },
        Err(error) => AdapterOutcome {
            ok: false,
            summary: "unica.skd.compile failed in native DCS compiler".to_string(),
            changes: Vec::new(),
            warnings: Vec::new(),
            errors: vec![error.clone()],
            artifacts: Vec::new(),
            stdout: None,
            stderr: Some(format!("{error}\n")),
            command: None,
        },
    }
}

pub(crate) fn skd_compile_xml(
    defn: &Value,
    query_base_dir: &Path,
    cwd: &Path,
) -> Result<String, String> {
    let data_sources = skd_compile_data_sources(defn);
    let default_source = data_sources
        .first()
        .map(|source| source.0.clone())
        .unwrap_or_else(|| "ИсточникДанных1".to_string());
    let mut lines = vec![
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>".to_string(),
        "<DataCompositionSchema xmlns=\"http://v8.1c.ru/8.1/data-composition-system/schema\" xmlns:dcscom=\"http://v8.1c.ru/8.1/data-composition-system/common\" xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xmlns:dcsset=\"http://v8.1c.ru/8.1/data-composition-system/settings\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">".to_string(),
    ];

    for (name, source_type) in &data_sources {
        lines.push("\t<dataSource>".to_string());
        lines.push(format!("\t\t<name>{}</name>", escape_xml(name)));
        lines.push(format!(
            "\t\t<dataSourceType>{}</dataSourceType>",
            escape_xml(source_type)
        ));
        lines.push("\t</dataSource>".to_string());
    }

    if let Some(data_sets) = defn.get("dataSets").and_then(Value::as_array) {
        for data_set in data_sets {
            skd_compile_emit_data_set(
                &mut lines,
                data_set,
                "\t",
                &default_source,
                query_base_dir,
                cwd,
            )?;
        }
    }

    skd_compile_emit_default_settings_variant(&mut lines);
    lines.push("</DataCompositionSchema>".to_string());
    Ok(format!("{}\n", lines.join("\n")))
}

pub(crate) fn skd_compile_data_sources(defn: &Value) -> Vec<(String, String)> {
    if let Some(items) = defn.get("dataSources").and_then(Value::as_array) {
        let mut result = Vec::new();
        for item in items {
            let name = json_string_field(item, "name").unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            let source_type =
                json_string_field(item, "type").unwrap_or_else(|| "Local".to_string());
            result.push((name, source_type));
        }
        if !result.is_empty() {
            return result;
        }
    }
    vec![("ИсточникДанных1".to_string(), "Local".to_string())]
}

pub(crate) fn skd_compile_emit_data_set(
    lines: &mut Vec<String>,
    data_set: &Value,
    indent: &str,
    default_source: &str,
    query_base_dir: &Path,
    cwd: &Path,
) -> Result<(), String> {
    let ds_type = if data_set.get("items").is_some() {
        "DataSetUnion"
    } else if data_set.get("objectName").is_some() {
        "DataSetObject"
    } else {
        "DataSetQuery"
    };
    lines.push(format!("{indent}<dataSet xsi:type=\"{ds_type}\">"));
    lines.push(format!(
        "{indent}\t<name>{}</name>",
        escape_xml(&json_string_field(data_set, "name").unwrap_or_default())
    ));
    if let Some(fields) = data_set.get("fields").and_then(Value::as_array) {
        for field in fields {
            skd_compile_emit_field(lines, field, &format!("{indent}\t"));
        }
    }
    if ds_type != "DataSetUnion" {
        let source =
            json_string_field(data_set, "source").unwrap_or_else(|| default_source.to_string());
        lines.push(format!(
            "{indent}\t<dataSource>{}</dataSource>",
            escape_xml(&source)
        ));
    }
    match ds_type {
        "DataSetQuery" => {
            let query = json_string_field(data_set, "query").unwrap_or_default();
            let query = skd_compile_resolve_query_value(&query, query_base_dir, cwd)?;
            lines.push(format!("{indent}\t<query>{}</query>", escape_xml(&query)));
            if data_set
                .get("autoFillFields")
                .and_then(Value::as_bool)
                .is_some_and(|value| !value)
            {
                lines.push(format!("{indent}\t<autoFillFields>false</autoFillFields>"));
            }
        }
        "DataSetObject" => {
            let object_name = json_string_field(data_set, "objectName").unwrap_or_default();
            lines.push(format!(
                "{indent}\t<objectName>{}</objectName>",
                escape_xml(&object_name)
            ));
        }
        "DataSetUnion" => {
            if let Some(items) = data_set.get("items").and_then(Value::as_array) {
                for item in items {
                    skd_compile_emit_data_set(
                        lines,
                        item,
                        &format!("{indent}\t"),
                        default_source,
                        query_base_dir,
                        cwd,
                    )?;
                }
            }
        }
        _ => {}
    }
    lines.push(format!("{indent}</dataSet>"));
    Ok(())
}

pub(crate) fn skd_compile_emit_field(lines: &mut Vec<String>, field: &Value, indent: &str) {
    let (data_path, field_name, title, field_type) = if let Some(text) = field.as_str() {
        let parsed = skd_compile_parse_field_shorthand(text);
        (
            parsed.0.clone(),
            parsed.1,
            String::new(),
            skd_compile_resolve_type(&parsed.2),
        )
    } else {
        let data_path = json_string_field(field, "dataPath")
            .or_else(|| json_string_field(field, "field"))
            .unwrap_or_default();
        let field_name = json_string_field(field, "field").unwrap_or_else(|| data_path.clone());
        let title = json_string_field(field, "title").unwrap_or_default();
        let field_type = field
            .get("type")
            .map(skd_compile_type_value)
            .unwrap_or_default();
        (data_path, field_name, title, field_type)
    };

    lines.push(format!("{indent}<field xsi:type=\"DataSetFieldField\">"));
    lines.push(format!(
        "{indent}\t<dataPath>{}</dataPath>",
        escape_xml(&data_path)
    ));
    lines.push(format!(
        "{indent}\t<field>{}</field>",
        escape_xml(&field_name)
    ));
    if !title.is_empty() {
        skd_compile_emit_mltext(lines, &format!("{indent}\t"), "title", &title);
    }
    if !field_type.is_empty() {
        lines.push(format!("{indent}\t<valueType>"));
        skd_compile_emit_value_type(lines, &field_type, &format!("{indent}\t\t"));
        lines.push(format!("{indent}\t</valueType>"));
    }
    lines.push(format!("{indent}</field>"));
}

pub(crate) fn skd_compile_parse_field_shorthand(text: &str) -> (String, String, String) {
    let value = text
        .split_whitespace()
        .filter(|part| !part.starts_with('@') && !part.starts_with('#'))
        .collect::<Vec<_>>()
        .join(" ");
    let value = value.trim();
    if let Some((left, right)) = value.split_once(':') {
        let data_path = left.trim().to_string();
        (
            data_path.clone(),
            data_path,
            skd_compile_resolve_type(right.trim()),
        )
    } else {
        (value.to_string(), value.to_string(), String::new())
    }
}

pub(crate) fn skd_compile_type_value(value: &Value) -> String {
    if let Some(items) = value.as_array() {
        return items
            .iter()
            .map(skd_compile_type_value)
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>()
            .join("|");
    }
    json_value_to_python_string(value)
        .split('|')
        .map(str::trim)
        .map(skd_compile_resolve_type)
        .collect::<Vec<_>>()
        .join("|")
}

pub(crate) fn skd_compile_resolve_type(type_str: &str) -> String {
    if type_str.is_empty() {
        return String::new();
    }
    if let Some(open) = type_str.find('(') {
        if type_str.ends_with(')') {
            let base = type_str[..open].trim();
            let params = &type_str[open + 1..type_str.len() - 1];
            if let Some(resolved) = skd_compile_type_synonym(base) {
                return format!("{resolved}({params})");
            }
        }
    }
    if let Some(dot_idx) = type_str.find('.') {
        let prefix = &type_str[..dot_idx];
        if let Some(resolved) = skd_compile_type_synonym(prefix) {
            return format!("{resolved}{}", &type_str[dot_idx..]);
        }
    }
    skd_compile_type_synonym(type_str)
        .unwrap_or(type_str)
        .to_string()
}

pub(crate) fn skd_compile_type_synonym(type_str: &str) -> Option<&'static str> {
    match type_str.to_lowercase().as_str() {
        "число" | "int" | "integer" | "number" | "num" => Some("decimal"),
        "bool" => Some("boolean"),
        "строка" | "str" => Some("string"),
        "булево" => Some("boolean"),
        "дата" => Some("date"),
        "датавремя" => Some("dateTime"),
        "стандартныйпериод" => Some("StandardPeriod"),
        "справочникссылка" => Some("CatalogRef"),
        "документссылка" => Some("DocumentRef"),
        "перечислениессылка" => Some("EnumRef"),
        "плансчетовссылка" => Some("ChartOfAccountsRef"),
        "планвидовхарактеристикссылка" => {
            Some("ChartOfCharacteristicTypesRef")
        }
        _ => None,
    }
}

pub(crate) fn skd_compile_emit_value_type(lines: &mut Vec<String>, type_spec: &str, indent: &str) {
    for part in type_spec
        .split('|')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        skd_compile_emit_single_value_type(lines, part, indent);
    }
}

pub(crate) fn skd_compile_emit_single_value_type(
    lines: &mut Vec<String>,
    type_str: &str,
    indent: &str,
) {
    let type_str = skd_compile_resolve_type(type_str);
    if type_str == "boolean" {
        lines.push(format!("{indent}<v8:Type>xs:boolean</v8:Type>"));
        return;
    }
    if type_str == "StandardPeriod" {
        lines.push(format!("{indent}<v8:Type>v8:StandardPeriod</v8:Type>"));
        return;
    }
    if let Some(length) = skd_compile_string_length(&type_str) {
        lines.push(format!("{indent}<v8:Type>xs:string</v8:Type>"));
        lines.push(format!("{indent}<v8:StringQualifiers>"));
        lines.push(format!("{indent}\t<v8:Length>{length}</v8:Length>"));
        lines.push(format!(
            "{indent}\t<v8:AllowedLength>Variable</v8:AllowedLength>"
        ));
        lines.push(format!("{indent}</v8:StringQualifiers>"));
        return;
    }
    if let Some((digits, fraction, sign)) = skd_compile_decimal_qualifiers(&type_str) {
        lines.push(format!("{indent}<v8:Type>xs:decimal</v8:Type>"));
        lines.push(format!("{indent}<v8:NumberQualifiers>"));
        lines.push(format!("{indent}\t<v8:Digits>{digits}</v8:Digits>"));
        lines.push(format!(
            "{indent}\t<v8:FractionDigits>{fraction}</v8:FractionDigits>"
        ));
        lines.push(format!("{indent}\t<v8:AllowedSign>{sign}</v8:AllowedSign>"));
        lines.push(format!("{indent}</v8:NumberQualifiers>"));
        return;
    }
    if matches!(type_str.as_str(), "date" | "dateTime") {
        let fractions = if type_str == "date" {
            "Date"
        } else {
            "DateTime"
        };
        lines.push(format!("{indent}<v8:Type>xs:dateTime</v8:Type>"));
        lines.push(format!("{indent}<v8:DateQualifiers>"));
        lines.push(format!(
            "{indent}\t<v8:DateFractions>{fractions}</v8:DateFractions>"
        ));
        lines.push(format!("{indent}</v8:DateQualifiers>"));
        return;
    }
    if type_str.contains('.') {
        lines.push(format!(
            "{indent}<v8:Type xmlns:d5p1=\"http://v8.1c.ru/8.1/data/enterprise/current-config\">d5p1:{}</v8:Type>",
            escape_xml(&type_str)
        ));
    } else {
        lines.push(format!(
            "{indent}<v8:Type>{}</v8:Type>",
            escape_xml(&type_str)
        ));
    }
}

pub(crate) fn skd_compile_string_length(type_str: &str) -> Option<&str> {
    if type_str == "string" {
        return Some("0");
    }
    let rest = type_str.strip_prefix("string(")?.strip_suffix(')')?;
    (!rest.is_empty() && rest.chars().all(|ch| ch.is_ascii_digit())).then_some(rest)
}

pub(crate) fn skd_compile_decimal_qualifiers(type_str: &str) -> Option<(&str, &str, &'static str)> {
    if type_str == "decimal" {
        return Some(("10", "2", "Any"));
    }
    let rest = type_str.strip_prefix("decimal(")?.strip_suffix(')')?;
    let parts = rest.split(',').map(str::trim).collect::<Vec<_>>();
    if parts.is_empty() || parts[0].is_empty() {
        return None;
    }
    let fraction = parts
        .get(1)
        .copied()
        .filter(|value| !value.is_empty())
        .unwrap_or("0");
    let sign = if parts
        .iter()
        .any(|part| matches!(*part, "nonneg" | "nonnegative"))
    {
        "Nonnegative"
    } else {
        "Any"
    };
    Some((parts[0], fraction, sign))
}

pub(crate) fn skd_compile_emit_mltext(
    lines: &mut Vec<String>,
    indent: &str,
    tag: &str,
    text: &str,
) {
    if text.is_empty() {
        lines.push(format!("{indent}<{tag}/>"));
        return;
    }
    lines.push(format!("{indent}<{tag} xsi:type=\"v8:LocalStringType\">"));
    lines.push(format!("{indent}\t<v8:item>"));
    lines.push(format!("{indent}\t\t<v8:lang>ru</v8:lang>"));
    lines.push(format!(
        "{indent}\t\t<v8:content>{}</v8:content>",
        escape_xml(text)
    ));
    lines.push(format!("{indent}\t</v8:item>"));
    lines.push(format!("{indent}</{tag}>"));
}

pub(crate) fn skd_compile_emit_default_settings_variant(lines: &mut Vec<String>) {
    lines.push("\t<settingsVariant>".to_string());
    lines.push("\t\t<dcsset:name>Основной</dcsset:name>".to_string());
    skd_compile_emit_mltext(lines, "\t\t", "dcsset:presentation", "Основной");
    lines.push("\t\t<dcsset:settings xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\">".to_string());
    lines.push("\t\t\t<dcsset:selection>".to_string());
    lines.push("\t\t\t\t<dcsset:item xsi:type=\"dcsset:SelectedItemAuto\"/>".to_string());
    lines.push("\t\t\t</dcsset:selection>".to_string());
    lines.push("\t\t\t<dcsset:item xsi:type=\"dcsset:StructureItemGroup\">".to_string());
    lines.push("\t\t\t\t<dcsset:order>".to_string());
    lines.push("\t\t\t\t\t<dcsset:item xsi:type=\"dcsset:OrderItemAuto\"/>".to_string());
    lines.push("\t\t\t\t</dcsset:order>".to_string());
    lines.push("\t\t\t\t<dcsset:selection>".to_string());
    lines.push("\t\t\t\t\t<dcsset:item xsi:type=\"dcsset:SelectedItemAuto\"/>".to_string());
    lines.push("\t\t\t\t</dcsset:selection>".to_string());
    lines.push("\t\t\t</dcsset:item>".to_string());
    lines.push("\t\t</dcsset:settings>".to_string());
    lines.push("\t</settingsVariant>".to_string());
}

pub(crate) fn skd_compile_resolve_query_value(
    value: &str,
    base_dir: &Path,
    cwd: &Path,
) -> Result<String, String> {
    let Some(file_path) = value.strip_prefix('@') else {
        return Ok(value.to_string());
    };
    let raw = PathBuf::from(file_path);
    let candidates = if raw.is_absolute() {
        vec![raw]
    } else {
        vec![base_dir.join(file_path), cwd.join(file_path)]
    };
    for candidate in &candidates {
        if candidate.exists() {
            let text = fs::read_to_string(candidate)
                .map_err(|err| format!("failed to read {}: {err}", candidate.display()))?;
            return Ok(text.trim_end().to_string());
        }
    }
    Err(format!(
        "Query file not found: {file_path} (searched: {})",
        candidates
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

pub(crate) fn edit_skd(args: &Map<String, Value>, context: &WorkspaceContext) -> AdapterOutcome {
    let edit_result = (|| -> Result<(String, PathBuf, bool), String> {
        let template_path = resolve_skd_validate_path(args, context)?;
        let operation = required_string(args, &["operation", "Operation"], "Operation")?;
        let value_arg = required_string(args, &["value", "Value"], "Value")?;
        let data_set = string_arg(args, &["dataSet", "DataSet"]).unwrap_or("");
        let variant = string_arg(args, &["variant", "Variant"]).unwrap_or("");
        let no_selection = bool_arg(args, &["noSelection", "NoSelection"]);

        let mut xml_text = fs::read_to_string(&template_path)
            .map_err(|err| format!("failed to read {}: {err}", template_path.display()))?;
        if xml_text.starts_with('\u{feff}') {
            xml_text = xml_text.trim_start_matches('\u{feff}').to_string();
        }
        Document::parse(&xml_text).map_err(|err| format!("[ERROR] XML parse error: {err}"))?;

        let original_xml_text = xml_text.clone();
        let base_dir = template_path.parent().unwrap_or(context.cwd.as_path());
        let values = skd_edit_split_values(operation, value_arg);
        let mut stdout = String::new();
        for value in values {
            match operation {
                "add-field" => skd_edit_add_field(
                    &mut xml_text,
                    data_set,
                    variant,
                    &value,
                    no_selection,
                    &mut stdout,
                )?,
                "add-total" => skd_edit_add_top_level(
                    &mut xml_text,
                    "totalField",
                    "dataPath",
                    &value,
                    &mut stdout,
                    skd_edit_total_fragment,
                )?,
                "add-calculated-field" => {
                    let parsed = skd_edit_parse_calc_field(&value);
                    skd_edit_add_top_level_fragment(
                        &mut xml_text,
                        "calculatedField",
                        "dataPath",
                        &parsed.data_path,
                        &skd_edit_calc_field_fragment(&parsed, "\t"),
                        &format!(
                            "[OK] CalculatedField \"{}\" = {} added\n",
                            parsed.data_path, parsed.expression
                        ),
                        &mut stdout,
                    )?;
                    if !no_selection {
                        let fragment = skd_edit_selection_fragment(&parsed.data_path, "\t\t\t");
                        if skd_edit_insert_prefixed_item(
                            &mut xml_text,
                            variant,
                            "dcsset:selection",
                            &fragment,
                        )
                        .is_ok()
                        {
                            stdout.push_str(&format!(
                                "[OK] Field \"{}\" added to selection of variant \"{}\"\n",
                                parsed.data_path,
                                skd_edit_variant_name(&xml_text, variant)
                                    .unwrap_or_else(|| variant.to_string())
                            ));
                        }
                    }
                }
                "add-parameter" => {
                    let parsed = skd_edit_parse_parameter(&value);
                    skd_edit_add_top_level_fragment(
                        &mut xml_text,
                        "parameter",
                        "name",
                        &parsed.name,
                        &skd_edit_parameter_fragment(&parsed, "\t"),
                        &format!("[OK] Parameter \"{}\" added\n", parsed.name),
                        &mut stdout,
                    )?;
                    if parsed.auto_dates {
                        for suffix in ["ДатаНачала", "ДатаОкончания"] {
                            let auto = SkdEditParameter {
                                name: suffix.to_string(),
                                title: if suffix == "ДатаНачала" {
                                    "Начало периода".to_string()
                                } else {
                                    "Конец периода".to_string()
                                },
                                type_name: "dateTime".to_string(),
                                value: Some("0001-01-01T00:00:00".to_string()),
                                hidden: true,
                                always: false,
                                value_list_allowed: false,
                                available_values: Vec::new(),
                                auto_dates: false,
                                expression: Some(format!("&{}.{}", parsed.name, suffix)),
                            };
                            let _ = skd_edit_add_top_level_fragment(
                                &mut xml_text,
                                "parameter",
                                "name",
                                &auto.name,
                                &skd_edit_parameter_fragment(&auto, "\t"),
                                "",
                                &mut String::new(),
                            );
                        }
                        stdout.push_str(
                            "[OK] Auto-parameters \"ДатаНачала\", \"ДатаОкончания\" added\n",
                        );
                    }
                }
                "add-filter" => {
                    let parsed = skd_edit_parse_filter(&value);
                    let fragment = skd_edit_filter_fragment(&parsed, "\t\t\t");
                    skd_edit_insert_or_create_settings_item(
                        &mut xml_text,
                        variant,
                        "dcsset:filter",
                        &fragment,
                    )?;
                    stdout.push_str(&format!(
                        "[OK] Filter \"{} {}\" added to variant \"{}\"\n",
                        parsed.field,
                        parsed.operator,
                        skd_edit_variant_name(&xml_text, variant)
                            .unwrap_or_else(|| variant.to_string())
                    ));
                }
                "add-dataParameter" => {
                    let parsed = skd_edit_parse_data_parameter(&value);
                    let fragment = skd_edit_data_parameter_fragment(&parsed, "\t\t\t");
                    skd_edit_insert_or_create_settings_item(
                        &mut xml_text,
                        variant,
                        "dcsset:dataParameters",
                        &fragment,
                    )?;
                    stdout.push_str(&format!(
                        "[OK] DataParameter \"{}\" added to variant \"{}\"\n",
                        parsed.parameter,
                        skd_edit_variant_name(&xml_text, variant)
                            .unwrap_or_else(|| variant.to_string())
                    ));
                }
                "set-query" => {
                    let query = skd_compile_resolve_query_value(&value, base_dir, &context.cwd)?;
                    skd_edit_set_query(&mut xml_text, data_set, &query)?;
                    stdout.push_str(&format!(
                        "[OK] Query replaced in dataset \"{}\"\n",
                        skd_edit_dataset_name(&xml_text, data_set)
                            .unwrap_or_else(|| data_set.to_string())
                    ));
                }
                "patch-query" => {
                    let (value, once) = skd_edit_extract_once_marker(&value);
                    let Some((old, new)) = value.split_once(" => ") else {
                        return Err(
                            "patch-query value must contain ' => ' separator: old => new"
                                .to_string(),
                        );
                    };
                    let count = skd_edit_patch_query(&mut xml_text, data_set, old, new, once)?;
                    let suffix = if once {
                        " (1 occurrence)".to_string()
                    } else {
                        format!(" ({count} occurrence(s))")
                    };
                    stdout.push_str(&format!(
                        "[OK] Query patched in dataset \"{}\": replaced '{}'{}\n",
                        skd_edit_dataset_name(&xml_text, data_set)
                            .unwrap_or_else(|| data_set.to_string()),
                        old,
                        suffix
                    ));
                }
                "clear-selection" => {
                    skd_edit_clear_prefixed_container(&mut xml_text, variant, "dcsset:selection")?;
                    stdout.push_str(&format!(
                        "[OK] Selection cleared in variant \"{}\"\n",
                        skd_edit_variant_name(&xml_text, variant)
                            .unwrap_or_else(|| variant.to_string())
                    ));
                }
                "clear-order" => {
                    skd_edit_clear_prefixed_container(&mut xml_text, variant, "dcsset:order")?;
                    stdout.push_str(&format!(
                        "[OK] Order cleared in variant \"{}\"\n",
                        skd_edit_variant_name(&xml_text, variant)
                            .unwrap_or_else(|| variant.to_string())
                    ));
                }
                "clear-filter" => {
                    skd_edit_clear_prefixed_container(&mut xml_text, variant, "dcsset:filter")?;
                    stdout.push_str(&format!(
                        "[OK] Filter cleared in variant \"{}\"\n",
                        skd_edit_variant_name(&xml_text, variant)
                            .unwrap_or_else(|| variant.to_string())
                    ));
                }
                "clear-conditionalAppearance" => {
                    skd_edit_clear_prefixed_container(
                        &mut xml_text,
                        variant,
                        "dcsset:conditionalAppearance",
                    )?;
                    stdout.push_str(&format!(
                        "[OK] ConditionalAppearance cleared in variant \"{}\"\n",
                        skd_edit_variant_name(&xml_text, variant)
                            .unwrap_or_else(|| variant.to_string())
                    ));
                }
                "add-selection" => {
                    let fragment = skd_edit_selection_fragment(&value, "\t\t\t\t");
                    skd_edit_insert_prefixed_item(
                        &mut xml_text,
                        variant,
                        "dcsset:selection",
                        &fragment,
                    )?;
                    stdout.push_str(&format!(
                        "[OK] Selection \"{}\" added to variant \"{}\"\n",
                        value,
                        skd_edit_variant_name(&xml_text, variant)
                            .unwrap_or_else(|| variant.to_string())
                    ));
                }
                "add-order" => {
                    let fragment = skd_edit_order_fragment(&value, "\t\t\t\t");
                    skd_edit_insert_prefixed_item(
                        &mut xml_text,
                        variant,
                        "dcsset:order",
                        &fragment,
                    )?;
                    stdout.push_str(&format!(
                        "[OK] Order \"{}\" added to variant \"{}\"\n",
                        value,
                        skd_edit_variant_name(&xml_text, variant)
                            .unwrap_or_else(|| variant.to_string())
                    ));
                }
                "add-dataSetLink" => {
                    let parsed = skd_edit_parse_data_set_link(&value)?;
                    let fragment = skd_edit_data_set_link_fragment(&parsed, "\t");
                    skd_edit_insert_before_root_close(&mut xml_text, &fragment)?;
                    stdout.push_str(&format!(
                        "[OK] DataSetLink \"{} > {}\" added\n",
                        parsed.source, parsed.dest
                    ));
                }
                "add-dataSet" => {
                    let parsed = skd_edit_parse_data_set(&value, base_dir, &context.cwd)?;
                    if skd_edit_top_level_contains(&xml_text, "dataSet", "name", &parsed.name) {
                        stdout.push_str(&format!(
                            "[WARN] DataSet \"{}\" already exists -- skipped\n",
                            parsed.name
                        ));
                    } else {
                        let source = skd_edit_first_data_source(&xml_text)
                            .unwrap_or_else(|| "ИсточникДанных1".to_string());
                        let fragment = skd_edit_data_set_fragment(&parsed, &source, "\t");
                        skd_edit_insert_before_first_root_child(
                            &mut xml_text,
                            &[
                                "dataSetLink",
                                "calculatedField",
                                "totalField",
                                "parameter",
                                "settingsVariant",
                            ],
                            &fragment,
                        )?;
                        stdout.push_str(&format!(
                            "[OK] DataSet \"{}\" added (dataSource={source})\n",
                            parsed.name
                        ));
                    }
                }
                "add-variant" => {
                    let parsed = skd_edit_parse_variant(&value);
                    if skd_edit_variant_exists(&xml_text, &parsed.name) {
                        stdout.push_str(&format!(
                            "[WARN] Variant \"{}\" already exists -- skipped\n",
                            parsed.name
                        ));
                    } else {
                        let fragment = skd_edit_variant_fragment(&parsed, "\t");
                        skd_edit_insert_before_root_close(&mut xml_text, &fragment)?;
                        stdout.push_str(&format!(
                            "[OK] Variant \"{}\" [\"{}\"] added\n",
                            parsed.name, parsed.presentation
                        ));
                    }
                }
                "add-conditionalAppearance" => {
                    let parsed = skd_edit_parse_conditional_appearance(&value);
                    let fragment = skd_edit_conditional_appearance_fragment(&parsed, "\t\t\t");
                    skd_edit_insert_or_create_settings_item(
                        &mut xml_text,
                        variant,
                        "dcsset:conditionalAppearance",
                        &fragment,
                    )?;
                    stdout.push_str(&format!(
                        "[OK] ConditionalAppearance \"{}\" added to variant \"{}\"\n",
                        parsed.parameter,
                        skd_edit_variant_name(&xml_text, variant)
                            .unwrap_or_else(|| variant.to_string())
                    ));
                }
                "add-drilldown" => {
                    let changed = skd_edit_add_drilldown(&mut xml_text, &value);
                    if changed {
                        stdout.push_str(&format!("[OK] DrillDown added for \"{}\"\n", value));
                    } else {
                        stdout.push_str(&format!(
                            "[WARN] Expression \"{}\" not found in templates -- skipped\n",
                            value
                        ));
                    }
                }
                "set-outputParameter" => {
                    let parsed = skd_edit_parse_output_parameter(&value)?;
                    let fragment = skd_edit_output_parameter_fragment(&parsed, "\t\t\t");
                    let full_range = (0, xml_text.len());
                    let _ = skd_edit_remove_item_by_child(
                        &mut xml_text,
                        full_range,
                        "dcsset:item",
                        "dcscor:parameter",
                        &parsed.key,
                    );
                    skd_edit_insert_or_create_settings_item(
                        &mut xml_text,
                        variant,
                        "dcsset:outputParameters",
                        &fragment,
                    )?;
                    stdout.push_str(&format!(
                        "[OK] OutputParameter \"{}\" added to variant \"{}\"\n",
                        parsed.key,
                        skd_edit_variant_name(&xml_text, variant)
                            .unwrap_or_else(|| variant.to_string())
                    ));
                }
                "set-structure" => {
                    let parsed = skd_edit_parse_structure(&value);
                    let fragments = skd_edit_structure_fragments(&parsed, "\t\t\t");
                    skd_edit_replace_structure(&mut xml_text, variant, &fragments)?;
                    stdout.push_str(&format!(
                        "[OK] Structure set in variant \"{}\": {}\n",
                        skd_edit_variant_name(&xml_text, variant)
                            .unwrap_or_else(|| variant.to_string()),
                        value
                    ));
                }
                "modify-structure" => {
                    let parsed = skd_edit_parse_structure(&value);
                    skd_edit_modify_structure(&mut xml_text, variant, &parsed, &mut stdout)?;
                }
                "remove-field" => {
                    let removed = skd_edit_remove_dataset_item(
                        &mut xml_text,
                        data_set,
                        "field",
                        "dataPath",
                        &value,
                    )?;
                    if removed {
                        stdout.push_str(&format!(
                            "[OK] Field \"{}\" removed from dataset \"{}\"\n",
                            value,
                            skd_edit_dataset_name(&xml_text, data_set)
                                .unwrap_or_else(|| data_set.to_string())
                        ));
                    } else {
                        stdout.push_str(&format!(
                            "[WARN] Field \"{}\" not found in dataset \"{}\"\n",
                            value,
                            skd_edit_dataset_name(&xml_text, data_set)
                                .unwrap_or_else(|| data_set.to_string())
                        ));
                    }
                    let _ = skd_edit_remove_prefixed_selection_field(&mut xml_text, &value);
                }
                "remove-parameter" => {
                    let removed =
                        skd_edit_remove_top_level_item(&mut xml_text, "parameter", "name", &value)?;
                    if removed {
                        stdout.push_str(&format!("[OK] Parameter \"{}\" removed\n", value));
                    } else {
                        stdout.push_str(&format!("[WARN] Parameter \"{}\" not found\n", value));
                    }
                }
                "modify-field" => {
                    let parsed = skd_edit_parse_field(&value);
                    let removed = skd_edit_remove_dataset_item(
                        &mut xml_text,
                        data_set,
                        "field",
                        "dataPath",
                        &parsed.data_path,
                    )?;
                    if removed {
                        let mut lines = Vec::new();
                        skd_edit_emit_field(&mut lines, &parsed, "\t\t");
                        let range = skd_edit_dataset_range(&xml_text, data_set)?;
                        skd_edit_insert_before_dataset_close(
                            &mut xml_text,
                            range,
                            &lines.join("\n"),
                        )?;
                        stdout.push_str(&format!(
                            "[OK] Field \"{}\" modified in dataset \"{}\"\n",
                            parsed.data_path,
                            skd_edit_dataset_name(&xml_text, data_set)
                                .unwrap_or_else(|| data_set.to_string())
                        ));
                    } else {
                        stdout.push_str(&format!(
                            "[WARN] Field \"{}\" not found in dataset \"{}\"\n",
                            parsed.data_path,
                            skd_edit_dataset_name(&xml_text, data_set)
                                .unwrap_or_else(|| data_set.to_string())
                        ));
                    }
                }
                "set-field-role" => {
                    skd_edit_set_field_role(&mut xml_text, data_set, &value, &mut stdout)?;
                }
                "modify-filter" => {
                    let parsed = skd_edit_parse_filter(&value);
                    let full_range = (0, xml_text.len());
                    let _ = skd_edit_remove_item_by_child(
                        &mut xml_text,
                        full_range,
                        "dcsset:item",
                        "dcsset:leftValue",
                        &parsed.field,
                    );
                    let fragment = skd_edit_filter_fragment(&parsed, "\t\t\t");
                    skd_edit_insert_or_create_settings_item(
                        &mut xml_text,
                        variant,
                        "dcsset:filter",
                        &fragment,
                    )?;
                    stdout.push_str(&format!(
                        "[OK] Filter \"{}\" modified in variant \"{}\"\n",
                        parsed.field,
                        skd_edit_variant_name(&xml_text, variant)
                            .unwrap_or_else(|| variant.to_string())
                    ));
                }
                "modify-dataParameter" => {
                    let parsed = skd_edit_parse_data_parameter(&value);
                    let full_range = (0, xml_text.len());
                    let _ = skd_edit_remove_item_by_child(
                        &mut xml_text,
                        full_range,
                        "dcsset:item",
                        "dcscor:parameter",
                        &parsed.parameter,
                    );
                    let fragment = skd_edit_data_parameter_fragment(&parsed, "\t\t\t");
                    skd_edit_insert_or_create_settings_item(
                        &mut xml_text,
                        variant,
                        "dcsset:dataParameters",
                        &fragment,
                    )?;
                    stdout.push_str(&format!(
                        "[OK] DataParameter \"{}\" modified in variant \"{}\"\n",
                        parsed.parameter,
                        skd_edit_variant_name(&xml_text, variant)
                            .unwrap_or_else(|| variant.to_string())
                    ));
                }
                "modify-parameter" => {
                    let parsed = skd_edit_parse_parameter_patch(&value);
                    skd_edit_modify_parameter(&mut xml_text, &parsed, &mut stdout)?;
                }
                "rename-parameter" => {
                    skd_edit_rename_parameter(&mut xml_text, &value, &mut stdout)?;
                }
                "reorder-parameters" => {
                    skd_edit_reorder_parameters(&mut xml_text, &value, &mut stdout)?;
                }
                "remove-total" => {
                    let removed = skd_edit_remove_top_level_item(
                        &mut xml_text,
                        "totalField",
                        "dataPath",
                        &value,
                    )?;
                    if removed {
                        stdout.push_str(&format!("[OK] TotalField \"{}\" removed\n", value));
                    } else {
                        stdout.push_str(&format!("[WARN] TotalField \"{}\" not found\n", value));
                    }
                }
                "remove-calculated-field" => {
                    let removed = skd_edit_remove_top_level_item(
                        &mut xml_text,
                        "calculatedField",
                        "dataPath",
                        &value,
                    )?;
                    if removed {
                        stdout.push_str(&format!("[OK] CalculatedField \"{}\" removed\n", value));
                    } else {
                        stdout
                            .push_str(&format!("[WARN] CalculatedField \"{}\" not found\n", value));
                    }
                    let _ = skd_edit_remove_prefixed_selection_field(&mut xml_text, &value);
                }
                "remove-filter" => {
                    let full_range = (0, xml_text.len());
                    let removed = skd_edit_remove_item_by_child(
                        &mut xml_text,
                        full_range,
                        "dcsset:item",
                        "dcsset:leftValue",
                        &value,
                    )?;
                    if removed {
                        stdout.push_str(&format!(
                            "[OK] Filter for \"{}\" removed from variant \"{}\"\n",
                            value,
                            skd_edit_variant_name(&xml_text, variant)
                                .unwrap_or_else(|| variant.to_string())
                        ));
                    } else {
                        stdout.push_str(&format!(
                            "[WARN] Filter for \"{}\" not found in variant \"{}\"\n",
                            value,
                            skd_edit_variant_name(&xml_text, variant)
                                .unwrap_or_else(|| variant.to_string())
                        ));
                    }
                }
                other => {
                    return Err(format!(
                        "native skd-edit does not support Operation '{other}' yet"
                    ));
                }
            }
        }

        let changed = xml_text != original_xml_text;
        if changed {
            write_utf8_bom(&template_path, &xml_text)?;
            stdout.push_str(&format!("[OK] Saved {}\n", template_path.display()));
        } else {
            stdout.push_str("[INFO] No changes -- file untouched\n");
        }
        Ok((stdout, template_path, changed))
    })();

    match edit_result {
        Ok((stdout, template_path, changed)) => AdapterOutcome {
            ok: true,
            summary: "unica.skd.edit completed with native DCS editor".to_string(),
            changes: if changed {
                vec![format!("updated {}", template_path.display())]
            } else {
                Vec::new()
            },
            warnings: Vec::new(),
            errors: Vec::new(),
            artifacts: vec![template_path.display().to_string()],
            stdout: Some(stdout),
            stderr: None,
            command: None,
        },
        Err(error) => AdapterOutcome {
            ok: false,
            summary: "unica.skd.edit failed in native DCS editor".to_string(),
            changes: Vec::new(),
            warnings: Vec::new(),
            errors: vec![error.clone()],
            artifacts: Vec::new(),
            stdout: None,
            stderr: Some(format!("{error}\n")),
            command: None,
        },
    }
}

pub(crate) fn skd_edit_split_values(operation: &str, value: &str) -> Vec<String> {
    if matches!(
        operation,
        "set-query" | "set-structure" | "modify-structure" | "add-dataSet"
    ) {
        return vec![value.to_string()];
    }
    if operation == "add-drilldown" && !value.contains(";;") {
        return value
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(ToOwned::to_owned)
            .collect();
    }
    value
        .split(";;")
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub(crate) fn skd_edit_add_field(
    xml_text: &mut String,
    data_set: &str,
    variant: &str,
    value: &str,
    no_selection: bool,
    stdout: &mut String,
) -> Result<(), String> {
    let parsed = skd_edit_parse_field(value);
    let range = skd_edit_dataset_range(xml_text, data_set)?;
    let escaped_data_path = escape_xml(&parsed.data_path);
    let duplicate_probe = format!("<dataPath>{escaped_data_path}</dataPath>");
    let dataset_text = &xml_text[range.0..range.1];
    let data_set_name =
        skd_edit_dataset_name(xml_text, data_set).unwrap_or_else(|| data_set.to_string());
    if dataset_text.contains(&duplicate_probe) {
        stdout.push_str(&format!(
            "[WARN] Field \"{}\" already exists in dataset \"{}\" -- skipped\n",
            parsed.data_path, data_set_name
        ));
        return Ok(());
    }

    let mut lines = Vec::new();
    skd_edit_emit_field(&mut lines, &parsed, "\t\t");
    skd_edit_insert_before_dataset_close(xml_text, range, &lines.join("\n"))?;
    stdout.push_str(&format!(
        "[OK] Field \"{}\" added to dataset \"{}\"\n",
        parsed.data_path, data_set_name
    ));

    if !no_selection {
        let fragment = skd_edit_selection_fragment(&parsed.data_path, "\t\t\t");
        if skd_edit_prefixed_container_contains_field(
            xml_text,
            variant,
            "dcsset:selection",
            &parsed.data_path,
        ) {
            stdout.push_str(&format!(
                "[INFO] Field \"{}\" already in selection -- skipped\n",
                parsed.data_path
            ));
        } else if skd_edit_insert_prefixed_item(xml_text, variant, "dcsset:selection", &fragment)
            .is_ok()
        {
            stdout.push_str(&format!(
                "[OK] Field \"{}\" added to selection of variant \"{}\"\n",
                parsed.data_path,
                skd_edit_variant_name(xml_text, variant).unwrap_or_else(|| variant.to_string())
            ));
        }
    }
    Ok(())
}

pub(crate) fn skd_edit_add_top_level(
    xml_text: &mut String,
    item: &str,
    child: &str,
    value: &str,
    stdout: &mut String,
    build: fn(&str, &str) -> String,
) -> Result<(), String> {
    let (key, expression) = value
        .split_once(':')
        .map(|(left, right)| (left.trim(), right.trim()))
        .unwrap_or((value.trim(), ""));
    skd_edit_add_top_level_fragment(
        xml_text,
        item,
        child,
        key,
        &build(key, expression),
        &format!("[OK] {} \"{}\" added\n", item, key),
        stdout,
    )
}

pub(crate) fn skd_edit_add_top_level_fragment(
    xml_text: &mut String,
    item: &str,
    child: &str,
    key: &str,
    fragment: &str,
    ok_message: &str,
    stdout: &mut String,
) -> Result<(), String> {
    if skd_edit_top_level_contains(xml_text, item, child, key) {
        stdout.push_str(&format!(
            "[WARN] {} \"{}\" already exists -- skipped\n",
            item, key
        ));
        return Ok(());
    }
    skd_edit_insert_before_first_root_child(
        xml_text,
        &[
            "totalField",
            "calculatedField",
            "parameter",
            "template",
            "groupTemplate",
            "settingsVariant",
        ],
        fragment,
    )?;
    stdout.push_str(ok_message);
    Ok(())
}

pub(crate) fn skd_edit_top_level_contains(
    xml_text: &str,
    item: &str,
    child: &str,
    key: &str,
) -> bool {
    let child_probe = format!("<{child}>{}</{child}>", escape_xml(key));
    let mut cursor = 0;
    let open_prefix = format!("<{item}");
    let close = format!("</{item}>");
    while let Some(open_rel) = xml_text[cursor..].find(&open_prefix) {
        let start = cursor + open_rel;
        let Some(close_rel) = xml_text[start..].find(&close) else {
            return false;
        };
        let end = start + close_rel + close.len();
        if xml_text[start..end].contains(&child_probe) {
            return true;
        }
        cursor = end;
    }
    false
}

pub(crate) fn skd_edit_insert_before_root_close(
    xml_text: &mut String,
    fragment: &str,
) -> Result<(), String> {
    let Some(pos) = xml_text.rfind("</DataCompositionSchema>") else {
        return Err("No closing </DataCompositionSchema> found".to_string());
    };
    xml_text.insert_str(pos, &format!("{fragment}\n"));
    Ok(())
}

pub(crate) fn skd_edit_insert_before_first_root_child(
    xml_text: &mut String,
    before: &[&str],
    fragment: &str,
) -> Result<(), String> {
    let mut insert_pos = None;
    for tag in before {
        let needle = format!("\n\t<{tag}");
        if let Some(pos) = xml_text.find(&needle) {
            insert_pos = Some(insert_pos.map_or(pos + 1, |current: usize| current.min(pos + 1)));
        }
    }
    if let Some(pos) = insert_pos {
        xml_text.insert_str(pos, &format!("{fragment}\n"));
        Ok(())
    } else {
        skd_edit_insert_before_root_close(xml_text, fragment)
    }
}

pub(crate) fn skd_edit_total_fragment(data_path: &str, expression: &str) -> String {
    let expression = if expression.is_empty() {
        format!("Сумма({data_path})")
    } else {
        expression.to_string()
    };
    format!(
        "\t<totalField>\n\t\t<dataPath>{}</dataPath>\n\t\t<expression>{}</expression>\n\t</totalField>",
        escape_xml(data_path),
        escape_xml(&expression)
    )
}

pub(crate) struct SkdEditCalcField {
    pub(crate) data_path: String,
    pub(crate) title: String,
    pub(crate) field_type: String,
    pub(crate) expression: String,
}

pub(crate) fn skd_edit_parse_calc_field(value: &str) -> SkdEditCalcField {
    let (left, expression) = value
        .split_once('=')
        .map(|(left, right)| (left.trim(), right.trim()))
        .unwrap_or((value.trim(), ""));
    let (mut name_type, title) = skd_edit_extract_bracket_title(left);
    name_type = skd_edit_strip_markers(&name_type);
    let (data_path, field_type) = name_type
        .split_once(':')
        .map(|(name, type_name)| {
            (
                name.trim().to_string(),
                skd_compile_resolve_type(type_name.trim()),
            )
        })
        .unwrap_or((name_type.trim().to_string(), String::new()));
    SkdEditCalcField {
        data_path,
        title,
        field_type,
        expression: expression.to_string(),
    }
}

pub(crate) fn skd_edit_calc_field_fragment(field: &SkdEditCalcField, indent: &str) -> String {
    let mut lines = vec![
        format!("{indent}<calculatedField>"),
        format!(
            "{indent}\t<dataPath>{}</dataPath>",
            escape_xml(&field.data_path)
        ),
    ];
    if !field.title.is_empty() {
        skd_compile_emit_mltext(&mut lines, &format!("{indent}\t"), "title", &field.title);
    }
    if !field.field_type.is_empty() {
        lines.push(format!("{indent}\t<valueType>"));
        skd_compile_emit_value_type(&mut lines, &field.field_type, &format!("{indent}\t\t"));
        lines.push(format!("{indent}\t</valueType>"));
    }
    lines.push(format!(
        "{indent}\t<expression>{}</expression>",
        escape_xml(&field.expression)
    ));
    lines.push(format!("{indent}</calculatedField>"));
    lines.join("\n")
}

pub(crate) struct SkdEditParameter {
    pub(crate) name: String,
    pub(crate) title: String,
    pub(crate) type_name: String,
    pub(crate) value: Option<String>,
    pub(crate) hidden: bool,
    pub(crate) always: bool,
    pub(crate) value_list_allowed: bool,
    pub(crate) available_values: Vec<(String, String)>,
    pub(crate) auto_dates: bool,
    pub(crate) expression: Option<String>,
}

pub(crate) fn skd_edit_parse_parameter(value: &str) -> SkdEditParameter {
    let auto_dates = value.contains("@autoDates");
    let hidden = value.contains("@hidden");
    let always = value.contains("@always");
    let value_list_allowed = value.contains("@valueList");
    let available_values = skd_edit_extract_available_values(value);
    let cleaned = value
        .split("availableValue=")
        .next()
        .unwrap_or(value)
        .replace("@autoDates", "")
        .replace("@hidden", "")
        .replace("@always", "")
        .replace("@valueList", "");
    let (left, val) = cleaned
        .split_once('=')
        .map(|(left, right)| (left.trim(), Some(right.trim().to_string())))
        .unwrap_or((cleaned.trim(), None));
    let (mut name_type, title) = skd_edit_extract_bracket_title(left);
    name_type = skd_edit_strip_markers(&name_type);
    let (name, type_name) = name_type
        .split_once(':')
        .map(|(name, type_name)| {
            (
                name.trim().to_string(),
                skd_compile_resolve_type(type_name.trim()),
            )
        })
        .unwrap_or((name_type.trim().to_string(), String::new()));
    SkdEditParameter {
        name,
        title,
        type_name,
        value: val,
        hidden,
        always,
        value_list_allowed,
        available_values,
        auto_dates,
        expression: None,
    }
}

pub(crate) fn skd_edit_parameter_fragment(param: &SkdEditParameter, indent: &str) -> String {
    let mut lines = vec![
        format!("{indent}<parameter>"),
        format!("{indent}\t<name>{}</name>", escape_xml(&param.name)),
    ];
    if !param.title.is_empty() {
        skd_compile_emit_mltext(&mut lines, &format!("{indent}\t"), "title", &param.title);
    }
    if !param.type_name.is_empty() {
        lines.push(format!("{indent}\t<valueType>"));
        skd_compile_emit_value_type(&mut lines, &param.type_name, &format!("{indent}\t\t"));
        lines.push(format!("{indent}\t</valueType>"));
    }
    if let Some(value) = &param.value {
        lines.push(format!("{indent}\t<value>{}</value>", escape_xml(value)));
    }
    if param.value_list_allowed {
        lines.push(format!(
            "{indent}\t<valueListAllowed>true</valueListAllowed>"
        ));
    }
    if param.hidden {
        lines.push(format!("{indent}\t<useRestriction>true</useRestriction>"));
    }
    if param.always {
        lines.push(format!("{indent}\t<use>Always</use>"));
    }
    if let Some(expression) = &param.expression {
        lines.push(format!(
            "{indent}\t<expression>{}</expression>",
            escape_xml(expression)
        ));
    }
    if !param.available_values.is_empty() {
        lines.push(format!("{indent}\t<availableValues>"));
        for (value, presentation) in &param.available_values {
            lines.push(format!("{indent}\t\t<item>"));
            lines.push(format!(
                "{indent}\t\t\t<value>{}</value>",
                escape_xml(value)
            ));
            if !presentation.is_empty() {
                lines.push(format!(
                    "{indent}\t\t\t<presentation>{}</presentation>",
                    escape_xml(presentation)
                ));
            }
            lines.push(format!("{indent}\t\t</item>"));
        }
        lines.push(format!("{indent}\t</availableValues>"));
    }
    lines.push(format!("{indent}</parameter>"));
    lines.join("\n")
}

pub(crate) struct SkdEditFilter {
    pub(crate) field: String,
    pub(crate) operator: String,
    pub(crate) value: String,
    pub(crate) use_flag: Option<bool>,
}

pub(crate) fn skd_edit_parse_filter(value: &str) -> SkdEditFilter {
    let use_flag = if value.contains("@off") {
        Some(false)
    } else if value.contains("@on") {
        Some(true)
    } else {
        None
    };
    let cleaned = value
        .replace("@off", "")
        .replace("@on", "")
        .replace("@user", "")
        .replace("@quickAccess", "")
        .replace("@normal", "")
        .replace("@inaccessible", "");
    let parts = cleaned.split_whitespace().collect::<Vec<_>>();
    let field = parts.first().copied().unwrap_or("").to_string();
    let operator = parts.get(1).copied().unwrap_or("=").to_string();
    let filter_value = if parts.len() > 2 {
        parts[2..].join(" ")
    } else {
        String::new()
    };
    SkdEditFilter {
        field,
        operator,
        value: filter_value,
        use_flag,
    }
}

pub(crate) fn skd_edit_filter_fragment(filter: &SkdEditFilter, indent: &str) -> String {
    let mut lines = vec![
        format!("{indent}<dcsset:item>"),
        format!(
            "{indent}\t<dcsset:leftValue>{}</dcsset:leftValue>",
            escape_xml(&filter.field)
        ),
        format!(
            "{indent}\t<dcsset:comparisonType>{}</dcsset:comparisonType>",
            escape_xml(&filter.operator)
        ),
    ];
    if !filter.value.is_empty() {
        lines.push(format!(
            "{indent}\t<dcsset:rightValue>{}</dcsset:rightValue>",
            escape_xml(&filter.value)
        ));
    }
    if let Some(false) = filter.use_flag {
        lines.push(format!("{indent}\t<dcsset:use>false</dcsset:use>"));
    }
    lines.push(format!("{indent}</dcsset:item>"));
    lines.join("\n")
}

pub(crate) struct SkdEditDataParameter {
    pub(crate) parameter: String,
    pub(crate) value: Option<String>,
    pub(crate) use_flag: Option<bool>,
}

pub(crate) fn skd_edit_parse_data_parameter(value: &str) -> SkdEditDataParameter {
    let use_flag = if value.contains("@off") {
        Some(false)
    } else if value.contains("@on") {
        Some(true)
    } else {
        None
    };
    let cleaned = value
        .replace("@off", "")
        .replace("@on", "")
        .replace("@user", "");
    let (parameter, val) = cleaned
        .split_once('=')
        .map(|(left, right)| (left.trim().to_string(), Some(right.trim().to_string())))
        .unwrap_or((cleaned.trim().to_string(), None));
    SkdEditDataParameter {
        parameter,
        value: val,
        use_flag,
    }
}

pub(crate) fn skd_edit_data_parameter_fragment(
    param: &SkdEditDataParameter,
    indent: &str,
) -> String {
    let mut lines = vec![
        format!("{indent}<dcsset:item>"),
        format!(
            "{indent}\t<dcscor:parameter>{}</dcscor:parameter>",
            escape_xml(&param.parameter)
        ),
    ];
    if let Some(value) = &param.value {
        lines.push(format!(
            "{indent}\t<dcscor:value>{}</dcscor:value>",
            escape_xml(value)
        ));
    }
    if let Some(false) = param.use_flag {
        lines.push(format!("{indent}\t<dcscor:use>false</dcscor:use>"));
    }
    lines.push(format!("{indent}</dcsset:item>"));
    lines.join("\n")
}

pub(crate) fn skd_edit_insert_or_create_settings_item(
    xml_text: &mut String,
    variant: &str,
    container: &str,
    fragment: &str,
) -> Result<(), String> {
    match skd_edit_insert_prefixed_item(xml_text, variant, container, fragment) {
        Ok(()) => Ok(()),
        Err(_) => {
            let settings = skd_edit_settings_element_range(xml_text, variant)?;
            let settings_pos = settings.1 - "</dcsset:settings>".len();
            xml_text.insert_str(
                settings_pos,
                &format!("\t\t\t<{container}>\n{fragment}\n\t\t\t</{container}>\n"),
            );
            Ok(())
        }
    }
}

pub(crate) struct SkdEditDataSetLink {
    pub(crate) source: String,
    pub(crate) dest: String,
    pub(crate) source_expr: String,
    pub(crate) dest_expr: String,
    pub(crate) parameter: String,
}

pub(crate) fn skd_edit_parse_data_set_link(value: &str) -> Result<SkdEditDataSetLink, String> {
    let (source, rest) = value
        .split_once('>')
        .ok_or_else(|| "dataSetLink value must contain '>'".to_string())?;
    let (dest, rest) = rest
        .split_once(" on ")
        .ok_or_else(|| "dataSetLink value must contain ' on '".to_string())?;
    let (expr, parameter) = if let Some((expr, param)) = rest.split_once("[param ") {
        (expr.trim(), param.trim_end_matches(']').trim().to_string())
    } else {
        (rest.trim(), String::new())
    };
    let (source_expr, dest_expr) = expr
        .split_once('=')
        .ok_or_else(|| "dataSetLink expression must contain '='".to_string())?;
    Ok(SkdEditDataSetLink {
        source: source.trim().to_string(),
        dest: dest.trim().to_string(),
        source_expr: source_expr.trim().to_string(),
        dest_expr: dest_expr.trim().to_string(),
        parameter,
    })
}

pub(crate) fn skd_edit_data_set_link_fragment(link: &SkdEditDataSetLink, indent: &str) -> String {
    let mut lines = vec![
        format!("{indent}<dataSetLink>"),
        format!(
            "{indent}\t<sourceDataSet>{}</sourceDataSet>",
            escape_xml(&link.source)
        ),
        format!(
            "{indent}\t<destinationDataSet>{}</destinationDataSet>",
            escape_xml(&link.dest)
        ),
        format!(
            "{indent}\t<sourceExpression>{}</sourceExpression>",
            escape_xml(&link.source_expr)
        ),
        format!(
            "{indent}\t<destinationExpression>{}</destinationExpression>",
            escape_xml(&link.dest_expr)
        ),
    ];
    if !link.parameter.is_empty() {
        lines.push(format!(
            "{indent}\t<parameter>{}</parameter>",
            escape_xml(&link.parameter)
        ));
    }
    lines.push(format!("{indent}</dataSetLink>"));
    lines.join("\n")
}

pub(crate) struct SkdEditDataSet {
    pub(crate) name: String,
    pub(crate) query: String,
}

pub(crate) fn skd_edit_parse_data_set(
    value: &str,
    base_dir: &Path,
    cwd: &Path,
) -> Result<SkdEditDataSet, String> {
    let (name, query) = if let Some((left, right)) = value.split_once(':') {
        (left.trim().to_string(), right.trim())
    } else {
        ("НаборДанных".to_string(), value.trim())
    };
    let query = skd_compile_resolve_query_value(query, base_dir, cwd)?;
    Ok(SkdEditDataSet { name, query })
}

pub(crate) fn skd_edit_first_data_source(xml_text: &str) -> Option<String> {
    let start = xml_text.find("<dataSource>")?;
    let end = xml_text[start..].find("</dataSource>")? + start;
    let name = skd_edit_child_text_range(xml_text, (start, end), "name").ok()?;
    Some(xml_text[name].trim().to_string())
}

pub(crate) fn skd_edit_data_set_fragment(
    data_set: &SkdEditDataSet,
    source: &str,
    indent: &str,
) -> String {
    format!(
        "{indent}<dataSet xsi:type=\"DataSetQuery\">\n{indent}\t<name>{}</name>\n{indent}\t<dataSource>{}</dataSource>\n{indent}\t<query>{}</query>\n{indent}</dataSet>",
        escape_xml(&data_set.name),
        escape_xml(source),
        escape_xml(&data_set.query)
    )
}

pub(crate) struct SkdEditVariant {
    pub(crate) name: String,
    pub(crate) presentation: String,
}

pub(crate) fn skd_edit_parse_variant(value: &str) -> SkdEditVariant {
    let (name, presentation) = skd_edit_extract_bracket_title(value);
    let name = name.trim().to_string();
    let presentation = if presentation.is_empty() {
        name.clone()
    } else {
        presentation
    };
    SkdEditVariant { name, presentation }
}

pub(crate) fn skd_edit_variant_exists(xml_text: &str, name: &str) -> bool {
    xml_text.contains(&format!("<dcsset:name>{}</dcsset:name>", escape_xml(name)))
        || xml_text.contains(&format!("<name>{}</name>", escape_xml(name)))
}

pub(crate) fn skd_edit_variant_fragment(variant: &SkdEditVariant, indent: &str) -> String {
    let mut lines = vec![
        format!("{indent}<settingsVariant>"),
        format!(
            "{indent}\t<dcsset:name>{}</dcsset:name>",
            escape_xml(&variant.name)
        ),
    ];
    skd_compile_emit_mltext(
        &mut lines,
        &format!("{indent}\t"),
        "dcsset:presentation",
        &variant.presentation,
    );
    lines.push(format!("{indent}\t<dcsset:settings>"));
    lines.push(format!("{indent}\t\t<dcsset:selection>"));
    lines.push(format!(
        "{indent}\t\t\t<dcsset:item xsi:type=\"dcsset:SelectedItemAuto\"/>"
    ));
    lines.push(format!("{indent}\t\t</dcsset:selection>"));
    lines.push(format!("{indent}\t</dcsset:settings>"));
    lines.push(format!("{indent}</settingsVariant>"));
    lines.join("\n")
}

pub(crate) struct SkdEditConditionalAppearance {
    pub(crate) parameter: String,
    pub(crate) value: String,
    pub(crate) fields: Vec<String>,
    pub(crate) condition: String,
}

pub(crate) fn skd_edit_parse_conditional_appearance(value: &str) -> SkdEditConditionalAppearance {
    let (head, fields) = if let Some((left, right)) = value.split_once(" for ") {
        (
            left.trim(),
            right
                .split(',')
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect(),
        )
    } else {
        (value.trim(), Vec::new())
    };
    let (head, condition) = if let Some((left, right)) = head.split_once(" when ") {
        (left.trim(), right.trim().to_string())
    } else {
        (head, String::new())
    };
    let (parameter, val) = head
        .split_once('=')
        .map(|(left, right)| (left.trim().to_string(), right.trim().to_string()))
        .unwrap_or((head.to_string(), String::new()));
    SkdEditConditionalAppearance {
        parameter,
        value: val,
        fields,
        condition,
    }
}

pub(crate) fn skd_edit_conditional_appearance_fragment(
    item: &SkdEditConditionalAppearance,
    indent: &str,
) -> String {
    let mut lines = vec![
        format!("{indent}<dcsset:item>"),
        format!(
            "{indent}\t<dcsset:parameter>{}</dcsset:parameter>",
            escape_xml(&item.parameter)
        ),
        format!(
            "{indent}\t<dcsset:value>{}</dcsset:value>",
            escape_xml(&item.value)
        ),
    ];
    if !item.condition.is_empty() {
        lines.push(format!(
            "{indent}\t<dcsset:condition>{}</dcsset:condition>",
            escape_xml(&item.condition)
        ));
    }
    for field in &item.fields {
        lines.push(format!(
            "{indent}\t<dcsset:field>{}</dcsset:field>",
            escape_xml(field)
        ));
    }
    lines.push(format!("{indent}</dcsset:item>"));
    lines.join("\n")
}

pub(crate) struct SkdEditOutputParameter {
    pub(crate) key: String,
    pub(crate) value: String,
}

pub(crate) fn skd_edit_parse_output_parameter(
    value: &str,
) -> Result<SkdEditOutputParameter, String> {
    let (key, val) = value
        .split_once('=')
        .ok_or_else(|| "outputParameter value must contain '='".to_string())?;
    Ok(SkdEditOutputParameter {
        key: key.trim().to_string(),
        value: val.trim().to_string(),
    })
}

pub(crate) fn skd_edit_output_parameter_fragment(
    item: &SkdEditOutputParameter,
    indent: &str,
) -> String {
    format!(
        "{indent}<dcsset:item>\n{indent}\t<dcscor:parameter>{}</dcscor:parameter>\n{indent}\t<dcscor:value>{}</dcscor:value>\n{indent}</dcsset:item>",
        escape_xml(&item.key),
        escape_xml(&item.value)
    )
}

#[derive(Clone, Debug)]
pub(crate) struct SkdEditStructureItem {
    pub(crate) name: Option<String>,
    pub(crate) group_by: Vec<String>,
    pub(crate) children: Vec<SkdEditStructureItem>,
}

pub(crate) fn skd_edit_parse_structure(value: &str) -> Vec<SkdEditStructureItem> {
    let segments = value
        .split('>')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let mut innermost = None;
    for segment in segments.into_iter().rev() {
        let (segment, name) = skd_edit_extract_structure_name(segment);
        let group_by = if segment.eq_ignore_ascii_case("details") || segment == "детали" {
            Vec::new()
        } else {
            segment
                .split(',')
                .map(str::trim)
                .filter(|field| !field.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        };
        let children = innermost.into_iter().collect::<Vec<_>>();
        innermost = Some(SkdEditStructureItem {
            name,
            group_by,
            children,
        });
    }
    innermost.into_iter().collect()
}

pub(crate) fn skd_edit_extract_structure_name(segment: &str) -> (String, Option<String>) {
    let Some(marker) = segment.find("@name=") else {
        return (segment.trim().to_string(), None);
    };
    let before = segment[..marker].trim_end();
    let after = &segment[marker + "@name=".len()..];
    let (name, consumed) = if let Some(rest) = after.strip_prefix('"') {
        match rest.find('"') {
            Some(end) => (rest[..end].to_string(), end + 2),
            None => (rest.to_string(), after.len()),
        }
    } else if let Some(rest) = after.strip_prefix('\'') {
        match rest.find('\'') {
            Some(end) => (rest[..end].to_string(), end + 2),
            None => (rest.to_string(), after.len()),
        }
    } else {
        let end = after.find(char::is_whitespace).unwrap_or(after.len());
        (after[..end].to_string(), end)
    };
    let rest = after[consumed..].trim_start();
    let mut cleaned = before.to_string();
    if !rest.is_empty() {
        if !cleaned.is_empty() {
            cleaned.push(' ');
        }
        cleaned.push_str(rest);
    }
    (cleaned.trim().to_string(), Some(name.trim().to_string()))
}

pub(crate) fn skd_edit_structure_fragments(
    structures: &[SkdEditStructureItem],
    indent: &str,
) -> String {
    structures
        .iter()
        .map(|item| skd_edit_structure_item_fragment(item, indent))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn skd_edit_structure_item_fragment(
    item: &SkdEditStructureItem,
    indent: &str,
) -> String {
    let mut lines = vec![format!(
        "{indent}<dcsset:item xsi:type=\"dcsset:StructureItemGroup\">"
    )];
    if let Some(name) = &item.name {
        lines.push(format!(
            "{indent}\t<dcsset:name>{}</dcsset:name>",
            escape_xml(name)
        ));
    }
    if item.group_by.is_empty() {
        lines.push(format!("{indent}\t<dcsset:groupItems/>"));
    } else {
        lines.push(format!("{indent}\t<dcsset:groupItems>"));
        for field in &item.group_by {
            lines.push(skd_edit_group_item_field_fragment(
                field,
                &format!("{indent}\t\t"),
            ));
        }
        lines.push(format!("{indent}\t</dcsset:groupItems>"));
    }
    lines.push(format!("{indent}\t<dcsset:order>"));
    lines.push(format!(
        "{indent}\t\t<dcsset:item xsi:type=\"dcsset:OrderItemAuto\"/>"
    ));
    lines.push(format!("{indent}\t</dcsset:order>"));
    lines.push(format!("{indent}\t<dcsset:selection>"));
    lines.push(format!(
        "{indent}\t\t<dcsset:item xsi:type=\"dcsset:SelectedItemAuto\"/>"
    ));
    lines.push(format!("{indent}\t</dcsset:selection>"));
    for child in &item.children {
        lines.push(skd_edit_structure_item_fragment(
            child,
            &format!("{indent}\t"),
        ));
    }
    lines.push(format!("{indent}</dcsset:item>"));
    lines.join("\n")
}

pub(crate) fn skd_edit_group_item_field_fragment(field: &str, indent: &str) -> String {
    [
        format!("{indent}<dcsset:item xsi:type=\"dcsset:GroupItemField\">"),
        format!(
            "{indent}\t<dcsset:field>{}</dcsset:field>",
            escape_xml(field)
        ),
        format!("{indent}\t<dcsset:groupType>Items</dcsset:groupType>"),
        format!("{indent}\t<dcsset:periodAdditionType>None</dcsset:periodAdditionType>"),
        format!(
            "{indent}\t<dcsset:periodAdditionBegin xsi:type=\"xs:dateTime\">0001-01-01T00:00:00</dcsset:periodAdditionBegin>"
        ),
        format!(
            "{indent}\t<dcsset:periodAdditionEnd xsi:type=\"xs:dateTime\">0001-01-01T00:00:00</dcsset:periodAdditionEnd>"
        ),
        format!("{indent}</dcsset:item>"),
    ]
    .join("\n")
}

pub(crate) fn skd_edit_replace_structure(
    xml_text: &mut String,
    variant: &str,
    fragment: &str,
) -> Result<(), String> {
    loop {
        let settings = skd_edit_settings_content_range(xml_text, variant)?;
        let Some(open_rel) = xml_text[settings.0..settings.1]
            .find("<dcsset:item xsi:type=\"dcsset:StructureItemGroup\"")
        else {
            break;
        };
        let start = settings.0 + open_rel;
        let Some(end) = skd_edit_matching_dcsset_item_end(xml_text, start, settings.1) else {
            return Err("No closing </dcsset:item> found for structure item".to_string());
        };
        xml_text.replace_range(start..end, "");
    }
    let settings = skd_edit_settings_element_range(xml_text, variant)?;
    let insert_pos = settings.1 - "</dcsset:settings>".len();
    xml_text.insert_str(insert_pos, &format!("{fragment}\n\t\t\t"));
    Ok(())
}

pub(crate) fn skd_edit_modify_structure(
    xml_text: &mut String,
    variant: &str,
    structures: &[SkdEditStructureItem],
    stdout: &mut String,
) -> Result<(), String> {
    let mut targets = Vec::new();
    for structure in structures {
        skd_edit_collect_structure_targets(structure, &mut targets);
    }
    if targets.is_empty() {
        return Err(format!(
            "modify-structure requires @name= for at least one group: {}",
            skd_edit_structure_description(structures)
        ));
    }
    for (name, group_by) in targets {
        if skd_edit_replace_named_group_items(xml_text, variant, &name, &group_by)? {
            let desc = if group_by.is_empty() {
                "details".to_string()
            } else {
                group_by.join(", ")
            };
            stdout.push_str(&format!(
                "[OK] Group \"{}\" groupItems updated: {}\n",
                name, desc
            ));
        } else {
            stdout.push_str(&format!(
                "[WARN] Group with @name=\"{}\" not found -- skipped\n",
                name
            ));
        }
    }
    Ok(())
}

pub(crate) fn skd_edit_collect_structure_targets(
    item: &SkdEditStructureItem,
    targets: &mut Vec<(String, Vec<String>)>,
) {
    if let Some(name) = &item.name {
        targets.push((name.clone(), item.group_by.clone()));
    }
    for child in &item.children {
        skd_edit_collect_structure_targets(child, targets);
    }
}

pub(crate) fn skd_edit_structure_description(structures: &[SkdEditStructureItem]) -> String {
    structures
        .iter()
        .flat_map(|item| item.group_by.iter().cloned())
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn skd_edit_replace_named_group_items(
    xml_text: &mut String,
    variant: &str,
    name: &str,
    group_by: &[String],
) -> Result<bool, String> {
    let Some(group_range) = skd_edit_find_named_structure_group(xml_text, variant, name)? else {
        return Ok(false);
    };
    let Some(group_items) = skd_edit_find_group_items_range(xml_text, group_range)? else {
        let insert_pos = group_range.0
            + xml_text[group_range.0..group_range.1]
                .find("</dcsset:name>")
                .map(|rel| rel + "</dcsset:name>".len())
                .unwrap_or(0);
        let fragment = skd_edit_group_items_fragment(group_by, "\n\t\t\t\t");
        xml_text.insert_str(insert_pos, &fragment);
        return Ok(true);
    };
    if group_items.self_closing {
        let group_indent = skd_edit_line_indent(xml_text, group_items.start);
        let child_indent = format!("{group_indent}\t");
        let fragment = skd_edit_group_items_inner_fragment(group_by, &child_indent);
        xml_text.replace_range(
            group_items.start..group_items.end,
            &format!("<dcsset:groupItems>\n{fragment}{group_indent}</dcsset:groupItems>"),
        );
    } else {
        let group_indent = skd_edit_line_indent(xml_text, group_items.start);
        let child_indent = format!("{group_indent}\t");
        let fragment = skd_edit_group_items_inner_fragment(group_by, &child_indent);
        xml_text.replace_range(
            group_items.open_end..group_items.close_start,
            &format!("\n{fragment}{group_indent}"),
        );
    }
    Ok(true)
}

pub(crate) fn skd_edit_group_items_fragment(group_by: &[String], indent: &str) -> String {
    if group_by.is_empty() {
        return format!("{indent}<dcsset:groupItems/>");
    }
    format!(
        "{indent}<dcsset:groupItems>\n{}{indent}</dcsset:groupItems>",
        skd_edit_group_items_inner_fragment(group_by, &(indent.to_string() + "\t")),
    )
}

pub(crate) fn skd_edit_group_items_inner_fragment(group_by: &[String], indent: &str) -> String {
    group_by
        .iter()
        .map(|field| skd_edit_group_item_field_fragment(field, indent))
        .map(|fragment| format!("{fragment}\n"))
        .collect::<String>()
}

pub(crate) fn skd_edit_line_indent(xml_text: &str, pos: usize) -> String {
    let line_start = xml_text[..pos].rfind('\n').map(|idx| idx + 1).unwrap_or(0);
    xml_text[line_start..pos]
        .chars()
        .take_while(|ch| ch.is_whitespace() && *ch != '\n' && *ch != '\r')
        .collect()
}

pub(crate) fn skd_edit_find_named_structure_group(
    xml_text: &str,
    variant: &str,
    name: &str,
) -> Result<Option<(usize, usize)>, String> {
    let settings = skd_edit_settings_content_range(xml_text, variant)?;
    let name_probe = format!("<dcsset:name>{}</dcsset:name>", escape_xml(name));
    let open_probe = "<dcsset:item xsi:type=\"dcsset:StructureItemGroup\"";
    let mut cursor = settings.0;
    while let Some(name_rel) = xml_text[cursor..settings.1].find(&name_probe) {
        let name_start = cursor + name_rel;
        let Some(open_rel) = xml_text[settings.0..name_start].rfind(open_probe) else {
            cursor = name_start + name_probe.len();
            continue;
        };
        let start = settings.0 + open_rel;
        let Some(end) = skd_edit_matching_dcsset_item_end(xml_text, start, settings.1) else {
            return Err("No closing </dcsset:item> found for structure item".to_string());
        };
        if end > name_start {
            return Ok(Some((start, end)));
        }
        cursor = name_start + name_probe.len();
    }
    Ok(None)
}

pub(crate) fn skd_edit_find_group_items_range(
    xml_text: &str,
    group_range: (usize, usize),
) -> Result<Option<SkdEditElementRange>, String> {
    let Some(open_rel) = xml_text[group_range.0..group_range.1].find("<dcsset:groupItems") else {
        return Ok(None);
    };
    let start = group_range.0 + open_rel;
    let Some(open_end_rel) = xml_text[start..group_range.1].find('>') else {
        return Err("Malformed <dcsset:groupItems> element".to_string());
    };
    let open_end = start + open_end_rel + 1;
    let open_tag = &xml_text[start..open_end];
    if open_tag.trim_end().ends_with("/>") {
        return Ok(Some(SkdEditElementRange {
            start,
            open_end,
            close_start: open_end,
            end: open_end,
            self_closing: true,
        }));
    }
    let close = "</dcsset:groupItems>";
    let Some(close_rel) = xml_text[open_end..group_range.1].find(close) else {
        return Err("No </dcsset:groupItems> element found".to_string());
    };
    let close_start = open_end + close_rel;
    Ok(Some(SkdEditElementRange {
        start,
        open_end,
        close_start,
        end: close_start + close.len(),
        self_closing: false,
    }))
}

pub(crate) fn skd_edit_remove_first_block(
    xml_text: &mut String,
    range: (usize, usize),
    open_prefix: &str,
    close: &str,
) -> bool {
    let Some(open_rel) = xml_text[range.0..range.1].find(open_prefix) else {
        return false;
    };
    let start = range.0 + open_rel;
    let Some(close_rel) = xml_text[start..range.1].find(close) else {
        return false;
    };
    let end = start + close_rel + close.len();
    xml_text.replace_range(start..end, "");
    true
}

pub(crate) fn skd_edit_matching_dcsset_item_end(
    xml_text: &str,
    start: usize,
    limit: usize,
) -> Option<usize> {
    let open = "<dcsset:item";
    let close = "</dcsset:item>";
    let first_open_end = xml_text[start..limit].find('>')? + start;
    let mut depth = 1usize;
    let mut cursor = first_open_end + 1;
    while cursor < limit {
        let next_open = xml_text[cursor..limit].find(open).map(|rel| cursor + rel);
        let next_close = xml_text[cursor..limit].find(close).map(|rel| cursor + rel);
        match (next_open, next_close) {
            (Some(open_pos), Some(close_pos)) if open_pos < close_pos => {
                let open_end = xml_text[open_pos..limit].find('>')? + open_pos;
                let tag = &xml_text[open_pos..=open_end];
                if !tag.trim_end().ends_with("/>") {
                    depth += 1;
                }
                cursor = open_end + 1;
            }
            (_, Some(close_pos)) => {
                depth = depth.saturating_sub(1);
                let end = close_pos + close.len();
                if depth == 0 {
                    return Some(end);
                }
                cursor = end;
            }
            _ => return None,
        }
    }
    None
}

pub(crate) fn skd_edit_add_drilldown(xml_text: &mut String, resource: &str) -> bool {
    if !xml_text.contains(resource) {
        return false;
    }
    let marker = format!("DrillDown{}", sanitize_xml_identifier(resource));
    if xml_text.contains(&marker) {
        return false;
    }
    let fragment = format!(
        "\t<parameter>\n\t\t<name>{}</name>\n\t\t<expression>{}</expression>\n\t</parameter>",
        escape_xml(&marker),
        escape_xml(resource)
    );
    skd_edit_insert_before_root_close(xml_text, &fragment).is_ok()
}

pub(crate) fn skd_edit_set_field_role(
    xml_text: &mut String,
    data_set: &str,
    value: &str,
    stdout: &mut String,
) -> Result<(), String> {
    let mut parts = value.split_whitespace().collect::<Vec<_>>();
    if parts.is_empty() {
        stdout.push_str("[WARN] set-field-role: empty dataPath\n");
        return Ok(());
    }
    let data_path = parts.remove(0);
    let range = skd_edit_dataset_range(xml_text, data_set)?;
    let field_range = skd_edit_find_item_by_child(xml_text, range, "field", "dataPath", data_path);
    let Some(field_range) = field_range else {
        stdout.push_str(&format!("[WARN] Field \"{}\" not found\n", data_path));
        return Ok(());
    };
    let _ = skd_edit_remove_child_block(xml_text, field_range, "role");
    if !parts.is_empty() {
        let role = parts.join(" ");
        let insert = field_range.0
            + xml_text[field_range.0..field_range.1]
                .rfind("</field>")
                .unwrap_or(field_range.1 - field_range.0);
        xml_text.insert_str(
            insert,
            &format!("\t\t<role>{}</role>\n\t\t", escape_xml(&role)),
        );
        stdout.push_str(&format!(
            "[OK] Field \"{}\" role set: {}\n",
            data_path, role
        ));
    } else {
        stdout.push_str(&format!("[OK] Field \"{}\" role cleared\n", data_path));
    }
    Ok(())
}

pub(crate) struct SkdEditParameterPatch {
    pub(crate) name: String,
    pub(crate) title: String,
    pub(crate) value: Option<String>,
    pub(crate) hidden: bool,
    pub(crate) always: bool,
}

pub(crate) fn skd_edit_parse_parameter_patch(value: &str) -> SkdEditParameterPatch {
    let hidden = value.contains("@hidden");
    let always = value.contains("@always");
    let cleaned = value.replace("@hidden", "").replace("@always", "");
    let value_part = cleaned
        .split(" value=")
        .nth(1)
        .map(str::trim)
        .map(ToOwned::to_owned);
    let before_value = cleaned.split(" value=").next().unwrap_or(cleaned.as_str());
    let (name, title) = skd_edit_extract_bracket_title(before_value);
    SkdEditParameterPatch {
        name: name.split_whitespace().next().unwrap_or("").to_string(),
        title,
        value: value_part,
        hidden,
        always,
    }
}

pub(crate) fn skd_edit_modify_parameter(
    xml_text: &mut String,
    patch: &SkdEditParameterPatch,
    stdout: &mut String,
) -> Result<(), String> {
    let Some(range) = skd_edit_find_item_by_child(
        xml_text,
        (0, xml_text.len()),
        "parameter",
        "name",
        &patch.name,
    ) else {
        stdout.push_str(&format!(
            "[WARN] Parameter \"{}\" not found -- skipped\n",
            patch.name
        ));
        return Ok(());
    };
    if !patch.title.is_empty() {
        skd_edit_replace_or_insert_simple_child(xml_text, range, "title", &patch.title);
        stdout.push_str(&format!(
            "[OK] Parameter \"{}\": title set to \"{}\"\n",
            patch.name, patch.title
        ));
    }
    if let Some(value) = &patch.value {
        skd_edit_replace_or_insert_simple_child(xml_text, range, "value", value);
        stdout.push_str(&format!(
            "[OK] Parameter \"{}\": value set to {}\n",
            patch.name, value
        ));
    }
    if patch.hidden {
        skd_edit_replace_or_insert_simple_child(xml_text, range, "useRestriction", "true");
        stdout.push_str(&format!(
            "[OK] Parameter \"{}\": @hidden applied\n",
            patch.name
        ));
    }
    if patch.always {
        skd_edit_replace_or_insert_simple_child(xml_text, range, "use", "Always");
        stdout.push_str(&format!(
            "[OK] Parameter \"{}\": @always applied\n",
            patch.name
        ));
    }
    Ok(())
}

pub(crate) fn skd_edit_rename_parameter(
    xml_text: &mut String,
    value: &str,
    stdout: &mut String,
) -> Result<(), String> {
    let Some((old, new)) = value.split_once("=>") else {
        stdout.push_str(&format!(
            "[WARN] rename-parameter expects \"OldName => NewName\", got: {value}\n"
        ));
        return Ok(());
    };
    let old = old.trim();
    let new = new.trim();
    if old == new {
        stdout.push_str("[WARN] rename-parameter: old and new names are equal -- skipped\n");
        return Ok(());
    }
    if !skd_edit_top_level_contains(xml_text, "parameter", "name", old) {
        stdout.push_str(&format!(
            "[WARN] Parameter \"{}\" not found -- skipped\n",
            old
        ));
        return Ok(());
    }
    let parameter_limit = skd_edit_parameter_limit(xml_text);
    let range =
        skd_edit_find_item_by_child(xml_text, (0, parameter_limit), "parameter", "name", old)
            .ok_or_else(|| format!("Parameter \"{}\" not found", old))?;
    skd_edit_replace_child_text(xml_text, range, "name", new)?;
    let expr_updated = skd_edit_update_parameter_expression_refs(xml_text, old, new);
    let dp_updated = skd_edit_replace_exact_data_parameter_refs(xml_text, old, new);
    stdout.push_str(&format!(
        "[OK] Parameter renamed: \"{}\" => \"{}\" (expressions updated: {}, dataParameters updated: {})\n",
        old, new, expr_updated, dp_updated
    ));
    Ok(())
}

pub(crate) fn skd_edit_parameter_limit(xml_text: &str) -> usize {
    xml_text
        .find("\n\t<settingsVariant")
        .or_else(|| xml_text.find("</DataCompositionSchema>"))
        .unwrap_or(xml_text.len())
}

pub(crate) fn skd_edit_update_parameter_expression_refs(
    xml_text: &mut String,
    old: &str,
    new: &str,
) -> usize {
    let mut updated = 0usize;
    let mut cursor = 0usize;
    loop {
        let limit = skd_edit_parameter_limit(xml_text);
        if cursor >= limit {
            break;
        }
        let Some(open_rel) = xml_text[cursor..limit].find("<parameter") else {
            break;
        };
        let start = cursor + open_rel;
        let Some(end) = skd_edit_matching_element_end(xml_text, start, limit, "parameter") else {
            break;
        };
        let Ok(expr_range) = skd_edit_child_text_range(xml_text, (start, end), "expression") else {
            cursor = end;
            continue;
        };
        let current = xml_text[expr_range.clone()].to_string();
        let (replacement, count) = skd_edit_replace_parameter_tokens(&current, old, new);
        if count > 0 {
            let old_len = expr_range.end - expr_range.start;
            xml_text.replace_range(expr_range.start..expr_range.end, &replacement);
            updated += count;
            cursor = end + replacement.len() - old_len;
        } else {
            cursor = end;
        }
    }
    updated
}

pub(crate) fn skd_edit_replace_parameter_tokens(
    text: &str,
    old: &str,
    new: &str,
) -> (String, usize) {
    let needle = format!("&amp;{}", escape_xml(old));
    let replacement = format!("&amp;{}", escape_xml(new));
    let mut result = String::with_capacity(text.len());
    let mut cursor = 0usize;
    let mut count = 0usize;
    while let Some(rel) = text[cursor..].find(&needle) {
        let start = cursor + rel;
        let end = start + needle.len();
        result.push_str(&text[cursor..start]);
        let boundary = text[end..]
            .chars()
            .next()
            .is_none_or(|ch| !(ch.is_alphanumeric() || ch == '_'));
        if boundary {
            result.push_str(&replacement);
            count += 1;
        } else {
            result.push_str(&text[start..end]);
        }
        cursor = end;
    }
    result.push_str(&text[cursor..]);
    (result, count)
}

pub(crate) fn skd_edit_replace_exact_data_parameter_refs(
    xml_text: &mut String,
    old: &str,
    new: &str,
) -> usize {
    let old_tag = format!("<dcscor:parameter>{}</dcscor:parameter>", escape_xml(old));
    let new_tag = format!("<dcscor:parameter>{}</dcscor:parameter>", escape_xml(new));
    let count = xml_text.matches(&old_tag).count();
    if count > 0 {
        *xml_text = xml_text.replace(&old_tag, &new_tag);
    }
    count
}

pub(crate) fn skd_edit_reorder_parameters(
    xml_text: &mut String,
    value: &str,
    stdout: &mut String,
) -> Result<(), String> {
    let order = value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if order.is_empty() {
        stdout.push_str("[WARN] reorder-parameters: empty list -- skipped\n");
        return Ok(());
    }
    let parameter_limit = skd_edit_parameter_limit(xml_text);
    let mut blocks = skd_edit_collect_blocks_in_range(xml_text, "parameter", (0, parameter_limit));
    if blocks.is_empty() {
        stdout.push_str("[WARN] reorder-parameters: no parameters in schema\n");
        return Ok(());
    }
    let mut selected = Vec::new();
    let mut remaining = Vec::new();
    for (name, block) in blocks.drain(..) {
        if order.iter().any(|item| item == &name) {
            selected.push((name, block));
        } else {
            remaining.push((name, block));
        }
    }
    selected.sort_by_key(|(name, _)| {
        order
            .iter()
            .position(|item| item == name)
            .unwrap_or(usize::MAX)
    });
    let all = selected
        .into_iter()
        .chain(remaining)
        .map(|(_, block)| block)
        .collect::<Vec<_>>();
    let current_blocks =
        skd_edit_collect_blocks_in_range(xml_text, "parameter", (0, parameter_limit));
    let first_start = current_blocks
        .first()
        .map(|(_, block)| xml_text.find(block).unwrap_or(0))
        .ok_or_else(|| "No parameter block found".to_string())?;
    let last_end = current_blocks
        .last()
        .and_then(|(_, block)| xml_text.rfind(block).map(|start| start + block.len()))
        .ok_or_else(|| "No parameter block found".to_string())?;
    xml_text.replace_range(first_start..last_end, &all.join("\n"));
    stdout.push_str(&format!("[OK] Parameters reordered ({})\n", all.len()));
    Ok(())
}

pub(crate) fn skd_edit_collect_blocks(xml_text: &str, item: &str) -> Vec<(String, String)> {
    skd_edit_collect_blocks_in_range(xml_text, item, (0, xml_text.len()))
}

pub(crate) fn skd_edit_collect_blocks_in_range(
    xml_text: &str,
    item: &str,
    range: (usize, usize),
) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let open_prefix = format!("<{item}");
    let close = format!("</{item}>");
    let mut cursor = range.0;
    while cursor < range.1 {
        let Some(open_rel) = xml_text[cursor..range.1].find(&open_prefix) else {
            break;
        };
        let start = cursor + open_rel;
        let Some(close_rel) = xml_text[start..range.1].find(&close) else {
            break;
        };
        let end = start + close_rel + close.len();
        let block = xml_text[start..end].to_string();
        let name = skd_edit_child_text_range(xml_text, (start, end), "name")
            .map(|range| xml_text[range].trim().to_string())
            .unwrap_or_default();
        result.push((name, block));
        cursor = end;
    }
    result
}

pub(crate) fn skd_edit_find_item_by_child(
    xml_text: &str,
    range: (usize, usize),
    item: &str,
    child: &str,
    value: &str,
) -> Option<(usize, usize)> {
    let open_prefix = format!("<{item}");
    let child_probe = format!("<{child}>{}</{child}>", escape_xml(value));
    let mut cursor = range.0;
    while cursor < range.1 {
        let open_rel = xml_text[cursor..range.1].find(&open_prefix)?;
        let start = cursor + open_rel;
        let end = skd_edit_matching_element_end(xml_text, start, range.1, item)?;
        if xml_text[start..end].contains(&child_probe) {
            return Some((start, end));
        }
        cursor = end;
    }
    None
}

pub(crate) fn skd_edit_remove_child_block(
    xml_text: &mut String,
    range: (usize, usize),
    child: &str,
) -> bool {
    let open = format!("<{child}");
    let close = format!("</{child}>");
    let Some(open_rel) = xml_text[range.0..range.1].find(&open) else {
        return false;
    };
    let start = range.0 + open_rel;
    let Some(open_end_rel) = xml_text[start..range.1].find('>') else {
        return false;
    };
    let content_start = start + open_end_rel + 1;
    let Some(close_rel) = xml_text[content_start..range.1].find(&close) else {
        return false;
    };
    let end = content_start + close_rel + close.len();
    xml_text.replace_range(start..end, "");
    true
}

pub(crate) fn skd_edit_replace_or_insert_simple_child(
    xml_text: &mut String,
    range: (usize, usize),
    child: &str,
    value: &str,
) {
    if let Ok(text_range) = skd_edit_child_text_range(xml_text, range, child) {
        xml_text.replace_range(text_range, &escape_xml(value));
        return;
    }
    let close = "</parameter>";
    if let Some(close_rel) = xml_text[range.0..range.1].find(close) {
        let pos = range.0 + close_rel;
        xml_text.insert_str(
            pos,
            &format!("\t\t<{child}>{}</{child}>\n\t", escape_xml(value)),
        );
    }
}

pub(crate) fn skd_edit_extract_bracket_title(value: &str) -> (String, String) {
    let mut text = value.to_string();
    if let (Some(open), Some(close)) = (text.find('['), text.find(']')) {
        if close > open {
            let title = text[open + 1..close].trim().to_string();
            text.replace_range(open..=close, "");
            return (text.trim().to_string(), title);
        }
    }
    (text.trim().to_string(), String::new())
}

pub(crate) fn skd_edit_strip_markers(value: &str) -> String {
    value
        .split_whitespace()
        .filter(|part| !part.starts_with('@') && !part.starts_with('#'))
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn skd_edit_extract_available_values(value: &str) -> Vec<(String, String)> {
    let Some((_, tail)) = value.split_once("availableValue=") else {
        return Vec::new();
    };
    tail.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty() && !item.starts_with('@'))
        .map(|item| {
            item.split_once(':')
                .map(|(left, right)| (left.trim().to_string(), right.trim().to_string()))
                .unwrap_or((item.to_string(), String::new()))
        })
        .collect()
}

pub(crate) fn sanitize_xml_identifier(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_alphanumeric() || *ch == '_')
        .collect()
}

pub(crate) struct SkdEditField {
    pub(crate) data_path: String,
    pub(crate) field: String,
    pub(crate) title: String,
    pub(crate) field_type: String,
}

pub(crate) fn skd_edit_parse_field(value: &str) -> SkdEditField {
    let mut text = value.to_string();
    let title = if let (Some(open), Some(close)) = (text.find('['), text.find(']')) {
        if close > open {
            let title = text[open + 1..close].trim().to_string();
            text.replace_range(open..=close, "");
            title
        } else {
            String::new()
        }
    } else {
        String::new()
    };
    let text = text
        .split_whitespace()
        .filter(|part| !part.starts_with('@') && !part.starts_with('#'))
        .collect::<Vec<_>>()
        .join(" ");
    let (data_path, field_type) = if let Some((left, right)) = text.split_once(':') {
        (
            left.trim().to_string(),
            skd_compile_resolve_type(right.trim()),
        )
    } else {
        (text.trim().to_string(), String::new())
    };
    SkdEditField {
        field: data_path.clone(),
        data_path,
        title,
        field_type,
    }
}

pub(crate) fn skd_edit_emit_field(lines: &mut Vec<String>, field: &SkdEditField, indent: &str) {
    lines.push(format!("{indent}<field xsi:type=\"DataSetFieldField\">"));
    lines.push(format!(
        "{indent}\t<dataPath>{}</dataPath>",
        escape_xml(&field.data_path)
    ));
    lines.push(format!(
        "{indent}\t<field>{}</field>",
        escape_xml(&field.field)
    ));
    if !field.title.is_empty() {
        skd_compile_emit_mltext(lines, &format!("{indent}\t"), "title", &field.title);
    }
    if !field.field_type.is_empty() {
        lines.push(format!("{indent}\t<valueType>"));
        skd_compile_emit_value_type(lines, &field.field_type, &format!("{indent}\t\t"));
        lines.push(format!("{indent}\t</valueType>"));
    }
    lines.push(format!("{indent}</field>"));
}

pub(crate) fn skd_edit_set_query(
    xml_text: &mut String,
    data_set: &str,
    query: &str,
) -> Result<(), String> {
    let range = skd_edit_dataset_range(xml_text, data_set)?;
    skd_edit_replace_child_text(xml_text, range, "query", query)
}

pub(crate) fn skd_edit_extract_once_marker(value: &str) -> (String, bool) {
    let mut once = false;
    let cleaned = value
        .split_whitespace()
        .filter(|part| {
            if *part == "@once" {
                once = true;
                false
            } else {
                true
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    (cleaned, once)
}

pub(crate) fn skd_edit_patch_query(
    xml_text: &mut String,
    data_set: &str,
    old: &str,
    new: &str,
    once: bool,
) -> Result<usize, String> {
    let range = skd_edit_dataset_range(xml_text, data_set)?;
    let query_range = skd_edit_child_text_range(xml_text, range, "query")?;
    let current = &xml_text[query_range.clone()];
    let escaped_old = escape_xml(old);
    let count = current.matches(&escaped_old).count();
    if count == 0 {
        return Err(format!(
            "Substring not found in query of dataset '{}': {}",
            skd_edit_dataset_name(xml_text, data_set).unwrap_or_else(|| data_set.to_string()),
            old
        ));
    }
    if once && count != 1 {
        return Err(format!(
            "@once: expected 1 occurrence of '{}' in dataset '{}', found {}",
            old,
            skd_edit_dataset_name(xml_text, data_set).unwrap_or_else(|| data_set.to_string()),
            count
        ));
    }
    let patched = current.replace(&escaped_old, &escape_xml(new));
    xml_text.replace_range(query_range, &patched);
    Ok(count)
}

pub(crate) fn skd_edit_dataset_range(
    xml_text: &str,
    data_set: &str,
) -> Result<(usize, usize), String> {
    let mut cursor = 0;
    while let Some(rel_start) = xml_text[cursor..].find("<dataSet") {
        let start = cursor + rel_start;
        let Some(rel_end) = xml_text[start..].find("</dataSet>") else {
            return Err("No closing </dataSet> found".to_string());
        };
        let end = start + rel_end + "</dataSet>".len();
        let block = &xml_text[start..end];
        if data_set.is_empty() || block.contains(&format!("<name>{}</name>", escape_xml(data_set)))
        {
            return Ok((start, end));
        }
        cursor = end;
    }
    if data_set.is_empty() {
        Err("No dataSet found in DCS".to_string())
    } else {
        Err(format!("DataSet '{data_set}' not found"))
    }
}

pub(crate) fn skd_edit_dataset_name(xml_text: &str, data_set: &str) -> Option<String> {
    let range = skd_edit_dataset_range(xml_text, data_set).ok()?;
    let name_range = skd_edit_child_text_range(xml_text, range, "name").ok()?;
    Some(xml_text[name_range].trim().to_string())
}

pub(crate) fn skd_edit_variant_name(xml_text: &str, variant: &str) -> Option<String> {
    if !variant.is_empty() {
        return Some(variant.to_string());
    }
    let (start, end) = skd_edit_variant_range(xml_text, variant).ok()?;
    let name_range = skd_edit_prefixed_child_text_range(xml_text, (start, end), "dcsset:name")
        .or_else(|_| skd_edit_child_text_range(xml_text, (start, end), "name"))
        .ok()?;
    Some(xml_text[name_range].trim().to_string())
}

pub(crate) fn skd_edit_variant_range(
    xml_text: &str,
    variant: &str,
) -> Result<(usize, usize), String> {
    let mut cursor = 0;
    while let Some(rel_start) = xml_text[cursor..].find("<settingsVariant") {
        let start = cursor + rel_start;
        let Some(rel_end) = xml_text[start..].find("</settingsVariant>") else {
            return Err("No closing </settingsVariant> found".to_string());
        };
        let end = start + rel_end + "</settingsVariant>".len();
        if variant.is_empty() || skd_edit_variant_block_has_name(&xml_text[start..end], variant) {
            return Ok((start, end));
        }
        cursor = end;
    }
    if variant.is_empty() {
        Err("No settingsVariant found in DCS".to_string())
    } else {
        Err(format!("Variant '{variant}' not found"))
    }
}

pub(crate) fn skd_edit_variant_block_has_name(block: &str, variant: &str) -> bool {
    let escaped = escape_xml(variant);
    block.contains(&format!("<dcsset:name>{escaped}</dcsset:name>"))
        || block.contains(&format!("<name>{escaped}</name>"))
}

pub(crate) fn skd_edit_settings_element_range(
    xml_text: &str,
    variant: &str,
) -> Result<(usize, usize), String> {
    let variant_range = skd_edit_variant_range(xml_text, variant)?;
    let Some(open_rel) = xml_text[variant_range.0..variant_range.1].find("<dcsset:settings") else {
        return Err("No <dcsset:settings> found in variant".to_string());
    };
    let start = variant_range.0 + open_rel;
    let Some(close_rel) = xml_text[start..variant_range.1].find("</dcsset:settings>") else {
        return Err("No </dcsset:settings> found in variant".to_string());
    };
    let end = start + close_rel + "</dcsset:settings>".len();
    Ok((start, end))
}

pub(crate) fn skd_edit_settings_content_range(
    xml_text: &str,
    variant: &str,
) -> Result<(usize, usize), String> {
    let settings = skd_edit_settings_element_range(xml_text, variant)?;
    let Some(open_end_rel) = xml_text[settings.0..settings.1].find('>') else {
        return Err("Malformed <dcsset:settings> element".to_string());
    };
    let content_start = settings.0 + open_end_rel + 1;
    let content_end = settings.1 - "</dcsset:settings>".len();
    Ok((content_start, content_end))
}

pub(crate) fn skd_edit_insert_before_dataset_close(
    xml_text: &mut String,
    range: (usize, usize),
    fragment: &str,
) -> Result<(), String> {
    let close = "</dataSet>";
    let Some(close_rel) = xml_text[range.0..range.1].rfind(close) else {
        return Err("No closing </dataSet> found".to_string());
    };
    let pos = range.0 + close_rel;
    xml_text.insert_str(pos, &format!("{fragment}\n\t"));
    Ok(())
}

pub(crate) fn skd_edit_replace_child_text(
    xml_text: &mut String,
    range: (usize, usize),
    child: &str,
    value: &str,
) -> Result<(), String> {
    let text_range = skd_edit_child_text_range(xml_text, range, child)?;
    xml_text.replace_range(text_range, &escape_xml(value));
    Ok(())
}

pub(crate) fn skd_edit_child_text_range(
    xml_text: &str,
    range: (usize, usize),
    child: &str,
) -> Result<std::ops::Range<usize>, String> {
    let open = format!("<{child}>");
    let close = format!("</{child}>");
    let block = &xml_text[range.0..range.1];
    let Some(open_rel) = block.find(&open) else {
        return Err(format!("No <{child}> element found"));
    };
    let text_start = range.0 + open_rel + open.len();
    let Some(close_rel) = xml_text[text_start..range.1].find(&close) else {
        return Err(format!("No </{child}> element found"));
    };
    Ok(text_start..text_start + close_rel)
}

pub(crate) fn skd_edit_prefixed_child_text_range(
    xml_text: &str,
    range: (usize, usize),
    child: &str,
) -> Result<std::ops::Range<usize>, String> {
    skd_edit_child_text_range(xml_text, range, child)
}

pub(crate) fn skd_edit_selection_fragment(field_name: &str, indent: &str) -> String {
    if field_name == "Auto" {
        return format!("{indent}<dcsset:item xsi:type=\"dcsset:SelectedItemAuto\"/>");
    }
    format!(
        "{indent}<dcsset:item xsi:type=\"dcsset:SelectedItemField\">\n{indent}\t<dcsset:field>{}</dcsset:field>\n{indent}</dcsset:item>",
        escape_xml(field_name)
    )
}

pub(crate) fn skd_edit_order_fragment(value: &str, indent: &str) -> String {
    let value = value.trim();
    if value == "Auto" {
        return format!("{indent}<dcsset:item xsi:type=\"dcsset:OrderItemAuto\"/>");
    }
    let mut parts = value.split_whitespace();
    let field = parts.next().unwrap_or(value);
    let direction = if parts
        .next()
        .is_some_and(|item| item.eq_ignore_ascii_case("desc"))
    {
        "Desc"
    } else {
        "Asc"
    };
    format!(
        "{indent}<dcsset:item xsi:type=\"dcsset:OrderItemField\">\n{indent}\t<dcsset:field>{}</dcsset:field>\n{indent}\t<dcsset:orderType>{direction}</dcsset:orderType>\n{indent}</dcsset:item>",
        escape_xml(field)
    )
}

pub(crate) struct SkdEditElementRange {
    pub(crate) start: usize,
    pub(crate) open_end: usize,
    pub(crate) close_start: usize,
    pub(crate) end: usize,
    pub(crate) self_closing: bool,
}

pub(crate) fn skd_edit_prefixed_container_range(
    xml_text: &str,
    variant: &str,
    container: &str,
) -> Result<SkdEditElementRange, String> {
    let settings = skd_edit_settings_content_range(xml_text, variant)?;
    let open_prefix = format!("<{container}");
    let Some(open_rel) = xml_text[settings.0..settings.1].find(&open_prefix) else {
        return Err(format!("No <{container}> section found in DCS"));
    };
    let start = settings.0 + open_rel;
    let Some(open_end_rel) = xml_text[start..settings.1].find('>') else {
        return Err(format!("Malformed <{container}> section in DCS"));
    };
    let open_end = start + open_end_rel + 1;
    let open_tag = &xml_text[start..open_end];
    if open_tag.trim_end().ends_with("/>") {
        return Ok(SkdEditElementRange {
            start,
            open_end,
            close_start: open_end,
            end: open_end,
            self_closing: true,
        });
    }
    let close = format!("</{container}>");
    let Some(close_rel) = xml_text[open_end..settings.1].find(&close) else {
        return Err(format!("No </{container}> section found in DCS"));
    };
    let close_start = open_end + close_rel;
    Ok(SkdEditElementRange {
        start,
        open_end,
        close_start,
        end: close_start + close.len(),
        self_closing: false,
    })
}

pub(crate) fn skd_edit_insert_prefixed_item(
    xml_text: &mut String,
    variant: &str,
    container: &str,
    fragment: &str,
) -> Result<(), String> {
    let range = skd_edit_prefixed_container_range(xml_text, variant, container)?;
    if range.self_closing {
        xml_text.replace_range(
            range.start..range.end,
            &format!("<{container}>\n{fragment}\n\t\t\t</{container}>"),
        );
    } else {
        let insert_pos = xml_text[..range.close_start]
            .rfind('\n')
            .map(|idx| idx + 1)
            .unwrap_or(range.close_start);
        xml_text.insert_str(insert_pos, &format!("{fragment}\n"));
    }
    Ok(())
}

pub(crate) fn skd_edit_clear_prefixed_container(
    xml_text: &mut String,
    variant: &str,
    container: &str,
) -> Result<(), String> {
    let range = skd_edit_prefixed_container_range(xml_text, variant, container)?;
    if range.self_closing {
        return Ok(());
    }
    xml_text.replace_range(range.open_end..range.close_start, "\n\t\t\t");
    Ok(())
}

pub(crate) fn skd_edit_prefixed_container_contains_field(
    xml_text: &str,
    variant: &str,
    container: &str,
    field: &str,
) -> bool {
    let Ok(range) = skd_edit_prefixed_container_range(xml_text, variant, container) else {
        return false;
    };
    if range.self_closing {
        return false;
    }
    xml_text[range.open_end..range.close_start].contains(&format!(
        "<dcsset:field>{}</dcsset:field>",
        escape_xml(field)
    ))
}

pub(crate) fn skd_edit_remove_dataset_item(
    xml_text: &mut String,
    data_set: &str,
    item: &str,
    child: &str,
    value: &str,
) -> Result<bool, String> {
    let range = skd_edit_dataset_range(xml_text, data_set)?;
    skd_edit_remove_item_by_child(xml_text, range, item, child, value)
}

pub(crate) fn skd_edit_remove_top_level_item(
    xml_text: &mut String,
    item: &str,
    child: &str,
    value: &str,
) -> Result<bool, String> {
    skd_edit_remove_item_by_child(xml_text, (0, xml_text.len()), item, child, value)
}

pub(crate) fn skd_edit_remove_item_by_child(
    xml_text: &mut String,
    range: (usize, usize),
    item: &str,
    child: &str,
    value: &str,
) -> Result<bool, String> {
    let open_prefix = format!("<{item}");
    let child_probe = format!("<{child}>{}</{child}>", escape_xml(value));
    let mut cursor = range.0;
    while cursor < range.1 {
        let Some(open_rel) = xml_text[cursor..range.1].find(&open_prefix) else {
            return Ok(false);
        };
        let start = cursor + open_rel;
        let Some(end) = skd_edit_matching_element_end(xml_text, start, range.1, item) else {
            return Err(format!("No closing </{item}> found"));
        };
        if xml_text[start..end].contains(&child_probe) {
            xml_text.replace_range(start..end, "");
            return Ok(true);
        }
        cursor = end;
    }
    Ok(false)
}

pub(crate) fn skd_edit_matching_element_end(
    xml_text: &str,
    start: usize,
    limit: usize,
    tag: &str,
) -> Option<usize> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let first_open_end = xml_text[start..limit].find('>')? + start;
    let first_tag = &xml_text[start..=first_open_end];
    if first_tag.trim_end().ends_with("/>") {
        return Some(first_open_end + 1);
    }
    let mut depth = 1usize;
    let mut cursor = first_open_end + 1;
    while cursor < limit {
        let next_open = xml_text[cursor..limit].find(&open).map(|rel| cursor + rel);
        let next_close = xml_text[cursor..limit].find(&close).map(|rel| cursor + rel);
        match (next_open, next_close) {
            (Some(open_pos), Some(close_pos)) if open_pos < close_pos => {
                let open_end = xml_text[open_pos..limit].find('>')? + open_pos;
                let open_tag = &xml_text[open_pos..=open_end];
                if !open_tag.trim_end().ends_with("/>") {
                    depth += 1;
                }
                cursor = open_end + 1;
            }
            (_, Some(close_pos)) => {
                depth = depth.saturating_sub(1);
                let end = close_pos + close.len();
                if depth == 0 {
                    return Some(end);
                }
                cursor = end;
            }
            _ => return None,
        }
    }
    None
}

pub(crate) fn skd_edit_remove_prefixed_selection_field(
    xml_text: &mut String,
    field: &str,
) -> Result<bool, String> {
    skd_edit_remove_item_by_child(
        xml_text,
        (0, xml_text.len()),
        "dcsset:item",
        "dcsset:field",
        field,
    )
}

pub(crate) fn invoke_read(
    operation: &str,
    _tool_name: &str,
    args: &Map<String, Value>,
    context: &WorkspaceContext,
) -> Option<Result<AdapterOutcome, String>> {
    match operation {
        "skd-info" => Some(Ok(analyze_skd_info(args, context))),
        "skd-validate" => Some(Ok(validate_skd(args, context))),
        _ => None,
    }
}

pub(crate) fn invoke_mutation(
    operation: &str,
    _tool_name: &str,
    args: &Map<String, Value>,
    context: &WorkspaceContext,
) -> Option<AdapterOutcome> {
    match operation {
        "skd-compile" => Some(compile_skd(args, context)),
        "skd-edit" => Some(edit_skd(args, context)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::workspace::WorkspaceContext;
    use serde_json::{json, Map};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn native_skd_edit_accepts_documented_operations_without_script_fallback() {
        let context = temp_context("skd-edit-ops");
        let template_path = context.cwd.join("Template.xml");
        fs::write(&template_path, base_skd_xml()).unwrap();

        let operations = [
            ("add-field", "Quantity: decimal(10,0)"),
            ("add-total", "Amount: Сумма(Amount)"),
            (
                "add-calculated-field",
                "Margin: decimal(10,2) = Amount - Cost",
            ),
            (
                "add-parameter",
                "Период [Период]: StandardPeriod = LastMonth @autoDates",
            ),
            ("add-filter", "Amount > 0 @user"),
            ("add-dataParameter", "Период = LastMonth @user"),
            ("add-order", "Amount desc"),
            ("add-selection", "Quantity"),
            (
                "add-dataSetLink",
                "НаборДанных1 > Доп on Amount = Amount [param LinkParam]",
            ),
            ("add-dataSet", "Доп: ВЫБРАТЬ 1 КАК Amount"),
            ("add-variant", "Alt [Alt presentation]"),
            (
                "add-conditionalAppearance",
                "ЦветТекста = web:Red when Amount < 0 for Amount",
            ),
            ("add-drilldown", "Amount"),
            ("set-query", "ВЫБРАТЬ 2 КАК Amount"),
            ("patch-query", "2 => 3"),
            ("set-outputParameter", "Заголовок = Test"),
            ("set-structure", "Amount > details @name=Данные"),
            ("modify-field", "Quantity [Qty]: decimal(15,2)"),
            ("modify-filter", "Amount >= 1 @off"),
            ("modify-dataParameter", "Период = ThisMonth @off"),
            (
                "modify-parameter",
                "Период [Period title] value=ThisYear @hidden @always",
            ),
            ("modify-structure", "Quantity > details @name=Данные"),
            ("set-field-role", "Quantity dimension"),
            ("rename-parameter", "Период => ПериодОтчета"),
            (
                "reorder-parameters",
                "ПериодОтчета, ДатаНачала, ДатаОкончания",
            ),
            ("clear-selection", "*"),
            ("clear-order", "*"),
            ("clear-filter", "*"),
            ("clear-conditionalAppearance", "*"),
            ("remove-field", "Quantity"),
            ("remove-total", "Amount"),
            ("remove-calculated-field", "Margin"),
            ("remove-parameter", "ПериодОтчета"),
            ("remove-filter", "Amount"),
        ];

        for (operation, value) in operations {
            let mut args = Map::new();
            args.insert("TemplatePath".to_string(), json!("Template.xml"));
            args.insert("Operation".to_string(), json!(operation));
            args.insert("Value".to_string(), json!(value));
            let outcome = edit_skd(&args, &context);
            assert!(
                outcome.ok,
                "{operation} failed: {:?}\nstderr={:?}",
                outcome.errors, outcome.stderr
            );
            assert_eq!(outcome.command, None);
            let updated = fs::read_to_string(&template_path).unwrap();
            roxmltree::Document::parse(updated.trim_start_matches('\u{feff}'))
                .unwrap_or_else(|err| panic!("{operation} left invalid XML: {err}\n{updated}"));
        }

        let _ = fs::remove_dir_all(&context.cwd);
    }

    #[test]
    fn native_skd_edit_structure_preserves_nested_named_groups() {
        let context = temp_context("skd-edit-structure");
        let template_path = context.cwd.join("Template.xml");
        fs::write(&template_path, base_skd_xml()).unwrap();

        let mut args = Map::new();
        args.insert("TemplatePath".to_string(), json!("Template.xml"));
        args.insert("Operation".to_string(), json!("set-structure"));
        args.insert(
            "Value".to_string(),
            json!("Amount @name=G1 > Quantity @name=G2 > details"),
        );
        let outcome = edit_skd(&args, &context);
        assert!(outcome.ok, "{outcome:?}");

        args.insert("Operation".to_string(), json!("modify-structure"));
        args.insert("Value".to_string(), json!("Price @name=G2"));
        let outcome = edit_skd(&args, &context);
        assert!(outcome.ok, "{outcome:?}");

        let updated = fs::read_to_string(&template_path).unwrap();
        assert!(
            updated.contains("<dcsset:name>G1</dcsset:name>"),
            "{updated}"
        );
        assert!(
            updated.contains("<dcsset:name>G2</dcsset:name>"),
            "{updated}"
        );
        assert!(
            updated.contains("xsi:type=\"dcsset:GroupItemField\""),
            "{updated}"
        );
        assert!(
            updated.contains("<dcsset:groupType>Items</dcsset:groupType>"),
            "{updated}"
        );
        assert!(
            updated.contains("<dcsset:field>Amount</dcsset:field>"),
            "{updated}"
        );
        assert!(
            updated.contains("<dcsset:field>Price</dcsset:field>"),
            "{updated}"
        );
        assert!(
            !updated.contains("<dcsset:field>Quantity</dcsset:field>"),
            "{updated}"
        );

        let _ = fs::remove_dir_all(&context.cwd);
    }

    #[test]
    fn native_skd_edit_scopes_settings_changes_to_requested_variant() {
        let context = temp_context("skd-edit-variant");
        let template_path = context.cwd.join("Template.xml");
        fs::write(&template_path, two_variant_skd_xml()).unwrap();

        let mut args = Map::new();
        args.insert("TemplatePath".to_string(), json!("Template.xml"));
        args.insert("Operation".to_string(), json!("add-selection"));
        args.insert("Value".to_string(), json!("Amount"));
        args.insert("Variant".to_string(), json!("Дополнительный"));

        let outcome = edit_skd(&args, &context);
        assert!(outcome.ok, "{outcome:?}");

        let updated = fs::read_to_string(&template_path).unwrap();
        let primary = variant_block(&updated, "Основной");
        let secondary = variant_block(&updated, "Дополнительный");
        assert!(
            !primary.contains("<dcsset:field>Amount</dcsset:field>"),
            "{primary}"
        );
        assert!(
            secondary.contains("<dcsset:field>Amount</dcsset:field>"),
            "{secondary}"
        );

        let _ = fs::remove_dir_all(&context.cwd);
    }

    #[test]
    fn native_skd_edit_patch_query_honors_once_marker() {
        let context = temp_context("skd-edit-patch-once");
        let template_path = context.cwd.join("Template.xml");
        fs::write(
            &template_path,
            base_skd_xml().replace("ВЫБРАТЬ Amount КАК Amount", "ВЫБРАТЬ Code КАК Code"),
        )
        .unwrap();

        let mut args = Map::new();
        args.insert("TemplatePath".to_string(), json!("Template.xml"));
        args.insert("Operation".to_string(), json!("patch-query"));
        args.insert("Value".to_string(), json!("Code => ItemCode @once"));

        let outcome = edit_skd(&args, &context);
        assert!(!outcome.ok, "{outcome:?}");
        let stderr = outcome.stderr.unwrap_or_default();
        assert!(stderr.contains("@once: expected 1 occurrence"), "{stderr}");
        let unchanged = fs::read_to_string(&template_path).unwrap();
        assert!(unchanged.contains("ВЫБРАТЬ Code КАК Code"), "{unchanged}");

        let _ = fs::remove_dir_all(&context.cwd);
    }

    #[test]
    fn native_skd_edit_rename_parameter_uses_token_boundaries() {
        let context = temp_context("skd-edit-rename-boundary");
        let template_path = context.cwd.join("Template.xml");
        fs::write(&template_path, parameter_skd_xml()).unwrap();

        let mut args = Map::new();
        args.insert("TemplatePath".to_string(), json!("Template.xml"));
        args.insert("Operation".to_string(), json!("rename-parameter"));
        args.insert("Value".to_string(), json!("Период => ПериодОтчета"));

        let outcome = edit_skd(&args, &context);
        assert!(outcome.ok, "{outcome:?}");

        let updated = fs::read_to_string(&template_path).unwrap();
        assert!(updated.contains("<name>ПериодОтчета</name>"), "{updated}");
        assert!(
            updated.contains("<expression>&amp;ПериодОтчета</expression>"),
            "{updated}"
        );
        assert!(
            updated.contains("<expression>&amp;ПериодОтчетаДокумента</expression>"),
            "{updated}"
        );
        assert!(
            updated.contains("<dcscor:parameter>ПериодОтчета</dcscor:parameter>"),
            "{updated}"
        );
        assert!(
            updated.contains("<dcscor:parameter>ПериодОтчетаДокумента</dcscor:parameter>"),
            "{updated}"
        );

        let _ = fs::remove_dir_all(&context.cwd);
    }

    #[test]
    fn native_skd_edit_noop_leaves_file_untouched() {
        let context = temp_context("skd-edit-noop");
        let template_path = context.cwd.join("Template.xml");
        fs::write(&template_path, base_skd_xml()).unwrap();
        let before = fs::read(&template_path).unwrap();

        let mut args = Map::new();
        args.insert("TemplatePath".to_string(), json!("Template.xml"));
        args.insert("Operation".to_string(), json!("remove-filter"));
        args.insert("Value".to_string(), json!("MissingField"));

        let outcome = edit_skd(&args, &context);
        assert!(outcome.ok, "{outcome:?}");
        assert!(outcome.changes.is_empty(), "{outcome:?}");
        assert!(
            outcome
                .stdout
                .as_deref()
                .unwrap_or("")
                .contains("[INFO] No changes -- file untouched"),
            "{outcome:?}"
        );
        assert_eq!(fs::read(&template_path).unwrap(), before);

        let _ = fs::remove_dir_all(&context.cwd);
    }

    #[test]
    fn native_skd_validate_rejects_ref_type_bound_to_unexpected_namespace() {
        let context = temp_context("skd-validate-bad-prefix");
        let template_path = context.cwd.join("Template.xml");
        fs::write(
            &template_path,
            base_skd_xml().replace(
                "<field>Amount</field>",
                "<field>Amount</field>\n\t\t\t<valueType>\n\t\t\t\t<v8:Type xmlns:bad=\"http://example.com\">bad:CatalogRef.X</v8:Type>\n\t\t\t</valueType>",
            ),
        )
        .unwrap();

        let mut args = Map::new();
        args.insert("TemplatePath".to_string(), json!("Template.xml"));
        let outcome = validate_skd(&args, &context);
        let stdout = outcome.stdout.unwrap_or_default();
        assert!(!outcome.ok, "{stdout}");
        assert!(
            stdout.contains("uses prefix 'bad' bound to unexpected namespace 'http://example.com'"),
            "{stdout}"
        );

        let _ = fs::remove_dir_all(&context.cwd);
    }

    fn temp_context(name: &str) -> WorkspaceContext {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let cwd = std::env::temp_dir().join(format!("unica-{name}-{nanos}"));
        fs::create_dir_all(&cwd).unwrap();
        WorkspaceContext {
            cwd: cwd.clone(),
            workspace_root: cwd.clone(),
            cache_root: cwd.join(".build/unica"),
            workspace_epoch: 0,
        }
    }

    fn base_skd_xml() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<DataCompositionSchema xmlns="http://v8.1c.ru/8.1/data-composition-system/schema"
		xmlns:dcscom="http://v8.1c.ru/8.1/data-composition-system/common"
		xmlns:dcscor="http://v8.1c.ru/8.1/data-composition-system/core"
		xmlns:dcsset="http://v8.1c.ru/8.1/data-composition-system/settings"
		xmlns:v8="http://v8.1c.ru/8.1/data/core"
		xmlns:v8ui="http://v8.1c.ru/8.1/data/ui"
		xmlns:xs="http://www.w3.org/2001/XMLSchema"
		xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
	<dataSource>
		<name>ИсточникДанных1</name>
		<dataSourceType>Local</dataSourceType>
	</dataSource>
	<dataSet xsi:type="DataSetQuery">
		<name>НаборДанных1</name>
		<field xsi:type="DataSetFieldField">
			<dataPath>Amount</dataPath>
			<field>Amount</field>
		</field>
		<dataSource>ИсточникДанных1</dataSource>
		<query>ВЫБРАТЬ Amount КАК Amount</query>
	</dataSet>
	<settingsVariant>
		<dcsset:name>Основной</dcsset:name>
		<dcsset:settings>
			<dcsset:selection>
			</dcsset:selection>
			<dcsset:filter>
			</dcsset:filter>
			<dcsset:order>
			</dcsset:order>
			<dcsset:item xsi:type="dcsset:StructureItemGroup">
				<dcsset:selection>
					<dcsset:item xsi:type="dcsset:SelectedItemAuto"/>
				</dcsset:selection>
			</dcsset:item>
		</dcsset:settings>
	</settingsVariant>
</DataCompositionSchema>
"#
    }

    fn two_variant_skd_xml() -> String {
        base_skd_xml().replace(
            "</settingsVariant>\n</DataCompositionSchema>",
            "</settingsVariant>\n\t<settingsVariant>\n\t\t<dcsset:name>Дополнительный</dcsset:name>\n\t\t<dcsset:settings>\n\t\t\t<dcsset:selection>\n\t\t\t</dcsset:selection>\n\t\t</dcsset:settings>\n\t</settingsVariant>\n</DataCompositionSchema>",
        )
    }

    fn parameter_skd_xml() -> String {
        base_skd_xml().replace(
            "\t<settingsVariant>",
            "\t<parameter>\n\t\t<name>Период</name>\n\t\t<expression>&amp;Период</expression>\n\t</parameter>\n\t<parameter>\n\t\t<name>ПериодОтчетаДокумента</name>\n\t\t<expression>&amp;ПериодОтчетаДокумента</expression>\n\t</parameter>\n\t<settingsVariant>",
        )
        .replace(
            "\t\t\t<dcsset:selection>",
            "\t\t\t<dcsset:dataParameters>\n\t\t\t\t<dcsset:item>\n\t\t\t\t\t<dcscor:parameter>Период</dcscor:parameter>\n\t\t\t\t</dcsset:item>\n\t\t\t\t<dcsset:item>\n\t\t\t\t\t<dcscor:parameter>ПериодОтчетаДокумента</dcscor:parameter>\n\t\t\t\t</dcsset:item>\n\t\t\t</dcsset:dataParameters>\n\t\t\t<dcsset:selection>",
        )
    }

    fn variant_block(xml_text: &str, name: &str) -> String {
        let marker = format!("<dcsset:name>{name}</dcsset:name>");
        let name_pos = xml_text
            .find(&marker)
            .unwrap_or_else(|| panic!("{name} not found"));
        let start = xml_text[..name_pos].rfind("<settingsVariant>").unwrap();
        let end = xml_text[name_pos..].find("</settingsVariant>").unwrap()
            + name_pos
            + "</settingsVariant>".len();
        xml_text[start..end].to_string()
    }
}
