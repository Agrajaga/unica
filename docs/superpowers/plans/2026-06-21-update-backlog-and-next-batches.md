# Unica Update Backlog And Next Batches Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring the product and donor-skill update backlog current as of 2026-06-21, then execute the remaining updates in small, independently testable batches.

**Architecture:** Keep bundled product updates separate from donor skill adaptation. New upstream capabilities must cross the typed `unica.*` MCP boundary; skills must not teach raw `bsl-analyzer`, raw `rlm-tools-bsl`, or donor MCP/server contracts. Baselines move only after review/adaptation, not because the upstream tag is newer.

**Tech Stack:** Rust `unica-coder`, Python CI scripts, JSON provenance metadata, packaged Unica plugin tools, GitHub donor repositories, npm Playwright package lock, Python test requirements, Cargo lock.

---

## Current Facts Checked On 2026-06-21

- `bsl-analyzer`: locked `v0.2.37`, latest `v0.2.43`, peeled commit `168e4d166e3a508c0c03a72d5b3fd9973b93bba2`.
- `rlm-tools-bsl` / `rlm-bsl-index`: locked `v1.21.0`, latest `v1.23.0`, peeled commit `3672401335f04bdf43a02aa975e316ac43f92a7c`.
- `v8-runner`: locked `v0.5.1`, latest `v0.5.1`, peeled commit `ad72f64222ab0a7e6dfd391adb437a956c0a2428`.
- `playwright`: package and lock are `1.61.0`; npm latest is `1.61.0`.
- `lxml`: source pin is `tests/ci/requirements.txt: lxml==6.1.1`; PyPI latest is `6.1.1`; local interpreter may still have `6.1.0` until requirements are installed.
- Rust lock drift: `cargo update --dry-run` reports compatible patch updates only: `cc 1.2.64 -> 1.2.65`, `log 0.4.32 -> 0.4.33`.
- Donor drift remains: `cc-1c-skills` has 541 commits since baseline, 148 watched paths, 43 affected skills; `ai-rules-1c` has 23 commits, 36 watched paths, 17 affected skills; `v8-runner-rust` has no drift.

## Task 1: Commit Backlog Refresh

**Files:**
- Modify: `plugins/unica/provenance/reviews/2026-06-18-product-update-backlog.json`
- Modify: `tests/ci/test_skill_provenance.py`
- Create: `docs/superpowers/plans/2026-06-21-update-backlog-and-next-batches.md`

- [ ] **Step 1: Verify product backlog JSON and assertions**

Run:

```bash
python3.12 -m unittest tests.ci.test_skill_provenance.SkillProvenanceTests.test_product_update_backlog_tracks_all_planned_product_batches
```

Expected: PASS.

- [ ] **Step 2: Verify provenance still validates**

Run:

```bash
python3.12 scripts/ci/check-skill-upstreams.py --validate-only
```

Expected: `Skill upstream validation passed`.

- [ ] **Step 3: Commit**

```bash
git add plugins/unica/provenance/reviews/2026-06-18-product-update-backlog.json \
  tests/ci/test_skill_provenance.py \
  docs/superpowers/plans/2026-06-21-update-backlog-and-next-batches.md
git commit -m "Refresh update backlog for June 21"
```

## Task 2: RLM v1.23 Product Review And Update

**Files:**
- Modify: `plugins/unica/third-party/tools.lock.json`
- Modify: `scripts/ci/check-tool-contracts.py`
- Modify: `tests/ci/test_product_contracts.py`
- Modify: `tests/ci/test_skill_provenance.py`
- Modify: `plugins/unica/provenance/reviews/2026-06-18-product-update-backlog.json`

- [ ] **Step 1: Write failing lock expectation**

Add an assertion in `tests/ci/test_skill_provenance.py` that `rlm-tools-bsl` and `rlm-bsl-index` are locked to:

```python
self.assertEqual(locked_tools[name]["version"], "1.23.0")
self.assertEqual(locked_tools[name]["sourceTag"], "v1.23.0")
self.assertEqual(
    locked_tools[name]["sourceCommit"],
    "3672401335f04bdf43a02aa975e316ac43f92a7c",
)
```

Run:

```bash
python3.12 -m unittest tests.ci.test_skill_provenance.SkillProvenanceTests.test_rlm_tools_are_locked_to_reviewed_1_23_0
```

Expected: FAIL while `tools.lock.json` still points to `1.21.0`.

- [ ] **Step 2: Update RLM lock**

In `plugins/unica/third-party/tools.lock.json`, update both `rlm-tools-bsl` and `rlm-bsl-index`:

```json
"version": "1.23.0",
"sourceTag": "v1.23.0",
"sourceCommit": "3672401335f04bdf43a02aa975e316ac43f92a7c"
```

