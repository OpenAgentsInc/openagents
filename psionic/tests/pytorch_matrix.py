#!/usr/bin/env python3
"""Chapter 250's capability matrix: generated, never hand-written. Book tooling.

The matrix is rebuilt from the chapter registry (psionic/book/chapters.json),
the capability claims ledger (coax/book/capability-claims.json), the operator
table in psionic/lib/pytorch-audit.coax and the workflow record
psionic/fixtures/pytorch/workflows.json. problems() checks that:

- psionic/fixtures/pytorch/matrix.txt and chapter 250's generated block equal
  the regenerated text byte for byte;
- every operator row cites a chapter the registry marks implemented or partly
  implemented, and names code that exists at its level; the audit's limits
  equal the trainer's own constants;
- every workflow cites runnable chapters, claims only targets those chapters
  qualified, and each evidence line or value is in its file;
- the opt-in PyTorch comparator's record agrees with the runs it compared;
- chapter 250's prose claims: each <!-- claim: N target --> marker names a
  runnable chapter that qualified that target; a paragraph that names CUDA,
  Metal or a GPU carries such a marker or says what is missing; a link to a
  design draft sits in a paragraph that says it is a draft.

python3 psionic/tests/pytorch_matrix.py --write regenerates both copies.
"""
import json
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[2]
CHAPTER = "psionic/book/ch250-can-someone-use-psionic-instead-of-pytorch.md"
MATRIX = "psionic/fixtures/pytorch/matrix.txt"
WORKFLOWS = "psionic/fixtures/pytorch/workflows.json"
PARITY = "psionic/fixtures/pytorch/parity.json"
AUDIT = "psionic/lib/pytorch-audit.coax"
LEVELS = ["none", "eager", "graph", "trained"]
RUNNABLE = ("implemented", "partly implemented")
LEDGER = ("psionic.autodiff.current", "psionic.execution.current", "psionic.gpu.current")
MARK = f"<!-- generated: {MATRIX} -->"
NEGATION = re.compile(r"\b(not|no|none|only|remain|remains|missing|absent|refused|refuses|yet)\b", re.I)


def read(name):
    return (ROOT / name).read_text()


def registry():
    return {c["number"]: c for c in json.loads(read("psionic/book/chapters.json"))["chapters"]}


def constant(text, name, kind):
    found = re.search(name + r" :: \[PA_OPS\]" + kind + r"\.\[(.*?)\];", text, re.S)
    assert found, f"{AUDIT}: no {name}"
    body = found.group(1)
    return re.findall(r'"([^"]*)"', body) if kind == "string" else [int(n) for n in re.findall(r"-?\d+", body)]


def scalar(path, name):
    found = re.search(r"^" + name + r" :: (\d+);", read(path), re.M)
    assert found, f"{path}: no {name}"
    return int(found.group(1))


def operators():
    text = read(AUDIT)
    rows = list(zip(constant(text, "PA_NAME", "string"), constant(text, "PA_LEVEL", "int"),
                    constant(text, "PA_CHAPTER", "int"), constant(text, "PA_PSIONIC", "string")))
    assert len(rows) == scalar(AUDIT, "PA_OPS"), "operator table length differs from PA_OPS"
    return rows


def field(data, key):
    for part in key.split("."):
        data = data[part]
    return data


