# Dependency Refresh And Capability Adoption Plan

Date: 2026-06-26

## Scope

Refresh dependencies and bundled-tool pins only where current upstream state is verified, then adopt new functionality only when it fits an existing Unica boundary.

This PR scope is intentionally narrow:

- update reviewed bundled-tool pins for `bsl-analyzer` and RLM tools;
- apply compatible Rust lockfile updates;
- update Playwright for `web-test`;
- expose Playwright WebStorage through stable `web-test` helpers;
- record deferred direct dependency migrations instead of mixing them into this refresh.

## Verified Version State

- `bsl-analyzer`: latest `v0.2.48`, published 2026-06-24, peeled commit `12cce48b47999ff46c9afc01aec3b7cc438b63fc`.
- `rlm-tools-bsl` / `rlm-bsl-index`: latest `v1.25.0`, published 2026-06-22, peeled commit `3da3ca8ea27e1b893283e5053a7158765f3a01c6`.
- `v8-runner`: already current at `v0.5.1`.
- Cargo compatible lock drift: `cc`, `log`, `quote`, `rustls`.
- Playwright: `1.61.1`.
- `lxml`: source CI pin already current at `6.1.1`.

## Applied Batches

1. Product backlog and lock metadata
   - Update `plugins/unica/provenance/reviews/2026-06-18-product-update-backlog.json`.
   - Update `plugins/unica/third-party/tools.lock.json`.
   - Keep provenance tests as the reviewed baseline for tool tags and commits.

2. Low-risk dependency refresh
   - Apply lock-only Cargo updates for compatible transitive crates.
   - Update `plugins/unica/skills/web-test/scripts/package.json` and lockfile to Playwright `1.61.1`.

3. Capability adoption
   - Use Playwright `page.localStorage` / `page.sessionStorage`.
   - Expose the feature as `getStorage`, `setStorage`, `removeStorage`, and `clearStorage` from `web-test` rather than sending users to raw Playwright internals.
   - Document the helpers in `SKILL.md` and `regress.md`.

## Deferred Work

- `serde_yaml` replacement is a parser migration, not a safe lock refresh. Handle it in a focused PR with `v8project.yaml` fixture coverage.
- `ureq` 3.x requires Rust 1.85. Do not update it until the repo has an explicit MSRV decision.
- RLM `find_path` ambiguity behavior changed in v1.25.0, but no new public `unica.code.path` tool is added in this PR. Adding one would require a typed contract, adapter tests, and prompt-visible skill updates.
- Other Playwright 1.61 capabilities such as passkeys, API response network details, and video retention modes are not adopted because they do not map to a current 1C `web-test` workflow.

## Verification Gate

Before opening the PR:

- run Python provenance, skill, and product contract tests;
- run Rust workspace tests;
- validate JSON / Python CI files;
- verify `web-test` facade exports the new storage helpers after npm install;
- build a generated `darwin-arm64` tool bundle from `tools.lock.json`;
- assemble a temporary marketplace package and run `check-tool-contracts.py` against the generated manifest, not the checked-in placeholder manifest.
