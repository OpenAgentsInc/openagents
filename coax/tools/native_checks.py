#!/usr/bin/env python3
"""Run authored #test checks on emitted native code (r0-native-check-exe).

  python3 coax/tools/native_checks.py FILE.coax [FILE.coax ...] [--out DIR] [--json]
  python3 coax/tools/native_checks.py --selection coax/tests/native-checks.json [--out DIR]

For each selected file, every eligible `#test "NAME" { ... }` runs three ways:
the evaluator (`coax test`), and the same body emitted to C (`coax c
--arithmetic checked`) and compiled at -O0 and -O2. Each run says, per test,
"passed", "assertion N failed" (N counts the test's #asserts from 1, so the
evaluator's failing line and the native result name the same assertion), or
"trap" (a checked-arithmetic or bounds trap, or any signal). The three must
agree; otherwise the test is a counterexample, minimised to the test alone and
cut after its first disagreeing assertion, and written to --keep DIR.

The native harness is a separate, temporary source beside the file (so its
#load paths resolve): the file as it is (its tests still exercise every procedure), each
eligible body as `__native_check_K :: () -> int`, its `#assert E;` as
`if !(E) { return N; }`, and a `main` that runs them in order and prints a line
per check. A trap stops the process; the harness is rebuilt without the checks
already run and carries on. Product emission (`coax c FILE`) never contains
it: the harness is never written into a product file, and the check below is
that `__native_check` appears in no product emission.

A test is eligible when its body uses no compile-time directive but #assert
(#forall, #run, #expect... are not run natively) and its #asserts are
statements. Ineligible tests, files that don't emit or compile, timeouts and a
missing compiler are reported as rejected / not-checked / inconclusive /
infrastructure-error: none of them is ever "passed". The record names every
test in a selected file and what became of it.

Seeded defects (--seed): `emitter` swaps the emitted C's checked multiply for
a checked add; `evaluator` reports the evaluator's first passing check as
failed at its first assertion. Either must produce a counterexample.
"""
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REPO = ROOT.parent
SCHEMA = "coax.native-checks.v1"
EVALUATOR_STEPS = 400_000_000
TIMEOUT = 300
CC = [os.environ.get("COAX_NATIVE_CC", "/usr/bin/clang" if platform.system() == "Darwin" else "cc")]
OPTS = ("-O0", "-O2")
MUL = "if (__builtin_mul_overflow(a, b, &r))"
ADD = "if (__builtin_add_overflow(a, b, &r))"


def compiler() -> Path:
    return Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "bin/coax")).resolve()


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


# ---- the source: tests lifted out, asserts found

def skip_string(s: str, i: int) -> int:
    q = s[i]
    i += 1
    while i < len(s) and s[i] != q:
        if s[i] == "\\":
            i += 1
        i += 1
    return i + 1


def skip_comment(s: str, i: int) -> int:
    j = s.find("\n", i)
    return len(s) if j < 0 else j


def closing(s: str, i: int) -> int:
    """The index of the brace that closes the one at i."""
    depth = 0
    while i < len(s):
        c = s[i]
        if c in "\"'":
            i = skip_string(s, i)
            continue
        if s.startswith("//", i):
            i = skip_comment(s, i)
            continue
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                return i
        i += 1
    raise ValueError("unbalanced braces")


TEST = re.compile(r'(?m)^#test\s+"((?:[^"\\]|\\.)*)"\s*\{')


def lift(src: str):
    """(code without its #test blocks, [(name, body, body_offset)])."""
    tests, parts, pos = [], [], 0
    for m in TEST.finditer(src):
        if m.start() < pos:
            continue
        end = closing(src, m.end() - 1)
        tests.append((m.group(1), src[m.end():end], m.end()))
        parts.append(src[pos:m.start()])
        pos = end + 1
    parts.append(src[pos:])
    return "".join(parts), tests


