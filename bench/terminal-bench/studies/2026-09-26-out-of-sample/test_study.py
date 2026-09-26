"""Fixture tests for confirm.py and report.py.

    python3 -m unittest -v test_study     # from this directory
    python3 -m pytest test_study.py

Each test builds a small study directory (outcome lines, logs, and
summary.json files shaped like the real ones) and a fake microcoder binary,
so nothing real runs.
"""

from __future__ import annotations

import json
import os
import stat
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import report  # noqa: E402
import study  # noqa: E402

SCREEN2 = textwrap.dedent("""\
    #!/usr/bin/env bash
    set -u
    queue=$1; round=$2; par=$3; shift 3
    export EXTRA="$*"
    export OPENAGENTS_KNOWLEDGE=$HOME/.openagents/knowledge/empty-local
    export bin=$HOME/.local/bin/microcoder-study-$round out=$HOME/study-oos/$round; mkdir -p "$out"
    run_one() {
      "$bin" "$task" --kb "$kb" --max-steps 60 --max-minutes "$mins" --max-usd 1.00 $EXTRA > "$log" 2>&1
    }
    """)

# A fake microcoder: passes with knowledge on, fails with it off, writes a
# record and prints the outcome line and the record path like the real one.
FAKE = textwrap.dedent("""\
    #!/usr/bin/env bash
    task=$1; kb=$3; mins=$7
    ms=$(python3 -c 'import time;print(int(time.time()*1000))')
    echo "start $ms $task $kb" >> "$FAKE_TRACE"
    sleep 0.4
    rec="$FAKE_RUNS/$task-$ms"; mkdir -p "$rec"
    if [ "$kb" = candidates ]; then reward=1; else reward=0; fi
    cat > "$rec/summary.json" <<JSON
    {"task": "$task", "provider": "openrouter", "model": "openai/gpt-6-luna", "kb": "$kb",
     "cost_basis": "billed", "reward": $reward, "reward_unknown_because": null,
     "outcome": {"ending": {"reason": "tests_held"}, "steps": 20, "seconds": 120.0, "usd": 0.05,
                 "known_usd": 0.05, "cost_unknown": [], "knowledge": []},
     "verifier_output": "SECRET-VERIFIER", "test_results": ["SECRET-TEST"]}
    JSON
    echo "model reasoning that must never be read: SECRET-REASONING"
    echo "$task · reward $reward · 20 steps · 02:00 · \\$0.0500 (model \\$0.04 billed, Jev \\$0.01, embeddings \\$0.0) · ended by TestsHeld · knowledge-assisted (0 entries)"
    echo "Record: $rec"
    end=$(python3 -c 'import time;print(int(time.time()*1000))')
    echo "end $end $task $kb $* " >> "$FAKE_TRACE"
    """)


def summary(task, reward, steps, seconds, usd, ending="tests_held", unknown=None, basis="billed", detail=None):
    e = {"reason": ending}
    if detail:
        e["detail"] = detail
    return {"task": task, "provider": "openrouter", "model": "openai/gpt-6-luna", "kb": "candidates",
            "cost_basis": basis, "reward": reward, "reward_unknown_because": unknown,
            "outcome": {"ending": e, "steps": steps, "seconds": seconds, "usd": usd, "known_usd": usd,
                        "cost_unknown": [], "knowledge": [{"id": "k1", "digest": "d", "kept_steps": 1,
                                                           "expanded_steps": 0}]},
            "verifier_output": "SECRET-VERIFIER", "test_results": ["SECRET-TEST"]}


class Fixture:
    """A study directory with a round r9 on disk."""

    def __init__(self, root: Path):
        self.root = root
        self.study = root / "study-oos"
        self.runs = root / "runs"
        self.rdir = self.study / "r9"
        self.rdir.mkdir(parents=True)
        self.runs.mkdir()
        self.ms = 1790446429000
        self.lines = []
        (self.study / "screen2.sh").write_text(SCREEN2)
        (self.study / "r9.meta").write_text("round r9 commit abc provider openrouter started 2026-09-26T13:13:49-05:00\n")
        (self.study / "burned.txt").write_text("BURNED ks-solver-cpp: operator read reasoning 2026-09-26\n")
        (self.study / "notes.txt").write_text("UNGRADEABLE freecad-platform-drawing: verifier image fails\n")
        tasks = ["react-lead-form", "formal-crypto", "ks-solver-cpp", "freecad-platform-drawing",
                 "atrx-vep-crispr", "wdm-design", "retro-console-soc", "cad-model", "ctr-optimization"]
        (self.study / "queue-r9.txt").write_text("".join(f"{t} {120 if t == 'wdm-design' else 60} on\n" for t in tasks))

    def run(self, task, arm, s=None, line=None, record_in_tail=True, rc=1):
        self.ms += 1000
        log = self.rdir / f"{task}.{arm}.{self.ms}.log"
        body = "step 1 · SECRET-REASONING\n"
        if s is not None:
            rec = self.runs / f"{task}-{self.ms + 20}"
            rec.mkdir()
            (rec / "summary.json").write_text(json.dumps(s))
            (rec / "events.jsonl").write_text('{"secret": "SECRET-EVENT"}\n')
            if record_in_tail:
                body += f"Record: {rec}\n"
        log.write_text(body)
        if line is not None:
            self.lines.append(f"2026-09-26T13:{len(self.lines):02d}:00-05:00 {task} {arm} rc={rc} {line}")
            (self.rdir / "outcomes.txt").write_text("\n".join(self.lines) + "\n")


