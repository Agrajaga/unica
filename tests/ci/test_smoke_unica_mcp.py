from __future__ import annotations

import importlib.util
import json
import subprocess
import unittest
from pathlib import Path
from unittest import mock


SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "ci" / "smoke-unica-mcp.py"


def load_module():
    spec = importlib.util.spec_from_file_location("smoke_unica_mcp", SCRIPT)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class SmokeUnicaMcpTests(unittest.TestCase):
    def test_decodes_mcp_json_as_utf8_independently_of_windows_locale(self) -> None:
        module = load_module()
        tools = sorted(module.REQUIRED_TOOLS)
        responses = [
            {"jsonrpc": "2.0", "id": 1, "result": {"serverInfo": {"name": "Уника"}}},
            {
                "jsonrpc": "2.0",
                "id": 2,
                "result": {"tools": [{"name": name} for name in tools]},
            },
        ]
        stdout = "".join(json.dumps(value, ensure_ascii=False) + "\n" for value in responses)

        with mock.patch.object(
            module.subprocess,
            "run",
            return_value=subprocess.CompletedProcess(["unica"], 0, stdout, ""),
        ) as run:
            module.smoke(["unica"], Path("."), 20)

        self.assertEqual(run.call_args.kwargs["encoding"], "utf-8")


if __name__ == "__main__":
    unittest.main()
