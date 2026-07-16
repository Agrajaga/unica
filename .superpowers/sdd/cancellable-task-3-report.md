# Cancellable Workspace Service — Task 3 Report

## Scope

Implemented internal operation IDs and cancellation-aware workspace-service connector behavior in `workspace_services.rs` only. The concurrent service runtime remains deferred to Task 4.

## RED evidence

Command:

```text
cargo test -p unica-coder cancellable_connector -- --nocapture
```

The new connector test failed to compile for the intended missing contract:

```text
ServiceRequestKind::BslMcp has no field named operation_id
ServiceConnector::send takes 2 arguments but 3 were supplied
no variant named Cancel found for ServiceRequestKind
```

After the production contract was added, the first test-harness version exposed an EOF race because the fixture dropped the work connection. The fixture was corrected to retain that connection while accepting the independent cancel connection.

## Implementation decisions

- `BslMcp` and `RlmReady` carry UUID v4 operation IDs generated at the manager boundary.
- `ServiceConnector::send` receives the caller's `CancellationToken`.
- Response reads poll with a 100 ms socket timeout and preserve the 120-second overall request deadline.
- `WouldBlock` and `TimedOut` continue polling; EOF is reported as a service disconnect; a complete JSON line is deserialized as the response.
- On cancellation, the connector writes `Cancel { operation_id }` over a separate TCP connection and returns a stable `cancelled:` error without waiting for a control response.
- Best-effort cancel connection/write time is bounded to 500 ms. Other control requests use fresh uncancelled tokens through the connector.
- The current sequential server recognizes `Cancel` as a wire-compatible acknowledgement. Actual operation lookup and responsive concurrent control handling remain Task 4.

## GREEN evidence

Focused connector test:

```text
cargo test -p unica-coder cancellable_connector -- --nocapture
1 passed; 0 failed
```

Workspace service suite:

```text
cargo test -p unica-coder workspace_services::tests -- --nocapture
10 passed; 0 failed
```

Full verification:

```text
cargo fmt --all -- --check                         PASS
cargo clippy -p unica-coder --all-targets -- -D warnings  PASS
cargo test -p unica-coder                         289 passed; 0 failed
git diff --check                                  PASS
```

## Risks / follow-up

- Until Task 4 makes the service listener concurrent, a cancel message can queue on the listener but cannot interrupt server-side work immediately. The client nevertheless returns promptly and the operation ID/control message contract is now available for Task 4.
- The cancel send is intentionally best-effort and fire-and-forget so cancellation cannot become blocked waiting for a control response.
