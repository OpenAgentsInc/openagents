#!/usr/bin/env python3
"""The frozen authoring-task corpus (ROADMAP "Early evidence and authoring tools",
r0-authoring-corpus): four small Coax edits an agent makes, each judged by an
oracle the agent can't edit.

  python3 coax/tests/authoring.py list
  python3 coax/tests/authoring.py prepare TASK DIR     # the task's starting files into DIR
  python3 coax/tests/authoring.py check DIR            # one check of the edit (a repair iteration)
  python3 coax/tests/authoring.py record DIR --agent NAME [--model M]   # the final check, kept in runs.jsonl
  python3 coax/tests/authoring.py                      # the corpus's own tests

The tasks live in coax/tests/authoring/tasks/TASK: start/ (what the agent gets,
task.md says what to do), oracle/oracle.coax (#loads the edited file; copied in
only for a check and removed after) and solution/ (a reference edit that shows
the oracle can pass; an agent doing a task doesn't read it). manifest.json pins
every start file's and oracle's SHA-256, the files a task may edit and its
budget: a changed oracle or start is an infrastructure error, never a pass.

Outcomes, as in coax.evidence.v1: passed (every oracle check ran and passed),
counterexample (an oracle check failed), rejected (the edit doesn't compile),
infrastructure-error (the oracle or start changed, the compiler is missing or
the check ran out of time), not-checked (no check was run). A record also keeps
the repair iterations (checks before the final one), unintended edits (files
changed that the task doesn't allow), the lines changed (review effort) and the
seconds from prepare to record, against the pinned starting revision.
Only the check tool's runs are counted: an agent that runs the compiler itself
gets feedback the iteration count doesn't see (first run, type-error).
"""
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REPO = ROOT.parent
CORPUS = Path(__file__).resolve().parent / "authoring"
MANIFEST = CORPUS / "manifest.json"
RUNS = CORPUS / "runs.jsonl"
COAX = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "bin/coax")).resolve()
STATE = ".authoring.json"
ORACLE_NAME = "__oracle.coax"


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def manifest() -> dict:
    return json.loads(MANIFEST.read_text())


def task_of(name: str) -> dict:
    for t in manifest()["tasks"]:
        if t["id"] == name:
            return t
    raise SystemExit(f"authoring: no task {name!r} (see: authoring.py list)")


def prepare(name: str, work: Path) -> dict:
    t = task_of(name)
    work.mkdir(parents=True, exist_ok=True)
    if any(work.iterdir()):
        raise SystemExit(f"authoring: {work} is not empty")
    start = CORPUS / "tasks" / name / "start"
    for rel in t["start"]:
        shutil.copy2(start / rel, work / rel)
    state = {"task": name, "prepared": time.time(), "checks": 0, "revision": git_revision()}
    (work / STATE).write_text(json.dumps(state, indent=2) + "\n")
    return state


def git_revision() -> str:
    try:
        return subprocess.run(["git", "-C", str(REPO), "rev-parse", "HEAD"], capture_output=True, text=True, check=True).stdout.strip()
    except Exception:
        return "unknown"


def changed_lines(a: str, b: str) -> int:
    import difflib
    return sum(1 for line in difflib.unified_diff(a.splitlines(), b.splitlines(), lineterm="", n=0)
               if line[:1] in "+-" and not line.startswith(("+++", "---")))


