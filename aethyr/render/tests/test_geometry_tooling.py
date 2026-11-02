"""Failure controls for the producer qualification harness, without rerunning it."""
import copy
import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("geometry_qualification", Path(__file__).with_name("check_geometry.py"))
q = importlib.util.module_from_spec(spec)
spec.loader.exec_module(q)


class GeometryTooling(unittest.TestCase):
    def fixture(self):
        counts = dict(draws=2, hud=6, grass=0, particles=0, surface=0, caster_vertices=0, caster_instances=0)
        result = dict(geometry_status=0, frame_status=0, bad_draw=-1, bad_resource=-1, work=5,
                      frame_bad_draw=-1, frame_bad_upload=-1, frame_bad_direct=-1,
                      frame_admitted_draws=2, frame_checked_indices=3,
                      domains=[[0, 3, 0, 0, 0, 1, 3], [0, 3, 0, 0, 0, 1, 3]], direct=[8, 0, 0, 0, 0, 0])
        return result, counts, (0, 3, 0, 1, 0, 3, 0x80000000, 1), (0, 2, 1)

    def test_wire_audit_detects_lost_ranges_counts_and_indices(self):
        values = self.fixture()
        q.wire_audit(*values)
        for fault in ("vertex", "instance", "direct", "checked", "admitted", "status"):
            result, counts, draws, indices = copy.deepcopy(values)
            if fault == "vertex": result["domains"][0][1] = 2
            if fault == "instance": result["domains"][1][5] = 0
            if fault == "direct": result["direct"][0] = 5
            if fault == "checked": result["frame_checked_indices"] = 0
            if fault == "admitted": result["frame_admitted_draws"] = 1
            if fault == "status": result["frame_status"] = 1
            with self.subTest(fault=fault), self.assertRaises(AssertionError):
                q.wire_audit(result, counts, draws, indices)
        with self.assertRaises(AssertionError):
            q.wire_audit(*values[:3], (0, 2, 3))

    def test_refusal_detects_partial_output_publication(self):
        result = dict(geometry_status=4, domains=[[0, 99, 0, 0, 0, 0, 0]])
        q.atomic_refusal(result, 1)
        result["domains"][0][1] = 3
        with self.assertRaises(AssertionError): q.atomic_refusal(result, 1)

    def test_header_inventory_refuses_an_added_or_omitted_observable(self):
        header = "\n".join("struct coax_t_" + name + " {" + "".join("int " + field + ";" for field in fields) + "};" for name, fields in q.FIELDS.items())
        q.header_inventory(header)
        for bad in (header.replace("int work;", ""), header.replace("int work;", "int work;int hidden;")):
            with self.assertRaises(AssertionError): q.header_inventory(bad)

    def test_stale_compiler_stops_before_emission_or_capture_loading(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / "coax/bin/coax"
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b"not executed")
            (binary.parent / "source-digest").write_text("old\n")
            calls = []

            def fake_run(argv, **kwargs):
                calls.append(argv[1])
                return subprocess.CompletedProcess(argv, 0, b"coax0 test\n" if argv[1] == "version" else b"new\n", b"")

            argv = ["check_geometry.py", "--captures", str(root / "missing"), "--work-dir", str(root / "work"), "--receipt", str(root / "receipt.json")]
            with patch.object(q, "ROOT", root), patch.object(sys, "argv", argv), patch.object(q.subprocess, "run", fake_run):
                with self.assertRaisesRegex(RuntimeError, "source digests differ"):
                    q.main()
            self.assertEqual(calls, ["version", "source-digest"])


if __name__ == "__main__": unittest.main()