def reward_line(task, reward, steps, clock, usd, ending="TestsHeld", billed=True):
    b = " billed" if billed else ""
    return (f"{task} · reward {reward} · {steps} steps · {clock} · ${usd:.4f} (model ${usd:.4f}{b}, Jev $0.0, "
            f"embeddings $0.0) · ended by {ending} · knowledge-assisted (1 entries)")


def build_fixture(root: Path) -> Fixture:
    f = Fixture(root)
    # Held-out passes: react-lead-form is a cost win but slower than Fable's
    # fastest (378 s); formal-crypto is a cost and time win.
    f.run("react-lead-form", "on", summary("react-lead-form", 1.0, 30, 400.0, 0.11),
          reward_line("react-lead-form", 1, 30, "06:40", 0.11))
    f.run("formal-crypto", "on", summary("formal-crypto", 1.0, 20, 300.0, 0.09),
          reward_line("formal-crypto", 1, 20, "05:00", 0.09), record_in_tail=False)
    # Burned and ungradeable passes are never confirmed.
    f.run("ks-solver-cpp", "on", summary("ks-solver-cpp", 1.0, 13, 132.0, 0.01),
          reward_line("ks-solver-cpp", 1, 13, "02:12", 0.01))
    f.run("freecad-platform-drawing", "on", summary("freecad-platform-drawing", 1.0, 10, 100.0, 0.01),
          reward_line("freecad-platform-drawing", 1, 10, "01:40", 0.01))
    f.run("atrx-vep-crispr", "on", summary("atrx-vep-crispr", 0.0, 60, 1134.0, 0.25, "step_limit"),
          reward_line("atrx-vep-crispr", 0, 60, "18:54", 0.25, "StepLimit"))
    f.run("wdm-design", "on", summary("wdm-design", 0.0, 3, 50.0, 0.0006, "bad_replies",
                                      detail='the provider returned HTTP 429: {"error":{"type":"usage_limit_reached"}}'),
          'wdm-design · reward 0 · 3 steps · 00:50 · $0.0006 (model $0.0000, Jev $0.00058) · ended by BadReplies("the provider returned HTTP 429: {\\"error\\":{\\"type\\":\\"usage_limit_reached\\"}}")')
    f.run("ctr-optimization", "on", None,
          "microcoder: ctr-optimization needs several services (a Compose file), which microcoder doesn't run yet")
    # retro-console-soc: screen pass and a complete confirmation. On: pass
    # $0.20 (win), pass $9.00 (not a cost win), pass $0.30 (win) -> confirmed.
    # Off: one win of three -> knowledge-off does not win.
    retro = [("on", 1.0, 0.20), ("off", 1.0, 0.25), ("on", 1.0, 9.00), ("off", 0.0, 0.10),
             ("on", 1.0, 0.30), ("off", 0.0, 0.10)]
    for arm, reward, usd in retro:
        f.run("retro-console-soc", arm, summary("retro-console-soc", reward, 30, 600.0, usd),
              reward_line("retro-console-soc", int(reward), 30, "10:00", usd))
    # Fable-fails: one pass.
    f.run("cad-model", "on", summary("cad-model", 1.0, 25, 500.0, 0.3),
          reward_line("cad-model", 1, 25, "08:20", 0.3))
    # A run still going: a log, no outcome line.
    f.run("uefi-bootkit", "on", None, None)
    (f.rdir / "confirm-state.json").write_text(json.dumps(
        {"round": "r9", "driver": None, "tasks": {"retro-console-soc": {"runs": []}}}))
    return f


class ReportTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.f = build_fixture(Path(self.tmp.name))
        self.collected = study.collect(str(self.f.study), "r9", str(self.f.runs))
        self.rep = report.build(self.collected)

    def tearDown(self):
        self.tmp.cleanup()

    def test_collection_never_carries_secrets(self):
        blob = json.dumps(self.collected)
        for secret in ("SECRET-VERIFIER", "SECRET-TEST", "SECRET-REASONING", "SECRET-EVENT"):
            self.assertNotIn(secret, blob)

    def test_record_found_by_start_time_when_tail_lacks_it(self):
        log = next(l for l in self.collected["logs"] if l["task"] == "formal-crypto")
        self.assertEqual(log["record_matched_by"], "start time")
        self.assertTrue(log["record"].startswith("formal-crypto-"))

    def run_of(self, task, arm="on"):
        return next(r for r in self.rep["runs"] if r["task"] == task and r["arm"] == arm)

    def test_win_rules(self):
        rl = self.run_of("react-lead-form")
        self.assertEqual((rl["kind"], rl["cost_win"], rl["time_win"]), ("pass", True, False))
        fc = self.run_of("formal-crypto")
        self.assertEqual((fc["cost_win"], fc["time_win"]), (True, True))
        v = {x["task"]: x for x in self.rep["held_out_verdicts"]}
        self.assertEqual(v["retro-console-soc"]["verdict"], "Confirmed out-of-sample win")
        self.assertEqual(v["retro-console-soc"]["on_cost_wins"], 2)
        self.assertEqual(v["retro-console-soc"]["knowledge_off"], "knowledge-off does not win")
        self.assertEqual(v["react-lead-form"]["verdict"], "Awaiting confirmation")
        self.assertEqual(v["atrx-vep-crispr"]["verdict"], "Screen failed")
        self.assertEqual(v["ks-solver-cpp"]["verdict"], "In-sample (burned): not counted")

    def test_faults_are_separate(self):
        self.assertEqual(self.run_of("wdm-design")["kind"], "provider_fault")
        self.assertEqual(self.run_of("wdm-design")["detail"], "HTTP 429 usage_limit_reached")
        self.assertEqual(self.run_of("ctr-optimization")["kind"], "not_supported")
        self.assertEqual(self.run_of("uefi-bootkit")["kind"], "running")
        md = report.markdown(self.rep)
        graded = md.split("#### Held-out tasks: graded runs")[1].split("####")[0]
        self.assertNotIn("wdm-design", graded)
        self.assertIn("`wdm-design` | held-out | on | Provider fault", md)

    def test_fable_fails_pool(self):
        ff = {x["task"]: x for x in self.rep["fable_fails"]}
        self.assertEqual(ff["cad-model"]["verdict"], "Pass: needs a second pass to confirm")
        self.assertFalse(self.run_of("cad-model")["cost_win"])

    def test_labels(self):
        self.assertIn("in-sample (burned)", self.run_of("ks-solver-cpp")["labels"])
        self.assertIn("studied by earlier harnesses", self.run_of("react-lead-form")["labels"])
        self.assertIn("**(in-sample: burned)**", report.markdown(self.rep))

    def test_pools_and_reference(self):
        held, fails = study.pools()
        self.assertEqual((len(held), len(fails)), (26, 23))
        ref = study.fable_reference()
        self.assertAlmostEqual(ref["react-lead-form"]["cheapest"]["cost_usd"], 2.4558, places=3)
        self.assertAlmostEqual(ref["react-lead-form"]["fastest"]["seconds"], 377.7, places=0)

    def test_outcome_line_parsing(self):
        o = study.parse_outcome("2026-09-26T09:41:22-05:00 freecad-platform-drawing on exit=0 "
                                "freecad-platform-drawing · reward unknown · 40 steps · 09:47 · $0.0896 "
                                "(model $0.0787, Jev $0.01077) · ended by the model finishing · "
                                "knowledge-assisted (3 entries) Fable 5.1 low: 1 of 5 passed")
        self.assertIsNone(o["reward"])
        self.assertEqual((o["steps"], o["seconds"], o["usd"], o["ending"], o["billed"]),
                         (40, 587.0, 0.0896, "the model finishing", False))
        o = study.parse_outcome("2026-09-26T13:25:23-05:00 x on rc=1 x · reward unknown (the verifier's "
                                "environment didn't start) · 52 steps · 1:10:42 · $0.2029 (model $0.1871 billed)")
        self.assertEqual((o["reward_unknown_because"], o["seconds"], o["billed"]),
                         ("the verifier's environment didn't start", 4242.0, True))


class ConfirmTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        self.f = build_fixture(root)
        self.fake = root / "microcoder-study-r9"
        self.fake.write_text(FAKE)
        self.fake.chmod(self.fake.stat().st_mode | stat.S_IEXEC)
        self.trace = root / "trace.txt"
        self.env = dict(os.environ, FAKE_RUNS=str(self.f.runs), FAKE_TRACE=str(self.trace))

    def tearDown(self):
        self.tmp.cleanup()

    def confirm(self, *extra):
        cmd = [sys.executable, str(HERE / "confirm.py"), "--round", "r9", "--study-dir", str(self.f.study),
               "--runs-dir", str(self.f.runs), "--bin", str(self.fake),
               "--driver-args", "--provider openrouter --model openai/gpt-6-luna", *extra]
        return subprocess.run(cmd, capture_output=True, text=True, env=self.env, timeout=120)

    def test_dry_run_plans_only_eligible_tasks_and_changes_nothing(self):
        before = (self.f.rdir / "outcomes.txt").read_text()
        out = self.confirm("--dry-run")
        self.assertEqual(out.returncode, 0, out.stderr)
        self.assertIn("would queue react-lead-form", out.stdout)
        self.assertIn("would queue formal-crypto", out.stdout)
        self.assertIn("skip ks-solver-cpp: burned", out.stdout)
        self.assertIn("skip freecad-platform-drawing: ungradeable", out.stdout)
        self.assertNotIn("would queue retro-console-soc", out.stdout)  # already queued once
        self.assertNotIn("would queue cad-model", out.stdout)  # Fable-fails pool
        self.assertIn("--kb off --max-steps 60 --max-minutes 60 --max-usd 1.00 --provider openrouter "
                      "--model openai/gpt-6-luna", out.stdout)
        plan = [l.split()[3] for l in out.stdout.splitlines() if "react-lead-form --kb" in l]
        self.assertEqual(plan, ["off", "candidates", "off", "candidates", "off"])
        self.assertEqual(before, (self.f.rdir / "outcomes.txt").read_text())
        self.assertFalse(self.trace.exists())

    def test_runs_exactly_once_at_most_three_at_a_time(self):
        out = self.confirm("--once")
        self.assertEqual(out.returncode, 0, out.stderr)
        lines = (self.f.rdir / "outcomes.txt").read_text().splitlines()
        new = [l for l in lines if "log=" in l]
        self.assertEqual(len(new), 10)
        for task in ("react-lead-form", "formal-crypto"):
            arms = sorted(l.split()[2] for l in new if l.split()[1] == task)
            self.assertEqual(arms, ["off", "off", "off", "on", "on"])
        for l in new:
            self.assertRegex(l, r"^\S+ \S+ (on|off) rc=0 \S+ · reward [01] · .* log=\S+\.log$")
            self.assertNotIn("SECRET", l)
        # Concurrency never went above three.
        events = []
        for l in self.trace.read_text().splitlines():
            kind, ms = l.split()[:2]
            events.append((int(ms), 1 if kind == "start" else -1))
        live = peak = 0
        for _, d in sorted(events, key=lambda e: (e[0], e[1])):
            live += d
            peak = max(peak, live)
        self.assertLessEqual(peak, 3)
        self.assertGreaterEqual(peak, 2)
        # The same flags the screen used.
        self.assertIn("--max-steps 60 --max-minutes 60 --max-usd 1.00 --provider openrouter", self.trace.read_text())
        # A second pass queues nothing more.
        again = self.confirm("--once")
        self.assertEqual(again.returncode, 0, again.stderr)
        self.assertEqual(len([l for l in (self.f.rdir / "outcomes.txt").read_text().splitlines() if "log=" in l]), 10)
        # The report reads the new runs: knowledge-on passes are cost wins.
        rep = report.build(study.collect(str(self.f.study), "r9", str(self.f.runs)))
        v = {x["task"]: x for x in rep["held_out_verdicts"]}
        self.assertEqual(v["react-lead-form"]["verdict"], "Confirmed out-of-sample win")
        self.assertEqual((v["react-lead-form"]["on_results"], v["react-lead-form"]["off_results"]), (3, 3))
        self.assertEqual(v["react-lead-form"]["knowledge_off"], "knowledge-off does not win")
        state = json.loads((self.f.rdir / "confirm-state.json").read_text())
        self.assertEqual([r["status"] for r in state["tasks"]["formal-crypto"]["runs"]], ["done"] * 5)

    def test_meta_provider_must_match_extras(self):
        cmd = [sys.executable, str(HERE / "confirm.py"), "--round", "r9", "--study-dir", str(self.f.study),
               "--runs-dir", str(self.f.runs), "--bin", str(self.fake), "--driver-args", "--provider codex",
               "--dry-run"]
        out = subprocess.run(cmd, capture_output=True, text=True, env=self.env)
        self.assertNotEqual(out.returncode, 0)
        self.assertIn("disagree", out.stderr)


if __name__ == "__main__":
    unittest.main()
