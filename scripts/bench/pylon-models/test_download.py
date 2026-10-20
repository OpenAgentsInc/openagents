import hashlib
import importlib.util
import io
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch


spec = importlib.util.spec_from_file_location("qualification", Path(__file__).with_name("run.py"))
qualification = importlib.util.module_from_spec(spec)
spec.loader.exec_module(qualification)


class Response(io.BytesIO):
    def __init__(self, data, status=200, headers=None):
        super().__init__(data)
        self.status = status
        self.headers = headers or {}


class ModelDownloadTests(unittest.TestCase):
    def setUp(self):
        disk = patch.object(qualification.shutil, "disk_usage", return_value=SimpleNamespace(free=100*1024**3))
        disk.start()
        self.addCleanup(disk.stop)
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.path = Path(self.directory.name) / "model.gguf"
        self.blobs = {"one": b"first-chunk", "two": b"second-chunk"}
        self.row = {
            "id": "fixture", "repo": "owner/model", "revision": "pinned-revision",
            "file": "model.gguf", "bytes": sum(map(len, self.blobs.values())),
            "download_parts": [
                {"file": name, "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
                for name, data in self.blobs.items()
            ],
        }

    def respond(self, request, timeout):
        name = request.full_url.rsplit("/", 1)[1]
        self.assertIn("/resolve/pinned-revision/", request.full_url)
        data = self.blobs[name]
        span = request.get_header("Range")
        if span:
            begin, end = span.removeprefix("bytes=").split("-")
            offset = int(begin)
            last = int(end) if end else len(data)-1
            return Response(data[offset:last+1], 206, {"Content-Range": f"bytes {offset}-{last}/{len(data)}"})
        return Response(data)

    def test_joins_verified_parts_and_resumes_inside_second_part(self):
        self.path.with_name("model.gguf.download").write_bytes(self.blobs["one"]+self.blobs["two"][:4])
        with patch.object(qualification.urllib.request, "urlopen", side_effect=self.respond) as fetch:
            digest = qualification.download_model(self.row, self.path)
        self.assertEqual(fetch.call_count, 1)
        self.assertEqual(fetch.call_args.args[0].get_header("Range"), "bytes=4-")
        self.assertEqual(self.path.read_bytes(), b"first-chunksecond-chunk")
        self.assertEqual(digest, qualification.verify_model(self.row, self.path))

    def test_range_ignored_restarts_only_the_current_part(self):
        self.path.with_name("model.gguf.download").write_bytes(self.blobs["one"]+b"seco")
        with patch.object(qualification.urllib.request, "urlopen", return_value=Response(self.blobs["two"])):
            qualification.download_model(self.row, self.path)
        self.assertEqual(self.path.read_bytes(), b"first-chunksecond-chunk")

    def test_parallel_ranges_join_in_order_and_resume_existing_prefix(self):
        self.row["download_workers"] = 3
        self.path.with_name("model.gguf.download").write_bytes(self.blobs["one"]+b"seco")
        with patch.object(qualification, "RANGE_CHUNK_BYTES", 3), patch.object(qualification.urllib.request, "urlopen", side_effect=self.respond):
            qualification.download_model(self.row, self.path)
        self.assertEqual(self.path.read_bytes(), b"first-chunksecond-chunk")

    def test_corrupted_finished_prefix_is_redownloaded(self):
        self.path.with_name("model.gguf.download").write_bytes(b"bad-prefix!"+self.blobs["two"])
        with patch.object(qualification.urllib.request, "urlopen", side_effect=self.respond):
            qualification.download_model(self.row, self.path)
        self.assertEqual(self.path.read_bytes(), b"first-chunksecond-chunk")

    def test_bad_checksum_preserves_previous_model(self):
        self.path.write_bytes(b"previous model")
        self.row["download_parts"][0]["sha256"] = "0"*64
        with patch.object(qualification.urllib.request, "urlopen", side_effect=self.respond):
            with self.assertRaisesRegex(ValueError, "SHA-256"):
                qualification.download_model(self.row, self.path)
        self.assertEqual(self.path.read_bytes(), b"previous model")

    def test_legacy_single_file_still_checks_its_checksum(self):
        row = dict(self.row, file="one", bytes=len(self.blobs["one"]), sha256=hashlib.sha256(self.blobs["one"]).hexdigest())
        del row["download_parts"]
        with patch.object(qualification.urllib.request, "urlopen", side_effect=self.respond):
            qualification.download_model(row, self.path)
        self.assertEqual(self.path.read_bytes(), self.blobs["one"])
        self.path.write_bytes(b"x"*row["bytes"])
        with self.assertRaisesRegex(ValueError, "SHA-256"):
            qualification.verify_model(row, self.path)


if __name__ == "__main__":
    unittest.main()
