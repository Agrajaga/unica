# BSP Parity Fixtures Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expand `unica-coder` parity coverage by harvesting small, stable fixtures from the pinned BSP (`1c-syntax/ssl_3_2` ref `3.2.1.446`) and adding focused Rust-vs-reference parity scenarios across native XML/DSL tools.

**Architecture:** Keep regular parity tests repo-local and deterministic. Reuse BSP only as an offline fixture source: a harvest script selects and copies minimal real-world XML samples into `tests/fixtures/unica_mcp_script_parity/bsp`, then `tests/ci/test_unica_mcp_script_parity.py` runs Rust MCP output against existing Python reference fixtures without network access.

**Tech Stack:** Python 3.12 unittest harness, Rust `unica-coder` native operation modules, committed XML/JSON fixtures under `tests/fixtures/unica_mcp_script_parity`, existing `scripts/ci/release-assessment.py` BSP download helper.

---

## Current Facts

- CI already downloads BSP in `.github/workflows/unica-plugin-release.yml` through `scripts/ci/release-assessment.py --bsp-ref 3.2.1.446`.
- `scripts/ci/release-assessment.py` clones `https://github.com/1c-syntax/ssl_3_2` and checks the packaged `unica` against `src/cf`.
- Current parity tests are in `tests/ci/test_unica_mcp_script_parity.py`.
- Current reference models are test-only Python scripts under `tests/fixtures/unica_mcp_script_parity/reference_skills`.
- Runtime script fallback is forbidden; native tools must return `command: None`.
- Do not make normal parity tests clone BSP from GitHub. Network-bound release assessment and deterministic parity tests must stay separate.
- Do not commit runtime cache artifacts such as `.build`, `*.db`, `*.db-wal`, or `*.db-shm` into fixture directories.
- BSP parity fixtures are byte-for-byte harvested evidence: preserve upstream BOM, CRLF, and mixed line endings, then verify bytes through manifest `size`/`sha256`. `.gitattributes` marks this subtree `-text -whitespace` so Git does not normalize or reject those fixture bytes.

## Files

- Create: `scripts/ci/harvest-bsp-parity-fixtures.py`
- Create: `tests/fixtures/unica_mcp_script_parity/bsp/manifest.json`
- Create: `tests/fixtures/unica_mcp_script_parity/bsp/cf/Configuration.xml`
- Create: `tests/fixtures/unica_mcp_script_parity/bsp/meta/*`
- Create: `tests/fixtures/unica_mcp_script_parity/bsp/forms/*/Form.xml`
- Create: `tests/fixtures/unica_mcp_script_parity/bsp/skd/*/Template.xml`
- Create: `tests/fixtures/unica_mcp_script_parity/bsp/mxl/*/Template.xml`
- Create: `tests/fixtures/unica_mcp_script_parity/bsp/roles/*/Rights.xml`
- Create: `tests/fixtures/unica_mcp_script_parity/bsp/subsystems/*`
- Modify: `.gitattributes`
- Modify: `tests/ci/test_unica_mcp_script_parity.py`
- Modify: `tests/ci/test_release_assessment.py`
- Modify: `tests/ci/test_package_unica_plugin.py`
- Optional modify: `scripts/ci/release-assessment.py` only if a shared BSP import helper is needed.

## Fixture Selection Rules

Harvest only small text fixtures:

- `Configuration.xml`: one full BSP configuration root.
- Metadata objects: one `Catalog`, one `Document`, one `Report`, one `CommonModule`, one `Enum`, one `InformationRegister`, one `AccumulationRegister`, when present.
- Forms: one form with pages/groups, one form with table data paths, one form with commands/buttons/events, one extension-style form if present.
- SKD templates: one with multiple data sets and links, one with parameters/resources/calculated fields, one with variants/settings/conditional appearance.
- MXL templates: one with named areas and placeholders, one with fonts/styles/borders/merged cells.
- Roles: one role with allowed and denied rights, one with RLS/templates if present.
- Subsystems/command interface: one subsystem with content, one command interface with explicit command visibility.

