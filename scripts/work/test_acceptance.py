"""Tests for the #11313 acceptance verdict and report mode."""
import importlib.util
import json
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("acceptance", HERE / "acceptance.py")
acc = importlib.util.module_from_spec(spec)
spec.loader.exec_module(acc)


def r(ok, cost, engine="briefed"):
    return {"ok": ok, "cost_usd": cost, "engine": engine, "secs": 60}


def test_verdicts():
    assert acc.verdict(r(True, 0.2), r(True, 0.5, "bare"))[0]
    assert not acc.verdict(r(True, 0.6), r(True, 0.5, "bare"))[0]
    assert acc.verdict(r(True, 0.6), r(False, None, "bare"))[0]
    assert not acc.verdict(r(False, 0.1), r(True, 0.5, "bare"))[0]
    assert not acc.verdict(r(True, None), r(True, 0.5, "bare"))[0]
    # a briefed run that fell back to bare is not a briefed win
    assert not acc.verdict(r(True, 0.1, "bare"), r(False, None, "bare"))[0]


def test_report_mode():
    with tempfile.TemporaryDirectory() as d:
        root = Path(d)
        for issue, (b, a) in {1: (r(True, 0.2), r(True, 0.4, "bare")),
                              2: (r(True, 0.3), r(False, None, "bare"))}.items():
            for arm, res in (("briefed", b), ("bare", a)):
                (root / str(issue) / arm).mkdir(parents=True)
                (root / str(issue) / arm / "result.json").write_text(json.dumps(res))
        p = subprocess.run([sys.executable, str(HERE / "acceptance.py"), "--report", d, "--issues", "1", "2"],
                           capture_output=True, text=True)
        assert p.returncode == 0, p.stdout + p.stderr
        assert json.loads((root / "acceptance.json").read_text())["all_won"]
        p = subprocess.run([sys.executable, str(HERE / "acceptance.py"), "--report", d, "--issues", "1", "2", "3"],
                           capture_output=True, text=True)
        assert p.returncode == 1


if __name__ == "__main__":
    test_verdicts()
    test_report_mode()
    print("ok")
