"""Exercise artifact acquisition with a small, mutable local Hub substitute."""

import contextlib
import copy
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("fetch_kev", Path(__file__).with_name("fetch-kev-artifacts.py"))
fetcher = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fetcher)


def record(content):
    return {"sha256": hashlib.sha256(content).hexdigest(), "bytes": len(content)}


class FetchTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.requests = []
        self.converted = 0
        self.sources = {"adapter_model.safetensors": b"adapter", "head.pt": b"raw head"}
        self.base = {"config.json": b"config", "model.safetensors": b"weights"}
        self.derived = {"head.safetensors": b"converted", "head_meta.json": b'{"base":"test/base"}'}
        self.lock = {
            "schema_version": 1, "variant": "tiny",
            "adapter": {"repo": "test/tiny", "revision": "a" * 40, "directory": "tiny",
                        "files": {n: record(v) for n, v in self.sources.items()}},
            "base": {"repo": "test/base", "revision": "b" * 40, "directory": "base",
                     "files": {n: record(v) for n, v in self.base.items()}},
            "derived_files": {n: record(v) for n, v in self.derived.items()},
        }
        self.remote = {}
        for part, files in (("adapter", self.sources), ("base", self.base)):
            source = self.lock[part]
            for name, data in files.items():
                self.remote[f"https://hub.test/{source['repo']}/resolve/{source['revision']}/{name}"] = data
        self.addCleanup(patch.stopall)
        patch.object(fetcher.urllib.request, "urlopen", side_effect=self.download).start()
        self.output = contextlib.redirect_stdout(io.StringIO())
        self.output.__enter__()
        self.addCleanup(self.output.__exit__, None, None, None)

    def download(self, url, **kwargs):
        self.requests.append(url)
        return io.BytesIO(self.remote[url])

    def convert(self, source, output, root):
        self.assertEqual(source.read_bytes(), self.sources["head.pt"])
        self.converted += 1
        for name, data in self.derived.items():
            (output / name).write_bytes(data)

    def run_fetch(self):
        return fetcher.fetch_lock(self.lock, self.root, hub="https://hub.test", converter=self.convert)

    def test_cold_and_warm_agree_when_main_moves(self):
        cold = self.run_fetch()
        self.remote["https://hub.test/test/tiny/resolve/main/head.pt"] = b"replacement"
        self.requests.clear()
        self.assertEqual(cold, self.run_fetch())
        self.assertEqual(self.requests, [])
        self.assertEqual(self.converted, 1)
        self.assertEqual(json.loads((self.root / "tiny/artifact-lock.json").read_text()), self.lock)

    def test_corrupt_cached_adapter_fails_before_network_or_conversion(self):
        directory = self.root / "tiny"
        directory.mkdir()
        (directory / "adapter_model.safetensors").write_bytes(b"corrupted")
        with self.assertRaisesRegex(ValueError, "mismatch"):
            self.run_fetch()
        self.assertEqual(self.requests, [])
        self.assertEqual(self.converted, 0)

    def test_wrong_source_head_is_never_converted(self):
        url = next(url for url in self.remote if url.endswith("head.pt"))
        self.remote[url] = b"untrusted head"
        with self.assertRaisesRegex(ValueError, "mismatch"):
            self.run_fetch()
        self.assertEqual(self.converted, 0)
        self.assertFalse(any("test/base" in url for url in self.requests))
        self.assertFalse((self.root / "tiny/head.pt").exists())
        self.assertFalse((self.root / "tiny/head.pt.part").exists())
        self.assertFalse((self.root / "tiny/artifact-lock.json").exists())

    def test_interrupted_partial_is_replaced_with_verified_download(self):
        directory = self.root / "tiny"
        directory.mkdir()
        (directory / "head.pt.part").write_bytes(b"interrupted")
        self.run_fetch()
        self.assertEqual((directory / "head.pt").read_bytes(), self.sources["head.pt"])
        self.assertFalse((directory / "head.pt.part").exists())

    def test_bad_conversion_does_not_publish_either_output_or_fetch_base(self):
        self.derived["head_meta.json"] = b"wrong conversion"
        with self.assertRaisesRegex(ValueError, "mismatch"):
            self.run_fetch()
        self.assertFalse((self.root / "tiny/head.safetensors").exists())
        self.assertFalse(any("test/base" in url for url in self.requests))

    def test_corrupt_base_and_unpinned_shards_are_refused(self):
        self.run_fetch()
        self.requests.clear()
        shard = self.root / "base/model-extra.safetensors"
        shard.write_bytes(b"extra")
        with self.assertRaisesRegex(ValueError, "Unpinned"):
            self.run_fetch()
        shard.unlink()
        (self.root / "base/model.safetensors").write_bytes(b"wrong base")
        with self.assertRaisesRegex(ValueError, "mismatch"):
            self.run_fetch()
        self.assertEqual(self.requests, [])

    def test_moving_revisions_and_unsafe_paths_are_rejected(self):
        for part, field, value in [("adapter", "revision", "main"), ("base", "directory", "../outside")]:
            original = copy.deepcopy(self.lock)
            self.lock[part][field] = value
            with self.assertRaises(ValueError):
                self.run_fetch()
            self.lock = original
        self.assertEqual(self.requests, [])

    def test_corrupt_download_can_be_retried_without_accepting_partial(self):
        url = next(url for url in self.remote if url.endswith("model.safetensors") and "test/base" in url)
        self.remote[url] = b"partial"
        with self.assertRaisesRegex(ValueError, "mismatch"):
            self.run_fetch()
        self.assertFalse((self.root / "base/model.safetensors").exists())
        self.assertFalse((self.root / "tiny/artifact-lock.json").exists())
        self.remote[url] = self.base["model.safetensors"]
        self.run_fetch()
        self.assertTrue((self.root / "tiny/artifact-lock.json").exists())

    def test_head_cannot_select_a_different_base(self):
        self.derived["head_meta.json"] = b'{"base":"test/other"}'
        self.lock["derived_files"]["head_meta.json"] = record(self.derived["head_meta.json"])
        with self.assertRaisesRegex(ValueError, "base repository disagree"):
            self.run_fetch()
        self.assertFalse(any("test/base" in url for url in self.requests))


if __name__ == "__main__":
    unittest.main()
