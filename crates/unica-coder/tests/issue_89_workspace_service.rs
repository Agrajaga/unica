use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const RESPONSE_DEADLINE: Duration = Duration::from_secs(10);

#[test]
fn issue_89_multi_source_workspace_uses_main_root_and_remains_cancellable() {
    let fixture = Fixture::new();
    let mut mcp = McpProcess::start(&fixture);

    mcp.send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}));
    assert_eq!(
        mcp.receive_ids(&[1], RESPONSE_DEADLINE)[&1]["result"]["serverInfo"]["name"],
        "unica"
    );

    mcp.send(tool_call(
        2,
        "unica.code.search",
        json!({
            "cwd": fixture.workspace,
            "query": "Procedure"
        }),
    ));
    fixture.wait_for_log("rlm|", RESPONSE_DEADLINE);

    mcp.send(tool_call(
        3,
        "unica.meta.profile",
        json!({
            "cwd": fixture.workspace,
            "name": "Catalog.Test"
        }),
    ));
    let ping_started = Instant::now();
    mcp.send(json!({"jsonrpc":"2.0","id":4,"method":"ping"}));
    mcp.send(json!({
        "jsonrpc":"2.0",
        "method":"notifications/cancelled",
        "params":{"requestId":2,"reason":"issue-89 regression"}
    }));

    let (responses, response_times) =
        mcp.receive_ids_timed(&[2, 3, 4], RESPONSE_DEADLINE, ping_started);
    assert!(response_times[&4] < Duration::from_secs(2));
    assert_eq!(responses[&2]["error"]["code"], -32800);
    assert!(responses[&3].get("result").is_some(), "{:#}", responses[&3]);
    assert!(responses[&4].get("result").is_some(), "{:#}", responses[&4]);

    mcp.send(tool_call(
        5,
        "unica.code.graph",
        json!({
            "cwd": fixture.workspace,
            "mode": "callers",
            "query": "Test"
        }),
    ));
    mcp.send(tool_call(
        6,
        "unica.meta.profile",
        json!({
            "cwd": fixture.workspace,
            "name": "Catalog.Test"
        }),
    ));
    let final_responses = mcp.receive_ids(&[5, 6], RESPONSE_DEADLINE);
    assert!(
        final_responses[&5].get("result").is_some(),
        "{:#}",
        final_responses[&5]
    );
    assert!(
        final_responses[&6].get("result").is_some(),
        "{:#}",
        final_responses[&6]
    );

    let expected_root = canonical_display(&fixture.workspace.join("src/cf"));
    let records = fixture.log_records();
    assert!(records.iter().any(|record| record.kind == "analyzer"));
    assert!(records.iter().any(|record| record.kind == "rlm"));
    assert!(
        records
            .iter()
            .all(|record| record.source_root == expected_root),
        "{records:#?}"
    );
    let service_records = fixture.service_records();
    assert_eq!(
        service_records.len(),
        1,
        "parallel calls for the same effective source root must reuse one service identity"
    );
    assert_eq!(service_records[0]["source_root"], expected_root);
    assert_eq!(
        service_records[0]["workspace_root"],
        canonical_display(&fixture.workspace)
    );

    fixture.shutdown_services();
    mcp.close();
    for pid in records
        .into_iter()
        .map(|record| record.pid)
        .collect::<HashSet<_>>()
    {
        wait_until_dead(pid, RESPONSE_DEADLINE);
    }
}

fn tool_call(id: u64, name: &str, arguments: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":arguments}})
}

struct McpProcess {
    child: Child,
    stdin: Option<ChildStdin>,
    responses: mpsc::Receiver<String>,
}

