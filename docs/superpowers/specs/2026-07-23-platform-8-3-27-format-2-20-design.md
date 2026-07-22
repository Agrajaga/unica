# Platform 8.3.27 / export format 2.20 design

## Status

The fixed-profile design and implementation scope were approved in conversation
on 2026-07-23. The current implementation supports one writable profile:
platform 8.3.27 and configuration export format `2.20`.

Native migration and multiple format profiles are not implemented by this
design. They are explicitly deferred future work and require a separate public
contract and approval.

## Authority

The 1C:Enterprise 8.3.27 Developer Guide maps platform line 8.3.27 to export
format `2.20`. It also states that:

- all configuration objects exported to files use the same export format;
- the version is present on object root XML files and on some subordinate root
  files;
- absence of a version on a version-owning root means format `1.0`;
- platform 8.3.27 can import export formats less than or equal to `2.20`.

The platform's ability to import an older format does not make that format a
writable Unica profile. Unica writes only `2.20` in the current implementation.

Runtime XDTO/XSD from exact build 8.3.27.2074 is useful evidence, but is not a
universal contract for every configuration XML family. Some schemas exist only
in the EDT corpus, some runtime schemas are type libraries without global
roots, and some raw schemas contradict files accepted and emitted by the real
platform. A platform 8.3.27 load and roundtrip remains the final compatibility
proof for generated XML.

## Goals

1. Keep `8.3.27 / 2.20` as the only writable platform-XML profile.
2. Detect unsupported owner formats before a modifying operation writes.
3. Let read-only inspection and validation continue where safe, with a distinct
   compatibility warning.
4. For older sources, refuse mutation and recommend an explicit user-driven
   load and re-export using 1C:Enterprise 8.3.27 tooling.
5. For newer sources, refuse mutation, never downgrade, and report that
   platform 8.5 is not supported yet but is planned.
6. Correct document-family deviations only when backed by code, schemas, EDT
   contracts, or real-platform evidence.

## Non-goals

- Supporting more than one platform or export-format profile now.
- Automatically migrating source as a side effect of any operation.
- Providing a native migration endpoint in the current public MCP contract.
- Rewriting only a root `version` attribute as a migration technique.
- Downgrading a format newer than `2.20`.
- Replacing semantic validators with raw XSD validation.
- Treating the runtime 8.3.27.2074 schema archive as universal.
- Implementing the complete XDTO/XSD service proposed by issue #126.

## Supported profile

| Property | Current value |
| --- | --- |
| Platform line | `8.3.27` |
| Configuration export format | `2.20` |
| Exact runtime build used by current platform proofs | `8.3.27.2074` |

`ACTIVE_FORMAT_PROFILE` owns the platform line and export format. Writers and
validators do not maintain local version allowlists.

## Compatibility classification

The effective version comes from the version-owning XML file for the selected
source set or standalone artifact. A legitimate subordinate XML document may
be versionless and inherit the source-set owner's format.

| Source format | Classification | Modifying tools | Read-only tools | User remediation |
| --- | --- | --- | --- | --- |
| missing on a version-owning root, or `< 2.20` | `older` | stop before write | continue where parsing is safe, with warning | explicitly load and re-export with 1C 8.3.27 tooling |
| `2.20` | `supported` | continue | continue | none |
| `> 2.20` | `newer` | stop before write | continue where parsing is safe, with warning | no downgrade; wait for a supported newer profile |
| malformed, unreadable, or ambiguous | `invalid` | stop before write | continue only as far as safe parsing permits | correct the source/owner selection |

## Architecture

### Central profile and owner resolution

The domain classifier compares numeric version components and treats a missing
owner version as `1.0`. One source-set-aware owner resolver is shared by the
application guard and native writers:

- CF and CFE use their source-set `Configuration.xml`;
- EPF and ERF use the top-level sibling artifact descriptor;
- recognized standalone version-owning roots own their version;
- versionless DCS, MXL, and ClientApplicationInterface documents inherit a
  resolved same-case source-set owner where applicable;
- owner lookup is normalized and stops at the configured source-set/workspace
  boundary rather than scanning to filesystem root.

Missing, malformed, unreadable, wrong-QName, or ambiguous owners are structured
`formatVersionInvalid` failures.

### Preflight guard

Every public modifying platform-XML workflow declares an effective path policy.
The guard resolves handler aliases, default target paths, and multi-input paths
before the native handler or support guard runs. It blocks an incompatible
owner before directory creation, temporary output, platform mutation, or file
write. New CF, EPF, and ERF scaffolds use the active profile; CFE initialization
also guards an optional base configuration.

Read-only `info`, `validate`, `diff`, and decompile workflows receive the same
classification as a warning and continue where their own parser can proceed.
The incompatibility is returned separately from XML syntax or semantic errors.

### Manual migration boundary

Unica currently has no public native format-migration operation and never
starts migration automatically. For an older source, the warning recommends an
explicit operator action using 1C:Enterprise 8.3.27 tooling: load the source and
re-export it, then retry the Unica mutation against the resulting `2.20` tree.

