# Issue 126: platform 8.3.27 deviation matrix

## Authority and scope

The official source is
`docs-local/1ci/8.3.27/en/developer/Chapter_2._Managing_configurations/2.17._Dumping_configurations_to_files_Restoring_configurations_from_files/2.17.2._Export_format_versions/index.md`.
Its export table maps platform `8.3.27` to format `2.20`, treats a missing
version on a version-owning root as `1.0`, and permits platform 8.3.27 to import
formats less than or equal to `2.20`.

Unica's current contract is narrower: `8.3.27 / 2.20` is the only writable
profile. The table below describes current behavior after the active-profile
writer work, the source-set-aware owner guard through commit `41a3ec0`, the
`cf.init` platform correction in `c3b31e3`, and the Task 6 document-contract
changes in this worktree.

Status vocabulary:

- **fixed/guarded**: code and tests enforce the fixed profile or no-write
  compatibility boundary;
- **proven/fixed**: a real 8.3.27.2074 load/check/export checkpoint established
  the stated document contract and the implementation matches it;
- **contract fixed; platform proof pending**: the known deviation is fixed by
  authoritative code/schema evidence, but the full writer branch has not yet
  completed a real-platform canonical roundtrip;
- **schema-profile limitation**: raw XSD cannot be treated as the platform
  contract for that family.

## Shared current behavior

`ACTIVE_FORMAT_PROFILE` centrally defines platform `8.3.27` and export format
`2.20`. The application guard and native writers share one owner resolver:

- CF/CFE use source-set `Configuration.xml`;
- EPF/ERF use their sibling top-level artifact descriptor;
- recognized standalone version-owning XML owns its version;
- subordinate versionless XML inherits its same-case source-set owner;
- lookup is path-normalized and bounded by the selected source set/workspace.

For every guarded mutation, an older, newer, malformed, unreadable, or
ambiguous owner stops the operation before the first write. New CF/EPF/ERF
scaffolds write `2.20`; CFE initialization writes `2.20` and guards its optional
base. Read-only tools warn and continue where their parser can safely proceed.
The exhaustive operation-descriptor test locks the effective handler paths and
aliases for every mutating platform-XML operation.

## Tool matrix

