#!/usr/bin/env python3
"""Qualify a complete original evaluator baseline and legacy native agreement.

  python3 coax/tools/native_check_baseline.py FILE.coax [--json] [--out FILE]
  python3 coax/tools/native_check_baseline.py --selection coax/tests/native-checks.json

The original source runs once with `test --library --evidence`, using the
compiler's ordinary check budget. Its complete coax.evidence.v1 record is
retained. The same process observation reaches the unchanged v1 differential
comparison, so no synthetic evaluator source or second baseline is needed.

Every authored check needs unambiguous correspondence to a supported local
legacy-lifted body before this companion can pass. Quantified, imported,
indented, duplicate-name and otherwise unsupported checks remain explicit
coverage gaps. Authority coverage is preserved as evaluator-only evidence.
This is not compiler-owned AST execution or a general native correctness proof.
"""
import argparse
from collections import Counter
from contextlib import contextmanager
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import sys
import tempfile
import threading
import time

SCHEMA = "coax.native-checks.baseline.v1"
DEFAULT_STEPS = 20_000_000
TOTAL_SECONDS = 840
OUTCOMES = ("counterexample", "infrastructure-error", "inconclusive", "not-checked", "passed")
ROOT = Path(__file__).resolve().parents[1]
REPO = ROOT.parent
_LOCK = threading.RLock()
_spec = importlib.util.spec_from_file_location(
    "coax_native_checks_legacy", Path(__file__).with_name("native_checks.py"))
nc = importlib.util.module_from_spec(_spec)
sys.modules[_spec.name] = nc
_spec.loader.exec_module(nc)


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def worst(outcomes):
    values = set(outcomes)
    return next((value for value in OUTCOMES if value in values), "not-checked")


def display(path):
    return str(path.relative_to(REPO)) if path.is_relative_to(REPO) else str(path)


def aliases(first, second):
    first, second = Path(first), Path(second)
    if first.resolve() == second.resolve():
        return True
    try:
        return os.path.samefile(first, second)
    except OSError:
        return False


@contextmanager
def bounded_execution():
    """Apply one command budget to all legacy subprocess boundaries as well."""
    execute = nc.execute
    deadline = time.monotonic() + TOTAL_SECONDS

    def bounded(command, cwd=None, timeout=None, **options):
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            return {"returncode": None, "stdout": "", "stderr": "aggregate command budget exhausted",
                    "timeout": True}
        limit = min(remaining, nc.TIMEOUT if timeout is None else timeout)
        return execute(command, cwd=cwd, timeout=limit, **options)

    nc.execute = bounded
    try:
        yield
    finally:
        nc.execute = execute


def physical_claim_path(claim, cwd, sources):
    at = claim.get("at")
    if not isinstance(at, dict) or not isinstance(at.get("file"), str):
        raise ValueError("an authored claim has no physical source location")
    if any(type(at.get(axis)) is not int or at[axis] < 1 for axis in ("line", "col")):
        raise ValueError("an authored claim has an invalid physical source location")
    name = Path(at["file"])
    choices = [name] if name.is_absolute() else [cwd / name, REPO / name]
    matches = {p.resolve() for p in choices if p.resolve() in sources}
    if len(matches) != 1:
        raise ValueError("an authored claim does not identify one admitted physical source")
    return matches.pop()


