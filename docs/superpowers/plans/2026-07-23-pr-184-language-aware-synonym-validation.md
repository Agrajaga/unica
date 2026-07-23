# PR 184 Language-Aware Synonym Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `unica.meta.validate` check the effective command-interface text for every applicable configuration language without diagnosing intentionally empty synonyms.

**Architecture:** Add small localized-text and configuration-language helpers beside the existing native metadata validator, then pass the resolved language set into property validation. Mirror the same deterministic algorithm in the Python parity oracle. Keep missing translations and malformed/missing language files outside this rule.

**Tech Stack:** Rust 2021, `roxmltree`, Python 3.12, `lxml`, unittest-based repository checks.

## Global Constraints

- Keep the public MCP boundary unchanged as `unica.meta.validate`.
- Remove `Synonym is empty`; missing translations are valid.
- Resolve configured codes from `Configuration.xml` and `Languages/<Name>.xml`.
- Use non-empty `ListPresentation`, otherwise non-empty `Synonym`, per language.
- Check every selected text against 38 Unicode characters.
- For standalone XML, use observed `v8:lang` values; check language-neutral non-empty items when no code is available.
- Rust and Python oracle outputs must remain byte-for-byte compatible after path normalization.
- Treat all adapted metadata conventions as general project rules.

---

### Task 1: Native language-aware command text validation

**Files:**
- Modify: `crates/unica-coder/src/infrastructure/native_operations/meta.rs`

**Interfaces:**
- Produces: `meta_validate_language_codes(config_dir: Option<&Path>) -> Vec<String>`
- Produces: `meta_validate_localized_values(node: Option<roxmltree::Node<'_, '_>>) -> Vec<(Option<String>, String)>`
- Changes: `meta_validate_check_properties(..., language_codes: &[String])`

- [ ] **Step 1: Add failing Rust behavior tests**

Extend the existing `edit_tests` helpers to create a complete configuration
fixture containing `Configuration.xml`, `Languages/Русский.xml`, and
`Languages/English.xml`. Add focused tests equivalent to:

```rust
#[test]
fn validate_meta_checks_every_configured_language() {
    let synonym = local_string(&[
        ("ru", "Отгрузка"),
        ("en", "A very long shipment title intended for the command interface"),
    ]);
    let stdout = validate_in_bilingual_config(synonym, None);
    assert!(stdout.contains("language 'en'"), "{stdout}");
}

#[test]
fn validate_meta_prefers_list_presentation_per_language() {
    let synonym = local_string(&[
        ("ru", "Очень длинное наименование для командного интерфейса"),
        ("en", "Shipment"),
    ]);
    let list = local_string(&[("ru", "Отгрузки")]);
    let stdout = validate_in_bilingual_config(synonym, Some(list));
    assert!(!stdout.contains("language 'ru'"), "{stdout}");
}

#[test]
fn validate_meta_allows_empty_synonym_and_unfinished_translation() {
    let stdout = validate_stdout_with_synonym(
        "empty-synonym",
        "<Synonym><v8:item><v8:lang>en</v8:lang><v8:content/></v8:item></Synonym>",
    );
    assert!(!stdout.contains("Synonym is empty"), "{stdout}");
}
```

Also cover a standalone XML with multiple observed languages and a localized
item without `v8:lang`.

- [ ] **Step 2: Run the new Rust tests and verify RED**

Run:

```bash
cargo test -p unica-coder validate_meta_ -- --nocapture
```

Expected: the new multilingual and `ListPresentation` tests fail because the
current code reads only the first synonym item; the empty-synonym test fails
because the current warning is still emitted.

- [ ] **Step 3: Implement localized-value parsing**

Add a helper that preserves all items and trims only for emptiness:

```rust
fn meta_validate_localized_values(
    node: Option<roxmltree::Node<'_, '_>>,
) -> Vec<(Option<String>, String)> {
    let Some(node) = node else { return Vec::new() };
    meta_info_children(node, "item")
        .into_iter()
        .filter_map(|item| {
            let language = meta_info_child_text(item, "lang")
                .filter(|value| !value.trim().is_empty());
            let text = meta_info_child_text(item, "content").unwrap_or_default();
            (!text.trim().is_empty()).then_some((language, text))
        })
        .collect()
}
```

- [ ] **Step 4: Implement configuration language resolution**

From the nearest `config_dir`, parse `Configuration.xml`, enumerate
`Configuration/ChildObjects/Language`, then read
`Languages/<Name>.xml/Language/Properties/LanguageCode`. Return unique,
non-empty codes in declaration order. Treat read/parse failures as an empty
result so this convention check does not duplicate `cf-validate`.

- [ ] **Step 5: Select and validate effective command text**

Pass the resolved codes into `meta_validate_check_properties`. For each
configured code, select non-empty `ListPresentation`, otherwise `Synonym`.
When no codes resolve, derive codes from both properties in encounter order.
If no coded items exist, check every non-empty language-neutral item.

Emit:

```text
3. Properties: ListPresentation '...' is longer than 38 characters (N) for the command interface, language 'en'
```

Remove the empty-synonym warning. Keep the informational
`Synonym present`/`no Synonym` summary based on whether any non-empty synonym
item exists.

- [ ] **Step 6: Run Rust tests and verify GREEN**

Run:

```bash
cargo test -p unica-coder validate_meta_ -- --nocapture
cargo test -p unica-coder infrastructure::native_operations::meta
```

Expected: all selected tests pass with zero failures.

- [ ] **Step 7: Commit the native validator change**

```bash
git add crates/unica-coder/src/infrastructure/native_operations/meta.rs
git -c commit.gpgsign=false commit -m "Исправить языковую проверку представлений"
```

---