def check(work: Path, count: bool = True) -> dict:
    state = json.loads((work / STATE).read_text())
    name = state["task"]
    t = task_of(name)
    base = CORPUS / "tasks" / name
    result = {"task": name, "outcome": "not-checked", "detail": "", "unintended": [], "lines_changed": 0}
    if count:
        state["checks"] += 1
        (work / STATE).write_text(json.dumps(state, indent=2) + "\n")
    # the frozen inputs: an oracle or a start that moved makes no verdict
    oracle = base / "oracle" / "oracle.coax"
    if sha(oracle) != t["oracle_sha256"]:
        return {**result, "outcome": "infrastructure-error", "detail": "the oracle changed from its pinned SHA-256"}
    for rel, want in t["start_sha256"].items():
        if sha(base / "start" / rel) != want:
            return {**result, "outcome": "infrastructure-error", "detail": f"start/{rel} changed from its pinned SHA-256"}
    if not COAX.exists():
        return {**result, "outcome": "infrastructure-error", "detail": f"no compiler at {COAX}"}
    # what the agent changed: allowed files and anything else
    unintended, lines = [], 0
    present = {p.relative_to(work).as_posix() for p in work.rglob("*") if p.is_file()} - {STATE}
    for rel in sorted(present | set(t["start"])):
        now = work / rel
        was = base / "start" / rel
        before = was.read_text() if was.exists() else ""
        after = now.read_text() if now.exists() else ""
        if before == after:
            continue
        if rel in t["editable"]:
            lines += changed_lines(before, after)
        else:
            unintended.append(rel)
    result["unintended"], result["lines_changed"] = unintended, lines
    if (work / ORACLE_NAME).exists():
        return {**result, "outcome": "infrastructure-error", "detail": f"{ORACLE_NAME} is the oracle's name in the work directory"}
    # the oracle, copied in for this run only
    shutil.copy2(oracle, work / ORACLE_NAME)
    try:
        run = subprocess.run([str(COAX), "test", ORACLE_NAME, "--steps", str(t["budget"]["steps"])], cwd=work,
                             capture_output=True, text=True, timeout=t["budget"]["seconds"])
    except subprocess.TimeoutExpired:
        return {**result, "outcome": "infrastructure-error", "detail": "the check ran past its time budget"}
    finally:
        (work / ORACLE_NAME).unlink(missing_ok=True)
    out = run.stdout + run.stderr
    last = [line for line in out.splitlines() if line.strip()][-1:] or [""]
    result["detail"] = last[0]
    if "error[" in out and "error[E0406]" not in out:
        result["outcome"] = "rejected"
    elif run.returncode == 0 and " 0 failed" in last[0] and "error[" not in out:
        want = t["oracle_checks"]
        result["outcome"] = "passed" if last[0].startswith(f"{want} checks, {want} passed") else "counterexample"
        if result["outcome"] != "passed":
            result["detail"] += f" (the oracle has {want} checks)"
    else:
        result["outcome"] = "counterexample"
    return result


def record(work: Path, agent: str, model: str) -> dict:
    state = json.loads((work / STATE).read_text())
    r = check(work, count=False)
    c = COAX.read_bytes() if COAX.exists() else b""
    entry = {
        "corpus": manifest()["version"], "task": state["task"], "agent": agent, "model": model,
        "revision": state["revision"], "compiler_sha256": hashlib.sha256(c).hexdigest(),
        "outcome": r["outcome"], "detail": r["detail"],
        "repair_iterations": max(0, state["checks"] - 1), "unintended_edits": r["unintended"],
        "lines_changed": r["lines_changed"], "seconds": round(time.time() - state["prepared"], 1),
        "budget": task_of(state["task"])["budget"],
    }
    with RUNS.open("a") as f:
        f.write(json.dumps(entry) + "\n")
    return entry


