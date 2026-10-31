#!/usr/bin/env python3
"""coax fmt (coax/web/fmt.coax over coax/lib/text/format.coax, r7a-format-evidence).

The formatter lays out whitespace outside literals only (CRLF, trailing
blanks, blank-line runs, leading blank lines, one final newline); the rules
and their zero-survivor mutation gate are in coax/lib/text/format.coax. These
fixtures run the built tool:

- every tracked repository .coax file, copied to a temporary corpus, formats
  and then checks clean (idempotent); files it refuses are refused by the
  compiler too, with the same code;
- comments and literals keep their bytes; malformed input is refused with
  its line and never written;
- --check lists what would change and modifies nothing;
- --write refuses a stale revision and leaves the file as it was;
- a deliberately messy copy of frozen Verse and Psionic fixtures formats back
  to the original bytes, checks the same, and its authored checks agree with
  the original's on the evaluator and checked -O0/-O2 native code.
No repository file is rewritten.
"""
import importlib.util
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REPO = ROOT.parent
TOOL = ROOT / "web/tool.sh"
COAX = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "bin/coax")).resolve()
SPEC = importlib.util.spec_from_file_location("coax_native_checks_fmt", ROOT / "tools/native_checks.py")
nc = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = nc
SPEC.loader.exec_module(nc)
FROZEN = ["verse/look.coax", "verse/stars.coax", "psionic/kernels/norm/agent/a1.coax"]


def fmt(*args, cwd=None):
    return subprocess.run([str(TOOL), "fmt", *map(str, args)], cwd=cwd, capture_output=True, text=True, timeout=600)


def fmt_many(mode, files, cwd):
    """One run per batch of files (the tool's argument storage is bounded)."""
    rc, out = 0, ""
    for k in range(0, len(files), 100):
        r = fmt(mode, *files[k:k + 100], cwd=cwd)
        if r.returncode == 2:
            raise AssertionError(r.stderr)
        rc = max(rc, r.returncode) if r.returncode != 4 or rc == 0 else rc
        out += r.stdout
    return rc, out


def messy(text: str) -> str:
    """The same program with every kind of trivia the formatter removes."""
    lines = text.split("\n")
    out = ["", "  "]
    for k, line in enumerate(lines):
        out.append(line + ("  \t" if k % 3 == 0 and line else ""))
        if not line:
            out += [" ", "\t", ""]   # a blank line becomes a run of them
    return "\r\n".join(out) + "\r\n\r\n"


