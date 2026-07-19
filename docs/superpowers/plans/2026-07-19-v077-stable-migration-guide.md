# Stable v0.7.7 Migration Guide Implementation Plan

> **Status:** Historical execution context after completion; live contracts are README and tests.
>
> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give users a version-only upgrade table backed by an immutable v0.7.7 migration catalog.

**Architecture:** Keep the released installer assets unchanged and pass them a dedicated signed marketplace ref created after v0.7.7 promotion. Lock the short README copy with a product-contract test.

**Tech Stack:** Git signed tags, Markdown, Python `unittest`, Codex CLI.

## Global Constraints

- Do not move or replace the existing source or marketplace `v0.7.7` tags.
- Use `0.7.5` as the ordinary-update boundary.
- Keep the README section version-based and free of installation-layout internals.
- Use the published `v0.7.7` installer assets with `migration-v0.7.7`.

---

### Task 1: Immutable migration catalog

**Files:**
- No repository file changes.

**Interfaces:**
- Consumes: marketplace promotion merge `81531ed115279ed3ecb53f181d46edaf6d508056`.
- Produces: signed remote tag `migration-v0.7.7`.

- [ ] **Step 1: Verify the target tree**

Run `git show` for the catalog and plugin descriptor at the promotion merge.
Expected: catalog ref `v0.7.7`; plugin version `0.7.7`.

- [ ] **Step 2: Create and verify the signed tag**

Run `git tag -s migration-v0.7.7 81531ed115279ed3ecb53f181d46edaf6d508056 -m "Unica migration catalog v0.7.7"` and `git tag -v migration-v0.7.7`.
Expected: valid local signature and the exact promotion commit.

- [ ] **Step 3: Push and re-read the remote tag**

Run `git push origin refs/tags/migration-v0.7.7` and verify its peeled SHA plus both contract files through the remote.
Expected: the remote tag resolves to the promotion merge, catalog `v0.7.7`, plugin `0.7.7`.

### Task 2: README product contract

**Files:**
- Modify: `tests/ci/test_product_contracts.py`
- Modify: `README.md`

**Interfaces:**
- Consumes: remote `migration-v0.7.7` tag and published installer URLs.
- Produces: `test_readme_documents_the_stable_v077_migration_guide`.

- [ ] **Step 1: Write the failing test**

Replace the current frozen-bridge assertions with assertions for `codex plugin list`, table headings, ranges `0.3.0`–`0.7.4` and `0.7.5`+, both installer URLs, explicit `migration-v0.7.7`, and absence of internal terminology from the section.

- [ ] **Step 2: Run the focused test and verify RED**

Run `python3.12 -m unittest tests.ci.test_product_contracts.ProductContractTests.test_readme_documents_the_stable_v077_migration_guide -v`.
Expected: FAIL because the current README lacks `migration-v0.7.7` and the approved table.

- [ ] **Step 3: Write the minimal README section**

Replace only `Переход со старой установки и откат` with the approved version command, two-row table, platform commands, update commands, and one rollback sentence.

- [ ] **Step 4: Run the focused test and verify GREEN**

Run the focused unittest again.
Expected: PASS.

### Task 3: Verification and publication

**Files:**
- Verify all files changed by Tasks 1 and 2.

**Interfaces:**
- Consumes: the signed remote tag and green focused test.
- Produces: a reviewable GitHub pull request against `main`.

- [ ] **Step 1: Run product contracts**

Run `python3.12 -m unittest tests.ci.test_product_contracts -v`.
Expected: all tests pass.

- [ ] **Step 2: Inspect the final diff and remote tag**

Run `git diff --check`, inspect `git diff`, and re-read `migration-v0.7.7` from GitHub.
Expected: no whitespace errors, only accepted documentation/test changes, exact remote versions.

- [ ] **Step 3: Commit, push, and open the PR**

Commit as `docs: simplify the legacy migration guide`, push branch `codex/fix-v077-migration-guide`, and open a ready PR against `main`.
Expected: PR contains the approved simple README contract.
