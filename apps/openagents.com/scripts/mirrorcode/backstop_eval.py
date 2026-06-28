#!/usr/bin/env python3
"""MirrorCode gym backstop runner (issue #6710) — the prio:4 density burner.

The fleet-saturation backstop task (`prio:4-backstop-burn`) needs a REAL runner
that does genuine own-capacity ($0) high-density work whenever the higher tiers
are clear, so no slot ever idles. This module is that runner's core.

What it does:
  * Resolves a bounded batch of small coding problems. If the read-only
    MirrorCode clone is present (default ~/work/projects/repos/MirrorCode) its
    public S-bucket task ids are surfaced for visibility, but the genuine $0
    high-density path here is the built-in public-domain FIXTURE problem set
    (classic toy functions with hidden test cases). The full Docker MirrorCode
    harness burns >=1B tokens per sample and is NOT a backstop-appropriate
    density burner — see the follow-up note in README.md.
  * For each problem it does GENUINE model work: it asks a model (live: Khala via
    the OpenAI-compatible endpoint, own capacity, $0) for a solution, extracts
    the code, EXECUTES it against the problem's hidden test cases in a bounded
    subprocess, and records pass/fail.
  * It computes per-problem and aggregate pass rates and writes execution traces
    (one JSON per problem plus a run summary) under the results dir.

Determinism + testability: all model I/O goes through an injected `model_fn`
(prompt -> raw text). Tests pass a stub model_fn so the extraction, execution,
pass-rate, and trace-writing logic are verified with no network and no Khala
spend. The live CLI path (`--live`) uses the Khala model_fn.

Honesty: this is a real measurement of our own model against public-domain toy
problems; it is NOT the MirrorCode paper benchmark and must never be published
as a MirrorCode score. Results are tagged `demand_kind=internal`,
`demand_source=gym_backstop`.

Python 3.9 compatible (system python3); no third-party deps.
"""

from __future__ import annotations

import argparse
import datetime as _dt
import json
import os
import re
import subprocess
import sys
import tempfile
import uuid
from typing import Any, Callable, Dict, List, Optional, Tuple

DEMAND = {"kind": "internal", "source": "gym_backstop", "client": "mirrorcode-backstop"}

# --- Built-in public-domain fixture problem set --------------------------------
# Small, classic, public-domain coding problems (NOT MirrorCode tasks, so no
# benchmark contamination). Each problem asks for one function; `tests` are
# (args, expected) pairs executed against the model's solution.
FIXTURE_PROBLEMS: List[Dict[str, Any]] = [
    {
        "id": "sum_list",
        "entrypoint": "sum_list",
        "prompt": "Write a Python function `sum_list(xs)` that returns the sum of a list of numbers. Return only the function in a single ```python code block.",
        "tests": [[[[1, 2, 3]], 6], [[[]], 0], [[[-1, 1, 10]], 10]],
    },
    {
        "id": "reverse_string",
        "entrypoint": "reverse_string",
        "prompt": "Write a Python function `reverse_string(s)` that returns the reverse of string s. Return only the function in a single ```python code block.",
        "tests": [[["abc"], "cba"], [[""], ""], [["racecar"], "racecar"]],
    },
    {
        "id": "is_palindrome",
        "entrypoint": "is_palindrome",
        "prompt": "Write a Python function `is_palindrome(s)` that returns True if string s reads the same forwards and backwards, else False. Return only the function in a single ```python code block.",
        "tests": [[["abba"], True], [["abc"], False], [[""], True]],
    },
    {
        "id": "factorial",
        "entrypoint": "factorial",
        "prompt": "Write a Python function `factorial(n)` returning n! for a non-negative integer n (factorial(0) == 1). Return only the function in a single ```python code block.",
        "tests": [[[0], 1], [[1], 1], [[5], 120]],
    },
    {
        "id": "count_vowels",
        "entrypoint": "count_vowels",
        "prompt": "Write a Python function `count_vowels(s)` that returns the number of vowels (a,e,i,o,u, case-insensitive) in string s. Return only the function in a single ```python code block.",
        "tests": [[["hello"], 2], [["XYZ"], 0], [["AEIOU"], 5]],
    },
    {
        "id": "max_of_list",
        "entrypoint": "max_of_list",
        "prompt": "Write a Python function `max_of_list(xs)` that returns the largest value in a non-empty list xs. Return only the function in a single ```python code block.",
        "tests": [[[[3, 1, 2]], 3], [[[-5, -2, -9]], -2], [[[42]], 42]],
    },
    {
        "id": "fizzbuzz_value",
        "entrypoint": "fizzbuzz_value",
        "prompt": "Write a Python function `fizzbuzz_value(n)` returning 'FizzBuzz' if n divisible by 15, 'Fizz' if by 3, 'Buzz' if by 5, else str(n). Return only the function in a single ```python code block.",
        "tests": [[[15], "FizzBuzz"], [[9], "Fizz"], [[10], "Buzz"], [[7], "7"]],
    },
    {
        "id": "gcd",
        "entrypoint": "gcd",
        "prompt": "Write a Python function `gcd(a, b)` returning the greatest common divisor of positive integers a and b. Return only the function in a single ```python code block.",
        "tests": [[[12, 8], 4], [[17, 5], 1], [[100, 75], 25]],
    },
]

