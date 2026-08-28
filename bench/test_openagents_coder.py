"""Packing the digest-pinned plugin catalog for Harbor A/B rows."""

import tarfile
import tempfile
import unittest
from pathlib import Path

from adapters.plugin_catalog import pack_plugin_catalog


class PackPluginCatalogTest(unittest.TestCase):
    def test_packs_manifest_and_wasm_skips_pdk(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            plugin = root / "git-lost-work"
            plugin.mkdir()
            (plugin / "manifest.json").write_text("{}", encoding="utf-8")
            (plugin / "git_lost_work.wasm").write_bytes(b"wasm")
            (plugin / "src").mkdir()
            (plugin / "src" / "lib.rs").write_text("fn main() {}", encoding="utf-8")
            (root / "pdk").mkdir()
            (root / "pdk" / "Cargo.toml").write_text("[package]\n", encoding="utf-8")
            dest = root / "out.tar"
            count = pack_plugin_catalog(root, dest)
            self.assertEqual(count, 1)
            with tarfile.open(dest) as tar:
                names = tar.getnames()
            self.assertIn("git-lost-work/manifest.json", names)
            self.assertIn("git-lost-work/git_lost_work.wasm", names)
            self.assertFalse(any(name.startswith("pdk/") for name in names))
            self.assertFalse(any("lib.rs" in name for name in names))

    def test_refuses_an_empty_catalog(self):
        with tempfile.TemporaryDirectory() as tmp:
            dest = Path(tmp) / "out.tar"
            with self.assertRaises(FileNotFoundError):
                pack_plugin_catalog(Path(tmp), dest)


if __name__ == "__main__":
    unittest.main()
