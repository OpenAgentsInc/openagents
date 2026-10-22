#!/usr/bin/env python3
"""Check the frozen G2 v2 task suite (#12405) on the oracle's and the served
runtime's complete answers.

    g2_check.py SUITE ANSWERS OUT

SUITE is `g2_v2/task-suite.json`; ANSWERS is `answers.json` from
`psionic-v41-stage qualify --answers`, which carries both answers per case.
A task passes when its answer ends with the end token within the cap, a
thinking-mode answer has reasoning, and every assertion holds on the final
text. Generated code runs in a scratch folder with a 60 s limit, under a
parent-written harness that saves returned values after every call finishes.
The parent compares those values; program output never supplies a verdict.
An early exit, a silent run, or a partial run fails. The verdict needs both
the floor (completion, parsing, and every must-pass assertion) and non-inferiority (the runtime's
pass count is at least the oracle's on this finite suite).
"""
import ast
import hashlib
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

LIMIT = 60


def code_block(text, langs):
    for lang in langs:
        m = re.search(r"```" + lang + r"[^\n]*\n(.*?)```", text, re.S)
        if m:
            return m.group(1)
    m = re.search(r"```[^\n]*\n(.*?)```", text, re.S)
    return m.group(1) if m else None


def returned_values(path, expected, *, rust=False):
    """Read completed calls, independently of stdout and the child exit code.

    These small generated programs are benchmark inputs, not a security
    sandbox. They run only after inspection or in a disposable sandbox.
    Expected values stay in this parent process, outside the answer program.
    """
    if not path.is_file():
        return False, "no returned values (early exit or incomplete run)"
    try:
        actual = path.read_text().splitlines() if rust else json.loads(path.read_text())
    except (OSError, ValueError):
        return False, "invalid returned values"
    if not isinstance(actual, list) or len(actual) != len(expected):
        return False, "incomplete returned values"
    bad = [i for i, (got, want) in enumerate(zip(actual, expected))
           if type(got) is not type(want) or got != want]
    return (False, f"cases {bad} failed") if bad else (True, "all cases hold")


def rust_expected(text):
    # The frozen Rust cases return integers or Option<integer>. Refuse a
    # new expression shape rather than guessing its Debug representation.
    value = re.sub(r"\s+", "", text)
    if not re.fullmatch(r"(?:-?\d+|Some\(-?\d+\)|None|true|false)", value):
        raise ValueError(f"unsupported Rust expected value: {text}")
    return value


def rust_tests(text, a):
    code = code_block(text, ["rust", "rs"])
    if code is None:
        return False, "no Rust code block"
    if re.search(r"\bfn\s+main\s*\(", code):
        # The parent writes main; an answer's own main is renamed, not run.
        code = re.sub(r"\bfn\s+main\s*\(", "fn answer_main(", code)
    expected = [rust_expected(want) for _, want in a["cases"]]
    calls = ",\n".join(f'format!("{{:?}}", {a["function"]}({args}))' for args, _ in a["cases"])
    src = ("#![allow(dead_code, unused)]\n" + code + "\nfn main() {\n"
           + "let returned: Vec<String> = vec![" + calls + "];\n"
           + 'std::fs::write("actual-values", returned.join("\\n")).unwrap();\n}\n')
    with tempfile.TemporaryDirectory() as d:
        p = Path(d) / "t.rs"
        p.write_text(src)
        c = subprocess.run(["rustc", "--edition", "2021", "-O", "-o", str(Path(d) / "t"), str(p)],
                           capture_output=True, text=True, timeout=LIMIT)
        if c.returncode:
            return False, "does not compile: " + c.stderr[-300:]
        try:
            r = subprocess.run([str(Path(d) / "t")], capture_output=True, text=True, timeout=LIMIT, cwd=d)
        except subprocess.TimeoutExpired:
            return False, "timed out"
        ok, why = returned_values(Path(d) / "actual-values", expected, rust=True)
    if ok and r.returncode != 0:
        return False, f"exit status {r.returncode}"
    return ok, why


def python_tests(text, a):
    code = code_block(text, ["python", "py"])
    if code is None:
        return False, "no Python code block"
    expected = [ast.literal_eval(want) for _, want in a["cases"]]
    calls = ", ".join(f'{a["function"]}({args})' for args, _ in a["cases"])
    checks = ("returned = [" + calls + "]\n"
              + 'import json\nwith open("actual-values", "w") as result:\n'
              + "    json.dump(returned, result)\n")
    with tempfile.TemporaryDirectory() as d:
        p = Path(d) / "t.py"
        p.write_text(code + "\n" + checks + "\n")
        try:
            r = subprocess.run([sys.executable, "-I", str(p)], capture_output=True, text=True, timeout=LIMIT, cwd=d)
        except subprocess.TimeoutExpired:
            return False, "timed out"
        ok, why = returned_values(Path(d) / "actual-values", expected)
    if ok and r.returncode != 0:
        return False, f"exit status {r.returncode}"
    return ok, why


def strip_fence(text):
    t = text.strip()
    m = re.fullmatch(r"```(?:json)?\s*\n(.*?)\n?```", t, re.S)
    return m.group(1).strip() if m else t