def rewrite(body: str):
    """The body with each statement #assert as an early return of its number;
    (text, [offset of each #assert in body]) or a reason it isn't eligible."""
    out, asserts, i, last = [], [], 0, 0
    while i < len(body):
        c = body[i]
        if c in "\"'":
            i = skip_string(body, i)
            continue
        if body.startswith("//", i):
            i = skip_comment(body, i)
            continue
        if c == "#":
            word = re.match(r"#[a-z_]+", body[i:])
            name = word.group(0) if word else "#"
            if name != "#assert":
                return None, f"uses {name}"
            before = previous_code(body, i)
            if before and before not in ";{}":
                return None, "an #assert inside an expression"
            j, depth = i + len("#assert"), 0
            while j < len(body):
                d = body[j]
                if d in "\"'":
                    j = skip_string(body, j)
                    continue
                if d == "#":
                    inner = re.match(r"#[a-z_]+", body[j:])
                    return None, f"uses {inner.group(0) if inner else '#'}"
                if d in "([{":
                    depth += 1
                elif d in ")]}":
                    depth -= 1
                elif d == ";" and depth == 0:
                    break
                j += 1
            if j >= len(body):
                return None, "an #assert without its ;"
            asserts.append(i)
            out.append(body[last:i])
            out.append(f"if !({body[i + len('#assert'):j].strip()}) {{ return {len(asserts)}; }}")
            i = last = j + 1
            continue
        i += 1
    out.append(body[last:])
    return ("".join(out), asserts), None


def previous_code(text: str, at: int) -> str:
    """The last character of code before at, past blank space and // comments ("" at the start)."""
    end = at
    while end > 0:
        start = text.rfind("\n", 0, end) + 1
        line = uncommented(text[start:end]).rstrip()
        if line.strip():
            return line[-1]
        end = start - 1 if start > 0 else 0
    return ""


def uncommented(text: str) -> str:
    """text with its // comments removed (strings kept)."""
    out, i = [], 0
    while i < len(text):
        if text[i] in "\"'":
            j = skip_string(text, i)
            out.append(text[i:j])
            i = j
        elif text.startswith("//", i):
            i = skip_comment(text, i)
        else:
            out.append(text[i])
            i += 1
    return "".join(out)


def line_of(src: str, offset: int) -> int:
    return src.count("\n", 0, offset) + 1


# ---- running things

def execute(command, cwd=None, timeout=None, *, expected_fault=False):
    timeout = TIMEOUT if timeout is None else min(timeout, TIMEOUT)
    command = [str(c) for c in command]
    if expected_fault and sys.platform == "darwin":
        # The launcher execs the target, preserving real signals and diagnostics.
        command = [sys.executable, str(ROOT / "tests/quiet_crash.py"), "--", *command]
    try:
        r = subprocess.run(command, cwd=cwd, capture_output=True, text=True, timeout=timeout)
        if expected_fault and sys.platform == "darwin" and r.returncode in (125, 127) and r.stderr.startswith("quiet_crash:"):
            return {"returncode": r.returncode, "stdout": r.stdout, "stderr": r.stderr, "unavailable": True}
        return {"returncode": r.returncode, "stdout": r.stdout, "stderr": r.stderr}
    except subprocess.TimeoutExpired:
        return {"returncode": None, "stdout": "", "stderr": "timeout", "timeout": True}
    except OSError as e:
        return {"returncode": None, "stdout": "", "stderr": str(e), "unavailable": True}


RESULT = re.compile(r"^(ok  |FAIL) #test (.*) \(")
DIAG = re.compile(r"^(.*?):(\d+):\d+: error\[(E\d+)\]")