Reject these files from committed fixtures:

- files larger than 256 KiB, unless the test explicitly needs the size and the reason is written in `manifest.json`;
- binary files;
- `.build/**`;
- `*.db`, `*.db-wal`, `*.db-shm`;
- generated assessment reports.

Do not normalize harvested BSP line endings. The manifest hash is the integrity
check for these fixtures; Git whitespace checks are disabled only for the BSP
fixture subtree.

## Task 1: BSP Fixture Harvester

**Files:**
- Create: `scripts/ci/harvest-bsp-parity-fixtures.py`
- Create: `tests/fixtures/unica_mcp_script_parity/bsp/manifest.json`
- Test: `tests/ci/test_release_assessment.py`

- [x] **Step 1: Write a failing test for deterministic harvest**

Add this test to `tests/ci/test_release_assessment.py`:

```python
def test_bsp_parity_harvest_selects_text_fixtures_and_writes_manifest(self) -> None:
    module_path = Path(__file__).resolve().parents[2] / "scripts" / "ci" / "harvest-bsp-parity-fixtures.py"
    spec = importlib.util.spec_from_file_location("harvest_bsp_parity_fixtures", module_path)
    self.assertIsNotNone(spec)
    self.assertIsNotNone(spec.loader)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)

    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        bsp = root / "bsp"
        src = bsp / "src" / "cf"
        (src / "Catalogs" / "Партнеры" / "Forms" / "ФормаЭлемента" / "Ext").mkdir(parents=True)
        (src / "Reports" / "ОтчетПродажи" / "Templates" / "ОсновнаяСхемаКомпоновкиДанных" / "Ext").mkdir(parents=True)
        (src / "Roles" / "ПолныеПрава" / "Ext").mkdir(parents=True)
        (src / ".build").mkdir(parents=True)
        (src / ".build" / "bsl-search.db").write_bytes(b"cache")
        (src / "Configuration.xml").write_text("<MetaDataObject/>", encoding="utf-8")
        (src / "Catalogs" / "Партнеры.xml").write_text("<MetaDataObject/>", encoding="utf-8")
        (src / "Catalogs" / "Партнеры" / "Forms" / "ФормаЭлемента" / "Ext" / "Form.xml").write_text(
            "<Form/>", encoding="utf-8"
        )
        (src / "Reports" / "ОтчетПродажи" / "Templates" / "ОсновнаяСхемаКомпоновкиДанных" / "Ext" / "Template.xml").write_text(
            "<DataCompositionSchema/>", encoding="utf-8"
        )
        (src / "Roles" / "ПолныеПрава" / "Ext" / "Rights.xml").write_text("<Rights/>", encoding="utf-8")

        out = root / "fixtures"
        manifest = module.harvest(bsp_root=bsp, out_root=out, bsp_ref="test-ref", bsp_commit="abc123")

        self.assertEqual(manifest["bsp"]["ref"], "test-ref")
        self.assertEqual(manifest["bsp"]["commit"], "abc123")
        harvested = sorted(path.relative_to(out).as_posix() for path in out.rglob("*") if path.is_file())
        self.assertIn("manifest.json", harvested)
        self.assertIn("cf/Configuration.xml", harvested)
        self.assertTrue(any(path.startswith("forms/") and path.endswith("/Form.xml") for path in harvested))
        self.assertTrue(any(path.startswith("skd/") and path.endswith("/Template.xml") for path in harvested))
        self.assertTrue(any(path.startswith("roles/") and path.endswith("/Rights.xml") for path in harvested))
        self.assertFalse(any(".build" in path or path.endswith(".db") for path in harvested))
```

- [x] **Step 2: Run the failing test**

Run:

```bash
python3.12 -m unittest tests.ci.test_release_assessment.ReleaseAssessmentTests.test_bsp_parity_harvest_selects_text_fixtures_and_writes_manifest
```