class Format(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def test_repository_sources_format_idempotently(self):
        tracked = subprocess.run(["git", "ls-files", "*.coax"], cwd=REPO, capture_output=True, text=True, check=True).stdout.split()
        self.assertGreater(len(tracked), 500)
        corpus = self.dir / "corpus"
        for f in tracked:
            dst = corpus / f
            dst.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(REPO / f, dst)
        rc, out = fmt_many("--write", tracked, corpus)
        refused = {}
        for line in out.splitlines():
            path, _, rest = line.partition(":")
            if path in tracked:
                refused[path] = rest.split(" ")[1]
        self.assertIn(rc, (0, 1), out[-2000:])
        self.assertEqual(rc == 1, bool(refused), out[-2000:])
        for path, code in refused.items():
            c = subprocess.run([str(COAX), "check", Path(path).name], cwd=(REPO / path).parent, capture_output=True, text=True, timeout=300)
            self.assertIn(f"error[{code}]", c.stdout + c.stderr, f"{path}: refused by fmt with {code}, not by the compiler")
        ok = [f for f in tracked if f not in refused]
        before = {f: (corpus / f).read_bytes() for f in ok}
        rc, out = fmt_many("--check", ok, corpus)
        self.assertEqual(rc, 0, "not idempotent: " + out[:2000])
        rc, out = fmt_many("--write", ok, corpus)
        self.assertEqual(rc, 0, out[:2000])
        self.assertEqual([f for f in ok if (corpus / f).read_bytes() != before[f]], [])

    def test_literals_and_comments_keep_their_bytes(self):
        src = 's :: "a  \\t";  \r\n// keep  "this\r\n/* x  \n\n\n y */\r\nc :: \' \';\t\n\n\n'
        p = self.dir / "lit.coax"
        p.write_bytes(src.encode())
        r = fmt(p)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(r.stdout, 's :: "a  \\t";\n// keep  "this\n/* x\n\n\n y */\nc :: \' \';\n')

    def test_malformed_input_is_refused_and_never_written(self):
        cases = {"comment.coax": ("x :: 1;\n/* open\n", ":3: E0101 block comment is never closed"),
                 "string.coax": ('x :: 1;  \ns :: "open\n', ":2: E0106 string is never closed"),
                 "cr.coax": ("x :: 1;\ry :: 2;\n", ":1:  a lone carriage return or a NUL byte")}
        for name, (src, said) in cases.items():
            p = self.dir / name
            p.write_bytes(src.encode())
            r = fmt("--write", p)
            self.assertEqual(r.returncode, 1, name)
            self.assertIn(str(p) + said, r.stdout)
            self.assertEqual(p.read_bytes(), src.encode(), name + " was written")
        r = fmt("--write", self.dir / "missing.coax")
        self.assertEqual(r.returncode, 1)
        self.assertEqual(fmt().returncode, 2)
        self.assertEqual(fmt("--check", "--write", self.dir / "cr.coax").returncode, 2)
        self.assertEqual(fmt("--revision", "0", self.dir / "cr.coax").returncode, 2)

    def test_check_mode_modifies_nothing(self):
        a, b = self.dir / "a.coax", self.dir / "b.coax"
        a.write_bytes(b"x :: 1;  \n\n\n")
        b.write_bytes(b"y :: 2;\n")
        stat = a.stat().st_mtime_ns
        r = fmt("--check", a, b)
        self.assertEqual(r.returncode, 4)
        lines = r.stdout.splitlines()
        self.assertEqual(len(lines), 1)
        self.assertTrue(lines[0].startswith(str(a) + " ") and len(lines[0].split(" ")[-1]) == 16)
        self.assertEqual(a.read_bytes(), b"x :: 1;  \n\n\n")
        self.assertEqual(a.stat().st_mtime_ns, stat)
        self.assertEqual(fmt("--check", b).returncode, 0)

    def test_a_stale_revision_is_refused(self):
        a = self.dir / "a.coax"
        a.write_bytes(b"x :: 1;  \n")
        rev = fmt("--check", a).stdout.split()[-1]
        a.write_bytes(b"x :: 2;  \n")   # someone else edited it after we looked
        r = fmt("--write", "--revision", rev, a)
        self.assertEqual(r.returncode, 3)
        self.assertIn("stale", r.stdout)
        self.assertEqual(a.read_bytes(), b"x :: 2;  \n")
        rev2 = fmt("--check", a).stdout.split()[-1]
        r = fmt("--write", "--revision", rev2, a)
        self.assertEqual(r.returncode, 0, r.stdout)
        self.assertEqual(a.read_bytes(), b"x :: 2;\n")

    def test_frozen_fixtures_keep_their_meaning(self):
        for f in FROZEN:
            orig = REPO / f
            work = orig.parent / f".fmt-check-{orig.name}"   # beside it, so #load resolves
            try:
                work.write_bytes(messy(orig.read_text()).encode())
                before = nc.run([work], keep=self.dir / "keep")
                r = fmt("--write", work)
                self.assertEqual(r.returncode, 0, r.stdout)
                self.assertEqual(work.read_bytes(), orig.read_bytes(), f + ": not back to the canonical original")
                after = nc.run([work], keep=self.dir / "keep")
                c = subprocess.run([str(COAX), "check", work.name], cwd=work.parent, capture_output=True, text=True, timeout=300)
                o = subprocess.run([str(COAX), "check", orig.name], cwd=orig.parent, capture_output=True, text=True, timeout=300)
                self.assertEqual(c.returncode, o.returncode, f)
            finally:
                work.unlink(missing_ok=True)
            outcomes = lambda rec: [(c["name"], c["outcome"]) for x in rec["files"] for c in x["checks"]]
            self.assertEqual(outcomes(before), outcomes(after), f)
            self.assertTrue(outcomes(after) and all(o in ("passed", "rejected") for _, o in outcomes(after)), f)


if __name__ == "__main__":
    unittest.main()