def evaluate(path: Path, src: str, tests, seed: str, *, observation=None):
    """Per test name: ("passed",) | ("assert", N) | ("trap", code) | ("inconclusive", why)."""
    probe = path
    if observation is None and not re.search(r"(?m)^main\s*::", src):
        fd, name = tempfile.mkstemp(prefix=".native-check-eval-", suffix=".coax", dir=path.parent)
        os.close(fd)
        probe = Path(name)
        probe.write_text(src.rstrip("\n") + "\nmain :: () {}\n")
    try:
        r = observation if observation is not None else execute(
            [compiler(), "test", probe.name, "--steps", str(EVALUATOR_STEPS)], cwd=path.parent)
    finally:
        if probe != path:
            probe.unlink(missing_ok=True)
    if r.get("timeout"):
        return None, "inconclusive", "the evaluator ran past its time limit"
    if r.get("unavailable"):
        return None, "infrastructure-error", r["stderr"]
    lines = (r["stdout"] + r["stderr"]).splitlines()
    results, current = {}, None
    assert_lines = {}
    for name, body, at in tests:
        rw, _ = rewrite(body)
        if rw:
            assert_lines[name] = [line_of(src, at + a) for a in rw[1]]
    for k, line in enumerate(lines):
        m = RESULT.match(line)
        if not m:
            continue
        name = m.group(2)
        if m.group(1) == "ok  ":
            results[name] = ("passed",)
            continue
        verdict = ("trap", "unknown")
        for follow in lines[k + 1:k + 8]:
            d = DIAG.match(follow)
            if d:
                code, at = d.group(3), int(d.group(2))
                if code == "E0406" and Path(d.group(1)).name == probe.name and at in assert_lines.get(name, []):
                    verdict = ("assert", assert_lines[name].index(at) + 1)
                elif code == "E0400":
                    verdict = ("inconclusive", "evaluator step limit")
                else:
                    verdict = ("trap", code)
                break
        results[name] = verdict
    if not results and r["returncode"] != 0:
        first = next((l.strip() for l in lines if "error[" in l), lines[-1] if lines else "coax test failed")
        return None, "rejected", "the evaluator doesn't run it: " + first
    if seed == "evaluator":
        for name in results:
            if results[name] == ("passed",):
                results[name] = ("assert", 1)
                break
    return results, None, None


def harness(code: str, checks) -> str:
    lines = [code, "", "// ---- native checks (coax/tools/native_checks.py; never part of product emission)"]
    if '#import "Basic"' not in code:
        lines.insert(0, '#import "Basic";')
    for k, (name, body) in enumerate(checks):
        lines.append(f"__native_check_{k} :: () -> int {{{body}\n  return 0;\n}}")
        # every procedure must be exercised by a check (E0380): this one is, by the evaluator's run
        lines.append(f'#test "__native_check_{k}" {{ #assert __native_check_{k}() >= 0; }}')
    lines.append("main :: () {")
    for k, _ in enumerate(checks):
        lines.append(f'  print("% %\\n", {k}, __native_check_{k}());')
    lines.append("}")
    return "\n".join(lines) + "\n"


def native(path: Path, code: str, checks, seed: str, keep: Path):
    """Per profile, the results of checks in order: ("passed",) | ("assert", N) | ("trap", rc);
    or a whole-file outcome (rejected / infrastructure-error / inconclusive) with why."""
    results = {opt: [] for opt in OPTS}
    identities = {}
    for opt in OPTS:
        start = 0
        while start < len(checks):
            rest = checks[start:]
            fd, name = tempfile.mkstemp(prefix=".native-check-", suffix=".coax", dir=path.parent)
            os.close(fd)
            h = Path(name)
            try:
                h.write_text(harness(code, rest))
                e = execute([compiler(), "c", h.name, "--arithmetic", "checked"], cwd=path.parent)
            finally:
                h.unlink(missing_ok=True)
            if e.get("unavailable"):
                return None, "infrastructure-error", e["stderr"], identities
            if e.get("timeout"):
                return None, "inconclusive", "C emission ran past its time limit", identities
            if e["returncode"] != 0:
                first = next((l for l in (e["stdout"] + e["stderr"]).splitlines() if "error[" in l), "C emission failed")
                return None, "rejected", "the harness doesn't emit: " + first.strip(), identities
            c = e["stdout"]
            if seed == "emitter":
                if c.count(MUL) < 1:
                    return None, "infrastructure-error", "the seeded emitter defect needs a checked multiply", identities
                c = c.replace(MUL, ADD)
            csrc = keep / f"{path.stem}-{opt[1:]}-{start}.c"
            csrc.write_text(c)
            binary = keep / f"{path.stem}-{opt[1:]}-{start}"
            b = execute([*CC, "-std=c11", opt, "-ffp-contract=off", str(csrc), "-lm", "-o", str(binary)])
            if b.get("unavailable"):
                return None, "infrastructure-error", f"no C compiler: {b['stderr']}", identities
            if b.get("timeout"):
                return None, "inconclusive", "the C compiler ran past its time limit", identities
            if b["returncode"] != 0:
                return None, "rejected", "the emitted C doesn't compile: " + b["stderr"].strip().splitlines()[0][:200], identities
            identities.setdefault(opt, []).append({"c_sha256": sha(c.encode()), "binary_sha256": sha(binary.read_bytes())})
            run = execute([binary], timeout=120, expected_fault=True)
            if run.get("unavailable"):
                return None, "infrastructure-error", "the native target did not start: " + run["stderr"], identities
            if run.get("timeout"):
                return None, "inconclusive", f"check {start} ran past its time limit at {opt}", identities
            got = []
            for line in run["stdout"].splitlines():
                parts = line.split()
                if len(parts) == 2 and parts[0].isdigit():
                    v = int(parts[1])
                    got.append(("passed",) if v == 0 else ("assert", v))
            results[opt].extend(got)
            start += len(got)
            if run["returncode"] != 0 and start >= len(checks):
                return None, "infrastructure-error", "the native process failed after all reported results", identities
            if run["returncode"] != 0 and start < len(checks):
                results[opt].append(("trap", run["returncode"]))
                start += 1
            elif len(got) < len(rest):
                return None, "infrastructure-error", f"the harness stopped at check {start} without a trap", identities
    return results, None, None, identities