### Task 2: Python parity oracle and parity scenario

**Files:**
- Modify: `tests/fixtures/unica_mcp_script_parity/reference_skills/meta-validate/scripts/meta-validate.py`
- Modify: `tests/ci/test_unica_mcp_script_parity.py`

**Interfaces:**
- Consumes: warning format and selection rules from Task 1.
- Produces: Python `localized_values(node)` and `configuration_language_codes(config_dir)`.

- [ ] **Step 1: Add a failing parity scenario**

Create fixtures through `FileFixture` entries for a bilingual
`Configuration.xml`, both `Languages/*.xml`, and a document containing a long
English synonym plus a short Russian list presentation. Register a
`ParityScenario` for `unica.meta.validate` so native and script output are
compared.

- [ ] **Step 2: Run the scenario and verify RED**

Run:

```bash
python3.12 tests/ci/test_unica_mcp_script_parity.py \
  -k meta_validate_language_aware
```

Expected: mismatch between the fixed native output and the first-item Python
implementation.

- [ ] **Step 3: Mirror the algorithm in Python**

Implement:

```python
def localized_values(node):
    values = []
    if node is None:
        return values
    for item in node.findall("v8:item", NS):
        lang = inner_text(find(item, "v8:lang")).strip()
        text = inner_text(find(item, "v8:content"))
        if text.strip():
            values.append((lang or None, text))
    return values
```

Add `configuration_language_codes(config_dir)` using `lxml` and the same
declaration-order, silent-fallback behavior as Rust. Apply the same per-language
`ListPresentation → Synonym` selection and warning text. Remove
`Synonym is empty`.

- [ ] **Step 4: Run parity and oracle checks**

Run:

```bash
python3.12 tests/ci/test_unica_mcp_script_parity.py
```

Expected: all scenarios pass.

- [ ] **Step 5: Commit parity changes**

```bash
git add tests/fixtures/unica_mcp_script_parity/reference_skills/meta-validate/scripts/meta-validate.py tests/ci/test_unica_mcp_script_parity.py
git -c commit.gpgsign=false commit -m "Синхронизировать языковую проверку meta-validate"
```

---

### Task 3: Reference and provenance corrections

**Files:**
- Modify: `plugins/unica/references/platform/metadata-conventions.md`
- Modify: `plugins/unica/ATTRIBUTIONS.md`
- Modify: `plugins/unica/provenance/skill-upstreams.json`
- Modify: `tests/ci/test_attributions.py` or `tests/ci/test_skill_provenance.py`

**Interfaces:**
- Consumes: final validator behavior from Tasks 1–2.
- Produces: complete reader-facing and machine-checked provenance scope.

- [ ] **Step 1: Add a failing semantic provenance assertion**

Add an assertion that the `templates-new-object-1c` entry names all adopted
areas:

```python
for phrase in (
    "naming",
    "synonym",
    "representation",
    "fill-check",
    "catalog code",
    "information-register command-interface",
):
    self.assertIn(phrase, entry["notes"])
```

- [ ] **Step 2: Run the provenance test and verify RED**

Run:

```bash
python3.12 tests/ci/test_skill_provenance.py
```

Expected: failure on `catalog code` or
`information-register command-interface`.

- [ ] **Step 3: Correct documentation and provenance**

Change the reference introduction so only deterministic length checks are
assigned to `meta-validate`; explicitly state that synonym completeness is a
manual semantic review because empty values and unfinished translations may be
intentional. Expand both provenance notes and attribution prose with the full
adopted scope. Do not add a 1C:Accounting limitation.

- [ ] **Step 4: Run documentation contract tests**

Run:

```bash
python3.12 tests/ci/test_attributions.py
python3.12 tests/ci/test_skill_provenance.py
python3.12 tests/ci/test_package_unica_plugin.py
```

Expected: all tests pass.

- [ ] **Step 5: Commit documentation corrections**

```bash
git add plugins/unica/references/platform/metadata-conventions.md plugins/unica/ATTRIBUTIONS.md plugins/unica/provenance/skill-upstreams.json tests/ci/test_skill_provenance.py
git -c commit.gpgsign=false commit -m "Уточнить контракт соглашений по метаданным"
```

---

### Task 4: Full verification and review handoff

**Files:**
- Verify all files changed since `origin/pr-184`.

**Interfaces:**
- Consumes: all prior tasks.
- Produces: a clean, reviewable branch based on PR 184.

- [ ] **Step 1: Run formatting and diff checks**

```bash
cargo fmt --all -- --check
git diff --check origin/pr-184...HEAD
```

Expected: both commands exit zero.

- [ ] **Step 2: Run complete relevant verification**

```bash
cargo test -p unica-coder
python3.12 tests/ci/test_unica_mcp_script_parity.py
python3.12 tests/ci/test_attributions.py
python3.12 tests/ci/test_skill_provenance.py
python3.12 tests/ci/test_package_unica_plugin.py
```

Expected: every command exits zero with no failed tests.

- [ ] **Step 3: Audit the final diff against the design**

Verify explicitly:

- no `Synonym is empty` warning remains;
- every configured or observed language is handled;
- `ListPresentation` wins per language;
- missing translations do not warn;
- Rust and Python messages match;
- full adapted scope appears in provenance and attribution;
- no unrelated files changed.

- [ ] **Step 4: Commit any verification-only formatting change**

If `cargo fmt` changed tracked files, stage only the files in this plan and
commit:

```bash
git -c commit.gpgsign=false commit -m "Отформатировать исправления PR 184"
```

- [ ] **Step 5: Present the branch for user review**

Report the branch, commits, exact test counts, remaining risks, and whether the
changes should be pushed to a new PR or supplied to the author of PR 184.