| Tool(s) | Current behavior | Status / evidence | Remaining platform work |
| --- | --- | --- | --- |
| `unica.cf.edit` | Resolves the actual Configuration owner; incompatible owners block atomically. HomePage output uses the active `2.20` profile. | fixed/guarded; `cf_edit_home_page_uses_active_format`, handler-path and byte-snapshot guard tests | Platform-canonical roundtrip for each edit branch remains part of the public-tool corpus gate. |
| `unica.cf.info`, `unica.cf.validate` | Directory/file aliases resolve to the same Configuration owner as the handler. Older/newer formats return a read-only warning and compatibility diagnostic; structural/semantic analysis continues where safe. | fixed/guarded; `read_only_path_aliases_warn_for_older_directory_owned_inputs` | No writer output; retain corpus coverage for diagnostic paths. |
| `unica.cf.init` | Creates `2.20` owner roots, a fresh non-nil `Configuration/@uuid`, default 8.3.27 compatibility values, and `TextToSpeech=false`. ClientApplicationInterface remains versionless. | **proven/fixed** by public-call source -> import/check/export -> second import/check/export on 8.3.27.2074; semantic gate passed | None for the minimal scaffold contract proved by Task 8. New optional branches need their own evidence. |
| `unica.support.edit` | XML impact is none; it edits the support-state binary only. The containing owner is nevertheless guarded before mutation. | fixed/guarded; descriptor coverage and XML-map equality corpus case | No XML canonicalization applies. The existing event name still overstates XML impact and is a separate semantic cleanup. |
| `unica.cfe.borrow` | Guards both CF and CFE owners and emits borrowed version-owning form roots as `2.20`, rather than copying an older subordinate version. | fixed/guarded; active-writer and multi-owner tests | Full platform roundtrip of all borrow branches is pending. |
| `unica.cfe.diff` | Read-only guard evaluates the declared extension and configuration inputs and continues with a warning. The current response reports the first incompatible resolved owner. | fixed/guarded for no-write safety | Aggregating both incompatibilities in one response is diagnostic improvement work, not a write-safety gap. |
| `unica.cfe.init` | Emits only `2.20`; an optional base is classified before the output directory is created. Unsupported bases produce no artifacts. | fixed/guarded; no-base, supported-base, older/newer-base regressions | Full platform-canonical extension scaffold roundtrip is pending. |
| `unica.epf.init`, `unica.erf.init` | New external descriptors and generated version-owning roots use `2.20`. These are explicit new-dump operations. | fixed profile; active-writer tests | Complete EPF/ERF platform roundtrip and branch corpus remain pending. |
| `unica.cfe.patch_method` | XML impact is none; it patches BSL only. The extension owner is still guarded before mutation. | fixed/guarded; descriptor and XML-map equality coverage | No XML canonicalization applies. |
| `unica.cfe.validate` | Resolves directory/file aliases to the extension Configuration owner, warns on incompatible format, and continues read-only where safe. | fixed/guarded | No writer output. |
| `unica.meta.compile` | Containing/new descriptors use `2.20`; unsupported existing owners fail before creation. ExchangePlan `Content.xml` uses the proven platform QName and namespace set. | fixed/guarded generally; ExchangePlan branch **proven/fixed** | Platform roundtrips for the other metadata kinds remain pending in the exhaustive corpus. |
| `unica.meta.edit`, `unica.meta.remove` | Resolve the effective metadata/Configuration owner and block unsupported mutation before planning or filesystem changes. | fixed/guarded; handler-path, default-path, and byte-snapshot tests | Per-operation platform-canonical deltas remain pending. |
| `unica.meta.info`, `unica.meta.validate` | Read-only compatibility warning comes from the resolved owner; existing semantic inspection/validation continues. | fixed/guarded | Batch aggregation in `meta.validate` remains separate diagnostic work. |
| `unica.meta.profile` | Indexed read-only metadata access performs no filesystem mutation and has no writer constraint. | aligned read-only behavior | No XML output. |
| `unica.help.add` | The default `SrcDir/ObjectName` target resolves its containing owner before mkdir/write; generated descriptor uses `2.20`. | fixed/guarded; default-path and active-writer tests | Full platform roundtrip is pending. |
| `unica.form.add`, `unica.form.compile` | Effective object/output paths are guarded before creation; generated version-owning Form roots use `2.20`. `form.compile` guards both output and inferred/explicit object input. | fixed/guarded | Full generated-form canonical roundtrip across element branches is pending. |
| `unica.form.edit` | Guarded owner plus exact `{http://v8.1c.ru/8.3/xcf/logform}Form` root. Emitted QName bindings are repaired or conflicting bindings fail atomically before write. | contract fixed; platform proof pending; root/QName regressions | Exhaustive real-platform roundtrip of edit branches is pending. |
| `unica.form.info`, `unica.form.validate` | Require the exact managed-form root. QName text prefixes must resolve, canonical prefixes must use canonical URIs, and declared aliases remain valid. Owner incompatibility is a read-only warning. | contract fixed; platform proof pending | Continue corpus/platform validation for real forms. |
| `unica.form.remove` | Default object target resolves to its owner and incompatible formats block before removal. | fixed/guarded | Platform delta proof is pending. |
| `unica.interface.edit` | Containing owner is guarded and emitted version-owning CommandInterface output uses `2.20`. | fixed/guarded; active-writer tests | Full platform roundtrip is pending. |
| `unica.interface.validate` | Read-only owner warning is separate from existing structural validation. | fixed/guarded | No writer output. |
| `unica.subsystem.compile`, `unica.subsystem.edit` | Output/effective subsystem paths resolve through the central owner guard; generated version-owning roots use `2.20`. Missing owner version is classified as `1.0`, not silently defaulted. | fixed/guarded; active-writer and owner tests | Full platform roundtrip is pending. |
| `unica.subsystem.info`, `unica.subsystem.validate` | Read-only compatibility warning is attached and analysis continues where safe. | fixed/guarded | No writer output. |
| `unica.template.add` | Descriptor uses `2.20` and format resolution occurs before target directory creation. Spreadsheet content reuses the exact normalized platform empty-MXL emitter. | spreadsheet branch **proven/fixed**; descriptor fixed/guarded | DCS and other template branches still need their corpus/platform checkpoints. |
| `unica.template.remove` | Default target resolves through the containing owner and incompatible formats block before removal. | fixed/guarded | Platform delta proof is pending. |
| `unica.dcs.compile` | Versionless DCS content inherits a guarded `2.20` owner; new version-owning descriptor output uses the active profile. | fixed/guarded | Full generated DCS canonical roundtrip remains pending. Raw XSD root case is advisory. |
| `unica.dcs.edit` | Requires exact `{http://v8.1c.ru/8.1/data-composition-system/schema}DataCompositionSchema` before planning/write; wrong QName is atomic failure. | contract fixed; platform proof pending | Full edit roundtrip is pending. |
| `unica.dcs.info`, `unica.dcs.validate` | Share the same exact uppercase root contract. Owner incompatibility warns read-only; wrong QName fails before optional info output. | contract fixed; platform proof pending | Raw runtime XSD lowercase global remains a verifier-profile limitation. |
| `unica.mxl.compile` | Correct spreadsheet `document` QName; existing owner is guarded while genuinely new standalone content is allowed. Empty generation uses the canonical sentinel. | empty-document branch **proven/fixed**; guard fixed | Non-empty JSON feature branches need platform roundtrip coverage. |
| `unica.mxl.decompile`, `unica.mxl.info`, `unica.mxl.validate` | Require exact `{http://v8.1c.ru/8.2/data/spreadsheet}document`. The exact platform empty sentinel is logical height zero; lookalikes are not. Wrong root fails before output. Owner mismatch warns read-only. | **proven/fixed** for root and empty-sentinel contract | Raw XSD rejection of platform `columns` remains a verifier-profile limitation. |
| `unica.role.compile` | Rights and descriptor writers use the active profile and are guarded by the resolved owner. | fixed/guarded; active-writer tests | Full Rights platform roundtrip is pending; runtime schema is type-only. |
| `unica.role.info`, `unica.role.validate` | Directory aliases resolve to `Ext/Rights.xml`; owner incompatibility warns and semantic analysis continues. | fixed/guarded | Raw XSD cannot validate a nonexistent global `Rights` declaration. |

