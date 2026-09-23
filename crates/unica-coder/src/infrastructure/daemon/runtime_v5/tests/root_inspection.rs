use super::*;
use crate::application::ports::Clock;
use crate::infrastructure::daemon::protocol_v5::V5DaemonTaskSnapshot;
use crate::infrastructure::daemon::server::V5CanonicalInvocationRuntime;
use crate::infrastructure::daemon::v13_workspace_bootstrap::test_control::HealthInspectionPause;

struct InspectionClock {
    start: Instant,
    elapsed_ms: AtomicU64,
    listener: Mutex<Option<thread::ThreadId>>,
    watchdog_read: mpsc::Sender<()>,
}

impl Clock for InspectionClock {
    fn now(&self) -> Instant {
        let elapsed_ms = self.elapsed_ms.load(Ordering::SeqCst);
        if elapsed_ms >= 9_000 && *self.listener.lock().unwrap() == Some(thread::current().id()) {
            let _ = self.watchdog_read.send(());
        }
        self.start + Duration::from_millis(elapsed_ms)
    }
}

#[derive(Default)]
struct InspectionHooks {
    fail_stopped: AtomicBool,
}

impl V5RuntimeHooks for InspectionHooks {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn forced_process_exit(&self, _grace: Option<Duration>) {
        self.fail_stopped.store(true, Ordering::SeqCst);
    }

    fn releases_authority_on_fail_stop(&self) -> bool {
        true
    }
}

#[derive(Default)]
struct SourceAdmissionProbe {
    prepares: AtomicUsize,
}

impl CanonicalInvocationService for SourceAdmissionProbe {
    fn prepare(
        &self,
        _invocation: &crate::infrastructure::daemon::server::ActorBoundInvocation,
    ) -> Result<ExecutionClass, Box<DomainResult>> {
        self.prepares.fetch_add(1, Ordering::SeqCst);
        panic!("root inspection must not prepare a source operation");
    }

    fn execute(
        &self,
        _invocation: &crate::infrastructure::daemon::server::ActorBoundExecution,
        _cancellation: CancellationToken,
    ) -> Result<DomainResult, InvocationFailure> {
        panic!("root inspection must not execute a source operation");
    }
}

struct InspectionDaemon {
    pause: HealthInspectionPause,
    stop: Arc<AtomicBool>,
    server: Option<thread::JoinHandle<Result<(), String>>>,
}

impl Drop for InspectionDaemon {
    fn drop(&mut self) {
        self.pause.release();
        self.stop.store(true, Ordering::SeqCst);
        if let Some(server) = self.server.take() {
            let _ = server.join();
        }
    }
}

