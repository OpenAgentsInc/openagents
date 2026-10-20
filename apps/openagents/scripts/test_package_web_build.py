"""Regression checks for the release transfer budget and asset dependency hashes."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    "package_web_build", Path(__file__).with_name("package-web-build.py")
)
packager = importlib.util.module_from_spec(spec)
spec.loader.exec_module(packager)


class PackageTest(unittest.TestCase):
    def fixture(self, directory):
        (directory / "app_bg.wasm").write_bytes(b"\0asm\x01\0\0\0")
        (directory / "app.js").write_text('new URL("app_bg.wasm", import.meta.url)')
        (directory / "Noto-test.ttf").write_bytes(b"font")
        (directory / "start.js").write_text(
            'import init from "./app.js"; WebAssembly.compileStreaming(fetch("app_bg.wasm")); fetch("Noto-test.ttf")'
        )

    def test_hashes_follow_dependencies_and_describe_the_shipped_bytes(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            self.fixture(directory)
            packager.package(directory, "app")
            assets = json.loads((directory / "assets.json").read_text())
            for original, hashed in assets.items():
                content = (directory / hashed).read_bytes()
                self.assertEqual(content, (directory / original).read_bytes())
                self.assertIn(hashlib.sha256(content).hexdigest(), hashed)
                self.assertEqual(
                    (directory / (original + ".br")).read_bytes(),
                    (directory / (hashed + ".br")).read_bytes(),
                )
            self.assertIn(assets["app_bg.wasm"], (directory / assets["app.js"]).read_text())
            start = (directory / assets["start.js"]).read_text()
            self.assertIn(assets["app.js"], start)
            self.assertIn(assets["app_bg.wasm"], start)
            self.assertIn(assets["Noto-test.ttf"], start)
            self.fixture(directory)
            packager.package(directory, "app")
            self.assertEqual(assets, json.loads((directory / "assets.json").read_text()))
            report = json.loads((directory / "size.json").read_text())
            self.assertEqual(report["brotli_bytes"], (directory / "app_bg.wasm.br").stat().st_size)
            self.assertEqual(report["budget_bytes"], 1_500_000)

    def test_a_stale_compressed_copy_cannot_hide_an_over_budget_module(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            self.fixture(directory)
            (directory / "app_bg.wasm.br").write_bytes(b"")
            with patch.object(packager, "BUDGET", 1):
                with self.assertRaisesRegex(SystemExit, "exceeds"):
                    packager.package(directory, "app")
            self.assertFalse((directory / "assets.json").exists())
            self.assertGreater(json.loads((directory / "size.json").read_text())["brotli_bytes"], 1)


if __name__ == "__main__":
    unittest.main()
