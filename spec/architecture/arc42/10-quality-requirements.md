# 10. Требования к качеству

## Token Efficiency

LLM-visible tool surface must avoid requiring the model to know which internal
engine owns which cache. Coordination belongs to `unica`.

## Safety

Mutating operations default to dry-run and return cache impact before applied
execution.

## Maintainability

Domain, application, infrastructure, and MCP transport code remain separated so
adapter replacement does not rewrite public MCP handling.

## Observability

Operation result must include summary, warnings, errors, artifacts, and cache
impact. Future native adapters should add structured diagnostics without
changing the top-level result shape casually.

## Packaging Reliability

Generated packages must verify checksums and must not depend on globally
installed tools when bundled equivalents exist.