# MirrorCode public S-bucket targets (mirrored from run_smoke.py) — surfaced for
# visibility when the clone is present; the heavy Docker harness is not invoked
# here. See README.md follow-up.
MIRRORCODE_S_TARGETS = [
    "qsv_select", "jq_simple", "gron", "bitwise", "hexyl",
    "uuidparse", "numfmt", "cal", "choose",
]


def _now_iso() -> str:
    return _dt.datetime.now(_dt.timezone.utc).isoformat()


def mirrorcode_clone_present(clone: Optional[str] = None) -> bool:
    """Whether the read-only MirrorCode clone is available locally."""
    clone = clone or os.environ.get(
        "MC_CLONE", os.path.expanduser("~/work/projects/repos/MirrorCode")
    )
    return os.path.isfile(os.path.join(clone, "mc", "task.py"))


def select_problems(limit: int) -> List[Dict[str, Any]]:
    """Return a bounded batch of problems from the fixture set."""
    if limit <= 0:
        return list(FIXTURE_PROBLEMS)
    return list(FIXTURE_PROBLEMS[:limit])


def extract_code(model_output: str) -> str:
    """Extract a python code block from a model response.

    Prefers a fenced ```python (or bare ```) block; falls back to the raw text
    so a model that returns a bare function definition still runs.
    """
    if not isinstance(model_output, str):
        return ""
    fence = re.search(r"```(?:python|py)?\s*\n(.*?)```", model_output, re.DOTALL | re.IGNORECASE)
    if fence:
        return fence.group(1).strip()
    return model_output.strip()