def generate():
    chapters = registry()
    own = chapters[250]
    ledger = {c["id"]: c for c in json.loads(read("coax/book/capability-claims.json"))["claims"]}
    rows = operators()
    flows = json.loads(read(WORKFLOWS))["workflows"]
    parity = {r["model"]: r for r in json.loads(read(PARITY))["results"]}
    out = [f"Chapters chapter 250 depends on (psionic/book/chapters.json): {len(own['depends_on'])}"]
    for n in own["depends_on"]:
        c = chapters[n]
        out.append(f"  {n:>3}  {c['status']:<18}  {','.join(c['qualified_targets']) or '-':<14}  {c['profile'] or '-'}")
    counts = {s: sum(1 for n in own["depends_on"] if chapters[n]["status"] == s) for s in ("implemented", "partly implemented", "design draft")}
    out.append(f"  implemented {counts['implemented']}, partly implemented {counts['partly implemented']}, design draft {counts['design draft']}")
    out.append("")
    out.append("Limits in the capability claims ledger (coax/book/capability-claims.json):")
    for key in LEDGER:
        out.append(f"  {key}: {ledger[key]['scope']}")
    out.append("")
    tally = {level: sum(1 for r in rows if r[1] == i) for i, level in enumerate(LEVELS)}
    out.append(f"PyTorch operators ({AUDIT}): {len(rows)}; trained {tally['trained']}, graph {tally['graph']}, eager only {tally['eager']}, absent {tally['none']}")
    for name, level, chapter, psionic in rows:
        where = f"chapter {chapter}, {psionic}" if level else "-"
        out.append(f"  {name:<28}  {LEVELS[level]:<7}  {where}")
    out.append("")
    out.append(f"Workflows ({WORKFLOWS}): {len(flows)}")
    for i, w in enumerate(flows, 1):
        status = ", ".join(f"{n} {chapters[n]['status']}" for n in w["chapters"])
        out.append(f"{i}. {w['name']}")
        out.append(f"   PyTorch: {w['pytorch']}")
        out.append(f"   chapters: {status}; targets: {', '.join(w['targets'])}")
        for e in w["evidence"]:
            if "line" in e:
                out.append(f"   {e['path']}: {e['line'].strip()}")
            else:
                out.append(f"   {e['path']}: {e['key']} = {e['value']}")
        if "parity" in w:
            p = parity[w["parity"]]
            out.append(f"   PyTorch parity: {p['argmax_agree']} of {p['rows']} predictions agree, largest score gap {p['max_abs_gap']:.3g}")
        for gap in w["gaps"]:
            out.append(f"   not yet: {gap}")
    return "\n".join(out) + "\n"


def block(text):
    """The generated block in the chapter: its body, or None."""
    found = re.search(re.escape(MARK) + r"\n```text\n(.*?)```\n", text, re.S)
    return found.group(1) if found else None


def paragraphs(text):
    """Prose paragraphs outside fences, with their line text."""
    out, current, fenced = [], [], False
    for line in text.splitlines():
        if line.startswith("```"):
            fenced = not fenced
            continue
        if fenced:
            continue
        if line.strip():
            current.append(line)
        elif current:
            out.append(" ".join(current))
            current = []
    if current:
        out.append(" ".join(current))
    return out