class Corpus(unittest.TestCase):
    """The corpus's own oracle: each start fails, each reference passes, and the
    frozen inputs can't be moved or edited away."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def work(self, name: str) -> Path:
        w = self.dir / name
        prepare(name, w)
        return w

    def test_manifest_pins_what_is_there(self):
        m = manifest()
        self.assertEqual(sorted(t["id"] for t in m["tasks"]), ["add-field", "kernel-taps", "protocol-adapter", "type-error"])
        for t in m["tasks"]:
            base = CORPUS / "tasks" / t["id"]
            self.assertEqual(sha(base / "oracle" / "oracle.coax"), t["oracle_sha256"], t["id"])
            for rel, want in t["start_sha256"].items():
                self.assertEqual(sha(base / "start" / rel), want, f"{t['id']}/{rel}")
            self.assertEqual(sorted(t["start_sha256"]), sorted(t["start"]))
            self.assertTrue(set(t["editable"]) <= set(t["start"]))

    def test_each_start_fails_and_each_reference_passes(self):
        for t in manifest()["tasks"]:
            w = self.work(t["id"])
            self.assertIn(check(w)["outcome"], ("counterexample", "rejected"), t["id"])
            for rel in t["editable"]:
                shutil.copy2(CORPUS / "tasks" / t["id"] / "solution" / rel, w / rel)
            r = check(w)
            self.assertEqual(r["outcome"], "passed", f"{t['id']}: {r}")
            self.assertEqual(r["unintended"], [])
            self.assertGreater(r["lines_changed"], 0)
            self.assertFalse((w / ORACLE_NAME).exists(), "the oracle is removed after a check")

    def test_type_error_start_is_rejected_not_failed(self):
        self.assertEqual(check(self.work("type-error"))["outcome"], "rejected")

    def test_an_edited_oracle_is_never_a_pass(self):
        t = task_of("kernel-taps")
        oracle = CORPUS / "tasks" / "kernel-taps" / "oracle" / "oracle.coax"
        saved = oracle.read_bytes()
        try:
            oracle.write_text("main :: () {}\n#test \"t\" { #assert true; }\n")
            r = check(self.work("kernel-taps"))
            self.assertEqual(r["outcome"], "infrastructure-error")
            self.assertIn("oracle changed", r["detail"])
        finally:
            oracle.write_bytes(saved)
        self.assertEqual(sha(oracle), t["oracle_sha256"])

    def test_an_oracle_planted_in_the_work_directory_is_refused(self):
        w = self.work("kernel-taps")
        (w / ORACLE_NAME).write_text("main :: () {}\n")
        self.assertEqual(check(w)["outcome"], "infrastructure-error")

    def test_a_weakened_oracle_set_is_a_counterexample(self):
        # an edit that defines a #test of its own changes the count: not the oracle's pass
        w = self.work("kernel-taps")
        shutil.copy2(CORPUS / "tasks" / "kernel-taps" / "solution" / "kernel.coax", w / "kernel.coax")
        with (w / "kernel.coax").open("a") as f:
            f.write('#test "mine" { #assert true; }\n')
        self.assertEqual(check(w)["outcome"], "counterexample")

    def test_unintended_edits_and_iterations_are_recorded(self):
        global RUNS
        w = self.work("protocol-adapter")
        check(w)
        shutil.copy2(CORPUS / "tasks" / "protocol-adapter" / "solution" / "frame.coax", w / "frame.coax")
        (w / "task.md").write_text("changed\n")
        (w / "notes.txt").write_text("extra\n")
        saved, RUNS = RUNS, self.dir / "runs.jsonl"
        try:
            e = record(w, "test", "none")
        finally:
            RUNS = saved
        self.assertEqual(e["outcome"], "passed")
        self.assertEqual(e["repair_iterations"], 0)
        self.assertEqual(e["unintended_edits"], ["notes.txt", "task.md"])
        self.assertEqual(json.loads((self.dir / "runs.jsonl").read_text())["task"], "protocol-adapter")


def main() -> None:
    a = sys.argv[1:]
    if not a:
        unittest.main(argv=[sys.argv[0]])
        return
    cmd = a[0]
    if cmd == "list":
        for t in manifest()["tasks"]:
            print(f"{t['id']:18} {t['title']}")
    elif cmd == "prepare" and len(a) == 3:
        prepare(a[1], Path(a[2]))
        print(f"prepared {a[1]} in {a[2]}: read task.md there")
    elif cmd == "check" and len(a) == 2:
        print(json.dumps(check(Path(a[1])), indent=2))
    elif cmd == "record" and len(a) >= 4 and a[2] == "--agent":
        model = a[a.index("--model") + 1] if "--model" in a else ""
        print(json.dumps(record(Path(a[1]), a[3], model), indent=2))
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