def run_solution(
    code: str, problem: Dict[str, Any], timeout: float = 8.0
) -> Tuple[int, int, Dict[str, Any]]:
    """Execute `code` against a problem's test cases in a bounded subprocess.

    Returns (passed, total, details). `details` carries a public-safe per-case
    result list and any harness error. The candidate code is model-generated, so
    it runs in an isolated `python -I` subprocess with a wall-clock timeout.
    """
    entry = problem["entrypoint"]
    cases = problem["tests"]
    total = len(cases)
    if not code.strip():
        return 0, total, {"error": "empty_solution", "results": [False] * total}

    harness = (
        "import json, sys\n"
        "__oa_results = []\n"
        "try:\n"
        "    __oa_ns = {}\n"
        "    exec(compile(__OA_CODE__, '<candidate>', 'exec'), __oa_ns, __oa_ns)\n"
        "    __oa_fn = __oa_ns.get(__OA_ENTRY__)\n"
        "    if not callable(__oa_fn):\n"
        "        print(json.dumps({'error': 'entrypoint_missing', 'results': []}))\n"
        "        sys.exit(0)\n"
        "    for __oa_args, __oa_expected in __OA_CASES__:\n"
        "        try:\n"
        "            __oa_got = __oa_fn(*__oa_args)\n"
        "            __oa_results.append(__oa_got == __oa_expected)\n"
        "        except Exception:\n"
        "            __oa_results.append(False)\n"
        "    print(json.dumps({'results': __oa_results}))\n"
        "except Exception as __oa_e:\n"
        "    print(json.dumps({'error': 'harness_exec_failed:' + type(__oa_e).__name__, 'results': []}))\n"
    )
    # Embed cases as a PYTHON literal (repr), not JSON: JSON would render
    # booleans/null as true/false/null, which are not valid Python identifiers
    # and would raise NameError when the harness source is compiled.
    harness = (
        harness.replace("__OA_CODE__", repr(code))
        .replace("__OA_ENTRY__", repr(entry))
        .replace("__OA_CASES__", repr(cases))
    )

    with tempfile.NamedTemporaryFile("w", suffix=".py", delete=False) as tf:
        tf.write(harness)
        path = tf.name
    try:
        proc = subprocess.run(
            [sys.executable, "-I", path],
            capture_output=True,
            text=True,
            timeout=timeout,
        )
    except subprocess.TimeoutExpired:
        return 0, total, {"error": "timeout", "results": [False] * total}
    finally:
        try:
            os.unlink(path)
        except OSError:
            pass

    out = (proc.stdout or "").strip().splitlines()
    parsed: Dict[str, Any] = {}
    for line in reversed(out):
        line = line.strip()
        if line.startswith("{"):
            try:
                parsed = json.loads(line)
                break
            except Exception:
                continue
    if not parsed:
        return 0, total, {"error": "no_harness_output", "results": [False] * total}
    results = parsed.get("results") or []
    passed = sum(1 for r in results if r is True)
    details: Dict[str, Any] = {"results": results}
    if parsed.get("error"):
        details["error"] = parsed["error"]
    return passed, total, details