impl McpProcess {
    fn start(fixture: &Fixture) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_unica"))
            .current_dir(&fixture.workspace)
            .env("UNICA_PLUGIN_ROOT", &fixture.plugin_root)
            .env("UNICA_CACHE_DIR", &fixture.cache)
            .env("ISSUE89_LOG", &fixture.log)
            .env("ISSUE89_BLOCK_ONCE", &fixture.block_once)
            .env("UNICA_WORKSPACE_SERVICE_IDLE_SECS", "1")
            .env("UNICA_WORKSPACE_SERVICE_MAX_AGE_SECS", "10")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start unica MCP");
        let stdin = child.stdin.take().expect("MCP stdin");
        let stdout = child.stdout.take().expect("MCP stdout");
        let (tx, responses) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                match line {
                    Ok(line) => {
                        if tx.send(line).is_err() {
                            return;
                        }
                    }
                    Err(_) => return,
                }
            }
        });
        Self {
            child,
            stdin: Some(stdin),
            responses,
        }
    }

    fn send(&mut self, message: Value) {
        let stdin = self.stdin.as_mut().expect("open MCP stdin");
        serde_json::to_writer(&mut *stdin, &message).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin.flush().unwrap();
    }

    fn receive_ids(&self, ids: &[u64], timeout: Duration) -> HashMap<u64, Value> {
        self.receive_ids_timed(ids, timeout, Instant::now()).0
    }

    fn receive_ids_timed(
        &self,
        ids: &[u64],
        timeout: Duration,
        started: Instant,
    ) -> (HashMap<u64, Value>, HashMap<u64, Duration>) {
        let deadline = Instant::now() + timeout;
        let expected = ids.iter().copied().collect::<HashSet<_>>();
        let mut found = HashMap::new();
        let mut response_times = HashMap::new();
        while found.len() < expected.len() {
            let remaining = deadline.saturating_duration_since(Instant::now());
            assert!(
                !remaining.is_zero(),
                "timed out waiting for MCP ids {expected:?}; got {found:?}"
            );
            let line = self
                .responses
                .recv_timeout(remaining)
                .expect("MCP response before deadline");
            let response: Value = serde_json::from_str(&line).expect("JSON MCP response");
            if let Some(id) = response.get("id").and_then(Value::as_u64) {
                if expected.contains(&id) {
                    response_times.insert(id, started.elapsed());
                    found.insert(id, response);
                }
            }
        }
        (found, response_times)
    }

    fn close(&mut self) {
        drop(self.stdin.take());
        let deadline = Instant::now() + RESPONSE_DEADLINE;
        while self.child.try_wait().unwrap().is_none() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(25));
        }
        if self.child.try_wait().unwrap().is_none() {
            let _ = self.child.kill();
        }
        let status = self.child.wait().unwrap();
        assert!(status.success(), "unica exited with {status}");
    }
}

impl Drop for McpProcess {
    fn drop(&mut self) {
        drop(self.stdin.take());
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

struct Fixture {
    root: PathBuf,
    workspace: PathBuf,
    plugin_root: PathBuf,
    cache: PathBuf,
    log: PathBuf,
    block_once: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("unica-issue-89-{}-{nonce}", std::process::id()));
        let workspace = root.join("workspace");
        let plugin_root = root.join("plugin");
        let cache = root.join("cache");
        let log = root.join("tool.log");
        let block_once = root.join("block-once");
        fs::create_dir_all(workspace.join("src/cf/Configuration")).unwrap();
        fs::create_dir_all(workspace.join("src/cf/CommonModules/Test/Ext")).unwrap();
        fs::create_dir_all(workspace.join("exts/TESTS/Configuration")).unwrap();
        fs::create_dir_all(plugin_root.join("skills")).unwrap();
        fs::create_dir_all(plugin_root.join("third-party")).unwrap();
        fs::create_dir_all(&cache).unwrap();
        fs::write(workspace.join("v8project.yaml"), "format: DESIGNER\nsource-set:\n  main:\n    type: CONFIGURATION\n    path: src/cf\n  TESTS:\n    type: CONFIGURATION\n    path: exts/TESTS\n").unwrap();
        fs::write(workspace.join("src/cf/Configuration.xml"), "<?xml version=\"1.0\" encoding=\"UTF-8\"?><MetaDataObject><Configuration/></MetaDataObject>").unwrap();
        fs::write(
            workspace.join("src/cf/CommonModules/Test/Ext/Module.bsl"),
            "Procedure Test() Export\nEndProcedure\n",
        )
        .unwrap();
        fs::write(workspace.join("exts/TESTS/Configuration.xml"), "<?xml version=\"1.0\" encoding=\"UTF-8\"?><MetaDataObject><Configuration/></MetaDataObject>").unwrap();
        compile_fake_tools(&root, &plugin_root);
        Self {
            root,
            workspace,
            plugin_root,
            cache,
            log,
            block_once,
        }
    }