The machine-readable code `formatMigrationAvailable` means that an explicit
manual remediation exists. It does not identify or imply a callable Unica tool.

A source newer than `2.20` is never downgraded. The current user-facing text is:

> Формат выгрузки `{actual}` новее поддерживаемого `2.20` для платформы 1С 8.3.27. Unica пока не поддерживает работу с этой выгрузкой. Поддержка платформы 1С 8.5 планируется в ближайших версиях.

This text does not infer that the source was created specifically by 8.5; only
the export format is known to be newer than the 8.3.27 contract.

## Diagnostics

Compatibility diagnostics include:

- `actualFormat` when it can be parsed;
- `targetFormat: "2.20"`;
- `targetPlatform: "8.3.27"`;
- `compatibility`: `supported`, `older`, `newer`, or `invalid`;
- the resolved owner path and owner kind;
- one machine-readable code.

Current codes are:

- `formatMigrationAvailable` for an older source and manual platform re-export;
- `platformVersionUnsupported` for a newer source;
- `formatVersionInvalid` for an owner that cannot be classified.

Diagnostics must not expose credentials, connection strings, infobase
identifiers, or unrelated local paths.

## Writer and document-family contract

All newly generated version-bearing XML roots use the active `2.20` profile.
This is an implementation invariant, not a claim that every emitted document
family has completed a real-platform canonical roundtrip.

Current real-platform proofs are narrower and explicit:

- empty MXL uses the platform-produced direct sentinel sequence
  `languageSettings, columns, rowsItem, templateMode, vgRows`;
- ExchangePlan `Content.xml` uses the proven `xcf/extrnprops`
  `ExchangePlanContent` QName with `xr`, `xs`, and `xsi` declarations;
- `unica.cf.init` emits a fresh non-nil Configuration UUID, defaults the 8.3.27
  compatibility properties, includes `TextToSpeech=false`, and is semantically
  stable across two 8.3.27.2074 import/check/export cycles;
- ClientApplicationInterface remains versionless and inherits `2.20` from the
  same-case Configuration owner.

Managed Form, DCS, and MXL consumers now enforce their exact proven root QNames.
Form QName text must resolve through in-scope namespace bindings. These
structural fixes do not by themselves prove every writer branch to be
platform-canonical; the remaining corpus and roundtrip work is recorded in the
deviation matrix.

## Validation authority

Validation is layered:

1. XML well-formedness;
2. central owner and export-format compatibility;
3. existing Unica structural and semantic validation;
4. XSD only for a compatibility-tested schema profile and document family;
5. platform 8.3.27 load/roundtrip as final proof for generated artifacts.

XSD failures retain their provenance. Known raw-schema limitations must not be
used to rewrite platform-valid XML.

## Verification strategy

### Guard and writer tests

- classify missing, older, supported, newer, and malformed owner versions;
- prove mutation is blocked before handler/write and bytes remain unchanged;
- prove read-only path aliases retain warnings and continue;
- prove every mutating platform-XML descriptor has an effective owner path;
- prove every generated version-owning root uses `2.20`.

### Document-family tests

- compare proven MXL and ExchangePlan output with normalized platform fixtures;
- reject wrong MXL, Form, and DCS root QNames before output or mutation;
- validate Form QName prefix bindings and atomically repair emitted bindings;
- retain existing semantic regression suites.

### Platform checkpoints

- retain exact platform build, commands, exit codes, and hashes for evidence;
- distinguish a code/test contract from a completed platform roundtrip;
- use the exhaustive public-tool XML corpus and platform gate to close remaining
  writer-family canonicalization evidence.

There are no native migration orchestration, staging-publication, migration
receipt, or migration provenance tests in the current scope because that
feature does not exist.

## Acceptance criteria

- One central profile defines `8.3.27 / 2.20`.
- Every modifying platform-XML operation resolves its effective owner before
  the first write, or is an explicit new-dump operation using `2.20`.
- Older formats produce a warning and no mutation, with a manual 1C 8.3.27
  load/re-export recommendation and no invented public tool name.
- Newer formats produce `platformVersionUnsupported`, the agreed 8.5 roadmap
  text, no mutation, and no downgrade recommendation.
- Read-only operations report incompatibility separately and continue where
  safe.
- Writers use `2.20`; platform-canonical claims are made only for families with
  recorded real-platform evidence.
- MXL, ExchangePlanContent, Form, DCS, and `cf.init` corrections are locked by
  their focused regressions and platform evidence.
- No automatic or native migration is exposed.

## Deferred future work

Multiple profiles, including a future 1C 8.5 profile, require new per-family
evidence and must not weaken the `8.3.27 / 2.20` behavior. A native migration
workflow, if later approved, needs its own public API design, platform-selection
policy, transaction/recovery model, security review, and tests. It is not an
unfinished part of this implementation.
