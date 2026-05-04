use crate::domain::workspace::WorkspaceContext;
use crate::infrastructure::legacy_scripts::{find_plugin_root, value_to_cli_string};
use crate::infrastructure::AdapterOutcome;
use serde_json::{json, Map, Value};
use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const DEFAULT_PROCESS_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone)]
pub struct ProcessCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct ProcessOutput {
    pub status_success: bool,
    pub status: String,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

pub trait ProcessRunner {
    fn run(&self, command: &ProcessCommand) -> Result<ProcessOutput, String>;
}

struct SystemProcessRunner;

static SYSTEM_PROCESS_RUNNER: SystemProcessRunner = SystemProcessRunner;

pub struct CliAdapter<'a> {
    launcher: &'static str,
    default_command: &'static [&'static str],
    label: &'static str,
    runner: &'a dyn ProcessRunner,
}

pub struct RuntimeAdapter<'a> {
    runner: &'a dyn ProcessRunner,
}

impl<'a> CliAdapter<'a> {
    pub fn new(
        launcher: &'static str,
        default_command: &'static [&'static str],
        label: &'static str,
    ) -> Self {
        Self {
            launcher,
            default_command,
            label,
            runner: &SYSTEM_PROCESS_RUNNER,
        }
    }

    pub fn with_runner(
        launcher: &'static str,
        default_command: &'static [&'static str],
        label: &'static str,
        runner: &'a dyn ProcessRunner,
    ) -> Self {
        Self {
            launcher,
            default_command,
            label,
            runner,
        }
    }

    pub fn invoke(
        &self,
        tool_name: &str,
        args: &Map<String, Value>,
        context: &WorkspaceContext,
        dry_run: bool,
        mutating: bool,
    ) -> Result<AdapterOutcome, String> {
        let plugin_root = find_plugin_root(&context.cwd).ok_or_else(|| {
            "could not locate Unica plugin root for internal adapter lookup".to_string()
        })?;
        let launcher = plugin_root.join("scripts").join(self.launcher);
        let mut command = vec![launcher.display().to_string()];
        command.extend(self.default_command.iter().map(|part| (*part).to_string()));
        command.extend(cli_args(args, true)?);
        let execution_args = cli_args(args, false)?;

        if dry_run {
            return Ok(AdapterOutcome {
                ok: true,
                summary: format!(
                    "dry run: {tool_name} would call internal {} adapter",
                    self.label
                ),
                changes: if mutating {
                    vec!["no files changed because dryRun is true".to_string()]
                } else {
                    Vec::new()
                },
                warnings: if launcher.exists() {
                    Vec::new()
                } else {
                    vec![format!(
                        "internal adapter launcher not found: {}",
                        launcher.display()
                    )]
                },
                errors: Vec::new(),
                artifacts: Vec::new(),
                stdout: None,
                stderr: None,
                command: Some(command),
            });
        }

        if !launcher.exists() {
            return Err(format!(
                "internal adapter launcher not found: {}",
                launcher.display()
            ));
        }

        let mut process_args = self
            .default_command
            .iter()
            .map(|part| (*part).to_string())
            .collect::<Vec<_>>();
        process_args.extend(execution_args);
        let output = self.runner.run(&ProcessCommand {
            program: launcher.clone(),
            args: process_args,
            cwd: context.cwd.clone(),
            timeout: DEFAULT_PROCESS_TIMEOUT,
        })?;
        let ok = output.status_success;
        Ok(AdapterOutcome {
            ok,
            summary: if ok {
                format!(
                    "{tool_name} completed through internal {} adapter",
                    self.label
                )
            } else {
                format!("{tool_name} failed through internal {} adapter", self.label)
            },
            changes: if mutating {
                vec![format!("internal {} adapter executed", self.label)]
            } else {
                Vec::new()
            },
            warnings: if ok {
                Vec::new()
            } else if output.timed_out {
                vec![format!("internal {} adapter timed out", self.label)]
            } else {
                vec![format!(
                    "internal {} adapter exited with status {}",
                    self.label, output.status
                )]
            },
            errors: if ok {
                Vec::new()
            } else if output.stderr.trim().is_empty() && output.timed_out {
                vec![format!(
                    "internal {} adapter timed out after {} seconds",
                    self.label,
                    DEFAULT_PROCESS_TIMEOUT.as_secs()
                )]
            } else {
                vec![output.stderr.trim().to_string()]
            },
            artifacts: Vec::new(),
            stdout: Some(output.stdout),
            stderr: Some(output.stderr),
            command: Some(command),
        })
    }
}

impl<'a> RuntimeAdapter<'a> {
    pub fn new() -> Self {
        Self {
            runner: &SYSTEM_PROCESS_RUNNER,
        }
    }

