# Config And Backends

Important `v8project.yaml` concepts:

- `format`: `designer` or `edt`.
- `builder`: `DESIGNER` or `IBCMD`.
- `source-set`: ordered configuration and extension source entries.
- `infobase.connection`: runtime connection string.
- `tools.client_mcp.extension`: optional generated tool extension prepared by `build`.
- external source-set types: `EXTERNAL_DATA_PROCESSORS` publishes `.epf`, `EXTERNAL_REPORTS` publishes `.erf`.

Use `v8project.local.yaml` for local `infobase.connection`, credentials, platform paths, VA paths, and MCP paths. Do not put shared `source-set`, `format`, or `builder` there.

Backend guidance:

- Designer format with Designer builder covers init/build/extensions/dump/syntax/tests/make/load.
- Designer format with IBCMD is narrower and intended mainly for file infobases.
- EDT format can build through export, run EDT syntax checks, synchronize extensions, and run configured tests.
- `convert` is a file workflow and does not use the infobase.
- `make` requires a backend that can publish the requested artifact. For external processors/reports, `output` is a publish directory.
