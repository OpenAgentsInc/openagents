#!/usr/bin/env python3
"""Compile original checks to verification-only C and exercise the public ABI.

Each method is independently selectable. COAX_TEST_COMPILER selects an already
qualified compiler; COAX_EMISSION_RECORD_DIR retains commands and generated C.
COAX_EMISSION_PRODUCT_REFERENCE optionally names an independently captured
normal-C file and its adjacent .json provenance for prechange byte parity.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest

import quiet_crash


COAX = Path(__file__).resolve().parents[1]
REPO = COAX.parent
FIXTURES = COAX / "tests/native-check-emission"
COMPILER = Path(os.environ.get("COAX_TEST_COMPILER", COAX / "bin/coax")).resolve()


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


class NativeEmissionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        configured = os.environ.get("COAX_EMISSION_RECORD_DIR")
        cls.records = Path(configured or tempfile.mkdtemp(prefix="coax-native-emission-"))
        cls.records.mkdir(parents=True, exist_ok=True)
        cls.compiler_sha = sha(COMPILER)
        print(f"native emission receipts: {cls.records}", file=sys.stderr)

    def setUp(self):
        self.dir = Path(tempfile.mkdtemp(prefix=self._testMethodName + "-", dir=self.records))
        self.commands = []
        self.sources = {}
        self.artifacts = {}
        self.observations = {}
        self.started = time.monotonic()

    def tearDown(self):
        self.assertEqual(sha(COMPILER), self.compiler_sha, "compiler changed during a focused method")
        record = {"method": self._testMethodName, "compiler": str(COMPILER),
                  "compiler_sha256": self.compiler_sha,
                  "harness_sha256": sha(Path(__file__)),
                  "driver_sha256": sha(FIXTURES / "table-driver.c"),
                  "commands": self.commands, "artifacts": self.artifacts,
                  "sources": self.sources, "observations": self.observations,
                  "seconds": time.monotonic() - self.started}
        (self.dir / "receipt.json").write_text(json.dumps(record, indent=2) + "\n")
        for path, original_hash in self.sources.items():
            self.assertEqual(sha(path), original_hash, "emission changed original source bytes")

    def command(self, argv, *, quiet=False, timeout=120):
        argv = [str(part) for part in argv]
        started = time.monotonic()
        execute = quiet_crash.run if quiet else subprocess.run
        result = execute(argv, cwd=REPO, text=True, capture_output=True, timeout=timeout)
        self.commands.append({"argv": argv, "quiet_crash_launcher": quiet,
                              "returncode": result.returncode, "stdout": result.stdout,
                              "stderr": result.stderr, "seconds": time.monotonic() - started})
        return result

    def source(self, name):
        path = FIXTURES / (name + ".coax")
        self.sources[str(path)] = sha(path)
        return path

    def emit(self, source, *options):
        destination = self.dir / f"emission-{len(self.commands)}.c"
        original = source.read_bytes()
        result = self.command([COMPILER, "checks-c", source, *options, "-o", destination])
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(source.read_bytes(), original, "emission rewrote the original source")
        self.assertTrue(destination.is_file(), result.stdout + result.stderr)
        text = destination.read_text()
        self.assertIn("Coax_VerificationCheck_v1", text)
        self.assertIn("coax_verification_checks_v1", text)
        self.assertNotRegex(text, r"\b(?:int|void)\s+main\s*\(")
        self.artifacts[str(destination)] = sha(destination)
        return destination

    def link(self, source, optimization):
        executable = self.dir / f"native-{len(self.commands)}-{optimization[1:]}"
        result = self.command([os.environ.get("CC", "cc"), "-std=c11", optimization,
                               "-ffp-contract=off", source, FIXTURES / "table-driver.c",
                               "-lm", "-o", executable])
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.artifacts[str(executable)] = sha(executable)
        return executable

    def metadata(self, executable):
        result = self.command([executable, "metadata"], quiet=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        rows = []
        for line in result.stdout.splitlines():
            index, line_number, column, name, source = line.split("\t")
            rows.append({"declaration_index": int(index), "line": int(line_number),
                         "column": int(column), "name": bytes.fromhex(name).decode("utf-8"),
                         "source": str(Path(bytes.fromhex(source).decode("utf-8")).resolve())})
        self.assertTrue(rows)
        return rows

    def run_pass(self, executable, count, *arguments):
        result = self.command([executable, *arguments], quiet=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(result.stdout, f"ran {count} native checks\n")
        self.assertEqual(result.stderr, "")

    def refused(self, source, *options, eligibility=False):
        destination = self.dir / f"refused-{len(self.commands)}.c"
        destination.write_text("untouched output sentinel\n")
        result = self.command([COMPILER, "checks-c", source, *options, "-o", destination])
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(destination.read_text(), "untouched output sentinel\n")
        if eligibility:
            diagnostic = result.stdout + result.stderr
            self.assertIn("E0413", diagnostic)
            self.assertRegex(diagnostic, re.escape(source.name) + r":\d+:\d+")
        return result

    def test_scalar_buffers_defer_and_unrelated_effect_closure(self):
        emitted = self.emit(self.source("closure"))
        text = emitted.read_text()
        self.assertNotIn("unrelated_foreign", text)
        self.assertNotIn("unrelated_effect", text)
        self.assertNotIn("coax_p_main", text)
        for optimization in ("-O0", "-O2"):
            executable = self.link(emitted, optimization)
            self.assertEqual(len(self.metadata(executable)), 2)
            self.run_pass(executable, 2)

    def test_generic_callees_and_frozen_owned_fixed_aggregate(self):
        emitted = self.emit(self.source("generic-frozen"))
        for optimization in ("-O0", "-O2"):
            executable = self.link(emitted, optimization)
            self.assertEqual(len(self.metadata(executable)), 1)
            self.run_pass(executable, 1)

    def test_exact_names_duplicate_imported_physical_identity_and_selection(self):
        location = self.dir / "original sources with space"
        location.mkdir()
        for name in ("identity.coax", "imported.coax"):
            shutil.copyfile(FIXTURES / name, location / name)
            self.sources[str(location / name)] = sha(location / name)
        original = location / "identity.coax"
        emitted = self.emit(original)
        expected_name = 'punctuation [x]; quote" slash\\ nul\0 lambda λ'
        expected = {(str((location / "imported.coax").resolve()), 2, 1, "duplicate name"),
                    (str(original.resolve()), 2, 1, "duplicate name"),
                    (str(original.resolve()), 3, 3, expected_name)}
        reference = None
        for optimization in ("-O0", "-O2"):
            executable = self.link(emitted, optimization)
            rows = self.metadata(executable)
            self.assertEqual({(r["source"], r["line"], r["column"], r["name"]) for r in rows}, expected)
            self.assertEqual(sorted(r["declaration_index"] for r in rows), [0, 1, 2])
            if reference is None:
                reference = rows
            else:
                self.assertEqual(rows, reference)
            self.run_pass(executable, 3)
        selected = self.emit(original, "--only", "lambda λ")
        executable = self.link(selected, "-O0")
        rows = self.metadata(executable)
        self.assertEqual(rows, [next(r for r in reference if r["name"] == expected_name)])
        self.run_pass(executable, 1)

    def test_duplicate_imported_names_invoke_their_distinct_original_bodies(self):
        source = self.source("duplicate-failure")
        imported = self.source("imported-failure")
        emitted = self.emit(source)
        for optimization in ("-O0", "-O2"):
            executable = self.link(emitted, optimization)
            rows = self.metadata(executable)
            self.assertEqual(len(rows), 2)
            self.assertEqual([r["name"] for r in rows], ["duplicate physical body"] * 2)
            passing = next(i for i, row in enumerate(rows) if row["source"] == str(source.resolve()))
            failing = next(i for i, row in enumerate(rows) if row["source"] == str(imported.resolve()))
            self.assertNotEqual(rows[passing]["declaration_index"], rows[failing]["declaration_index"])
            self.run_pass(executable, 1, "one", str(passing))
            result = self.command([executable, "one", failing], quiet=True, timeout=10)
            self.assertEqual(result.returncode, -signal.SIGABRT, result.stdout + result.stderr)
            self.assertIn(imported.name, result.stderr)

    def test_non_authority_checks_emit_then_actual_native_assertion_and_overflow_trap(self):
        for name in ("assertion-fails", "overflow"):
            with self.subTest(fixture=name):
                emitted = self.emit(self.source(name))
                for optimization in ("-O0", "-O2"):
                    executable = self.link(emitted, optimization)
                    self.assertEqual(len(self.metadata(executable)), 1)
                    result = self.command([executable], quiet=True, timeout=10)
                    self.assertEqual(result.returncode, -signal.SIGABRT, result.stdout + result.stderr)
                    self.assertNotIn("ran 1 native checks", result.stdout)
                    self.assertTrue(result.stderr, "a checked trap must produce a diagnostic")

    def test_unsupported_selected_forms_are_source_located_and_output_untouched(self):
        for name in ("mixed-forall", "foreign", "print", "borrow-return"):
            with self.subTest(fixture=name):
                self.refused(self.source(name), eligibility=True)

    def test_supported_selection_keeps_original_ordinal_while_unselected_forall_is_not_emitted(self):
        emitted = self.emit(self.source("mixed-forall"), "--only", "selected ordinary")
        executable = self.link(emitted, "-O0")
        rows = self.metadata(executable)
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["name"], "selected ordinary check")
        self.assertEqual(rows[0]["declaration_index"], 1)
        self.run_pass(executable, 1)

    def test_no_match_and_no_checks_never_emit_successful_empty_suite(self):
        self.refused(self.source("closure"), "--only", "absent-name", eligibility=True)
        self.refused(self.source("no-checks"), eligibility=True)

    def test_authority_failing_original_baseline_cannot_be_hidden_by_only(self):
        result = self.refused(self.source("authority"), "--only", "selected passing")
        self.assertIn("unselected failing baseline", result.stdout + result.stderr)

    def test_output_aliases_preserve_original_source_graph_bytes(self):
        source = self.dir / "original.coax"
        original = self.source("generic-frozen").read_bytes()
        source.write_bytes(original)
        hardlink = self.dir / "hardlink.coax"
        symlink = self.dir / "symlink.coax"
        os.link(source, hardlink)
        symlink.symlink_to(source)
        for destination in (source, hardlink, symlink):
            with self.subTest(destination=destination.name):
                result = self.command([COMPILER, "checks-c", source, "-o", destination])
                self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertEqual(source.read_bytes(), original)
                self.assertEqual(destination.read_bytes(), original)
        entry = self.dir / "identity.coax"
        imported = self.dir / "imported.coax"
        for destination in (entry, imported):
            shutil.copyfile(FIXTURES / destination.name, destination)
        before = {path: path.read_bytes() for path in (entry, imported)}
        result = self.command([COMPILER, "checks-c", entry, "-o", imported])
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        for path, contents in before.items():
            self.assertEqual(path.read_bytes(), contents)

    def test_source_map_sidecar_is_refused_until_check_debug_walker_is_supported(self):
        sidecar = self.dir / "unsupported-map.json"
        sidecar.write_text("untouched sidecar sentinel\n")
        self.refused(self.source("generic-frozen"), "--source-map", sidecar)
        self.assertEqual(sidecar.read_text(), "untouched sidecar sentinel\n")

    def test_verification_arithmetic_is_checked_in_debug_and_release_profiles(self):
        source = self.source("generic-frozen")
        for options in (("--arithmetic", "wrapping"), ("--release",)):
            with self.subTest(options=options):
                result = self.refused(source, *options)
                self.assertIn("checked", (result.stdout + result.stderr).lower())
        emitted = self.emit(source, "--release", "--arithmetic", "checked")
        self.run_pass(self.link(emitted, "-O2"), 1)

    def test_normal_product_c_retains_original_main_and_verification_stays_separate(self):
        source = self.source("closure")
        destination = self.dir / "normal-product.c"
        result = self.command([COMPILER, "c", source.relative_to(REPO), "-o", destination])
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.artifacts[str(destination)] = sha(destination)
        text = destination.read_text()
        self.assertIn("unrelated_foreign", text)
        self.assertIn("unrelated_effect", text)
        self.assertRegex(text, r"\bint\s+main\s*\(")
        self.assertNotIn("Coax_VerificationCheck_v1", text)
        self.assertNotIn("coax_verification_checks_v1", text)
        reference_path = os.environ.get("COAX_EMISSION_PRODUCT_REFERENCE")
        if not reference_path:
            self.observations["prechange_product_byte_parity"] = "not requested"
            return
        reference = Path(reference_path)
        provenance = json.loads(reference.with_suffix(reference.suffix + ".json").read_text())
        self.assertEqual(provenance["source_sha256"], sha(source))
        self.assertEqual(provenance["c_sha256"], sha(reference))
        self.assertEqual(destination.read_bytes(), reference.read_bytes())
        self.observations["prechange_product_byte_parity"] = {
            "outcome": "passed", "reference": str(reference),
            "reference_compiler_sha256": provenance["compiler_sha256"],
            "c_sha256": sha(destination)}


if __name__ == "__main__":
    unittest.main()