- [ ] **Step 3: Extend product contract gate for v1.23 read-only helper changes**

In `scripts/ci/check-tool-contracts.py`, keep the existing `index build/update/info` checks and add smoke coverage for helper server behavior only if it is routed through package scripts. Do not expose raw RLM MCP helper names in skills.

Required checks:

```bash
python3.12 scripts/ci/build-unica-tools.py --target darwin-arm64 --out-dir .build/unica-tools-smoke
python3.12 scripts/ci/package-unica-plugin.py --tools-root .build/unica-tools-smoke --out-dir .build/unica-package-smoke --allow-partial-targets --no-archives --target darwin-arm64
python3.12 scripts/ci/check-tool-contracts.py --scripts-dir .build/unica-package-smoke/marketplace/plugins/unica/scripts
```

Expected: existing index schema still reports `builder_version=14`, `methods_fts`, `methods`, `modules`, `regions`, and `module_headers`.

- [ ] **Step 4: Verify no forced rebuild is introduced**

Build/update/info a small fixture:

```bash
.build/unica-package-smoke/marketplace/plugins/unica/scripts/run-rlm-bsl-index.sh index build tests/fixtures/unica_mcp_script_parity/meta-remove
.build/unica-package-smoke/marketplace/plugins/unica/scripts/run-rlm-bsl-index.sh index info tests/fixtures/unica_mcp_script_parity/meta-remove
.build/unica-package-smoke/marketplace/plugins/unica/scripts/run-rlm-bsl-index.sh index update tests/fixtures/unica_mcp_script_parity/meta-remove
```

Expected: `builder_version` remains `14`; update does not demand full rebuild solely due to `v1.23.0`.

- [ ] **Step 5: Commit**

```bash
git add plugins/unica/third-party/tools.lock.json scripts/ci/check-tool-contracts.py tests/ci/test_product_contracts.py tests/ci/test_skill_provenance.py plugins/unica/provenance/reviews/2026-06-18-product-update-backlog.json
git commit -m "Update RLM tools to 1.23.0"
```

## Task 3: Decide Whether To Expose RLM v1.23 Capabilities In Unica

**Files:**
- Modify only if contract review confirms a typed boundary is useful:
  - `crates/unica-coder/src/application/mod.rs`
  - `crates/unica-coder/src/application/tool_contracts.rs`
  - `crates/unica-coder/src/infrastructure/internal_adapters.rs`
  - related Rust tests in the same modules
- Otherwise modify only:
  - `plugins/unica/provenance/reviews/2026-06-18-product-update-backlog.json`

- [ ] **Step 1: Review capability fit**

Evaluate `get_object_profile`, exact role/subscription/functional-option helpers, and efficiency hints from `rlm-tools-bsl v1.23.0`.

Decision rules:

```text
Expose through Unica only if the result can be represented as a typed, stable `unica.code.*` or `unica.meta.*` tool.
Do not add raw `rlm_execute`, raw helper names, or donor server guidance to skills.
If the helper is only an internal optimization of upstream RLM, record `ignored-with-reason` and leave skills unchanged.
```

- [ ] **Step 2A: If exposing, add a typed Unica tool**

Candidate contract:

```json
{
  "name": "unica.code.objectProfile",
  "arguments": {
    "name": "Document.SalesOrder",
    "sections": ["structure", "modules", "roles", "subscriptions", "functionalOptions"],
    "includeFlow": false,
    "includeCodeUsages": false,
    "limit": 20,
    "sourceDir": "src",
    "cwd": "<workspace>"
  }
}
```

Tests must prove:

```text
unknown raw args are rejected;
section names are allowlisted;
absence or staleness of index returns ok=true with warning;
old `unica.code.definition`, `outline`, `grep`, and `search` behavior is unchanged.
```

- [ ] **Step 2B: If not exposing, record the decision**

Update product backlog notes:

```json
"notes": "v1.23.0 applied as product update only; get_object_profile and helper batching remain internal upstream optimizations until a typed Unica contract is designed."
```

- [ ] **Step 3: Commit**

Use one of:

```bash
git commit -m "Expose RLM object profile through Unica"
git commit -m "Record RLM v1.23 capability review"
```

## Task 4: bsl-analyzer v0.2.43 Follow-Up Review

**Files:**
- Modify after review:
  - `plugins/unica/third-party/tools.lock.json`
  - `scripts/ci/check-tool-contracts.py`
  - `tests/ci/test_product_contracts.py`
  - `tests/ci/test_skill_provenance.py`
  - selected skills only if routing changes are needed

- [ ] **Step 1: Review breaking surface from `v0.2.37..v0.2.43`**

Focus on changed areas:

