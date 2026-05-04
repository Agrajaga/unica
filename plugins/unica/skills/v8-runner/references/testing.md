# Testing

Test operations build first, so avoid a separate build unless the user asked for build-only diagnostics.

Use `operation=test`, `testRunner=yaxunit`, `testScope=all` for full YaXUnit.

Use `operation=test`, `testRunner=yaxunit`, `testScope=module`, and `module=<name>` for narrow module-level tests.

Use `operation=test`, `testRunner=va` for the configured Vanessa Automation profile. Do not invent feature paths without inspecting project test configuration.

Use `operation=launch`, `clientMode=mcp-va` for interactive Vanessa Automation scenario authoring and debugging through client MCP.

Syntax validation uses `operation=syntax` with `mode=designer-modules`, `mode=designer-config`, or `mode=edt`.

Preserve failed test artifacts and report their path when the runner prints one.