    pub fn with_runner(runner: &'a dyn ProcessRunner) -> Self {
        Self { runner }
    }

    pub fn invoke(
        &self,
        tool_name: &str,
        args: &Map<String, Value>,
        context: &WorkspaceContext,
        dry_run: bool,
        mutating: bool,
    ) -> Result<AdapterOutcome, String> {
        let plugin_root = find_plugin_root(&context.cwd).ok_or_else(|| {
            "could not locate Unica plugin root for internal adapter lookup".to_string()
        })?;
        let launcher = plugin_root.join("scripts").join("run-v8-runner.sh");
        let report_args = runtime_args(args, true)?;
        let execution_args = runtime_args(args, false)?;
        let mut command = vec![launcher.display().to_string()];
        command.extend(report_args);

        if dry_run {
            return Ok(AdapterOutcome {
                ok: true,
                summary: format!(
                    "dry run: {tool_name} would call internal v8-runner runtime adapter"
                ),
                changes: if mutating {
                    vec!["no files changed because dryRun is true".to_string()]
                } else {
                    Vec::new()
                },
                warnings: if launcher.exists() {
                    Vec::new()
                } else {
                    vec![format!(
                        "internal adapter launcher not found: {}",
                        launcher.display()
                    )]
                },
                errors: Vec::new(),
                artifacts: Vec::new(),
                stdout: None,
                stderr: None,
                command: Some(command),
            });
        }

        if !launcher.exists() {
            return Err(format!(
                "internal adapter launcher not found: {}",
                launcher.display()
            ));
        }

        let output = self.runner.run(&ProcessCommand {
            program: launcher.clone(),
            args: execution_args,
            cwd: context.cwd.clone(),
            timeout: DEFAULT_PROCESS_TIMEOUT,
        })?;
        let ok = output.status_success;
        Ok(AdapterOutcome {
            ok,
            summary: if ok {
                format!("{tool_name} completed through internal v8-runner runtime adapter")
            } else {
                format!("{tool_name} failed through internal v8-runner runtime adapter")
            },
            changes: if mutating {
                vec!["internal v8-runner runtime adapter executed".to_string()]
            } else {
                Vec::new()
            },
            warnings: if ok {
                Vec::new()
            } else if output.timed_out {
                vec!["internal v8-runner runtime adapter timed out".to_string()]
            } else {
                vec![format!(
                    "internal v8-runner runtime adapter exited with status {}",
                    output.status
                )]
            },
            errors: if ok {
                Vec::new()
            } else if output.stderr.trim().is_empty() && output.timed_out {
                vec![format!(
                    "internal v8-runner runtime adapter timed out after {} seconds",
                    DEFAULT_PROCESS_TIMEOUT.as_secs()
                )]
            } else {
                vec![output.stderr.trim().to_string()]
            },
            artifacts: Vec::new(),
            stdout: Some(output.stdout),
            stderr: Some(output.stderr),
            command: Some(command),
        })
    }
}

impl<'a> Default for RuntimeAdapter<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessRunner for SystemProcessRunner {
    fn run(&self, command: &ProcessCommand) -> Result<ProcessOutput, String> {
        let mut child = Command::new(&command.program)
            .args(&command.args)
            .current_dir(&command.cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| format!("failed to execute process: {err}"))?;

        let started = Instant::now();
        loop {
            if child
                .try_wait()
                .map_err(|err| format!("failed to poll process: {err}"))?
                .is_some()
            {
                let output = child
                    .wait_with_output()
                    .map_err(|err| format!("failed to collect process output: {err}"))?;
                return Ok(ProcessOutput {
                    status_success: output.status.success(),
                    status: output.status.to_string(),
                    stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                    stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                    timed_out: false,
                });
            }

            if started.elapsed() >= command.timeout {
                let _ = child.kill();
                let output = child
                    .wait_with_output()
                    .map_err(|err| format!("failed to collect timed-out process output: {err}"))?;
                return Ok(ProcessOutput {
                    status_success: false,
                    status: "timeout".to_string(),
                    stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                    stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                    timed_out: true,
                });
            }

            std::thread::sleep(Duration::from_millis(25));
        }
    }
}

pub struct StandardsAdapter;

#[derive(Debug, Clone, PartialEq)]
pub struct StandardsRequest {
    pub method: &'static str,
    pub params: Value,
}

pub trait HttpClient {
    fn post_json(&self, endpoint: &str, payload: &Value) -> Result<String, String>;
}

struct UreqHttpClient;

static UREQ_HTTP_CLIENT: UreqHttpClient = UreqHttpClient;

impl StandardsAdapter {
    const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