def problems():
    errors = []
    chapters = registry()
    expected = generate()
    if read(MATRIX) != expected:
        errors.append(f"{MATRIX} differs from the registry; run {Path(__file__).name} --write")
    text = read(CHAPTER)
    if block(text) != expected:
        errors.append(f"{CHAPTER}: the generated matrix block is missing or differs")

    def runnable(n, target=None):
        c = chapters.get(n)
        return c is not None and c["status"] in RUNNABLE and (target is None or target in c["qualified_targets"])

    # Operator rows against the registry and the code they name.
    trained_src = read("psionic/lib/mnist-classifier.coax") + read("psionic/lib/cnn-classifier.coax")
    graph_src = read("psionic/lib/module.coax") + read("psionic/lib/module-cnn.coax")
    ti_ops = re.search(r"TiOp :: enum u8 \{(.*?)\}", read("psionic/lib/tensor-ir.coax")).group(1)
    ti_ops = {op.split("=")[0] for op in ti_ops.split(";") if op}
    eager = {o["name"] for o in json.loads(read("psionic/fixtures/tensor-ops/operators.json"))["operators"]}
    for name, level, chapter, psionic in operators():
        if not 0 <= level <= 3:
            errors.append(f"{name}: level {level}")
        elif level == 0:
            if chapter or psionic:
                errors.append(f"{name}: an absent operator cites chapter {chapter} or {psionic!r}")
        elif not runnable(chapter):
            errors.append(f"{name}: chapter {chapter} is not runnable in the registry")
        elif level == 3 and (chapter != 118 or psionic + "(" not in trained_src):
            errors.append(f"{name}: {psionic} is not in a recorded chapter 118 run's model")
        elif level == 2 and psionic not in ti_ops and not re.search(r"^" + psionic + r" ::", graph_src, re.M):
            errors.append(f"{name}: {psionic} is neither a TiOp nor a module constructor")
        elif level == 1 and psionic not in eager:
            errors.append(f"{name}: {psionic} is not in chapter 5's operator manifest")
    for mine, path, theirs in (("PA_PARAMETERS", "psionic/lib/checkpoint/record.coax", "CK_VALUES"),
                               ("PA_NODES", "psionic/lib/tensor-ir.coax", "TI_NODES"),
                               ("PA_ROWS", "psionic/lib/trainer.coax", "TR_EXAMPLES")):
        if scalar(AUDIT, mine) != scalar(path, theirs):
            errors.append(f"{mine} differs from {theirs} in {path}")
    for run in ("mnist", "cnn"):
        if json.loads(read(f"psionic/fixtures/{run}/run-v1/result.json"))["status"] != 0:
            errors.append(f"chapter 118's {run} run did not pass")

    # Workflows: runnable chapters, qualified targets, evidence present.
    parity = {r["model"]: r for r in json.loads(read(PARITY))["results"]}
    runs = {"chapter 118 MNIST MLP": "mnist", "chapter 118 MNIST CNN": "cnn"}
    for w in json.loads(read(WORKFLOWS))["workflows"]:
        for n in w["chapters"]:
            if not runnable(n):
                errors.append(f"{w['name']}: chapter {n} is not runnable")
        for t in w["targets"]:
            if not any(runnable(n, t) for n in w["chapters"]):
                errors.append(f"{w['name']}: no cited chapter qualified {t}")
        for e in w["evidence"]:
            if "line" in e:
                if e["line"] not in read(e["path"]).splitlines():
                    errors.append(f"{w['name']}: no line {e['line']!r} in {e['path']}")
            elif field(json.loads(read(e["path"])), e["key"]) != e["value"]:
                errors.append(f"{w['name']}: {e['path']} {e['key']} is not {e['value']}")
        if "parity" in w and w["parity"] not in parity:
            errors.append(f"{w['name']}: no parity record {w['parity']}")
    for model, run in runs.items():
        p = parity[model]
        result = json.loads(read(f"psionic/fixtures/{run}/run-v1/result.json"))
        if p["psionic_correct"] != result["final_correct"] or p["rows"] != result["test_rows"]:
            errors.append(f"{PARITY}: {model} disagrees with its run record")

    # Prose claims in chapter 250.
    for para in paragraphs(text):
        claims = re.findall(r"<!-- claim: (\d+) ([a-z]+) -->", para)
        for n, target in claims:
            if not runnable(int(n), target):
                errors.append(f"{CHAPTER}: claim {n} {target} is not in the registry")
        targets = {t for _, t in claims}
        for word, target in (("CUDA", "cuda"), ("Metal", "metal"), ("GPU", "cuda")):
            if re.search(r"\b" + word + r"\b", para) and target not in targets and not NEGATION.search(para):
                errors.append(f"{CHAPTER}: names {word} without a claim marker or a stated gap: {para[:80]}")
        for link in re.findall(r"\((ch(\d{3})-[a-z0-9-]+\.md)\)", para):
            c = chapters.get(int(link[1]))
            if c and c["status"] == "design draft" and "draft" not in para:
                errors.append(f"{CHAPTER}: links design draft {link[0]} without saying so")
    return errors


def write():
    expected = generate()
    (ROOT / MATRIX).write_text(expected)
    text = read(CHAPTER)
    old = block(text)
    assert old is not None, f"{CHAPTER}: add {MARK} followed by an empty text fence"
    (ROOT / CHAPTER).write_text(text.replace(MARK + "\n```text\n" + old + "```\n", MARK + "\n```text\n" + expected + "```\n", 1))


if __name__ == "__main__":
    if sys.argv[1:] == ["--write"]:
        write()
    errors = problems()
    for e in errors:
        print(e)
    sys.exit(1 if errors else 0)