```text
CLI analyze behavior and --format=jsonl output;
MCP graph/diagnostics schemas;
extension merge/effective module/weaving diagnostics;
IDE formatting changes that should not leak into Unica MCP contracts;
release asset names bsl-analyzer-app-*.
```

- [ ] **Step 2: Run product gates before lock bump**

Use current `v0.2.37` as control and `v0.2.43` as candidate. Required gates:

```bash
python3.12 scripts/ci/build-unica-tools.py --target darwin-arm64 --out-dir .build/unica-tools-smoke
python3.12 scripts/ci/package-unica-plugin.py --tools-root .build/unica-tools-smoke --out-dir .build/unica-package-smoke --allow-partial-targets --no-archives --target darwin-arm64
.build/unica-package-smoke/marketplace/plugins/unica/scripts/run-bsl-analyzer.sh analyze -s tests/fixtures/unica_mcp_script_parity/meta-remove --format=jsonl
python3.12 scripts/ci/check-tool-contracts.py --scripts-dir .build/unica-package-smoke/marketplace/plugins/unica/scripts
```

- [ ] **Step 3: Decide skill adaptation scope**

If new weaving/extension diagnostics are stable through existing `unica.code.diagnostics`, update only:

```text
code-diagnostics
code-review
cfe-borrow
cfe-patch-method
cfe-validate
release-support
```

Do not create new skills or raw analyzer instructions unless a typed Unica tool is added first.

- [ ] **Step 4: Commit**

```bash
git commit -m "Update bsl-analyzer to 0.2.43"
```

## Task 5: Low-Risk Dependency Refresh

**Files:**
- Modify: `Cargo.lock`
- Possibly update: `plugins/unica/provenance/reviews/2026-06-18-product-update-backlog.json`

- [ ] **Step 1: Apply compatible Rust lock updates**

Run:

```bash
cargo update -p cc -p log
```

Expected changes:

```text
cc 1.2.64 -> 1.2.65
log 0.4.32 -> 0.4.33
```

- [ ] **Step 2: Verify**

Run:

```bash
cargo test --workspace
git diff --check
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add Cargo.lock plugins/unica/provenance/reviews/2026-06-18-product-update-backlog.json
git commit -m "Refresh compatible Rust lock updates"
```

## Task 6: Donor Skill Review Queue

**Files:**
- Modify selected skill files only after review.
- Modify: `plugins/unica/provenance/skill-upstreams.json`
- Modify or add review artifacts under `plugins/unica/provenance/reviews/`

- [ ] **Step 1: Keep product and donor queues separate**

Run:

```bash
python3.12 scripts/ci/check-skill-upstreams.py --check --format json
python3.12 scripts/ci/check-skill-upstreams.py --prepare-upstream-review --format json
```

Expected current high-level drift:

```text
cc-1c-skills: 541 commits, 43 affected skills
ai-rules-1c: 23 commits, 17 affected skills
v8-runner-rust: 0 commits, 0 affected skills
```

- [ ] **Step 2: Review `ai-rules-1c` first**

Order:

```text
code-search
platform-help
code-diagnostics
code-review
test-authoring
api-design
db-performance
query-optimize
remaining domain guidance skills
```

Reason: lower file count than `cc-1c-skills`, and the changed upstream guidance overlaps the new graph/diagnostics/search routing.

- [ ] **Step 3: Review `cc-1c-skills` after product gates**

Order:

```text
web-test
form-compile
form-edit
form-info
form-validate
form-patterns
skd-compile
skd-edit
skd-info
skd-validate
remaining one-file XML DSL skills
```

Reason: 43 affected skills and 148 watched paths is too large for a single commit; split by workflow surface and test fixtures.

- [ ] **Step 4: Advance per-entry baselines only after review**

For every adapted skill, update entry-level `baselineCommit` or decision:

```json
"decision": "ported"
```

or:

```json
"decision": "ignored-with-reason",
"notes": "Upstream change is donor-specific and not applicable after Unica MCP adaptation."
```

Never move the whole upstream baseline just to hide drift.

## Final Gate For Every Batch

Run before each batch commit:

```bash
python3.12 scripts/ci/check-skill-upstreams.py --validate-only
python3.12 -m unittest discover -s tests/ci
python3.12 -m py_compile scripts/ci/*.py tests/ci/*.py
cargo test --workspace
git diff --check
```

Add package gates when `tools.lock.json` changes:

```bash
python3.12 scripts/ci/build-unica-tools.py --target darwin-arm64 --out-dir .build/unica-tools-smoke
python3.12 scripts/ci/package-unica-plugin.py --tools-root .build/unica-tools-smoke --out-dir .build/unica-package-smoke --allow-partial-targets --no-archives --target darwin-arm64
python3.12 scripts/ci/check-tool-contracts.py --scripts-dir .build/unica-package-smoke/marketplace/plugins/unica/scripts
```
