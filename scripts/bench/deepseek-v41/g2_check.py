#!/usr/bin/env python3
"""Check the frozen G2 v2 task suite (#12405) on the oracle's and the served
runtime's complete answers.

    g2_check.py SUITE ANSWERS OUT

SUITE is `g2_v2/task-suite.json`; ANSWERS is `answers.json` from
`psionic-v41-stage qualify --answers`, which carries both answers per case.
A task passes when its answer ends with the end token within the cap, a
thinking-mode answer has reasoning, and every assertion holds on the final
text. Generated code runs in a scratch folder with a 60 s limit. The verdict
is non-inferiority: the runtime's pass count must be at least the oracle's.
"""
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


def rust_tests(text, a):
    code = code_block(text, ["rust", "rs"])
    if code is None:
        return False, "no Rust code block"
    code = re.sub(r"(?m)^\s*fn main\s*\(\s*\)\s*\{.*", "", code, flags=re.S) if "fn main" in code else code
    calls = "\n".join(
        f'    let got = {a["function"]}({args}); let want = {want};\n'
        f'    println!("{{}}", if got == want {{ "ok" }} else {{ "FAIL {args}" }});'
        for args, want in a["cases"]
    )
    src = "#![allow(dead_code, unused)]\n" + code + "\nfn main() {\n" + calls + "\n}\n"
    with tempfile.TemporaryDirectory() as d:
        p = Path(d) / "t.rs"
        p.write_text(src)
        c = subprocess.run(["rustc", "--edition", "2021", "-O", "-o", str(Path(d) / "t"), str(p)],
                           capture_output=True, text=True, timeout=LIMIT)
        if c.returncode:
            return False, "does not compile: " + c.stderr[-300:]
        r = subprocess.run([str(Path(d) / "t")], capture_output=True, text=True, timeout=LIMIT)
    fails = [l for l in r.stdout.splitlines() if l != "ok"]
    return not fails and r.returncode == 0, "; ".join(fails) or "all cases hold"


def python_tests(text, a):
    code = code_block(text, ["python", "py"])
    if code is None:
        return False, "no Python code block"
    checks = "\n".join(
        f'print("ok" if {a["function"]}({args}) == {want} else "FAIL {args}")' for args, want in a["cases"]
    )
    with tempfile.TemporaryDirectory() as d:
        p = Path(d) / "t.py"
        p.write_text(code + "\n" + checks + "\n")
        r = subprocess.run([sys.executable, "-I", str(p)], capture_output=True, text=True, timeout=LIMIT, cwd=d)
    fails = [l for l in r.stdout.splitlines() if l != "ok"]
    return not fails and r.returncode == 0, "; ".join(fails) or r.stderr[-200:] or "all cases hold"


def check(text, a):
    kind = a["kind"]
    if kind == "sentences_at_most":
        n = len([s for s in re.split(r"(?<=[.!?])\s+", text.strip()) if s.strip()])
        return n <= a["value"], f"{n} sentences"
    if kind == "contains_any":
        hit = [v for v in a["values"] if v.lower() in text.lower()]
        return bool(hit), f"found {hit}"
    if kind == "last_line_number":
        lines = [l.strip() for l in text.strip().splitlines() if l.strip()]
        nums = re.findall(r"-?\d+(?:\.\d+)?", lines[-1]) if lines else []
        ok = bool(nums) and float(nums[-1]) == float(a["value"])
        return ok, f"last line {lines[-1]!r}" if lines else "empty"
    if kind == "json_fields":
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
    for a in task["assertions"]:
        try:
            passed, why = check(content, a)
        except Exception as e:  # noqa: BLE001
            passed, why = False, f"error: {e}"
        results.append({"check": a["kind"], "passed": passed, "detail": why})
        ok = ok and passed
    return ok, results


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
        o_ok, o_res = judge(task, bool(o.get("finished")), o_reason, o_content)
        r_ok, r_res = judge(task, bool(a["ended"]), a["reasoning"], a["content"])
        rows.append({"case": task["id"], "oracle_passed": o_ok, "runtime_passed": r_ok,
                     "oracle_checks": o_res, "runtime_checks": r_res,
                     "oracle_tokens": o.get("tokens"), "runtime_tokens": a["tokens"],
                     "same_ids": a.get("same_ids_as_oracle")})
    o_pass = sum(1 for r in rows if r.get("oracle_passed"))
    r_pass = sum(1 for r in rows if r.get("runtime_passed"))
    report = {"schema": "psionic.deepseek_v41.g2_v2_report.v1", "issue": 12405,
              "suite_sha256": __import__("hashlib").sha256(Path(sys.argv[1]).read_bytes()).hexdigest(),
              "oracle_passes": o_pass, "runtime_passes": r_pass, "tasks": len(suite["cases"]),
              "missing": [r["case"] for r in rows if r.get("missing")],
              "passed": r_pass >= o_pass and not any(r.get("missing") for r in rows),
              "rows": rows}
    Path(sys.argv[3]).write_text(json.dumps(report, indent=1) + "\n")
    print(json.dumps({k: report[k] for k in ("oracle_passes", "runtime_passes", "tasks", "missing", "passed")}))


if __name__ == "__main__":
    main()