Expected: FAIL because `scripts/ci/harvest-bsp-parity-fixtures.py` does not exist.

- [x] **Step 3: Implement `scripts/ci/harvest-bsp-parity-fixtures.py`**

The script must expose:

```python
def harvest(*, bsp_root: Path, out_root: Path, bsp_ref: str, bsp_commit: str) -> dict[str, Any]:
    ...
```

and CLI:

```bash
python3.12 scripts/ci/harvest-bsp-parity-fixtures.py \
  --bsp-root .build/release-assessment/bsp/ssl_3_2 \
  --out-root tests/fixtures/unica_mcp_script_parity/bsp \
  --bsp-ref 3.2.1.446 \
  --bsp-commit <commit>
```

Implementation rules:

- use `Path.rglob`, not shelling out;
- copy only UTF-8 text XML/BSL/JSON fixtures;
- write fixture payloads with `write_bytes` and do not normalize BOM or line endings;
- select deterministic first matches by sorted relative path and scoring helpers;
- write `manifest.json` with source path, target path, file size, sha256, and category;
- delete and recreate `out_root` on each harvest;
- never copy `.build`, `*.db`, `*.db-wal`, `*.db-shm`.

- [x] **Step 4: Run the test again**

Run:

```bash
python3.12 -m unittest tests.ci.test_release_assessment.ReleaseAssessmentTests.test_bsp_parity_harvest_selects_text_fixtures_and_writes_manifest
```

Expected: PASS.

## Task 2: Fixture Hygiene Guardrails

**Files:**
- Modify: `tests/ci/test_package_unica_plugin.py`
- Test: `tests/ci/test_package_unica_plugin.py`

- [x] **Step 1: Add a failing guardrail test**

Add:

```python
def test_parity_fixtures_do_not_contain_runtime_cache_artifacts(self) -> None:
    fixture_root = self.repo_root() / "tests" / "fixtures" / "unica_mcp_script_parity"
    forbidden = []
    for path in fixture_root.rglob("*"):
        rel = path.relative_to(fixture_root).as_posix()
        if "/.build/" in f"/{rel}/" or rel.endswith((".db", ".db-wal", ".db-shm")):
            forbidden.append(rel)
    self.assertEqual(forbidden, [])
```

- [x] **Step 2: Run the guardrail**

Run:

```bash
python3.12 -m unittest tests.ci.test_package_unica_plugin.PackageUnicaPluginTests.test_parity_fixtures_do_not_contain_runtime_cache_artifacts
```

Expected before cleanup: FAIL if local `.build` cache files are still under fixture root.

- [x] **Step 3: Remove local cache artifacts and make the guardrail pass**

Remove only untracked runtime cache files under `tests/fixtures/unica_mcp_script_parity/**/.build`.

Run:

```bash
git status --short
python3.12 -m unittest tests.ci.test_package_unica_plugin.PackageUnicaPluginTests.test_parity_fixtures_do_not_contain_runtime_cache_artifacts
```

Expected: PASS and no tracked cache removals unless such files were previously committed by mistake.

## Task 3: Form Parity From BSP Fixtures

**Files:**
- Modify: `tests/ci/test_unica_mcp_script_parity.py`
- Create: `tests/fixtures/unica_mcp_script_parity/bsp/forms/*`

- [x] **Step 1: Add read-only BSP form scenarios**

Add scenarios:

```python
ParityScenario(
    name="bsp-form-info-real-form-full",
    tool="unica.form.info",
    skill="form-info",
    script="form-info.py",
    arguments={"FormPath": "src/Form.xml", "Mode": "full", "Limit": 200},
    fixtures=(FileFixture("bsp/forms/command-rich/Form.xml", "src/Form.xml"),),
    expect_ok=True,
)
ParityScenario(
    name="bsp-form-validate-real-form-detailed",
    tool="unica.form.validate",
    skill="form-validate",
    script="form-validate.py",
    arguments={"FormPath": "src/Form.xml", "Detailed": True, "MaxErrors": 80},
    fixtures=(FileFixture("bsp/forms/command-rich/Form.xml", "src/Form.xml"),),
    expect_ok=True,
)
```

