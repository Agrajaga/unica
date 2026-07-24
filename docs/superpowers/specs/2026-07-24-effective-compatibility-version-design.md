# Effective Compatibility Version Design

## Problem

`meta.edit` currently treats `CompatibilityMode=DontUse` as an independently
editable state for `LineNumberLength`. The platform and BSP model is different:
`DontUse` means the behavior of the active platform line, while an explicit
`VersionX` means the behavior of that compatibility version.

The current result is correct for Unica's fixed 8.3.27 profile, but the code and
documentation hide the version normalization and can be copied incorrectly when
the active platform profile changes.

## Decision

Introduce one pure normalization step:

```text
effective version =
  active platform line, for DontUse
  version encoded by VersionX, for an explicit compatibility mode
```

The `LineNumberLength` policy then compares only the effective version with the
documented boundary:

```text
effective version <= 8.3.26 -> fixed at 5
effective version >  8.3.26 -> editable from 5 through 9
```

The production wrapper obtains the current line from
`ACTIVE_FORMAT_PROFILE.platform_line`. Explicit compatibility values remain
validated by the active 8.3.27 enum contract. Unknown explicit modes and an
invalid active platform line fail closed as `UnknownCompatibility`.

## Alternatives

1. Keep the direct `DontUse => Editable` branch. This is behaviorally correct
   for 8.3.27 but leaves the platform dependency implicit.
2. Read `tools.platform.version` from `v8project.yaml`. This is not used because
   native mutation is governed by Unica's verified active format profile, while
   a workspace setting alone is not platform provenance.
3. Normalize against `ACTIVE_FORMAT_PROFILE.platform_line`. This matches the
   existing mutation boundary and is the selected approach.

## Documentation

Public skill documentation will describe the effective compatibility version
instead of listing `DontUse` as a separate semantic case. It will also state
that `DontUse` is resolved to the active Unica platform profile.

## Verification

Unit tests will cover:

- `DontUse` on 8.3.26 is fixed;
- `DontUse` on 8.3.27 and 8.5.4 is editable;
- an explicit `Version8_3_24` remains fixed on 8.5.4;
- an explicit `Version8_3_27` remains editable on 8.5.4;
- invalid platform lines and unsupported explicit modes fail closed.

Focused policy tests, the full `unica-coder` suite, clippy, formatting, skill
guardrails, and diff validation must pass before publication.
