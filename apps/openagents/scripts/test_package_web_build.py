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

    def test_a_later_module_is_hashed_and_checked_too(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            self.fixture(directory)
            (directory / "pane_bg.wasm").write_bytes(b"\0asm\x01\0\0\1")
            (directory / "pane.js").write_text('new URL("pane_bg.wasm", import.meta.url)')
            start = (directory / "start.js").read_text()
            (directory / "start.js").write_text(
                start + '; import("./pane.js"); fetch("./pane_bg.wasm")'
            )
            packager.package(directory, "app", "pane")
            assets = json.loads((directory / "assets.json").read_text())
            start = (directory / assets["start.js"]).read_text()
            self.assertIn(assets["pane.js"], start)
            self.assertIn(assets["pane_bg.wasm"], start)
            self.assertIn(assets["pane_bg.wasm"], (directory / assets["pane.js"]).read_text())
            report = json.loads((directory / "size.json").read_text())
            self.assertEqual(report["module"], "app_bg.wasm")
            self.assertEqual(report["later"][0]["module"], "pane_bg.wasm")
            with patch.object(packager, "BUDGET", 1):
                with self.assertRaisesRegex(SystemExit, "exceeds"):
                    packager.package(directory, "app", "pane")

    def test_a_staging_build_warns_over_the_budget(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            self.fixture(directory)
            with patch.object(packager, "BUDGET", 1), patch.dict(
                packager.os.environ, {"OA_STAGING_BUILD": "1"}
            ):
                packager.package(directory, "app")
            self.assertTrue((directory / "assets.json").is_file())

    def test_renderer_selection_and_both_modules_have_content_hashes(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            self.fixture(directory)
            (directory / "app_webgl_bg.wasm").write_bytes(b"\0asm\x01\0\0\0")
            (directory / "app_webgl.js").write_text('new URL("app_webgl_bg.wasm", import.meta.url)')
            (directory / "renderer.js").write_text('export const renderer = "webgl"')
            (directory / "start.js").write_text(
                'import {renderer} from "./renderer.js"; '
                'load("./app.js", "./app_bg.wasm", "./app_webgl.js", "./app_webgl_bg.wasm")'
            )
            packager.package(directory, "app", "app_webgl")
            assets = json.loads((directory / "assets.json").read_text())
            source = (directory / assets["start.js"]).read_text()
            for name in ("renderer.js", "app.js", "app_bg.wasm", "app_webgl.js", "app_webgl_bg.wasm"):
                self.assertIn(assets[name], source)
                self.assertIn(hashlib.sha256((directory / name).read_bytes()).hexdigest(), assets[name])

    def test_renderer_variants_keep_the_release_cap(self):
        for module in ("openagents_web_chat_bg.wasm", "openagents_web_chat_webgl_bg.wasm",
                       "openagents_web_demo_bg.wasm", "openagents_web_agent_graph_webgl_bg.wasm"):
            self.assertEqual(packager.budget(module),
                             1_750_000 if "web_chat_" in module else 1_500_000)

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