    pub fn request_for(
        operation: &str,
        args: &Map<String, Value>,
    ) -> Result<StandardsRequest, String> {
        match operation {
            "search" => Ok(StandardsRequest {
                method: "v8std_search",
                params: select_params(args, &["query", "limit", "types", "mode"]),
            }),
            "explain" if args.contains_key("codes") => Ok(StandardsRequest {
                method: "v8std_explain_diagnostics",
                params: select_params(args, &["codes"]),
            }),
            "explain" if args.contains_key("snippet") => Ok(StandardsRequest {
                method: "v8std_explain_snippet",
                params: select_params(args, &["snippet", "language", "limit"]),
            }),
            "explain" if args.contains_key("id") || args.contains_key("idOrAliasOrUrl") => {
                let id = args
                    .get("idOrAliasOrUrl")
                    .or_else(|| args.get("id"))
                    .cloned()
                    .ok_or_else(|| "missing id".to_string())?;
                let mut params = Map::new();
                params.insert("id_or_alias_or_url".to_string(), id);
                if let Some(limit) = args.get("bodyLimit").or_else(|| args.get("body_limit")) {
                    params.insert("body_limit".to_string(), limit.clone());
                }
                Ok(StandardsRequest {
                    method: "v8std_get_page",
                    params: Value::Object(params),
                })
            }
            "explain" if args.contains_key("query") => Ok(StandardsRequest {
                method: "v8std_search",
                params: select_params(args, &["query", "limit", "types", "mode"]),
            }),
            "explain" => Err(
                "unica.standards.explain requires one of: codes, snippet, id, idOrAliasOrUrl, query"
                    .to_string(),
            ),
            other => Err(format!("unknown standards operation: {other}")),
        }
    }

    pub fn invoke(operation: &str, args: &Map<String, Value>) -> AdapterOutcome {
        Self::invoke_with_client(operation, args, &UREQ_HTTP_CLIENT)
    }

    pub fn invoke_with_client(
        operation: &str,
        args: &Map<String, Value>,
        http: &dyn HttpClient,
    ) -> AdapterOutcome {
        let endpoint = env::var("UNICA_STANDARDS_MCP_URL")
            .unwrap_or_else(|_| "https://ai.v8std.ru/mcp".to_string());
        let request = match Self::request_for(operation, args) {
            Ok(request) => request,
            Err(error) => {
                return AdapterOutcome {
                    ok: false,
                    summary: format!("unica.standards.{operation} rejected invalid arguments"),
                    changes: Vec::new(),
                    warnings: Vec::new(),
                    errors: vec![error],
                    artifacts: vec![endpoint],
                    stdout: None,
                    stderr: None,
                    command: None,
                }
            }
        };

        let payload = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": request.method,
                "arguments": request.params,
            }
        });

        match http.post_json(&endpoint, &payload) {
            Ok(text) => Self::outcome_from_http_body(operation, &endpoint, request.method, &text),
            Err(err) => AdapterOutcome {
                ok: false,
                summary: format!(
                    "unica.standards.{operation} failed through internal v8std MCP proxy"
                ),
                changes: Vec::new(),
                warnings: Vec::new(),
                errors: vec![err.to_string()],
                artifacts: vec![endpoint, request.method.to_string()],
                stdout: None,
                stderr: None,
                command: None,
            },
        }
    }

    pub fn outcome_from_http_body(
        operation: &str,
        endpoint: &str,
        remote_method: &str,
        text: &str,
    ) -> AdapterOutcome {
        let normalized = match normalize_mcp_http_body(text) {
            Ok(text) => text,
            Err(error) => {
                return AdapterOutcome {
                    ok: false,
                    summary: format!(
                        "unica.standards.{operation} received invalid v8std MCP response"
                    ),
                    changes: Vec::new(),
                    warnings: Vec::new(),
                    errors: vec![error],
                    artifacts: vec![endpoint.to_string(), remote_method.to_string()],
                    stdout: None,
                    stderr: None,
                    command: None,
                }
            }
        };

        match serde_json::from_str::<Value>(&normalized) {
            Ok(Value::Object(object)) if object.contains_key("error") => {
                let message = object
                    .get("error")
                    .and_then(|error| error.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("remote JSON-RPC error");
                AdapterOutcome {
                    ok: false,
                    summary: format!(
                        "unica.standards.{operation} failed through internal v8std MCP proxy"
                    ),
                    changes: Vec::new(),
                    warnings: Vec::new(),
                    errors: vec![message.to_string()],
                    artifacts: vec![endpoint.to_string(), remote_method.to_string()],
                    stdout: None,
                    stderr: None,
                    command: None,
                }
            }
            Ok(Value::Object(object)) if object.contains_key("result") => AdapterOutcome {
                ok: true,
                summary: format!(
                    "unica.standards.{operation} completed through internal v8std MCP proxy"
                ),
                changes: Vec::new(),
                warnings: Vec::new(),
                errors: Vec::new(),
                artifacts: vec![endpoint.to_string(), remote_method.to_string()],
                stdout: Some(normalized),
                stderr: None,
                command: None,
            },
            Ok(_) => AdapterOutcome {
                ok: false,
                summary: format!(
                    "unica.standards.{operation} received non-JSON-RPC v8std MCP response"
                ),
                changes: Vec::new(),
                warnings: Vec::new(),
                errors: vec!["missing JSON-RPC result or error".to_string()],
                artifacts: vec![endpoint.to_string(), remote_method.to_string()],
                stdout: None,
                stderr: None,
                command: None,
            },
            Err(error) => AdapterOutcome {
                ok: false,
                summary: format!("unica.standards.{operation} received invalid v8std MCP JSON"),
                changes: Vec::new(),
                warnings: Vec::new(),
                errors: vec![error.to_string()],
                artifacts: vec![endpoint.to_string(), remote_method.to_string()],
                stdout: None,
                stderr: None,
                command: None,
            },
        }
    }
}