def validate_baseline(record, execution, path, original_sha, compiler_sha, steps):
    """Check identity/transport consistency without replacing the raw evidence."""
    if not isinstance(record, dict) or record.get("schema") != "coax.evidence.v1":
        raise ValueError("missing or unsupported complete evaluator evidence")
    if record.get("command") != "test" or Path(record.get("input", "")).resolve() != path:
        raise ValueError("evaluator evidence identifies a different original command or source")
    status = execution.get("returncode")
    if (type(status) is not int or status not in (0, 1) or
            type(record.get("exit_status")) is not int or record["exit_status"] != status):
        raise ValueError("evaluator process status and evidence disagree")
    identity = record.get("compiler")
    if not isinstance(identity, dict) or not compiler_sha or identity.get("sha256") != compiler_sha:
        raise ValueError("evaluator evidence identifies a different compiler")
    if not isinstance(identity.get("version"), str) or not identity["version"]:
        raise ValueError("evaluator evidence omits the compiler version")
    target = record.get("target")
    if not isinstance(target, dict) or target.get("name") != "coax-evaluator":
        raise ValueError("the original baseline is not evaluator-target evidence")
    profile = record.get("profile")
    if (not isinstance(profile, dict) or profile.get("library") is not True or
            profile.get("arithmetic") != "checked" or profile.get("selected_runtime_arithmetic") != "checked"):
        raise ValueError("the original baseline does not identify checked library evaluation")
    selection = record.get("selection")
    if (not isinstance(selection, dict) or selection.get("only") is not None or
            selection.get("list") is not False or selection.get("root_only") is not False):
        raise ValueError("the original evaluator baseline was partially selected")
    budgets = record.get("budgets")
    if not isinstance(budgets, dict) or budgets.get("check_steps") != (DEFAULT_STEPS if steps is None else steps):
        raise ValueError("evaluator evidence does not identify the requested check budget")
    for name in ("check_steps", "check_slots", "check_containers", "run_steps"):
        if type(budgets.get(name)) is not int or budgets[name] <= 0:
            raise ValueError("evaluator evidence has missing or invalid resource budgets")
    units = record.get("sources")
    if not isinstance(units, list) or not units:
        raise ValueError("evaluator evidence has no admitted source graph")
    sources = {}
    for unit in units:
        if not isinstance(unit, dict) or not isinstance(unit.get("path"), str):
            raise ValueError("evaluator evidence has an invalid source unit")
        source = Path(unit["path"]).resolve()
        if source in sources or not re.fullmatch(r"[0-9a-f]{64}", str(unit.get("sha256", ""))):
            raise ValueError("evaluator evidence has duplicate or invalid source identities")
        if digest(source) != unit["sha256"]:
            raise ValueError("an evaluator source hash differs from the original bytes on disk")
        sources[source] = unit["sha256"]
    if sources.get(path) != original_sha:
        raise ValueError("the original source is absent or has a different consumed hash")
    claims = record.get("claims")
    if not isinstance(claims, list):
        raise ValueError("evaluator evidence omits its complete claims")
    locations, identities = {}, set()
    for index, claim in enumerate(claims):
        if not isinstance(claim, dict) or claim.get("kind") not in ("#test", "#forall", "#authority"):
            raise ValueError("evaluator evidence has an unsupported claim")
        if claim.get("outcome") not in OUTCOMES:
            raise ValueError("an evaluator claim has no recognized outcome")
        if claim["kind"] == "#authority":
            continue
        for field in ("cases", "steps"):
            if type(claim.get(field)) is not int or claim[field] < 0:
                raise ValueError("an authored claim has missing or invalid execution counts")
        if claim["outcome"] == "passed" and claim["cases"] == 0:
            raise ValueError("a passed authored claim executed no cases")
        if not isinstance(claim.get("name"), str):
            raise ValueError("an authored claim has no display name")
        source = physical_claim_path(claim, path.parent, sources)
        at = claim["at"]
        key = (claim["kind"], claim["name"], source, at["line"], at["col"])
        if key in identities:
            raise ValueError("evaluator evidence repeats an authored claim identity")
        identities.add(key)
        locations[index] = source
    verdict = record.get("verdict")
    if record.get("front_end") == "failed":
        if claims or verdict != "not-checked" or status == 0:
            raise ValueError("front-end refusal and evidence are inconsistent")
    elif record.get("front_end") == "ok":
        if verdict != worst(c["outcome"] for c in claims):
            raise ValueError("evaluator verdict does not cover all of its claims")
        if verdict == "passed" and status != 0:
            raise ValueError("a successful evaluator baseline has a failing process status")
        if verdict in ("counterexample", "inconclusive", "infrastructure-error") and status == 0:
            raise ValueError("a failing evaluator verdict has a successful process status")
    else:
        raise ValueError("evaluator evidence has no recognized front-end result")
    return sources, locations