def evaluate_batch(
    problems: List[Dict[str, Any]],
    model_fn: Callable[[str], str],
    out_dir: str,
    run_id: Optional[str] = None,
    case_timeout: float = 8.0,
    model_name: str = "openagents/khala",
) -> Dict[str, Any]:
    """Run the model over a batch, execute solutions, and write traces.

    Returns the run summary dict (also written to `<out_dir>/<run_id>.json`).
    Per-problem execution traces are written under `<out_dir>/traces/`.
    """
    run_id = run_id or ("mc-backstop-" + _dt.datetime.now(_dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ") + "-" + uuid.uuid4().hex[:8])
    traces_dir = os.path.join(out_dir, "traces")
    os.makedirs(traces_dir, exist_ok=True)

    started = _now_iso()
    per_problem: List[Dict[str, Any]] = []
    total_passed = 0
    total_cases = 0
    for prob in problems:
        try:
            raw = model_fn(prob["prompt"])
        except Exception as exc:  # model/network failure is fail-soft per problem
            raw = ""
            model_err = "model_call_failed:" + type(exc).__name__
        else:
            model_err = None
        code = extract_code(raw)
        passed, total, details = run_solution(code, prob, timeout=case_timeout)
        total_passed += passed
        total_cases += total
        pass_rate = (passed / total) if total else 0.0
        trace = {
            "runId": run_id,
            "problemId": prob["id"],
            "entrypoint": prob["entrypoint"],
            "model": model_name,
            "passed": passed,
            "total": total,
            "passRate": pass_rate,
            "status": "passed" if (total and passed == total) else "failed",
            "caseResults": details.get("results", []),
            "harnessError": details.get("error"),
            "modelError": model_err,
            "solutionChars": len(code),
            "solution": code,  # public-safe: fixtures are public-domain toy problems
            "demand": DEMAND,
            "recordedAt": _now_iso(),
        }
        per_problem.append(trace)
        with open(os.path.join(traces_dir, prob["id"] + ".json"), "w") as fh:
            json.dump(trace, fh, indent=2)

    finished = _now_iso()
    agg_pass_rate = (total_passed / total_cases) if total_cases else 0.0
    fully_solved = sum(1 for p in per_problem if p["status"] == "passed")
    summary = {
        "runId": run_id,
        "model": model_name,
        "benchmark": "OpenAgents gym backstop (public-domain fixture problems; NOT MirrorCode)",
        "problemCount": len(per_problem),
        "fullySolved": fully_solved,
        "casesPassed": total_passed,
        "casesTotal": total_cases,
        "passRate": agg_pass_rate,
        "status": "passed" if (per_problem and fully_solved == len(per_problem)) else "failed",
        "startedAt": started,
        "finishedAt": finished,
        "mirrorcodeClonePresent": mirrorcode_clone_present(),
        "mirrorcodeSTargets": MIRRORCODE_S_TARGETS if mirrorcode_clone_present() else [],
        "demand": DEMAND,
        "grade": "backstop",
        "decisionGrade": False,
        "perProblem": [
            {"problemId": p["problemId"], "passed": p["passed"], "total": p["total"], "passRate": p["passRate"], "status": p["status"]}
            for p in per_problem
        ],
    }
    with open(os.path.join(out_dir, run_id + ".json"), "w") as fh:
        json.dump(summary, fh, indent=2)
    return summary


# --- Live Khala model function -------------------------------------------------
def khala_model_fn(
    base_url: Optional[str] = None,
    api_key: Optional[str] = None,
    model: str = "openagents/khala",
    timeout: float = 120.0,
) -> Callable[[str], str]:
    """Build a model_fn that calls Khala's OpenAI-compatible chat endpoint.

    Own-capacity ($0) path. Uses only stdlib urllib so the backstop has no
    third-party dependency. Tags the request with the gym-backstop demand
    attribution headers so the load is auditably an internal eval.
    """
    import urllib.request

    base_url = (base_url or os.environ.get("OPENAI_BASE_URL") or "https://openagents.com/api/v1").rstrip("/")
    api_key = api_key or os.environ.get("OPENAI_API_KEY") or ""
    url = base_url + "/chat/completions"

    def _call(prompt: str) -> str:
        payload = json.dumps(
            {
                "model": model,
                "messages": [
                    {"role": "system", "content": "You are a precise Python coding assistant. Respond with only the requested function in a single python code block."},
                    {"role": "user", "content": prompt},
                ],
                "temperature": 0,
            }
        ).encode("utf-8")
        req = urllib.request.Request(url, data=payload, method="POST")
        req.add_header("Content-Type", "application/json")
        if api_key:
            req.add_header("Authorization", "Bearer " + api_key)
        req.add_header("x-openagents-demand-kind", DEMAND["kind"])
        req.add_header("x-openagents-demand-source", DEMAND["source"])
        req.add_header("x-openagents-client", DEMAND["client"])
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            body = json.loads(resp.read().decode("utf-8"))
        return body["choices"][0]["message"]["content"] or ""

    return _call


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(description="MirrorCode gym backstop runner (#6710)")
    parser.add_argument("--limit", type=int, default=int(os.environ.get("MC_BACKSTOP_LIMIT", "8")), help="Max problems in the bounded batch (0 = all).")
    parser.add_argument("--out", default=os.environ.get("MC_BACKSTOP_OUT", os.path.join(os.path.dirname(os.path.abspath(__file__)), "results", "backstop")), help="Results output dir.")
    parser.add_argument("--live", action="store_true", help="Use the live Khala model (needs OPENAI_API_KEY / OPENAI_BASE_URL).")
    parser.add_argument("--case-timeout", type=float, default=float(os.environ.get("MC_BACKSTOP_CASE_TIMEOUT", "8")), help="Per-problem execution timeout (s).")
    parser.add_argument("--model", default=os.environ.get("MC_BACKSTOP_MODEL", "openagents/khala"))
    args = parser.parse_args(argv)

    problems = select_problems(args.limit)
    if args.live:
        model_fn = khala_model_fn(model=args.model)
    else:
        # Diagnostic dry-run: a trivial stub that returns no code, so traces are
        # written and the pipeline is exercised with zero spend and no network.
        def model_fn(_prompt: str) -> str:
            return ""

    os.makedirs(args.out, exist_ok=True)
    summary = evaluate_batch(problems, model_fn, args.out, case_timeout=args.case_timeout, model_name=args.model)
    print(json.dumps(summary, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
