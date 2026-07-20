# ADR-0010: CI build cache and artifact flow

- Status: `accepted`
- Date: `2026-07-20`

## Context

The release workflow builds `unica` and `unica-bootstrap` in separate Cargo
target directories on the same platform runner. It then uploads a complete
`unica-tools-*` bundle, downloads that bundle in a separate Linux job to create
the runtime archive, and downloads both complete runtime and tools artifacts in
`package-thin` even though thin packaging needs only runtime metadata and the
three bootstrap binaries.

The reference PR run `29716722998` stored 464.4 MiB of artifacts: 233.3 MiB of
tool bundles and 226.5 MiB of runtime bundles. The duplicate artifacts do not
strengthen the release contract because the platform runner already has every
input required to create and verify its deterministic runtime archive.

## Decision

### Cargo build

1. Each platform build uses one target-specific Cargo target directory for all
   workspace binaries built on that runner.
2. `unica` and `unica-bootstrap` are selected in one mandatory `cargo build`
   invocation. A restored cache accelerates this command but never replaces it.
3. The Cargo target directory is cached with a key containing runner OS, Unica
   target, the resolved Rust toolchain cache key, and the `Cargo.lock` hash.
4. Cargo target directories and cache contents are never uploaded as workflow
   or release artifacts.
5. Every platform build reports its target, cache hit or miss, and Cargo build
   duration in the GitHub Actions job summary. Cold and warm runs therefore use
   the same measurement rather than separate build paths.

### Runtime packaging

The platform build job creates and verifies the complete tool bundle locally,
smokes the packaged Unica MCP, and invokes `package-unica-runtime.py` before the
runner is released. This preserves deterministic archive creation while
removing the intermediate `package-runtime` job and the `unica-tools-*`
artifact family.

The resulting data crosses job boundaries as three independently owned artifact
classes:

- `unica-runtime-metadata-<target>` contains only the runtime JSON metadata;
- `unica-bootstrap-<target>` contains only the native bootstrap subtree;
- `unica-runtime-<target>` contains only the publishable runtime archive.

Runtime metadata and bootstrap artifacts are uploaded for every full package
contour and retained for one day. Runtime archives are uploaded on pull requests
only when a downstream check consumes them; currently that is the Linux archive
used by release assessment. Tag runs upload all three runtime archives for
publication and byte-for-byte verification. These workflow artifacts are
intermediate and use one-day retention because the release assets become the
durable tag output.

`package-thin` downloads only the runtime metadata and bootstrap artifact
families. `unica-thin-marketplace` keeps the existing longer retention because
manual marketplace promotion retrieves it by `source_run_id`.

### Failure behavior

- A cache miss is an observable cold build, not an error.
- Cache restore or save failure must not bypass the mandatory Cargo build,
  package validation, smoke test, or deterministic archive contract.
- Missing metadata, bootstrap binaries, or a required runtime archive remains a
  hard artifact/download failure.
- Tag publication still requires all macOS, Linux, and Windows archives and
  metadata, followed by published-byte verification and thin-plugin smoke on
  all supported hosts.

## Verification

Contract tests must prove that:

- the build helper issues one Cargo build for `unica` and `unica-bootstrap`
  against one target directory;
- the workflow cache key includes OS, target, toolchain, and `Cargo.lock`;
- the workflow always executes the build after cache restoration;
- no `unica-tools-*` artifact or separate `package-runtime` job remains;
- thin packaging consumes only runtime metadata and bootstrap artifacts;
- intermediate artifacts use one-day retention while
  `unica-thin-marketplace` does not;
- pull requests upload only the runtime archive required by downstream
  assessment, while tag runs upload and publish all targets;
- package, bootstrap smoke, release assessment, deterministic archive, and
  published-asset contracts remain connected to the stable `Unica CI` gate.

The implementation PR records before/after wall time, aggregate runner time,
cache results, artifact sizes, upload/download volume, and `package-thin`
download volume from full workflow runs.

## Consequences

- Platform runners do more packaging locally but avoid uploading and
  re-downloading complete tool bundles.
- Pull-request artifact storage drops from two complete three-platform copies
  to metadata, bootstrap binaries, the thin marketplace payload, assessment
  output, and the one Linux runtime required by assessment.
- Warm builds reuse Cargo compilation products without treating cached output
  as a release artifact or proof of a valid bundle.
- Release/tag behavior remains stricter than pull-request storage behavior: all
  targets are still packaged, published, downloaded again, and verified.

