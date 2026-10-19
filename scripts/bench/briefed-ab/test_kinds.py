#!/usr/bin/env python3
"""Tests for the per-kind check contract (#11310), the acceptance fallback,
and the re-brief evidence (#11311).

    python3 -m pytest -q scripts/bench/briefed-ab/test_kinds.py
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import acceptance  # noqa: E402
import kinds  # noqa: E402
import rebrief  # noqa: E402


def git(root: Path, *args: str) -> str:
    return subprocess.run(["git", "-C", str(root), "-c", "user.name=t", "-c", "user.email=t@t",
                           "-c", "commit.gpgsign=false", *args],
                          check=True, capture_output=True, text=True).stdout


class Repo:
    def __init__(self):
        self.dir = tempfile.TemporaryDirectory()
        self.root = Path(self.dir.name)
        files = {
            "Cargo.toml": '[workspace]\nmembers = ["crates/*"]\nexclude = ["crates/openagents-mobile"]\n',
            "crates/ui/Cargo.toml": '[package]\nname = "ui"\nversion = "0.1.0"\n',
            "crates/ui/src/lib.rs": 'pub const CSS: &str = include_str!("../static/shell.css");\n',
            "crates/ui/static/shell.css": "a { color: red }\n",
            "crates/web/Cargo.toml": '[package]\nname = "web"\nversion = "0.1.0"\n',
            "crates/web/src/lib.rs": 'pub const PAGE: &str = include_str!("../../../docs/page.md");\n',
            "crates/openagents-mobile/Cargo.toml": '[workspace]\n[package]\nname = "openagents-mobile"\nversion = "0.1.0"\n',
            "crates/openagents-mobile/src/lib.rs": "pub fn m() {}\n",
            "scripts/dev/tool.py": "def answer():\n    return 1\n",
            "scripts/tests/test_tool.py": "# exercises scripts/dev/tool.py\nimport unittest\n",
            "scripts/run-thing.sh": "#!/usr/bin/env bash\necho hi\n",
            "scripts/run-thing.test.sh": "#!/usr/bin/env bash\nbash scripts/run-thing.sh\n",
            "docs/guide.md": "See [the tool](../scripts/dev/tool.py) and [gone](missing.md).\n",
            "docs/page.md": "# Page\n",
            "bins/android/app/Pay.kt": "fun pay() {}\n",
        }
        for path, text in files.items():
            p = self.root / path
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(text)
        git(self.root, "init", "-q")
        git(self.root, "add", "-A")
        git(self.root, "commit", "-q", "-m", "base")
        self.base = git(self.root, "rev-parse", "HEAD").strip()

    def close(self):
        self.dir.cleanup()


class KindsTests(unittest.TestCase):
    def setUp(self):
        self.repo = Repo()
        self.tree = kinds.Tree(self.repo.root)

    def tearDown(self):
        self.repo.close()

    def ids(self, files, **kw):
        return [c["id"] for c in kinds.plan(files, self.tree, self.repo.base, **kw)]

    def test_each_kind_gets_its_own_checks(self):
        got = self.ids(["scripts/dev/tool.py", "scripts/run-thing.sh", "docs/guide.md",
                        "bins/android/app/Pay.kt"])
        self.assertIn("py_compile:scripts/dev", got)
        self.assertIn("tests:scripts/dev", got)
        self.assertIn("bash_n[run-thing.sh]:scripts", got)
        self.assertIn("tests:scripts", got)
        self.assertIn("doccheck:docs", got)
        self.assertIn("kotlinc:bins/android", got)

    def test_script_tests_are_found_by_name_and_by_mention(self):
        plan = {c["id"]: c for c in kinds.plan(["scripts/dev/tool.py", "scripts/run-thing.sh"], self.tree)}
        self.assertIn("scripts/tests/test_tool.py", plan["tests:scripts/dev"]["argv"])
        self.assertEqual(plan["tests:scripts"]["argv"], ["bash", "scripts/run-thing.test.sh"])

    def test_a_test_named_test_dash_script_is_found(self):
        (self.repo.root / "scripts/test-run-thing.sh").write_text("bash scripts/run-thing.sh\n")
        plan = {c["id"]: c for c in kinds.plan(["scripts/run-thing.sh"], self.tree)}
        self.assertIn("scripts/test-run-thing.sh", " ".join(" ".join(c["argv"]) for c in plan.values()))

    def test_a_file_inside_a_crate_is_checked_by_the_crate(self):
        self.assertEqual(self.ids(["crates/ui/static/shell.css"]), ["check:ui", "test:ui", "fmt:ui"])

    def test_root_workspace_rust_can_be_left_to_verify(self):
        self.assertEqual(self.ids(["crates/ui/src/lib.rs"], rust=False), [])

    def test_a_nested_workspace_crate_uses_its_manifest(self):
        plan = kinds.plan(["crates/openagents-mobile/src/lib.rs"], self.tree, rust=False)
        self.assertEqual([c["kind"] for c in plan], ["rust-nested"] * 3)
        self.assertIn("--manifest-path", plan[1]["argv"])

    def test_the_apps_workspace_is_nested(self):
        self.assertEqual(kinds.nested_root("apps/openagents/crates/client/src/lib.rs"), "apps/openagents")

    def test_an_embedded_document_brings_its_consumer_crate(self):
        got = self.ids(["docs/page.md"])
        self.assertEqual(got[0], "doccheck:docs")
        self.assertIn("test:web", got)

    def test_kotlin_is_listed_but_never_passes(self):
        check = kinds.plan(["bins/android/app/Pay.kt"], self.tree)[0]
        self.assertFalse(check["available"])
        result = kinds.run_check(check, self.repo.root)
        self.assertIsNone(result["ok"])

    def test_checks_run_and_fail_with_a_tail(self):
        (self.repo.root / "scripts/dev/tool.py").write_text("def answer(:\n")
        check = kinds.plan(["scripts/dev/tool.py"], self.tree)[0]
        result = kinds.run_check(check, self.repo.root)
        self.assertFalse(result["ok"])
        self.assertIn("tool.py", result["tail"])

    def test_doccheck_reports_only_new_breakage(self):
        guide = self.repo.root / "docs/guide.md"
        guide.write_text(guide.read_text() + "And `scripts/dev/nope.py` and [new](also-missing.md).\n")
        out = subprocess.run([sys.executable, str(HERE / "kinds.py"), "doccheck", "--root", str(self.repo.root),
                              "--base", self.repo.base, "docs/guide.md"], capture_output=True, text=True)
        self.assertEqual(out.returncode, 1)
        self.assertIn("also-missing.md", out.stdout)
        self.assertIn("scripts/dev/nope.py", out.stdout)
        self.assertNotIn("missing.md (resolves to docs/missing.md)", out.stdout)

    def test_implicated_reads_tracebacks_and_shell_errors(self):
        text = 'File "scripts/dev/tool.py", line 3, in x\nscripts/run-thing.sh: line 2: oops\n'
        self.assertEqual([i["file"] for i in kinds.implicated(text)], ["scripts/dev/tool.py", "scripts/run-thing.sh"])


class AcceptanceTests(unittest.TestCase):
    def test_without_the_model_the_items_are_the_issue_bullets_and_say_so(self):
        os.environ["OA_ACCEPTANCE"] = "off"
        with tempfile.TemporaryDirectory() as tmp:
            os.environ["OA_ACCEPTANCE_CACHE"] = tmp
            got = acceptance.extract({"issue": 1, "title": "t", "body": "Do this:\n- one thing\n- another"},
                                     [{"id": "test:x", "required": True}, {"id": "fmt:x", "required": True}])
        del os.environ["OA_ACCEPTANCE"]
        self.assertEqual([i["text"] for i in got["items"]], ["one thing", "another"])
        self.assertEqual(got["items"][0]["check_ids"], ["test:x"])
        self.assertIn("off", got["source"])

    def test_the_schema_limits_check_ids_to_the_briefing_checks(self):
        s = acceptance.schema(["test:x", "doccheck:docs"])
        ids = s["properties"]["items"]["items"]["properties"]["check_ids"]["items"]
        self.assertEqual(ids["enum"], ["test:x", "doccheck:docs"])


class RebriefTests(unittest.TestCase):
    def test_evidence_puts_blamed_files_first_then_misses(self):
        with tempfile.TemporaryDirectory() as tmp:
            d = Path(tmp)
            (d / "summary.json").write_text(json.dumps({"misses": [{"file": "crates/a/src/extra.rs"}]}))
            rows = [{"status": "compile_error",
                     "compile_errors": [{"file": "crates/a/src/lib.rs", "line": 9, "msg": "E0425 nope"}],
                     "untouched_but_implicated": [{"file": "crates/a/src/other.rs", "reason": "co-change"}]}]
            (d / "verify.jsonl").write_text("\n".join(json.dumps(r) for r in rows))
            replay = [{"name": "cargo test -p a", "ok": False, "tail": "error: crates/a/src/lib.rs:9:5: nope"}]
            diff = "diff --git a/crates/a/src/lib.rs b/crates/a/src/lib.rs\n+++ b/crates/a/src/lib.rs\n+x\n"
            ev = rebrief.evidence(d, replay, diff, lambda p: not p.endswith("gone.rs"))
        self.assertEqual([s["path"] for s in ev["seeds"]],
                         ["crates/a/src/lib.rs", "crates/a/src/other.rs", "crates/a/src/extra.rs"])
        self.assertEqual(ev["seeds"][0]["lines"], [9, 9])
        self.assertIn("compile_error", ev["wrong"])
        self.assertIn("discarded", ev["wrong"])
        self.assertEqual(ev["sections"], ["its change", "last verify", "replayed checks", "misses"])

    def test_no_change_and_no_verify_is_said(self):
        with tempfile.TemporaryDirectory() as tmp:
            ev = rebrief.evidence(Path(tmp), [], "", lambda p: True)
        self.assertIn("no change", ev["wrong"])
        self.assertEqual(ev["seeds"], [])


if __name__ == "__main__":
    unittest.main()
