#!/usr/bin/env python3
"""`--diagnostics json`: one coax.diagnostic.v1 object per line on stderr.

One recorded diagnostic of each family (lexer, parser, checker, a failing
check) must match its `.jsonl`; every line must parse, and every span must lie
inside its file with a line and column that agree with its byte offset. An
"add 'X'" fix carries an edit, and applying the parser's makes the file check.
`coax mutate` gives each surviving mutant (E0414) and each procedure no check
reaches (E0415) as an object too.
After the first error the lexer, parser and source loader keep going: the
parser again from the next top-level declaration, the loader with the next
#load. The first error and the exit status are what the compiler printed
before it recovered (`NAME.before.txt`); a later error that may follow from
an earlier one carries a note naming it, and reporting stops at 20 errors.
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
FAMILIES = {"lexer": "check", "parser": "check", "checker": "check", "test-failure": "test",
            "recovery": "check", "cascade": "check"}
RECOVERED = ["lexer", "recovery", "cascade"]
NOTE = "this may follow from the error at "
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

    def test_mutate_reports_survivors_and_unreached_procedures(self):
        result = coax("mutate", "--diagnostics", "json", "tests/diagnostics/survivor.coax")
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertEqual(result.stdout, "2 mutants, 0 killed, 2 survived\n")
        self.assertEqual(result.stderr, (CASES / "survivor.jsonl").read_text())
        records = [json.loads(line) for line in result.stderr.splitlines()]
        self.assertEqual([r["code"] for r in records], ["E0414", "E0414", "E0415"])
        for r in records:
            self.assertEqual(list(r), KEYS)
            self.check_span(r["span"])
        self.assertEqual([r["span"]["line"] for r in records], [2, 2, 3])
        text = coax("mutate", "tests/diagnostics/survivor.coax")
        self.assertEqual(text.returncode, 1)
        self.assertIn("survivor.coax:2:37: mutant survived: '*' became '+'", text.stdout)
        self.assertIn("not reached by any check: next", text.stdout)
        self.assertEqual(text.stderr, "")

    def test_text_is_unchanged_without_the_flag(self):
        for name in FAMILIES:
            with self.subTest(family=name):
                for flags in ((), ("--diagnostics", "text")):
                    result = coax(FAMILIES[name], *flags, f"tests/diagnostics/{name}.coax")
                    self.assertEqual(timeless(result.stdout + result.stderr) if name == "test-failure" else result.stderr,
                                     timeless((CASES / f"{name}.txt").read_text()) if name == "test-failure"
                                     else (CASES / f"{name}.txt").read_text())

    def test_the_first_error_and_exit_status_are_unchanged(self):
        for name in RECOVERED:
            with self.subTest(family=name):
                before = (CASES / f"{name}.before.txt").read_text()
                first = re.sub(r"\d+ errors?\n$", "", before)
                result = coax("check", f"tests/diagnostics/{name}.coax")
                self.assertEqual(result.returncode, 1)
                self.assertTrue(result.stderr.startswith(first), result.stderr)
                self.assertGreater(result.stderr.count(": error[E0"), before.count(": error[E0"))

    def test_each_declaration_reports_its_own_syntax_error(self):
        records = [json.loads(line) for line in self.diagnostics("recovery").splitlines()]
        self.assertEqual([(r["code"], r["span"]["line"]) for r in records], [("E0201", 3), ("E0201", 5), ("E0201", 10)])
        self.assertEqual([r["notes"] for r in records], [[], [], []])
        self.assertTrue(coax("check", "tests/diagnostics/recovery.coax").stderr.endswith("\n3 errors\n"))

    def test_likely_cascades_name_the_error_they_may_follow(self):
        # A lexer error in the declaration: the parser's error after it.
        lexer = [json.loads(line) for line in self.diagnostics("lexer").splitlines()]
        self.assertEqual([r["code"] for r in lexer], ["E0106", "E0201"])
        [note] = lexer[1]["notes"]
        self.assertEqual(note["message"], NOTE + "tests/diagnostics/lexer.coax:2:8; fix that one first")
        self.assertEqual(note["span"], lexer[0]["span"])
        # An unclosed '{': the restart was a guess, so the next declaration's
        # error may follow it; the one after that stands on its own.
        cascade = [json.loads(line) for line in self.diagnostics("cascade").splitlines()]
        self.assertEqual([(r["code"], r["span"]["line"]) for r in cascade], [("E0206", 6), ("E0201", 6), ("E0201", 7)])
        self.assertEqual([len(r["notes"]) for r in cascade], [0, 1, 0])
        self.assertEqual(cascade[1]["notes"][0]["span"]["column"], 7)
        self.assertIn("  note: " + NOTE + "tests/diagnostics/cascade.coax:6:7; fix that one first\n",
                      coax("check", "tests/diagnostics/cascade.coax").stderr)

    def test_reporting_stops_at_twenty_errors(self):
        with tempfile.TemporaryDirectory(prefix="coax-diagnostics-") as d:
            path = Path(d) / "many.coax"
            path.write_text("".join(f"c{i} :: {i}\n" for i in range(25)))
            text = coax("check", str(path))
            self.assertEqual(text.returncode, 1)
            self.assertEqual(text.stderr.count(": error[E0201]"), 20)
            self.assertTrue(text.stderr.endswith("coax: stopped after 20 errors; fix these and check again\n20 errors\n"),
                            text.stderr[-200:])
            records = [json.loads(line) for line in coax("check", "--diagnostics", "json", str(path)).stderr.splitlines()]
            self.assertEqual([r["severity"] for r in records], ["error"] * 20 + ["note"])
            self.assertEqual(records[-1]["message"], "stopped after 20 errors; fix these and check again")
            # The lexer stops at the same limit.
            path.write_text("main :: () {\n" + "  $\n" * 25 + "}\n")
            text = coax("check", str(path))
            self.assertEqual((text.returncode, text.stderr.count(": error[E0108]")), (1, 20))
            self.assertIn("coax: stopped after 20 errors", text.stderr)

    def test_the_loader_tries_every_load(self):
        with tempfile.TemporaryDirectory(prefix="coax-diagnostics-") as d:
            root = Path(d)
            (root / "main.coax").write_text('#load "missing-one.coax";\n#load "badlex.coax";\n#load "missing-two.coax";\nmain :: () { }\n')
            (root / "badlex.coax").write_text("helper :: () -> int {\n  return 1 $ 2;\n}\nother :: 4\n")
            result = coax("check", "--diagnostics", "json", str(root / "main.coax"))
            self.assertEqual(result.returncode, 1)
            records = [json.loads(line) for line in result.stderr.splitlines()]
            self.assertEqual([(r["code"], Path(r["span"]["file"]).name, r["span"]["line"]) for r in records],
                             [("E0601", "main.coax", 1), ("E0108", "badlex.coax", 2), ("E0201", "badlex.coax", 2),
                              ("E0201", "badlex.coax", 4), ("E0601", "main.coax", 3)])
            self.assertEqual([len(r["notes"]) for r in records], [0, 0, 1, 0, 0])
            # A unit that failed is finished, not a cycle, when loaded again.
            (root / "main.coax").write_text('#load "badlex.coax";\n#load "again.coax";\nmain :: () { }\n')
            (root / "again.coax").write_text('#load "badlex.coax";\n')
            result = coax("check", "--diagnostics", "json", str(root / "main.coax"))
            self.assertEqual([json.loads(line)["code"] for line in result.stderr.splitlines()], ["E0108", "E0201", "E0201"])

    def test_an_unknown_format_is_refused(self):
        self.assertNotEqual(coax("check", "--diagnostics", "xml", "tests/diagnostics/checker.coax").returncode, 0)


if __name__ == "__main__":
    unittest.main()