def same(a, b) -> bool:
    if a[0] != b[0]:
        return False
    return a[0] != "assert" or a[1] == b[1]


def minimise(code: str, name: str, body: str, upto: int, keep: Path, stem: str) -> Path:
    """The disagreeing test alone, cut after its first disagreeing assertion."""
    (text, asserts), _ = rewrite(body)
    cut = body
    if upto and upto <= len(asserts):
        end = body.find(";", asserts[upto - 1])
        cut = body[:end + 1] + "\n"
    out = keep / f"{stem}-{re.sub('[^a-z0-9]+', '-', name.lower()).strip('-')[:48]}.coax"
    out.write_text(code.rstrip() + f'\n\n// minimised by native_checks.py: the evaluator and native code disagree here\n#test "{name}" {{{cut}}}\n')
    return out


def check_file(path: Path, seed: str = "", keep: Path | None = None, *, observation=None) -> dict:
    path = path.resolve()
    src = path.read_text()
    keep = keep or Path(tempfile.mkdtemp(prefix="native-checks-"))
    keep.mkdir(parents=True, exist_ok=True)
    rec = {"file": str(path.relative_to(REPO)) if path.is_relative_to(REPO) else str(path),
           "source_sha256": sha(src.encode()), "checks": [], "outcome": "not-checked"}
    code, tests = lift(src)
    if re.search(r"(?m)^main\s*::", src):
        # the harness's main runs the checks; a program's own main isn't replaced (not selected)
        rec["checks"] = [{"name": n, "outcome": "rejected", "why": "the file has its own main"} for n, _, _ in tests]
        rec["outcome"] = "rejected"
        return rec
    eligible = []
    for name, body, at in tests:
        rw, why = rewrite(body)
        if rw is None:
            rec["checks"].append({"name": name, "outcome": "rejected", "why": why})
        elif not rw[1]:
            rec["checks"].append({"name": name, "outcome": "rejected", "why": "no #assert to compare"})
        else:
            eligible.append((name, rw[0], body))
    if not eligible:
        rec["outcome"] = "not-checked" if not rec["checks"] else "rejected"
        return rec
    ev, outcome, why = evaluate(path, src, tests, seed, observation=observation)
    if ev is None:
        rec.update(outcome=outcome, why=why)
        for name, _, _ in eligible:
            rec["checks"].append({"name": name, "outcome": "not-checked", "why": why})
        return rec
    nat, outcome, why, ids = native(path, src, [(n, b) for n, b, _ in eligible], seed, keep)
    rec["native_identities"] = ids
    if nat is None:
        rec.update(outcome=outcome, why=why)
        for name, _, _ in eligible:
            rec["checks"].append({"name": name, "outcome": "not-checked", "why": why})
        return rec
    for k, (name, _, body) in enumerate(eligible):
        e = ev.get(name)
        n = {opt: nat[opt][k] if k < len(nat[opt]) else None for opt in OPTS}
        entry = {"name": name, "evaluator": list(e) if e else None, **{opt: list(v) if v else None for opt, v in n.items()}}
        if e is None or any(v is None for v in n.values()):
            entry.update(outcome="not-checked", why="a run didn't report this check")
        elif e[0] == "inconclusive":
            entry.update(outcome="inconclusive", why=e[1])
        elif all(same(e, v) for v in n.values()):
            entry["outcome"] = "passed" if e[0] == "passed" else "agreed-" + e[0]
        else:
            entry["outcome"] = "counterexample"
            firsts = [x[1] for x in [e, *n.values()] if x[0] == "assert"]
            entry["minimised"] = str(minimise(code, name, body, min(firsts) if firsts else 0, keep, path.stem))
        rec["checks"].append(entry)
    order = ("counterexample", "infrastructure-error", "inconclusive", "not-checked")
    outs = {c["outcome"] for c in rec["checks"] if c["outcome"] != "rejected"}
    rec["outcome"] = next((o for o in order if o in outs), "passed")
    return rec