def coverage(record, path, src, locations):
    """Only exact physical declaration correspondence can cover a compiler claim."""
    claims = record.get("claims", []) if isinstance(record, dict) else []
    authored = [(i, c) for i, c in enumerate(claims)
                if isinstance(c, dict) and c.get("kind") in ("#test", "#forall")]
    entries = [{"kind": c["kind"], "name": c.get("name"), "at": c.get("at"),
                "outcome": "not-checked", "why": "the original baseline did not qualify"}
               for _, c in authored]
    headers = {m.end(): m for m in nc.TEST.finditer(src)}
    lifted, problem = {}, None
    try:
        for name, body, offset in nc.lift(src)[1]:
            header = headers[offset]
            key = (name, nc.line_of(src, header.start()), 1)
            lifted[key] = (body, nc.rewrite(body))
    except (ValueError, KeyError):
        problem = "the legacy source lifter cannot inventory this source"
    names = Counter(c.get("name") for _, c in authored if c["kind"] == "#test")
    if any(count > 1 for count in names.values()):
        problem = "duplicate display names cannot be matched by the legacy evaluator parser"
    elif re.search(r"(?m)^\s*main\s*::", src):
        problem = "the original program's main is not replaced by a native harness"
    elif "/*" in src:
        problem = "block comments are outside the legacy source lifter's supported inventory"
    root_keys = {(c.get("name"), c.get("at", {}).get("line"), c.get("at", {}).get("col"))
                 for i, c in authored if c["kind"] == "#test" and locations.get(i) == path}
    if set(lifted) - root_keys:
        problem = "the legacy lifter found bodies absent from the original physical claim inventory"
    eligible = []
    for position, (index, claim) in enumerate(authored):
        entry = entries[position]
        if claim["kind"] != "#test":
            entry["why"] = "finite #forall bodies are not executed by the legacy native runner"
        elif locations.get(index) != path:
            entry["why"] = "an imported physical check is not a root native body"
        elif problem:
            entry["why"] = problem
        else:
            at = claim.get("at", {})
            match = lifted.get((claim.get("name"), at.get("line"), at.get("col")))
            if match is None:
                entry["why"] = "the physical check is not a supported column-zero legacy body"
            elif match[1][0] is None:
                entry["why"] = "legacy body is unsupported: " + match[1][1]
            elif not match[1][0][1]:
                entry["why"] = "the legacy body has no statement #assert to compare"
            else:
                entry["why"] = "native comparison has not executed"
                eligible.append(position)
    return entries, eligible, problem


def unqualified_claims(record):
    if not isinstance(record, dict) or not isinstance(record.get("claims"), list):
        return []
    return [{"kind": claim["kind"], "name": claim.get("name"), "at": claim.get("at"),
             "outcome": "not-checked", "why": "the original baseline has not qualified"}
            for claim in record["claims"] if isinstance(claim, dict) and claim.get("kind") in ("#test", "#forall")]


def empty_differential(compiler, compiler_sha, seed):
    return {"schema": nc.SCHEMA, "compiler": str(compiler), "compiler_sha256": compiler_sha,
            "target": f"{platform.system()}-{platform.machine()}",
            "profiles": {"arithmetic": "checked", "c": list(nc.OPTS), "cc": nc.CC,
                         "c_flags": ["-std=c11", "-ffp-contract=off"]},
            "seed": seed or None, "files": [], "counts": {}, "outcome": "not-checked"}