impl HttpClient for UreqHttpClient {
    fn post_json(&self, endpoint: &str, payload: &Value) -> Result<String, String> {
        ureq::AgentBuilder::new()
            .timeout(StandardsAdapter::DEFAULT_TIMEOUT)
            .build()
            .post(endpoint)
            .set("Content-Type", "application/json")
            .set("Accept", "application/json, text/event-stream")
            .send_string(&payload.to_string())
            .map_err(|err| err.to_string())?
            .into_string()
            .map_err(|err| err.to_string())
    }
}

fn select_params(args: &Map<String, Value>, keys: &[&str]) -> Value {
    let mut params = Map::new();
    for key in keys {
        if let Some(value) = args.get(*key) {
            params.insert((*key).to_string(), value.clone());
        }
    }
    Value::Object(params)
}

fn normalize_mcp_http_body(text: &str) -> Result<String, String> {
    let data_lines = text
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if data_lines.is_empty() {
        return Ok(text.trim().to_string());
    }
    let joined = data_lines.join("\n");
    serde_json::from_str::<Value>(&joined)
        .map_err(|err| format!("invalid JSON-RPC SSE data: {err}"))?;
    Ok(joined)
}

fn runtime_args(args: &Map<String, Value>, redact: bool) -> Result<Vec<String>, String> {
    if args.contains_key("args") {
        return Err(
            "raw args are not accepted by internal adapters; use typed tool arguments".to_string(),
        );
    }

    let operation = args
        .get("operation")
        .and_then(Value::as_str)
        .ok_or_else(|| "unica.runtime.execute requires string `operation` argument".to_string())?;
    let mut result = Vec::new();

    append_runtime_global_args(&mut result, operation, args, redact);

    match operation {
        "config-init" => {
            result.extend(["config".to_string(), "init".to_string()]);
            append_arg(&mut result, "--output", args, "config", redact);
            append_arg(&mut result, "--connection", args, "connection", redact);
            append_arg(&mut result, "--format", args, "format", redact);
            append_arg(&mut result, "--builder", args, "builder", redact);
        }
        "init" => result.push("init".to_string()),
        "build" => {
            result.push("build".to_string());
            append_bool_flag(&mut result, "--full-rebuild", args, "fullRebuild");
            append_arg(&mut result, "--source-set", args, "sourceSet", redact);
            append_arg(&mut result, "--extension", args, "extension", redact);
        }
        "dump" => {
            result.push("dump".to_string());
            append_arg(&mut result, "--mode", args, "mode", redact);
            append_arg(&mut result, "--object", args, "object", redact);
            append_arg(&mut result, "--source-set", args, "sourceSet", redact);
            append_arg(&mut result, "--extension", args, "extension", redact);
        }
        "convert" => {
            result.push("convert".to_string());
            append_arg(&mut result, "--source-set", args, "sourceSet", redact);
            append_arg(&mut result, "--output", args, "output", redact);
            append_arg(&mut result, "--path", args, "path", redact);
            append_arg(&mut result, "--format", args, "format", redact);
            append_arg(&mut result, "--extension", args, "extension", redact);
        }
        "make" => {
            result.push("make".to_string());
            append_arg(&mut result, "--output", args, "output", redact);
            append_arg(&mut result, "--source-set", args, "sourceSet", redact);
            append_arg(&mut result, "--extension", args, "extension", redact);
        }
        "load" => {
            result.push("load".to_string());
            append_arg(&mut result, "--path", args, "path", redact);
            append_arg(&mut result, "--mode", args, "mode", redact);
            append_arg(&mut result, "--settings", args, "settings", redact);
            append_arg(&mut result, "--extension", args, "extension", redact);
        }
        "syntax" => {
            result.push("syntax".to_string());
            if let Some(mode) = string_arg(args, "mode", redact) {
                result.push(mode);
            }
            append_bool_flag(&mut result, "--server", args, "server");
            append_bool_flag(&mut result, "--thin-client", args, "thinClient");
        }
        "test" => {
            result.push("test".to_string());
            if let Some(test_runner) = string_arg(args, "testRunner", redact) {
                result.push(test_runner);
            }
            append_bool_flag(&mut result, "--full", args, "fullRebuild");
            if let Some(test_scope) = string_arg(args, "testScope", redact) {
                result.push(test_scope);
            }
            if let Some(module) = string_arg(args, "module", redact) {
                result.push(module);
            }
            append_arg(&mut result, "--source-set", args, "sourceSet", redact);
            append_arg(&mut result, "--extension", args, "extension", redact);
        }
        "launch" => {
            result.push("launch".to_string());
            match args.get("clientMode").and_then(Value::as_str) {
                Some("mcp-va") => {
                    result.extend(["mcp".to_string(), "va".to_string()]);
                    append_arg(&mut result, "--mode", args, "mode", redact);
                    append_arg(&mut result, "--mcp-port", args, "mcpPort", redact);
                    append_arg(&mut result, "--mcp-config", args, "mcpConfig", redact);
                }
                Some("mcp") => {
                    result.push("mcp".to_string());
                    append_arg(&mut result, "--mode", args, "mode", redact);
                    append_arg(&mut result, "--mcp-port", args, "mcpPort", redact);
                    append_arg(&mut result, "--mcp-config", args, "mcpConfig", redact);
                }
                Some(client_mode) => result.push(client_mode.to_string()),
                None => {}
            }
        }
        "extensions" => {
            result.push("extensions".to_string());
            append_arg(&mut result, "--name", args, "sourceSet", redact);
            append_arg(&mut result, "--extension", args, "extension", redact);
        }
        other => return Err(format!("unknown runtime operation: {other}")),
    }

    Ok(result)
}

