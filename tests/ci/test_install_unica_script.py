from __future__ import annotations

import subprocess
import os
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "install-unica.sh"


def script_command(*args: str) -> list[str]:
    if os.name == "nt":
        return ["bash", str(SCRIPT), *args]
    return [str(SCRIPT), *args]


class InstallUnicaScriptTests(unittest.TestCase):
    def test_prints_latest_release_asset_url_for_target(self) -> None:
        result = subprocess.run(
            script_command("--target", "darwin-arm64", "--print-download-url"),
            check=True,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

        self.assertEqual(
            result.stdout.strip(),
            "https://github.com/ingvarvilkman/unica/releases/latest/download/"
            "unica-codex-marketplace-darwin-arm64.tar.gz",
        )

    def test_prints_pinned_release_asset_url_for_target(self) -> None:
        result = subprocess.run(
            script_command("--target", "linux-x64", "--version", "v0.3.3", "--print-download-url"),
            check=True,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

        self.assertEqual(
            result.stdout.strip(),
            "https://github.com/ingvarvilkman/unica/releases/download/v0.3.3/"
            "unica-codex-marketplace-linux-x64.tar.gz",
        )


if __name__ == "__main__":
    unittest.main()