fn root_inspection_survives_response_cutoff(tool: V5ToolIdentity) {
    let state = tempfile::tempdir().unwrap();
    let state_root = std::fs::canonicalize(state.path()).unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let workspace_root = std::fs::canonicalize(workspace.path()).unwrap();
    std::fs::create_dir(workspace_root.join("src")).unwrap();
    let project_bytes =
        b"format: DESIGNER\nsource-set:\n  - name: main\n    type: CONFIGURATION\n    path: src\n";
    let source_bytes = br#"<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20"><Configuration><Properties><Name>Store</Name></Properties><ChildObjects/></Configuration></MetaDataObject>"#;
    std::fs::write(workspace_root.join("v8project.yaml"), project_bytes).unwrap();
    std::fs::write(workspace_root.join("src/Configuration.xml"), source_bytes).unwrap();
    let independent_workspace = tempfile::tempdir().unwrap();
    let pause = HealthInspectionPause::install(workspace_root.clone());
    let (watchdog_tx, watchdog_rx) = mpsc::channel();
    let clock = Arc::new(InspectionClock {
        start: Instant::now(),
        elapsed_ms: AtomicU64::new(0),
        listener: Mutex::new(None),
        watchdog_read: watchdog_tx,
    });
    let service = Arc::new(SourceAdmissionProbe::default());
    let canonical_runtime = Arc::new(V5CanonicalInvocationRuntime::new(
        service.clone(),
        clock.clone(),
    ));
    let hooks = Arc::new(InspectionHooks::default());
    let identity = CoreIdentity::production_v5();
    let config = DaemonServerConfig::new(
        state_root.clone(),
        identity.clone(),
        Duration::from_secs(30),
    )
    .with_canonical_runtime_for_test(canonical_runtime)
    .with_runtime_hooks_for_test(hooks.clone());
    let stop = Arc::new(AtomicBool::new(false));
    let server_stop = stop.clone();
    let server_clock = clock.clone();
    let server = thread::spawn(move || {
        *server_clock.listener.lock().unwrap() = Some(thread::current().id());
        run_daemon_configured_until(
            config,
            |runtime| runtime,
            || server_stop.load(Ordering::SeqCst),
        )
    });
    let daemon = InspectionDaemon {
        pause,
        stop,
        server: Some(server),
    };
    wait_for_v5_record(&state_root, &identity);
    let owner = V5DaemonProcessOwner::connect_or_spawn(
        &state_root,
        identity,
        std::path::PathBuf::from("unused-existing-v5-endpoint"),
        Duration::from_secs(2),
    )
    .unwrap();
    let task_id = TaskId::new();
    let invocation_id = InvocationId::new();
    let invocation = V5InvocationRequest::new(
        invocation_id,
        task_id,
        tool,
        serde_json::Map::new(),
        workspace_root.to_string_lossy().into_owned(),
        7_000,
    )
    .unwrap();
    let submit = thread::spawn(move || {
        let mut owner = owner;
        let response = owner.submit_invocation(invocation);
        (owner, response)
    });
    daemon.pause.wait_until_entered();
    clock.elapsed_ms.store(7_000, Ordering::SeqCst);
    let (mut owner, submitted) = submit.join().unwrap();
    let V5ServerResponse::Invocation {
        outcome: V5InvocationResponse::Task { snapshot },
    } = submitted.expect("root inspection returns its task at the response cutoff")
    else {
        panic!("root inspection did not hand off to a task");
    };
    assert_eq!(snapshot.task_id(), task_id);
    assert_eq!(snapshot.invocation_id(), invocation_id);

    clock.elapsed_ms.store(9_000, Ordering::SeqCst);
    watchdog_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("listener evaluated the two-second grace boundary");
    let independent = owner
        .connect_peer_before(Instant::now() + Duration::from_secs(2))
        .map_err(|error| error.to_string())
        .and_then(|mut peer| {
            peer.submit_invocation(
                V5InvocationRequest::new(
                    InvocationId::new(),
                    TaskId::new(),
                    V5ToolIdentity::View,
                    serde_json::Map::new(),
                    independent_workspace.path().to_string_lossy().into_owned(),
                    7_000,
                )
                .unwrap(),
            )
        });
    daemon.pause.release();
    assert!(
        independent.is_ok(),
        "slow root {tool:?} inspection stopped an independent daemon read; fail_stop={}: {independent:?}",
        hooks.fail_stopped.load(Ordering::SeqCst),
    );
    let V5ServerResponse::Invocation {
        outcome: V5InvocationResponse::Direct { receipt },
    } = independent.unwrap()
    else {
        panic!("independent root read did not complete directly");
    };
    assert!(
        matches!(receipt.terminal(), ReceiptTerminalOutcome::Completed { result } if result.ok)
    );

    let settled = owner
        .wait_task(task_id, 7_000)
        .expect("same root task remains reachable");
    let V5ServerResponse::Task {
        snapshot:
            V5DaemonTaskSnapshot::Completed {
                task_id: completed_task_id,
                invocation_id: completed_invocation_id,
                result,
                ..
            },
    } = settled
    else {
        panic!("root task did not terminalize: {settled:?}");
    };
    assert_eq!(completed_task_id, task_id);
    assert_eq!(completed_invocation_id, invocation_id);
    assert!(
        result.ok,
        "expired health inspection still reports workspace facts: {result:?}"
    );
    let data = result.data.unwrap();
    if tool == V5ToolIdentity::Check {
        assert_eq!(data["readinessState"], "incomplete");
        assert_eq!(data["ready"], false);
    } else {
        assert_eq!(data["sourceSets"][0]["name"], "main");
    }
    assert_eq!(
        daemon.pause.entries(),
        1,
        "handoff never replays inspection"
    );
    assert_eq!(service.prepares.load(Ordering::SeqCst), 0);
    assert!(!hooks.fail_stopped.load(Ordering::SeqCst));
    assert_eq!(
        std::fs::read(workspace_root.join("v8project.yaml")).unwrap(),
        project_bytes
    );
    assert_eq!(
        std::fs::read(workspace_root.join("src/Configuration.xml")).unwrap(),
        source_bytes
    );
}

#[test]
fn root_view_keeps_the_same_task_and_daemon_past_the_admission_grace() {
    root_inspection_survives_response_cutoff(V5ToolIdentity::View);
}

#[test]
fn root_check_keeps_the_same_task_and_original_inspection_deadline() {
    root_inspection_survives_response_cutoff(V5ToolIdentity::Check);
}
