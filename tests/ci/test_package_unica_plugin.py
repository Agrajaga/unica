from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


def load_package_module():
    module_path = Path(__file__).resolve().parents[2] / "scripts" / "ci" / "package-unica-plugin.py"
    spec = importlib.util.spec_from_file_location("package_unica_plugin", module_path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"failed to load {module_path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class PackageUnicaPluginTests(unittest.TestCase):
    def make_lock(self) -> dict:
        return {
            "schemaVersion": 1,
            "targets": {
                "darwin-arm64": {"targetTriple": "aarch64-apple-darwin"},
                "linux-x64": {"targetTriple": "x86_64-unknown-linux-musl"},
            },
            "tools": [
                {
                    "name": "v8-runner",
                    "version": "0.3.0",
                    "repository": "https://example.invalid/v8-runner",
                    "sourceTag": "v0.3.0",
                    "sourceCommit": "abc",
                    "license": "MIT",
                    "assets": {
                        "darwin-arm64": {"assetName": "v8-runner"},
                        "linux-x64": {"assetName": "v8-runner"},
                    },
                }
            ],
        }

    def write_bundle(self, root: Path, target: str, module) -> Path:
        bundle = root / f"unica-tools-{target}"
        bin_dir = bundle / "bin" / target
        bin_dir.mkdir(parents=True)
        binary = bin_dir / "v8-runner"
        binary.write_text(f"binary for {target}", encoding="utf-8")
        target_triples = {
            "darwin-arm64": "aarch64-apple-darwin",
            "linux-x64": "x86_64-unknown-linux-musl",
        }
        (bundle / "tools.json").write_text(
            json.dumps(
                {
                    "target": target,
                    "targetTriple": target_triples[target],
                    "tools": [
                        {
                            "name": "v8-runner",
                            "version": "0.3.0",
                            "repository": "https://example.invalid/v8-runner",
                            "upstreamUrl": "https://example.invalid/v8-runner/releases/tag/v0.3.0",
                            "sourceTag": "v0.3.0",
                            "sourceCommit": "abc",
                            "license": "MIT",
                            "targetTriple": target_triples[target],
                            "binaryPath": f"bin/{target}/v8-runner",
                            "sha256": module.sha256(binary),
                        }
                    ],
                }
            ),
            encoding="utf-8",
        )
        return bundle

    def test_load_tool_bundles_allows_current_target_only_for_local_debug_package(self) -> None:
        module = load_package_module()

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            bundle = self.write_bundle(root, "darwin-arm64", module)

            grouped, bin_roots = module.load_tool_bundles(root, self.make_lock(), allow_partial_targets=True)

        self.assertEqual(bin_roots, [bundle / "bin"])
        self.assertEqual(sorted(grouped["v8-runner"]["binaries"]), ["darwin-arm64"])

    def test_load_tool_bundles_can_filter_one_release_target(self) -> None:
        module = load_package_module()

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            darwin_bundle = self.write_bundle(root, "darwin-arm64", module)
            self.write_bundle(root, "linux-x64", module)

            grouped, bin_roots = module.load_tool_bundles(
                root,
                self.make_lock(),
                allow_partial_targets=True,
                target="darwin-arm64",
            )

        self.assertEqual(bin_roots, [darwin_bundle / "bin"])
        self.assertEqual(sorted(grouped["v8-runner"]["binaries"]), ["darwin-arm64"])

    def test_archive_base_name_is_platform_specific_for_release_packages(self) -> None:
        module = load_package_module()

        self.assertEqual(
            module.archive_base_name("0.3.3", target="darwin-arm64"),
            "unica-codex-marketplace-darwin-arm64",
        )
        self.assertEqual(module.archive_base_name("0.3.3", target=None), "unica-codex-marketplace-0.3.3")

    def test_write_marketplace_can_use_local_debug_name(self) -> None:
        module = load_package_module()

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            source = root / "marketplace.json"
            dest = root / "out.json"
            source.write_text(
                json.dumps(
                    {
                        "name": "unica",
                        "interface": {"displayName": "Unica"},
                        "plugins": [
                            {
                                "name": "unica",
                                "source": {"source": "local", "path": "./plugins/unica"},
                                "category": "Coding",
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            module.write_official_marketplace(source, dest, marketplace_name="unica-local")

            data = json.loads(dest.read_text(encoding="utf-8"))
            self.assertEqual(data["name"], "unica-local")
            self.assertEqual(data["plugins"][0]["name"], "unica")


if __name__ == "__main__":
    unittest.main()