    fn wait_for_log(&self, prefix: &str, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if fs::read_to_string(&self.log)
                .unwrap_or_default()
                .lines()
                .any(|line| line.starts_with(prefix))
            {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!("timed out waiting for fake-tool log prefix {prefix}");
    }

    fn log_records(&self) -> Vec<ToolRecord> {
        fs::read_to_string(&self.log)
            .unwrap()
            .lines()
            .map(|line| {
                let mut fields = line.splitn(3, '|');
                ToolRecord {
                    kind: fields.next().unwrap().to_string(),
                    pid: fields.next().unwrap().parse().unwrap(),
                    source_root: fields.next().unwrap().to_string(),
                }
            })
            .collect()
    }

    fn shutdown_services(&self) {
        self.shutdown_services_with(false);
    }

    fn service_records(&self) -> Vec<Value> {
        let services = self.cache.join("services");
        fs::read_dir(services)
            .unwrap()
            .flatten()
            .filter_map(|entry| fs::read_to_string(entry.path().join("service.json")).ok())
            .map(|text| serde_json::from_str(&text).unwrap())
            .collect()
    }

    fn shutdown_services_with(&self, best_effort: bool) {
        let services = self.cache.join("services");
        if !services.is_dir() {
            return;
        }
        for entry in fs::read_dir(services).unwrap().flatten() {
            let record_path = entry.path().join("service.json");
            let Ok(text) = fs::read_to_string(record_path) else {
                continue;
            };
            let record: Value = serde_json::from_str(&text).unwrap();
            let connection =
                TcpStream::connect(("127.0.0.1", record["port"].as_u64().unwrap() as u16));
            if best_effort && connection.is_err() {
                continue;
            }
            let mut stream = connection.unwrap();
            stream.set_read_timeout(Some(RESPONSE_DEADLINE)).unwrap();
            serde_json::to_writer(
                &mut stream,
                &json!({"token":record["token"],"kind":{"type":"shutdown"}}),
            )
            .unwrap();
            stream.write_all(b"\n").unwrap();
            stream.flush().unwrap();
            let mut response = String::new();
            BufReader::new(stream).read_line(&mut response).unwrap();
            assert!(serde_json::from_str::<Value>(&response).unwrap()["ok"]
                .as_bool()
                .unwrap());
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.shutdown_services_with(true);
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Debug)]
struct ToolRecord {
    kind: String,
    pid: u32,
    source_root: String,
}

fn compile_fake_tools(root: &Path, plugin_root: &Path) {
    let source = root.join("fake_tool.rs");
    fs::write(&source, FAKE_TOOL_SOURCE).unwrap();
    let fake = root.join(format!("fake-tool{}", std::env::consts::EXE_SUFFIX));
    let output = Command::new("rustc")
        .args(["--edition=2021", "-O"])
        .arg(&source)
        .arg("-o")
        .arg(&fake)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "fake tool compile failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let lock_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../plugins/unica/third-party/tools.lock.json");
    let lock_text = fs::read_to_string(&lock_path).unwrap();
    let lock: Value = serde_json::from_str(&lock_text).unwrap();
    let target = host_target();
    let target_contract = &lock["targets"][target];
    let exe = target_contract["exe"].as_str().unwrap();
    let bin = plugin_root.join("bin").join(target);
    fs::create_dir_all(&bin).unwrap();
    let sha256 = sha256_file(&fake);
    let mut manifest_tools = Vec::new();
    for name in ["bsl-analyzer", "rlm-bsl-index"] {
        let contract = lock["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["name"] == name)
            .unwrap();
        let binary_name = contract["binaryName"].as_str().unwrap();
        let relative = format!("bin/{target}/{binary_name}{exe}");
        fs::copy(&fake, plugin_root.join(&relative)).unwrap();
        manifest_tools.push(json!({
            "name": name,
            "version": contract["version"],
            "binaries": {
                (target): {
                    "targetTriple": target_contract["targetTriple"],
                    "binaryPath": relative,
                    "sha256": sha256.clone(),
                }
            }
        }));
    }
    fs::write(
        plugin_root.join("third-party/manifest.json"),
        serde_json::to_vec_pretty(&json!({"schemaVersion": 2, "tools": manifest_tools})).unwrap(),
    )
    .unwrap();
    fs::write(plugin_root.join("third-party/tools.lock.json"), lock_text).unwrap();
}

fn sha256_file(path: &Path) -> String {
    let mut file = fs::File::open(path).unwrap();
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).unwrap();
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    format!("{:x}", digest.finalize())
}

fn host_target() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => "win-x64",
        ("linux", "x86_64") => "linux-x64",
        ("macos", "aarch64") => "darwin-arm64",
        host => panic!("unsupported integration-test host {host:?}"),
    }
}