## Real-platform proof retained by Task 6

The Task 6 probe used `/opt/1cv8/8.3.27.2074/ibcmd` (`8.3.27.2074`) with
evidence retained at `/tmp/unica-ibcmd-8327-evidence.GKkWas`:

- the old empty MXL shape failed import in log 15;
- corrected MXL import/check/export succeeded in logs 16-18;
- canonical MXL reimport/check/re-export succeeded in logs 25-27 and log 28
  proved byte identity;
- ExchangePlanContent import/check/export succeeded in logs 21-23 and log 24
  recorded the exact QName, namespace set, version, and hashes.

Platform files use UTF-8 BOM, CRLF, and no final newline. Repository fixtures
normalize only those lexical properties to no BOM, LF, and one final LF:

| Fixture | Original platform file | Normalized repository fixture |
| --- | --- | --- |
| empty MXL `Template.xml` | 785 bytes; SHA-256 `197eee7ae5f2912997f63cd9a1a4475085d139b7db68587c21c77a56e682d0df` | 761 bytes; SHA-256 `cfd17d8b9fb43b4d8650ba0a3e35aacd9bcc1c3b7d01c55a275a439ebff24836` |
| ExchangePlan `Content.xml` | 265 bytes; SHA-256 `22b5a6dcdfad07f29c3af911a487edc1c7c1222e5415ab65ca366d77f85181eb` | 262 bytes; SHA-256 `e4aa7daee39d8b0c0443c10e14f62f4ebb8d56044339433b8f8784cd1e5cc8fe` |

The runtime XSD archive SHA-256 is
`e7539a02520cf7bd73585d80b038c2c95078aac281d3700842a5f3a1f3c0c204`.
The EDT 8.3.27 JAR supporting the ExchangePlan QName has SHA-256
`a0c13bbff0527503c23cde14fb10f07742223c6e7d85bf9f06a753cfcc3707b8`.

## `cf.init` platform proof retained by Task 8

The public `UnicaApplication::call_tool("unica.cf.init", ...)` path generated an
untouched source tree at `/tmp/unica-task8-cf-init.Ipo6EN/source`. With
`ibcmd 8.3.27.2074`, source import/apply/check/export and a second
import/check/export all exited successfully. Canonical XML comparison proved
semantic equality for source -> export1 and export1 -> export2, excluding only
the platform-generated `ConfigDumpInfo.xml` and XML lexical formatting.

Across source, export1, and export2:

- `Configuration/@uuid` is present and non-nil;
- Configuration and Language owner roots remain `2.20`;
- the default compatibility properties are `Version8_3_27`;
- `TextToSpeech=false` is present;
- ClientApplicationInterface remains versionless and inherits the same-case
  Configuration owner's `2.20` format.

Task 8 established this proof; Task 6 does not claim authorship of its code.

## Resolved and remaining cross-family work

| Area | Current conclusion |
| --- | --- |
| Export version | **fixed/guarded**: one active profile; version-owning writers use `2.20`; no local writer fallback or validator allowlist controls the contract. |
| Owner resolution | **fixed/guarded**: CF, CFE, EPF, ERF, standalone, aliases, defaults, and multi-input handler paths resolve through the shared owner boundary. |
| Empty MXL | **proven/fixed** against an 8.3.27.2074 byte-stable roundtrip. |
| ExchangePlanContent | **proven/fixed** against EDT XDTO and an 8.3.27.2074 roundtrip. |
| Fresh CF scaffold | **proven/fixed** by the Task 8 two-cycle platform checkpoint. |
| Other writer branches | Version/guard contract is fixed, but full platform-canonical proof remains pending in the exhaustive public-tool corpus and platform gate. |
| Raw XSD strictness | Schema-profile limitations remain: uppercase DCS platform root versus lowercase raw global, platform MXL `columns`, CAI `uuid`, and type-only roles schema. They are not permission to alter platform-valid XML. |

## Migration boundary

Unica exposes no native format-migration operation and never migrates as a side
effect. For an older source, read-only operations warn and mutations are
refused. The warning recommends an explicit user-driven migration using
1C:Enterprise 8.3.27 tooling: load the source, re-export it as `2.20`, and retry
the Unica operation. The diagnostic code `formatMigrationAvailable` denotes
that manual remediation; it is not a public tool name.

For a source newer than `2.20`, read-only operations warn and mutations are
refused with `platformVersionUnsupported`. Unica states that platform 8.5 is not
supported yet but is planned, and never offers a downgrade.
