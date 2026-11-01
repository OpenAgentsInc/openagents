#!/usr/bin/env python3
"""`--diagnostics json`: one coax.diagnostic.v1 object per line on stderr.

One recorded diagnostic of each family (lexer, parser, checker, a failing
check) must match its `.jsonl`; every line must parse, and every span must lie
inside its file with a line and column that agree with its byte offset. An
"add 'X'" fix carries an edit, and applying the parser's makes the file check.
Without the flag, the text is byte-identical to the `.txt` recorded with the
release before the flag existed.
"""

import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
COMPILER = Path(os.environ.get("COAX_BIN", ROOT / "bin/coax"))
CASES = ROOT / "tests/diagnostics"
FAMILIES = {"lexer": "check", "parser": "check", "checker": "check", "test-failure": "test"}
KEYS = ["schema", "severity", "code", "message", "span", "notes", "fixes"]


def coax(*args):
    return subprocess.run([str(COMPILER), *args], cwd=ROOT, capture_output=True, text=True, timeout=60)


def timeless(text):
    return re.sub(r"\(\d+ ms\)", "(N ms)", text)


class DiagnosticsJson(unittest.TestCase):
    def check_span(self, span):
        self.assertEqual(sorted(span), ["column", "file", "length", "line", "offset"])
        data = (ROOT / span["file"]).read_bytes()
        self.assertGreaterEqual(span["offset"], 0)
        self.assertGreaterEqual(span["length"], 0)
        self.assertLessEqual(span["offset"] + span["length"], len(data), span)
        before = data[:span["offset"]]
        self.assertEqual(span["line"], before.count(b"\n") + 1, span)
        self.assertEqual(span["column"], span["offset"] - (before.rfind(b"\n") + 1) + 1, span)

    def diagnostics(self, name, *extra):
        result = coax(FAMILIES[name], "--diagnostics", "json", *extra, f"tests/diagnostics/{name}.coax")
        self.assertEqual(result.returncode, 1, result.stderr)
        return result.stderr

    def test_each_family_matches_its_record(self):
        for name in FAMILIES:
            with self.subTest(family=name):
                self.assertEqual(self.diagnostics(name), (CASES / f"{name}.jsonl").read_text())

    def test_every_line_parses_and_every_span_is_inside_its_file(self):
        seen = set()
        for name in FAMILIES:
            for line in self.diagnostics(name).splitlines():
                d = json.loads(line)
                self.assertEqual(list(d), KEYS)
                self.assertEqual(d["schema"], "coax.diagnostic.v1")
                self.assertEqual(d["severity"], "error")
                self.assertRegex(d["code"], r"^E0\d{3}$")
                seen.add(d["code"][:3])
                self.check_span(d["span"])
                for note in d["notes"]:
                    self.assertIsInstance(note["message"], str)
                    if note["span"] is not None:
                        self.check_span(note["span"])
                for fix in d["fixes"]:
                    for edit in fix["edits"]:
                        self.check_span(edit["span"])
                        self.assertIsInstance(edit["replacement"], str)
        self.assertEqual(seen, {"E01", "E02", "E03", "E04"})

    def test_a_check_failure_names_its_check_in_a_note(self):
        d = json.loads(self.diagnostics("test-failure"))
        self.assertEqual(d["message"], "assertion failed")
        self.assertEqual(d["notes"][0]["message"], "in #test doubling adds")
        self.assertEqual(d["notes"][0]["span"]["line"], 2)

    def test_applying_the_parser_fix_makes_the_file_check(self):
        d = json.loads(self.diagnostics("parser"))
        [edit] = d["fixes"][0]["edits"]
        data = (ROOT / edit["span"]["file"]).read_bytes()
        start = edit["span"]["offset"]
        fixed = data[:start] + edit["replacement"].encode() + data[start + edit["span"]["length"]:]
        with tempfile.TemporaryDirectory(prefix="coax-diagnostics-") as d:
            path = Path(d) / "fixed.coax"
            path.write_bytes(fixed)
            result = coax("check", str(path))
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_mutate_reports_the_same_diagnostic(self):
        result = coax("mutate", "--diagnostics", "json", "tests/diagnostics/checker.coax")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stderr, (CASES / "checker.jsonl").read_text())

    def test_text_is_unchanged_without_the_flag(self):
        for name in FAMILIES:
            with self.subTest(family=name):
                for flags in ((), ("--diagnostics", "text")):
                    result = coax(FAMILIES[name], *flags, f"tests/diagnostics/{name}.coax")
                    self.assertEqual(timeless(result.stdout + result.stderr) if name == "test-failure" else result.stderr,
                                     timeless((CASES / f"{name}.txt").read_text()) if name == "test-failure"
                                     else (CASES / f"{name}.txt").read_text())

    def test_an_unknown_format_is_refused(self):
        self.assertNotEqual(coax("check", "--diagnostics", "xml", "tests/diagnostics/checker.coax").returncode, 0)


if __name__ == "__main__":
    unittest.main()