def run(files, seed="", keep=None, steps=None):
    """Return strict companion evidence; no original source bytes are rewritten."""
    if seed not in ("", "emitter", "evaluator"):
        raise ValueError("seed must be emitter or evaluator")
    if steps is not None and (type(steps) is not int or not 1 <= steps <= DEFAULT_STEPS):
        raise ValueError("explicit steps must be positive and no greater than the ordinary 20000000 budget")
    paths = [Path(f).resolve() for f in files]
    if any(aliases(path, previous) for index, path in enumerate(paths) for previous in paths[:index]):
        raise ValueError("the same original source was selected more than once")
    if keep is not None:
        keep = Path(keep).resolve()
        if any(aliases(keep, source) for source in paths) or keep.is_file():
            raise ValueError("the artifact directory aliases an original source or existing file")
        keep.mkdir(parents=True, exist_ok=True)
    work = Path(tempfile.mkdtemp(prefix="native-check-baseline-", dir=keep))
    compiler = nc.compiler()
    try:
        compiler_sha = digest(compiler)
    except OSError:
        compiler_sha = None
    differential = empty_differential(compiler, compiler_sha, seed)
    result = {"schema": SCHEMA, "files": [], "differential": differential,
              "artifacts": str(work), "outcome": "not-checked",
              "method": {"baseline": "original source once: test --library --evidence",
                         "native": "legacy source-lifted v1 comparison at checked O0/O2",
                         "authority": "complete evaluator-only execution coverage, not a native body"},
              "limits": ["Native execution remains the legacy source lifter, not compiler-owned AST lowering.",
                         "Quantified, imported and otherwise unsupported bodies remain named native coverage gaps.",
                         "Legacy trap equivalence and the runtime report protocol keep their v1 scope.",
                         "Hash observations do not freeze the filesystem or establish general compiler correctness."]}
    with _LOCK, bounded_execution():
        for number, path in enumerate(paths):
            item = {"file": display(path), "baseline": None, "baseline_execution": None,
                    "native_coverage": [], "outcome": "not-checked"}
            result["files"].append(item)
            directory = work / str(number)
            directory.mkdir()
            evidence = directory / "evaluator.json"
            command = [str(compiler), "test", str(path), "--library", "--evidence", str(evidence)]
            if steps is not None:
                command += ["--steps", str(steps)]
            try:
                original = path.read_bytes()
                original_sha = hashlib.sha256(original).hexdigest()
                execution = nc.execute(command, cwd=path.parent)
                if not isinstance(execution, dict):
                    raise ValueError("the evaluator process returned no execution observation")
                item["baseline_execution"] = {**execution, "argv": command, "cwd": str(path.parent),
                                              "evidence": str(evidence)}
                try:
                    item["baseline"] = json.loads(evidence.read_text())
                except (OSError, ValueError):
                    pass
                item["native_coverage"] = unqualified_claims(item["baseline"])
                if execution.get("timeout"):
                    item.update(outcome="inconclusive", why="original evaluator exceeded its wall-clock budget")
                    continue
                if execution.get("unavailable"):
                    item.update(outcome="infrastructure-error", why="original evaluator is unavailable")
                    continue
                if any(not isinstance(execution.get(key), str) for key in ("stdout", "stderr")):
                    raise ValueError("the evaluator process observation omits its output streams")
                sources, locations = validate_baseline(item["baseline"], execution, path, original_sha,
                                                       compiler_sha, steps)
                if digest(compiler) != compiler_sha:
                    raise ValueError("the compiler changed during original baseline execution")
                item["validated_sources"] = [{"path": str(p), "sha256": value} for p, value in sources.items()]
                src = original.decode("utf-8")
                entries, eligible, problem = coverage(item["baseline"], path, src, locations)
                item["native_coverage"] = entries
                verdict = item["baseline"]["verdict"]
                if verdict != "passed":
                    for index in eligible:
                        entries[index]["why"] = "the complete original baseline did not pass; native comparison skipped"
                    item.update(outcome=verdict, why="the complete original evaluator baseline did not pass")
                    continue
                if not eligible or problem:
                    item.update(outcome="not-checked", why=problem or "no supported original native body executed")
                    continue
                part = nc.run([path], seed=seed, keep=directory / "native",
                              evaluator_observations={str(path): execution})
                if part.get("schema") != nc.SCHEMA or len(part.get("files", [])) != 1:
                    raise ValueError("legacy comparison returned an invalid differential record")
                native = part["files"][0]
                if (part.get("compiler_sha256") != compiler_sha or
                        native.get("source_sha256") != original_sha or native.get("file") != display(path)):
                    raise ValueError("legacy comparison identifies a different compiler or original source")
                differential["files"].extend(part["files"])
                compared = {}
                for row in native["checks"]:
                    if row["name"] in compared:
                        raise ValueError("legacy comparison repeated a display-name identity")
                    compared[row["name"]] = row
                for index in eligible:
                    entry = entries[index]
                    row = compared.get(entry["name"])
                    if row is None:
                        entry["why"] = "legacy comparison did not report this physical check"
                    elif row["outcome"] == "passed":
                        if row.get("evaluator") != ["passed"] or any(row.get(opt) != ["passed"] for opt in nc.OPTS):
                            raise ValueError("legacy passing row lacks complete evaluator/native observations")
                        entry.update(outcome="passed", why="the exact local body agreed with the original baseline at O0/O2")
                    else:
                        outcome = row["outcome"] if row["outcome"] in OUTCOMES else "not-checked"
                        entry.update(outcome=outcome, why=row.get("why", "legacy native comparison did not pass"))
                for source, value in sources.items():
                    if digest(source) != value:
                        raise ValueError("a consumed source changed during native comparison")
                if digest(compiler) != compiler_sha:
                    raise ValueError("the compiler changed during native comparison")
                native_outcome = native["outcome"] if native["outcome"] in OUTCOMES else "not-checked"
                item["outcome"] = worst([verdict, native_outcome, *(entry["outcome"] for entry in entries)])
            except (OSError, ValueError, KeyError, TypeError, IndexError) as error:
                item.update(outcome="infrastructure-error", why=str(error))
                for entry in item["native_coverage"]:
                    entry.update(outcome="not-checked", why="baseline or comparison identity was not established")
            finally:
                if item["baseline_execution"] is not None and evidence.is_file():
                    item["baseline_execution"]["evidence_sha256"] = digest(evidence)
    if differential["files"]:
        differential["outcome"] = worst(f["outcome"] if f["outcome"] in OUTCOMES else "not-checked"
                                          for f in differential["files"])
        differential["counts"] = dict(Counter(c["outcome"] for f in differential["files"] for c in f["checks"]))
    result["outcome"] = worst(item["outcome"] for item in result["files"])
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("files", nargs="*")
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--out", type=Path)
    parser.add_argument("--keep", type=Path)
    parser.add_argument("--seed", choices=("emitter", "evaluator"), default="")
    parser.add_argument("--selection", type=Path)
    parser.add_argument("--steps", type=int)
    args = parser.parse_intermixed_args(argv)
    try:
        files = [Path(f).resolve() for f in args.files]
        protected = files + [nc.compiler(), Path(__file__), Path(nc.__file__)]
        if args.selection:
            selection = json.loads(args.selection.read_text())
            if (not isinstance(selection, dict) or not isinstance(selection.get("files"), list) or
                    any(not isinstance(f, str) for f in selection["files"])):
                raise ValueError("selection must name a list of original source files")
            if selection.get("schema", "coax.native-checks.selection.v1") != "coax.native-checks.selection.v1":
                raise ValueError("unsupported native selection schema")
            files += [(REPO / f).resolve() for f in selection["files"]]
            protected += files + [args.selection]
        if not files:
            parser.error("select at least one original source file")
        if args.out and any(aliases(args.out, path) for path in protected):
            raise ValueError("output aliases an original source, selection or tool")
        record = run(files, seed=args.seed, keep=args.keep, steps=args.steps)
        for item in record["files"]:
            protected += [unit["path"] for unit in item.get("validated_sources", [])]
            raw = item.get("baseline")
            if isinstance(raw, dict) and isinstance(raw.get("sources"), list):
                protected += [unit["path"] for unit in raw["sources"]
                              if isinstance(unit, dict) and isinstance(unit.get("path"), str)]
            if item.get("baseline_execution"):
                protected.append(item["baseline_execution"]["evidence"])
        if args.out:
            if any(aliases(args.out, path) for path in protected):
                raise ValueError("output aliases a consumed source or evaluator evidence")
            if record["outcome"] == "infrastructure-error" and args.out.exists():
                raise ValueError("an infrastructure failure will not overwrite an existing output")
            args.out.write_text(json.dumps(record, indent=2) + "\n")
        if args.json:
            print(json.dumps(record, indent=2))
        else:
            for item in record["files"]:
                print(f"{item['outcome']:20} {item['file']}" + (f"  ({item['why']})" if item.get("why") else ""))
                for entry in item["native_coverage"]:
                    print(f"  {entry['outcome']:18} {entry['kind']} {entry['name']}  ({entry['why']})")
            print("native baseline: " + record["outcome"])
        return 0 if record["outcome"] == "passed" else 1
    except (OSError, ValueError, TypeError) as error:
        print("native baseline: " + str(error), file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