def run(files, seed="", keep=None, *, evaluator_observations=None) -> dict:
    c = compiler()
    record = {"schema": SCHEMA, "compiler": str(c), "compiler_sha256": sha(c.read_bytes()) if c.exists() else None,
              "target": f"{platform.system()}-{platform.machine()}", "profiles": {"arithmetic": "checked", "c": list(OPTS), "cc": CC,
              "c_flags": ["-std=c11", "-ffp-contract=off"]}, "seed": seed or None, "files": []}
    for f in files:
        path = Path(f).resolve()
        if evaluator_observations is not None and str(path) not in evaluator_observations:
            record["files"].append({
                "file": str(path.relative_to(REPO)) if path.is_relative_to(REPO) else str(path),
                "checks": [], "outcome": "infrastructure-error",
                "why": "the supplied evaluator observations omit this source; no evaluator was rerun",
            })
        else:
            observation = None if evaluator_observations is None else evaluator_observations[str(path)]
            record["files"].append(check_file(path, seed, keep, observation=observation))
        if os.environ.get("NATIVE_CHECKS_PROGRESS"):
            print(f"{record['files'][-1]['outcome']:20} {f}", file=sys.stderr, flush=True)
    outs = {f["outcome"] if f["outcome"] != "rejected" else "not-checked" for f in record["files"]}
    order = ("counterexample", "infrastructure-error", "inconclusive", "not-checked")
    counts = {}
    for f in record["files"]:
        for c in f["checks"]:
            counts[c["outcome"]] = counts.get(c["outcome"], 0) + 1
    record["counts"] = counts
    record["outcome"] = next((o for o in order if o in outs), "passed")
    return record


def unselected(selection: dict) -> list:
    """Every #test under the selection's roots that it doesn't run, by file and name."""
    chosen = set(selection["files"])
    out = []
    for root in selection.get("roots", ["verse", "psionic"]):
        for f in sorted((REPO / root).rglob("*.coax")):
            rel = str(f.relative_to(REPO))
            if rel in chosen or "/." in rel:
                continue
            try:
                names = [n for n, _, _ in lift(f.read_text())[1]]
            except (ValueError, UnicodeDecodeError):
                names = ["(unparsed)"]
            if names:
                out.append({"file": rel, "checks": names})
    return out


def main(argv=None) -> int:
    a = list(argv if argv is not None else sys.argv[1:])
    seed, keep, files, as_json, out, selection = "", None, [], False, None, None
    while a:
        x = a.pop(0)
        if x == "--seed":
            seed = a.pop(0)
        elif x == "--keep":
            keep = Path(a.pop(0))
        elif x == "--out":
            out = Path(a.pop(0))
        elif x == "--json":
            as_json = True
        elif x == "--selection":
            sel = json.loads(Path(a.pop(0)).read_text())
            files += [str(REPO / f) for f in sel["files"]]
            selection = sel
        else:
            files.append(x)
    if not files:
        print(__doc__, file=sys.stderr)
        return 2
    rec = run(files, seed, keep)
    if selection:
        rec["unselected"] = unselected(selection)
    if out:
        out.write_text(json.dumps(rec, indent=2) + "\n")
    if as_json:
        print(json.dumps(rec, indent=2))
    else:
        for f in rec["files"]:
            print(f"{f['outcome']:20} {f['file']}" + (f"  ({f['why']})" if f.get("why") else ""))
            for c in f["checks"]:
                print(f"  {c['outcome']:18} {c['name']}" + (f"  ({c['why']})" if c.get("why") else "") + (f"  -> {c['minimised']}" if c.get("minimised") else ""))
        print(f"native checks: {rec['outcome']} " + json.dumps(rec["counts"]))
    return 0 if rec["outcome"] == "passed" else 1


if __name__ == "__main__":
    sys.exit(main())