fn append_runtime_global_args(
    result: &mut Vec<String>,
    operation: &str,
    args: &Map<String, Value>,
    redact: bool,
) {
    if operation != "config-init" {
        append_arg(result, "--config", args, "config", redact);
    }
    append_arg(result, "--workdir", args, "workdir", redact);
}

fn cli_args(args: &Map<String, Value>, redact: bool) -> Result<Vec<String>, String> {
    if args.contains_key("args") {
        return Err(
            "raw args are not accepted by internal adapters; use typed tool arguments".to_string(),
        );
    }

    let mut result = Vec::new();
    for (key, value) in args {
        if matches!(key.as_str(), "dryRun" | "cwd" | "confirm") {
            continue;
        }
        let flag = format!("--{}", kebab_case(key));
        match value {
            Value::Bool(true) => result.push(flag),
            Value::Bool(false) | Value::Null => {}
            Value::Array(items) => {
                for item in items {
                    result.push(flag.clone());
                    result.push(value_to_cli_string(item));
                }
            }
            other => {
                result.push(flag);
                result.push(if redact && is_secret_key(key) {
                    "<redacted>".to_string()
                } else {
                    value_to_cli_string(other)
                });
            }
        }
    }
    Ok(result)
}

fn append_arg(
    result: &mut Vec<String>,
    flag: &str,
    args: &Map<String, Value>,
    key: &str,
    redact: bool,
) {
    if let Some(value) = string_arg(args, key, redact) {
        result.push(flag.to_string());
        result.push(value);
    }
}

fn append_bool_flag(result: &mut Vec<String>, flag: &str, args: &Map<String, Value>, key: &str) {
    if args.get(key).and_then(Value::as_bool).unwrap_or(false) {
        result.push(flag.to_string());
    }
}

fn string_arg(args: &Map<String, Value>, key: &str, redact: bool) -> Option<String> {
    args.get(key).and_then(|value| {
        if value.is_null() {
            return None;
        }
        if redact && is_secret_key(key) {
            Some("<redacted>".to_string())
        } else {
            Some(value_to_cli_string(value))
        }
    })
}

fn is_secret_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.contains("password") || key.contains("token") || key.contains("secret")
}

