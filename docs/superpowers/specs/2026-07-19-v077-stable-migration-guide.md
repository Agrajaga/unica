# Stable v0.7.7 Migration Guide

## Status

Approved by the maintainer on 2026-07-19.

## Problem

The `v0.7.7` installer assets are immutable, but their default marketplace ref
is the mutable `main` branch. The marketplace version tag cannot replace that
branch: `v0.7.7` identifies the staged plugin snapshot created before catalog
promotion, so its catalog still points to `v0.7.6`.

The README also asks users to distinguish installation layouts. The supported
consumer action can instead be selected by the first stable public-marketplace
boundary.

## Decision

- Create signed marketplace tag `migration-v0.7.7` at promotion merge
  `81531ed115279ed3ecb53f181d46edaf6d508056`.
- Require that this tag contains plugin `0.7.7` and a catalog whose source ref
  is `v0.7.7`.
- Historical versions `0.3.0` through `0.7.4` use the published `v0.7.7`
  installer with the explicit `migration-v0.7.7` ref.
- Versions `0.7.5` and later use the ordinary marketplace update commands.
- Users find their installed version with `codex plugin list` and the `VERSION`
  column in the `unica@...` row.

## README contract

The section `Переход со старой установки и откат` contains only:

1. the version command and where to read its result;
2. a two-column `Ваша версия` / `Что делать` table;
3. direct macOS/Linux and Windows migration commands;
4. the ordinary update commands;
5. one rollback sentence stating that a failed migration already restored the
   previous installation.

The section does not expose canonical/legacy layout terminology, transaction
internals, future-release policy, or migration implementation details.