def check(text, a):
    kind = a["kind"]
    if kind == "sentences_at_most":
        n = len([s for s in re.split(r"(?<=[.!?])\s+", text.strip()) if s.strip()])
        return 0 < n <= a["value"], f"{n} sentences"
    if kind == "contains_any":
        hit = [v for v in a["values"] if v.lower() in text.lower()]
        return bool(hit), f"found {hit}"
    if kind == "contains_all":
        low = text.lower()
        need = [v for v in a["values"] if v.lower() not in low]
        any_hit = [v for v in a.get("any_of", []) if v.lower() in low]
        ok = not need and (not a.get("any_of") or bool(any_hit))
        return ok, f"missing {need}, found {any_hit}"
    if kind == "last_line_number":
        lines = [l.strip() for l in text.strip().splitlines() if l.strip()]
        nums = re.findall(r"-?\d+(?:\.\d+)?", lines[-1]) if lines else []
        ok = bool(nums) and float(nums[-1]) == float(a["value"])
        return ok, f"last line {lines[-1]!r}" if lines else "empty"
    if kind == "last_line_exact":
        lines = [l.strip().strip("*").strip() for l in text.strip().splitlines() if l.strip()]
        if not lines:
            return False, "empty"
        return bool(re.fullmatch(a["pattern"], lines[-1])), f"last line {lines[-1]!r}"
    if kind in ("json_fields", "json_only"):
        if kind == "json_only":
            try:
                obj = json.loads(strip_fence(text))
            except json.JSONDecodeError:
                return False, "the final text is not only a JSON object"
        else:
            m = re.search(r"\{.*\}", text, re.S)
            try:
                obj = json.loads(m.group(0)) if m else None
            except json.JSONDecodeError:
                obj = None
        if not isinstance(obj, dict):
            return False, "no JSON object"
        for k, v in a["fields"].items():
            got = obj.get(k)
            if isinstance(v, str) and (not isinstance(got, str) or v.lower() not in got.lower()):
                return False, f"{k}={got!r}"
            if not isinstance(v, str) and got != v:
                return False, f"{k}={got!r}"
        return True, "fields hold"
    if kind == "rust_tests":
        return rust_tests(text, a)
    if kind == "python_tests":
        return python_tests(text, a)
    return False, f"unknown assertion {kind}"


def judge(task, ended, reasoning, content):
    results = []
    ok = ended
    if not ended:
        results.append({"check": "ends within the cap", "passed": False})
    if task["thinking_mode"] == "thinking" and not reasoning.strip():
        ok = False
        results.append({"check": "reasoning split", "passed": False})
    must = ok
    for a in task["assertions"]:
        try:
            passed, why = check(content, a)
        except Exception as e:  # noqa: BLE001
            passed, why = False, f"error: {e}"
        results.append({"check": a["kind"], "passed": passed, "detail": why,
                        "must_pass": a.get("must_pass", False),
                        "known_shared_failure": a.get("known_shared_failure")})
        ok = ok and passed
        if a.get("must_pass", False):
            must = must and passed
    return ok, results, must


def main():
    suite = json.loads(Path(sys.argv[1]).read_text())
    answers = {a["case"]: a for a in json.loads(Path(sys.argv[2]).read_text())["answers"]}
    rows = []
    for task in suite["cases"]:
        a = answers.get(task["id"])
        if a is None:
            rows.append({"case": task["id"], "missing": True})
            continue
        o = a["oracle"]
        parsed = o.get("parsed") or {}
        o_reason = parsed.get("reasoning_content") or parsed.get("reasoning") or ""
        o_content = parsed.get("content") or ""
        o_ok, o_res, _ = judge(task, bool(o.get("finished")), o_reason, o_content)
        r_ok, r_res, r_must = judge(task, bool(a["ended"]), a["reasoning"], a["content"])
        rows.append({"case": task["id"], "oracle_passed": o_ok, "runtime_passed": r_ok,
                     "runtime_must_pass_held": r_must,
                     "oracle_checks": o_res, "runtime_checks": r_res,
                     "oracle_tokens": o.get("tokens"), "runtime_tokens": a["tokens"],
                     "same_ids": a.get("same_ids_as_oracle")})
    o_pass = sum(1 for r in rows if r.get("oracle_passed"))
    r_pass = sum(1 for r in rows if r.get("runtime_passed"))
    report = {"schema": "psionic.deepseek_v41.g2_v2_report.v1", "issue": 12405,
              "suite_sha256": hashlib.sha256(Path(sys.argv[1]).read_bytes()).hexdigest(),
              "checker_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              "oracle_passes": o_pass, "runtime_passes": r_pass, "tasks": len(suite["cases"]),
              "missing": [r["case"] for r in rows if r.get("missing")],
              "floor_held": all(r.get("runtime_must_pass_held") for r in rows if not r.get("missing")),
              "passed": r_pass >= o_pass and not any(r.get("missing") for r in rows)
              and all(r.get("runtime_must_pass_held") for r in rows if not r.get("missing")),
              "rows": rows}
    Path(sys.argv[3]).write_text(json.dumps(report, indent=1) + "\n")
    print(json.dumps({k: report[k] for k in ("oracle_passes", "runtime_passes", "tasks", "missing", "floor_held", "passed")}))


if __name__ == "__main__":
    main()