fn canonical_display(path: &Path) -> String {
    let path = fs::canonicalize(path).unwrap();
    #[cfg(windows)]
    return path
        .display()
        .to_string()
        .trim_start_matches(r"\\?\")
        .to_string();
    #[cfg(not(windows))]
    path.display().to_string()
}

fn wait_until_dead(pid: u32, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while process_alive(pid) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(50));
    }
    assert!(
        !process_alive(pid),
        "fake tool pid {pid} survived cancellation/shutdown"
    );
}

#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(windows)]
fn process_alive(pid: u32) -> bool {
    Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", &format!("if (Get-Process -Id {pid} -ErrorAction SilentlyContinue) {{ exit 0 }} else {{ exit 1 }}")]).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|status| status.success())
}

const FAKE_TOOL_SOURCE: &str = r#"
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::thread;
use std::time::Duration;

fn main() {
    let exe = env::current_exe().unwrap();
    let name = exe.file_stem().unwrap().to_string_lossy();
    let args = env::args().skip(1).collect::<Vec<_>>();
    if name.contains("bsl-analyzer") { analyzer(&args); } else { rlm(&args); }
}

fn record(kind: &str, root: &str) {
    let mut file = OpenOptions::new().create(true).append(true).open(env::var("ISSUE89_LOG").unwrap()).unwrap();
    writeln!(file, "{}|{}|{}", kind, std::process::id(), root).unwrap();
}

fn analyzer(args: &[String]) {
    let root = args.windows(2).find(|pair| pair[0] == "--source-dir").map(|pair| pair[1].clone()).unwrap();
    record("analyzer", &root);
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        if !line.contains("\"id\"") { continue; }
        let id = line.split("\"id\":").nth(1).and_then(|tail| tail.split(|c: char| !c.is_ascii_digit()).next()).unwrap();
        if line.contains("\"method\":\"initialize\"") {
            println!("{{\"jsonrpc\":\"2.0\",\"id\":{},\"result\":{{\"protocolVersion\":\"2025-03-26\",\"capabilities\":{{}},\"serverInfo\":{{\"name\":\"fake\",\"version\":\"test\"}}}}}}", id);
        } else {
            println!("{{\"jsonrpc\":\"2.0\",\"id\":{},\"result\":{{\"content\":[{{\"type\":\"text\",\"text\":\"{{\\\"action\\\":\\\"callers\\\",\\\"nodes\\\":[]}}\"}}]}}}}", id);
        }
        io::stdout().flush().unwrap();
    }
}

fn rlm(args: &[String]) {
    let root = args.last().unwrap();
    record("rlm", root);
    let marker = env::var("ISSUE89_BLOCK_ONCE").unwrap();
    if OpenOptions::new().write(true).create_new(true).open(marker).is_ok() {
        loop { thread::sleep(Duration::from_secs(1)); }
    }
    println!("Index not found");
}
"#;