- [x] **Step 2: Add mutating form-edit clone scenario**

Use a copied BSP form, not the synthetic `form-simple.json`.

```python
ParityScenario(
    name="bsp-form-edit-add-attribute-command-element",
    tool="unica.form.edit",
    skill="form-edit",
    script="form-edit.py",
    arguments={
        "FormPath": "src/Form.xml",
        "JsonPath": "fixtures/form-edit-bsp-additions.json",
    },
    fixtures=(
        FileFixture("bsp/forms/command-rich/Form.xml", "src/Form.xml"),
        FileFixture("form-edit/bsp-additions.json", "fixtures/form-edit-bsp-additions.json"),
    ),
    expect_ok=True,
    compare_files=True,
)
```

Fixture `form-edit/bsp-additions.json` must add one attribute, one command, one button, and one field to hit ID allocation and insertion into non-trivial existing XML.

- [x] **Step 3: Run targeted parity**

Run:

```bash
python3.12 -m unittest tests.ci.test_unica_mcp_script_parity.UnicaMcpScriptParityTests.test_mcp_calls_match_reference_python_scripts
```

Expected: new BSP form scenarios pass and `command` remains `None`.

## Task 4: SKD Parity From BSP Fixtures

**Files:**
- Modify: `tests/ci/test_unica_mcp_script_parity.py`
- Create: `tests/fixtures/unica_mcp_script_parity/bsp/skd/*`

- [x] **Step 1: Add `skd.info` mode coverage**

Add one scenario per mode against a real BSP DCS fixture:

```python
for mode in ("overview", "query", "fields", "links", "calculated", "resources", "params", "variant", "trace", "templates", "full"):
    ParityScenario(
        name=f"bsp-skd-info-{mode}",
        tool="unica.skd.info",
        skill="skd-info",
        script="skd-info.py",
        arguments={"TemplatePath": "src/Template.xml", "Mode": mode, "Limit": 200},
        fixtures=(FileFixture("bsp/skd/full-featured/Template.xml", "src/Template.xml"),),
        expect_ok=True,
    )
```

If the harness keeps scenarios as a static list, expand this loop into explicit `ParityScenario(...)` entries.

- [x] **Step 2: Add grouped SKD edit scenarios**

Add four real-file `compare_files=True` scenarios:

1. `bsp-skd-edit-query`: `set-query`, then `patch-query @once`.
2. `bsp-skd-edit-fields-and-resources`: `modify-field`, `set-field-role`, `add-total`, `remove-total`.
3. `bsp-skd-edit-params`: `add-parameter`, `modify-parameter`, `rename-parameter`, `reorder-parameters`.
4. `bsp-skd-edit-settings`: `add-variant`, `set-structure`, `modify-structure`, `add-filter`, `remove-filter`, `add-conditionalAppearance`, `clear-conditionalAppearance`, `add-drilldown`.

Each scenario must run the same operation sequence in direct reference and MCP workspaces and compare the whole workspace snapshot.

- [x] **Step 3: Add one failure parity scenario for real SKD**

Add:

```python
ParityScenario(
    name="bsp-skd-edit-missing-variant-fails",
    tool="unica.skd.edit",
    skill="skd-edit",
    script="skd-edit.py",
    arguments={
        "TemplatePath": "src/Template.xml",
        "Operation": "add-selection",
        "Value": "Amount",
        "Variant": "DefinitelyMissingVariant",
    },
    fixtures=(FileFixture("bsp/skd/full-featured/Template.xml", "src/Template.xml"),),
    expect_ok=False,
)
```

- [x] **Step 4: Run targeted parity**

Run:

```bash
python3.12 -m unittest tests.ci.test_unica_mcp_script_parity.UnicaMcpScriptParityTests.test_mcp_calls_match_reference_python_scripts
```

Expected: Rust and reference script stdout/stderr and file snapshots match.

## Task 5: Metadata, CF, CFE, Subsystem, Interface, Template

**Files:**
- Modify: `tests/ci/test_unica_mcp_script_parity.py`
- Create: `tests/fixtures/unica_mcp_script_parity/bsp/meta/*`
- Create: `tests/fixtures/unica_mcp_script_parity/bsp/subsystems/*`

- [x] **Step 1: Add BSP `cf.info` and `cf.validate` scenarios**

Use `bsp/cf/Configuration.xml` with:

- `unica.cf.info` mode `brief`;
- `unica.cf.info` mode `full`;
- `unica.cf.validate` detailed output.

- [x] **Step 2: Add BSP `meta.info` and `meta.validate` scenarios**

Use real BSP metadata XML for:

- catalog;
- document;
- report;
- common module;
- enum;
- information register;
- accumulation register.

For each object, add:

```python
ParityScenario(
    name="bsp-meta-info-catalog-full",
    tool="unica.meta.info",
    skill="meta-info",
    script="meta-info.py",
    arguments={"ObjectPath": "src/Catalog.xml", "Mode": "full", "Limit": 200},
    fixtures=(FileFixture("bsp/meta/catalog/Catalog.xml", "src/Catalog.xml"),),
    expect_ok=True,
)
```

and matching `unica.meta.validate` with `Detailed=True`.

- [x] **Step 3: Add CFE borrow against BSP catalog/document**

Use a minimal generated extension from `cfe-init` and a real BSP object:

```python
ParityScenario(
    name="bsp-cfe-borrow-real-catalog-with-form",
    tool="unica.cfe.borrow",
    skill="cfe-borrow",
    script="cfe-borrow.py",
    arguments={
        "ExtensionPath": "src-cfe",
        "ConfigPath": "src",
        "ObjectPath": "Catalogs/Партнеры",
        "BorrowForms": True,
        "BorrowModules": True,
    },
    fixtures=(
        FileFixture("bsp/meta/catalog-object-tree", "src/Catalogs/Партнеры"),
    ),
    setup_steps=(
        SetupStep(
            skill="cfe-init",
            script="cfe-init.py",
            arguments={"Name": "ParityExtension", "NamePrefix": "PE_", "OutputDir": "src-cfe", "NoRole": True},
        ),
    ),
    expect_ok=True,
    compare_files=True,
)
```

If `FileFixture` cannot copy directories, extend it in `tests/ci/test_unica_mcp_script_parity.py` to copy directories with `shutil.copytree`.

- [x] **Step 4: Add subsystem/interface/template scenarios**

Add:

- `bsp-subsystem-info-full` for real subsystem XML;
- `bsp-subsystem-validate-detailed`;
- `bsp-interface-validate-real-command-interface`;
- `bsp-interface-edit-hide-show-real-command`;
- `bsp-template-remove-real-template-from-report-copy`;
- `bsp-template-add-real-report-copy`.

All mutating scenarios must set `compare_files=True`.

## Task 6: Role and MXL Parity From BSP Fixtures

**Files:**
- Modify: `tests/ci/test_unica_mcp_script_parity.py`
- Create: `tests/fixtures/unica_mcp_script_parity/bsp/roles/*`
- Create: `tests/fixtures/unica_mcp_script_parity/bsp/mxl/*`

- [x] **Step 1: Add role scenarios**

Add:

- `bsp-role-info-full`;
- `bsp-role-info-show-denied`;
- `bsp-role-validate-detailed`;
- `bsp-role-validate-rls-template` when the harvested role has RLS.

Use real `Rights.xml` from BSP and keep expected output compared to reference script.

- [x] **Step 2: Add MXL scenarios**

Add:

- `bsp-mxl-info-real-template`;
- `bsp-mxl-validate-real-template`;
- `bsp-mxl-decompile-real-template-outfile`;
- `bsp-mxl-roundtrip-real-template`: decompile real template to JSON, compile it back, compare Rust and reference workspace snapshots.

## Task 7: Coverage Accounting

**Files:**
- Modify: `tests/ci/test_unica_mcp_script_parity.py`

- [x] **Step 1: Add coverage expectation tables**

Add:

```python
BSP_PARITY_REQUIRED_TOOLS = {
    "unica.cf.info",
    "unica.cf.validate",
    "unica.meta.info",
    "unica.meta.validate",
    "unica.form.info",
    "unica.form.validate",
    "unica.form.edit",
    "unica.skd.info",
    "unica.skd.validate",
    "unica.skd.edit",
    "unica.mxl.info",
    "unica.mxl.validate",
    "unica.mxl.decompile",
    "unica.role.info",
    "unica.role.validate",
    "unica.subsystem.info",
    "unica.subsystem.validate",
    "unica.interface.validate",
}
```

Add a test:

```python
def test_bsp_fixture_parity_covers_real_world_read_and_edit_tools(self) -> None:
    covered = {scenario.tool for scenario in SCENARIOS if scenario.name.startswith("bsp-")}
    self.assertEqual(covered & BSP_PARITY_REQUIRED_TOOLS, BSP_PARITY_REQUIRED_TOOLS)
```

- [x] **Step 2: Add SKD edit operation accounting**

Add:

```python
SKD_EDIT_REQUIRED_OPS = {
    "add-total",
    "add-calculated-field",
    "add-parameter",
    "add-filter",
    "add-dataParameter",
    "add-dataSetLink",
    "add-dataSet",
    "add-variant",
    "add-conditionalAppearance",
    "add-drilldown",
    "set-outputParameter",
    "set-query",
    "patch-query",
    "set-structure",
    "modify-field",
    "modify-filter",
    "modify-dataParameter",
    "modify-parameter",
    "modify-structure",
    "set-field-role",
    "rename-parameter",
    "reorder-parameters",
    "clear-conditionalAppearance",
    "remove-total",
    "remove-calculated-field",
    "remove-filter",
}
```

Add a helper that scans `scenario.arguments` and `scenario.setup_steps` for `Operation`. Add a test that requires every op above to appear in at least one successful `compare_files=True` parity scenario.

## Task 8: Verification

**Files:**
- No new files beyond previous tasks.

- [x] **Step 1: Run focused checks**

Run:

```bash
python3.12 -m unittest tests.ci.test_release_assessment tests.ci.test_package_unica_plugin tests.ci.test_unica_mcp_script_parity
```

Expected: PASS.

- [x] **Step 2: Run Rust package checks**

Run:

```bash
cargo fmt --all -- --check
cargo test --package unica-coder
cargo clippy --package unica-coder --all-targets --all-features -- -D warnings
```

Expected: PASS.

- [x] **Step 3: Run provenance/package checks**

Run:

```bash
python3.12 -m unittest tests.ci.test_unica_skills tests.ci.test_skill_provenance tests.ci.test_package_unica_plugin
python3.12 scripts/ci/check-skill-upstreams.py --check --format json
git diff --check
```

Expected: PASS and `check-skill-upstreams.py` returns `"errors": []`.

## Self-Review

- Spec coverage: The plan uses the existing BSP download path only as a fixture source, keeps parity deterministic, and expands coverage across all native XML/DSL families in `crates/unica-coder/src/infrastructure/native_operations`.
- Gap called out: `cfe.*` has no natural BSP extension corpus, so CFE parity should combine synthetic extension setup with real BSP base objects.
- Risk called out: Full BSP must not be committed or cloned in normal parity CI.
- Placeholder scan: No `TBD` or open-ended "add tests" steps remain; each task names concrete files, scenario classes, and commands.