fn kebab_case(key: &str) -> String {
    let mut out = String::new();
    for (index, ch) in key.chars().enumerate() {
        if ch == '_' {
            out.push('-');
        } else if ch.is_ascii_uppercase() {
            if index > 0 {
                out.push('-');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

#[allow(dead_code)]
fn _path_list(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::cell::RefCell;

    #[test]
    fn standards_search_maps_to_v8std_search_request() {
        let mut args = Map::new();
        args.insert("query".to_string(), json!("modal windows"));
        args.insert("limit".to_string(), json!(3));

        let request = StandardsAdapter::request_for("search", &args).unwrap();

        assert_eq!(request.method, "v8std_search");
        assert_eq!(request.params["query"], "modal windows");
        assert_eq!(request.params["limit"], 3);
    }

    #[test]
    fn standards_explain_prefers_diagnostics_codes() {
        let mut args = Map::new();
        args.insert("codes".to_string(), json!(["acc:142"]));
        args.insert("query".to_string(), json!("ignored when codes are present"));

        let request = StandardsAdapter::request_for("explain", &args).unwrap();

        assert_eq!(request.method, "v8std_explain_diagnostics");
        assert_eq!(request.params["codes"][0], "acc:142");
    }

    #[test]
    fn build_runtime_adapter_dry_run_builds_v8_runner_command() {
        let context = WorkspaceContext::discover(std::env::current_dir().unwrap()).unwrap();
        let mut args = Map::new();
        args.insert("sourceSet".to_string(), json!("main"));

        let outcome = CliAdapter::new("run-v8-runner.sh", &["build"], "build/runtime")
            .invoke("unica.build.load", &args, &context, true, true)
            .unwrap();

        let command = outcome.command.unwrap().join(" ");
        assert!(command.contains("run-v8-runner.sh"));
        assert!(command.contains("build"));
        assert!(command.contains("--source-set main"));
    }

    #[test]
    fn runtime_adapter_maps_build_to_allowlisted_v8_runner_argv() {
        let context = WorkspaceContext::discover(std::env::current_dir().unwrap()).unwrap();
        let runner = RecordingProcessRunner {
            commands: RefCell::new(Vec::new()),
            output: ProcessOutput {
                status_success: true,
                status: "exit status: 0".to_string(),
                stdout: "ok".to_string(),
                stderr: String::new(),
                timed_out: false,
            },
        };
        let mut args = Map::new();
        args.insert("operation".to_string(), json!("build"));
        args.insert("sourceSet".to_string(), json!("main"));
        args.insert("fullRebuild".to_string(), json!(true));

        let outcome = RuntimeAdapter::with_runner(&runner)
            .invoke("unica.runtime.execute", &args, &context, false, true)
            .unwrap();

        assert!(outcome.ok);
        let commands = runner.commands.borrow();
        assert_eq!(
            commands[0].args,
            vec!["build", "--full-rebuild", "--source-set", "main"]
        );
    }

    #[test]
    fn runtime_adapter_maps_config_init_config_to_output_arg() {
        let mut args = Map::new();
        args.insert("operation".to_string(), json!("config-init"));
        args.insert("config".to_string(), json!("./v8project.yaml"));
        args.insert("connection".to_string(), json!("File=build/ib"));
        args.insert("format".to_string(), json!("edt"));
        args.insert("builder".to_string(), json!("IBCMD"));

        let argv = runtime_args(&args, false).unwrap();

        assert_eq!(
            argv,
            vec![
                "config",
                "init",
                "--output",
                "./v8project.yaml",
                "--connection",
                "File=build/ib",
                "--format",
                "edt",
                "--builder",
                "IBCMD"
            ]
        );
    }

    #[test]
    fn runtime_adapter_maps_test_and_launch_mcp_va() {
        let mut test_args = Map::new();
        test_args.insert("operation".to_string(), json!("test"));
        test_args.insert("testRunner".to_string(), json!("yaxunit"));
        test_args.insert("fullRebuild".to_string(), json!(true));
        test_args.insert("testScope".to_string(), json!("module"));
        test_args.insert("module".to_string(), json!("CommonModule.Тесты"));

        assert_eq!(
            runtime_args(&test_args, false).unwrap(),
            vec!["test", "yaxunit", "--full", "module", "CommonModule.Тесты"]
        );

        let mut launch_args = Map::new();
        launch_args.insert("operation".to_string(), json!("launch"));
        launch_args.insert("clientMode".to_string(), json!("mcp-va"));
        launch_args.insert("mode".to_string(), json!("thin"));
        launch_args.insert("mcpPort".to_string(), json!(1550));

        assert_eq!(
            runtime_args(&launch_args, false).unwrap(),
            vec![
                "launch",
                "mcp",
                "va",
                "--mode",
                "thin",
                "--mcp-port",
                "1550"
            ]
        );
    }

    #[test]
    fn runtime_adapter_maps_each_runtime_operation_to_expected_argv() {
        let cases = vec![
            (json!({"operation": "init"}), vec!["init"]),
            (
                json!({
                    "operation": "dump",
                    "mode": "partial",
                    "object": "Catalog:Номенклатура",
                    "sourceSet": "main",
                    "extension": "MyExtension",
                }),
                vec![
                    "dump",
                    "--mode",
                    "partial",
                    "--object",
                    "Catalog:Номенклатура",
                    "--source-set",
                    "main",
                    "--extension",
                    "MyExtension",
                ],
            ),
            (
                json!({
                    "operation": "convert",
                    "sourceSet": "main",
                    "output": "build/convert",
                }),
                vec![
                    "convert",
                    "--source-set",
                    "main",
                    "--output",
                    "build/convert",
                ],
            ),
            (
                json!({
                    "operation": "make",
                    "output": "build/config.cf",
                    "sourceSet": "main",
                }),
                vec![
                    "make",
                    "--output",
                    "build/config.cf",
                    "--source-set",
                    "main",
                ],
            ),
            (
                json!({
                    "operation": "load",
                    "path": "build/config.cf",
                    "mode": "merge",
                    "settings": "merge-settings.xml",
                }),
                vec![
                    "load",
                    "--path",
                    "build/config.cf",
                    "--mode",
                    "merge",
                    "--settings",
                    "merge-settings.xml",
                ],
            ),
            (
                json!({
                    "operation": "syntax",
                    "mode": "designer-modules",
                    "server": true,
                    "thinClient": true,
                }),
                vec!["syntax", "designer-modules", "--server", "--thin-client"],
            ),
            (
                json!({
                    "operation": "extensions",
                    "sourceSet": "MyExtension",
                }),
                vec!["extensions", "--name", "MyExtension"],
            ),
        ];

        for (input, expected) in cases {
            let args = input.as_object().unwrap().clone();
            assert_eq!(runtime_args(&args, false).unwrap(), expected);
        }
    }

    #[test]
    fn runtime_adapter_rejects_raw_args_vector() {
        let mut args = Map::new();
        args.insert("operation".to_string(), json!("build"));
        args.insert("args".to_string(), json!(["--unsafe", "../outside"]));

        let error = runtime_args(&args, false).unwrap_err();

        assert!(error.contains("raw args are not accepted"));
    }

    #[test]
    fn code_adapter_dry_run_builds_bsl_analyzer_command() {
        let context = WorkspaceContext::discover(std::env::current_dir().unwrap()).unwrap();
        let mut args = Map::new();
        args.insert("query".to_string(), json!("ОбщийМодуль"));

        let outcome = CliAdapter::new("run-bsl-analyzer.sh", &["search"], "code analysis")
            .invoke("unica.code.search", &args, &context, true, false)
            .unwrap();

        let command = outcome.command.unwrap().join(" ");
        assert!(command.contains("run-bsl-analyzer.sh"));
        assert!(command.contains("search"));
        assert!(command.contains("--query"));
        assert!(command.contains("ОбщийМодуль"));
    }

    #[test]
    fn cli_adapter_rejects_raw_args_vector() {
        let context = WorkspaceContext::discover(std::env::current_dir().unwrap()).unwrap();
        let mut args = Map::new();
        args.insert("args".to_string(), json!(["--unsafe", "../outside"]));

        let error = CliAdapter::new("run-v8-runner.sh", &["build"], "build/runtime")
            .invoke("unica.build.load", &args, &context, true, true)
            .unwrap_err();

        assert!(error.contains("raw args are not accepted"));
    }

    #[test]
    fn cli_adapter_redacts_secret_values_from_reported_command() {
        let context = WorkspaceContext::discover(std::env::current_dir().unwrap()).unwrap();
        let mut args = Map::new();
        args.insert("dbPassword".to_string(), json!("super-secret"));

        let outcome = CliAdapter::new("run-v8-runner.sh", &["build"], "build/runtime")
            .invoke("unica.build.load", &args, &context, true, true)
            .unwrap();

        let command = outcome.command.unwrap().join(" ");
        assert!(command.contains("--db-password <redacted>"));
        assert!(!command.contains("super-secret"));
    }

    #[test]
    fn cli_adapter_uses_fake_process_runner_for_status_and_output_contract() {
        let context = WorkspaceContext::discover(std::env::current_dir().unwrap()).unwrap();
        let runner = FakeProcessRunner {
            output: ProcessOutput {
                status_success: false,
                status: "exit status: 2".to_string(),
                stdout: "partial stdout".to_string(),
                stderr: "failure stderr".to_string(),
                timed_out: false,
            },
        };

        let outcome =
            CliAdapter::with_runner("run-v8-runner.sh", &["build"], "build/runtime", &runner)
                .invoke("unica.build.load", &Map::new(), &context, false, true)
                .unwrap();

        assert!(!outcome.ok);
        assert_eq!(outcome.stdout.as_deref(), Some("partial stdout"));
        assert_eq!(outcome.stderr.as_deref(), Some("failure stderr"));
        assert!(outcome.errors.contains(&"failure stderr".to_string()));
        assert!(outcome
            .warnings
            .iter()
            .any(|warning| warning.contains("exit status: 2")));
    }

    #[test]
    fn cli_adapter_reports_fake_process_timeout() {
        let context = WorkspaceContext::discover(std::env::current_dir().unwrap()).unwrap();
        let runner = FakeProcessRunner {
            output: ProcessOutput {
                status_success: false,
                status: "timeout".to_string(),
                stdout: String::new(),
                stderr: String::new(),
                timed_out: true,
            },
        };

        let outcome =
            CliAdapter::with_runner("run-v8-runner.sh", &["build"], "build/runtime", &runner)
                .invoke("unica.build.load", &Map::new(), &context, false, true)
                .unwrap();

        assert!(!outcome.ok);
        assert!(outcome
            .warnings
            .iter()
            .any(|warning| warning.contains("timed out")));
        assert!(outcome
            .errors
            .iter()
            .any(|error| error.contains("timed out after")));
    }

    #[test]
    fn standards_mcp_error_body_is_reported_as_failure() {
        let outcome = StandardsAdapter::outcome_from_http_body(
            "explain",
            "https://example.test/mcp",
            "v8std_get_page",
            r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32602,"message":"bad id"}}"#,
        );

        assert!(!outcome.ok);
        assert!(outcome.errors.iter().any(|error| error.contains("bad id")));
        assert!(outcome.stdout.is_none());
    }

    #[test]
    fn standards_sse_body_extracts_structured_json_result() {
        let outcome = StandardsAdapter::outcome_from_http_body(
            "search",
            "https://example.test/mcp",
            "v8std_search",
            "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"ok\":true}}\n\n",
        );

        assert!(outcome.ok);
        assert_eq!(
            outcome.stdout.as_deref(),
            Some(r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#)
        );
    }

    #[test]
    fn standards_protocol_mismatch_is_failure() {
        let outcome = StandardsAdapter::outcome_from_http_body(
            "search",
            "https://example.test/mcp",
            "v8std_search",
            r#"{"not":"json-rpc"}"#,
        );

        assert!(!outcome.ok);
        assert!(outcome
            .errors
            .iter()
            .any(|error| error.contains("missing JSON-RPC")));
    }

    #[test]
    fn standards_adapter_uses_fake_http_client_for_json_rpc_mapping() {
        let client = FakeHttpClient {
            payloads: RefCell::new(Vec::new()),
            response: r#"{"jsonrpc":"2.0","id":1,"result":{"content":[]}}"#.to_string(),
        };
        let mut args = Map::new();
        args.insert("query".to_string(), json!("модальные окна"));
        args.insert("limit".to_string(), json!(2));

        let outcome = StandardsAdapter::invoke_with_client("search", &args, &client);

        assert!(outcome.ok);
        let payloads = client.payloads.borrow();
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0]["method"], "tools/call");
        assert_eq!(payloads[0]["params"]["name"], "v8std_search");
        assert_eq!(
            payloads[0]["params"]["arguments"]["query"],
            "модальные окна"
        );
        assert_eq!(payloads[0]["params"]["arguments"]["limit"], 2);
    }

    struct FakeProcessRunner {
        output: ProcessOutput,
    }

    impl ProcessRunner for FakeProcessRunner {
        fn run(&self, _command: &ProcessCommand) -> Result<ProcessOutput, String> {
            Ok(self.output.clone())
        }
    }

    struct RecordingProcessRunner {
        commands: RefCell<Vec<ProcessCommand>>,
        output: ProcessOutput,
    }

    impl ProcessRunner for RecordingProcessRunner {
        fn run(&self, command: &ProcessCommand) -> Result<ProcessOutput, String> {
            self.commands.borrow_mut().push(command.clone());
            Ok(self.output.clone())
        }
    }

    struct FakeHttpClient {
        payloads: RefCell<Vec<Value>>,
        response: String,
    }

    impl HttpClient for FakeHttpClient {
        fn post_json(&self, _endpoint: &str, payload: &Value) -> Result<String, String> {
            self.payloads.borrow_mut().push(payload.clone());
            Ok(self.response.clone())
        }
    }
}
