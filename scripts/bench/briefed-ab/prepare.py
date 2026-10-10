#!/usr/bin/env python3
"""Turn a closed issue and the commit that fixed it into a bench task (#11211).

    prepare.py ISSUE:COMMIT ... [--validate]

For each pair it writes `tasks/ISSUE.json`: the issue text (title and body,
no comments), the fix commit and its parent, the package, the fix's files
split into source and tests, the fix's own test changes as `tests.patch`
(whole test files and the hunks inside a source file's `#[cfg(test)]`
module), and the names of the tests the fix added.

`--validate` grades the parent and the fix on the build host: a usable
task's tests pass on the fix, and fail (or do not compile) on the parent.
"""

from __future__ import annotations

import argparse
import io
import json
import os
import re
import sys
import subprocess
import tarfile

from common import GH_REPO, HERE, REPO, SSH, dump_json, git, is_test_file, load_json, package_of, run, show

TASKS = HERE / "tasks"


def split_diff(diff: str):
    """Yield (path, header, [hunks]) per file of a unified diff."""
    files = re.split(r"(?m)^(?=diff --git )", diff)
    for block in files:
        if not block.startswith("diff --git"):
            continue
        m = re.match(r"diff --git a/(\S+) b/(\S+)", block)
        path = m.group(2)
        parts = re.split(r"(?m)^(?=@@ )", block)
        yield path, parts[0], parts[1:]


def test_region_start(text: str | None) -> int | None:
    if not text:
        return None
    for i, line in enumerate(text.splitlines(), 1):
        if re.match(r"\s*#\[cfg\(test\)\]", line):
            rest = "\n".join(text.splitlines()[i - 1 : i + 3])
            if re.search(r"\bmod\s+\w+", rest):
                return i
    return None


def added_tests(hunks: list[str]) -> list[str]:
    names = []
    for hunk in hunks:
        lines = hunk.splitlines()
        for i, line in enumerate(lines):
            if line.startswith("+") and re.match(r"\+\s*#\[(tokio::)?test", line):
                for nxt in lines[i + 1 : i + 6]:
                    m = re.match(r"\+\s*(?:pub\s+)?(?:async\s+)?fn\s+(\w+)", nxt)
                    if m:
                        names.append(m.group(1))
                        break
    return names


def prepare(issue: int, commit: str) -> dict:
    fix = git("rev-parse", commit).strip()
    parent = git("rev-parse", f"{fix}^").strip()
    meta = json.loads(
        run(["gh", "issue", "view", str(issue), "-R", GH_REPO, "--json", "title,body,state"]).stdout
    )
    diff = git("diff", "--full-index", "--binary", parent, fix)
    source_files, test_files, tests_patch, source_patch, names = [], [], [], [], []
    for path, header, hunks in split_diff(diff):
        if path.endswith(".md") or path.endswith("Cargo.lock"):
            source_patch.append(header + "".join(hunks))
            if path.endswith(".md"):
                source_files.append(path)
            continue
        if is_test_file(path):
            test_files.append(path)
            tests_patch.append(header + "".join(hunks))
            names += added_tests(hunks)
            continue
        start = test_region_start(show(parent, path))
        src_h, test_h = [], []
        for hunk in hunks:
            m = re.match(r"@@ -(\d+)", hunk)
            old = int(m.group(1)) if m else 0
            adds_module = re.search(r"(?m)^\+\s*#\[cfg\(test\)\]", hunk)
            if (start is not None and old >= start) or (start is None and adds_module):
                test_h.append(hunk)
            else:
                src_h.append(hunk)
        if src_h:
            source_files.append(path)
            source_patch.append(header + "".join(src_h))
        if test_h:
            test_files.append(path)
            tests_patch.append(header + "".join(test_h))
            names += added_tests(test_h)
    crate_files = [p for p in test_files + source_files if p.startswith("crates/")]
    package = package_of(parent, crate_files[0]) if crate_files else None
    targets: list[str] = []
    for path in sorted(set(test_files)):
        m = re.match(r"(crates/[^/]+)/(.*)", path)
        if not m:
            continue
        crate_dir, rest = m.groups()
        if rest.startswith("tests/"):
            stem = rest.split("/")[1].removesuffix(".rs")
            targets += ["--test", stem]
        elif rest.startswith("src/bin/"):
            targets += ["--bin", rest.split("/")[2].removesuffix(".rs")]
        elif show(parent, f"{crate_dir}/src/lib.rs") is not None and rest != "src/main.rs":
            targets.append("--lib")
        else:
            targets.append("--bins")
    dedup: list[str] = []
    i = 0
    while i < len(targets):
        item = targets[i : i + 2] if targets[i] in ("--test", "--bin") else targets[i : i + 1]
        joined = " ".join(item)
        if joined not in [" ".join(x) for x in [dedup[j:j+2] for j in range(len(dedup))]]:
            dedup += item
        i += len(item)
    task = {
        "test_targets": dedup,
        "issue": issue,
        "title": meta["title"],
        "body": meta["body"],
        "fix": fix,
        "parent": parent,
        "package": package,
        "source_files": sorted(set(source_files)),
        "test_files": sorted(set(test_files)),
        "test_names": sorted(set(names)),
        "fix_lines": sum(
            1 for line in diff.splitlines() if line[:1] in "+-" and line[:3] not in ("+++", "---")
        ),
    }
    out = TASKS / str(issue)
    out.mkdir(parents=True, exist_ok=True)
    (out / "tests.patch").write_text("".join(tests_patch))
    (out / "source.patch").write_text("".join(source_patch))
    dump_json(out / "task.json", task)
    return task


def interface(task: dict) -> list[dict]:
    """The new items the fix's own tests call: names the tests use that do
    not exist at the parent but that the fix defines, with their
    signatures. Telling every arm about them (as SWE-bench Pro does) keeps
    the fix's tests a fair check of behavior, not of naming."""
    out_dir = TASKS / str(task["issue"])
    tests = (out_dir / "tests.patch").read_text()
    source = (out_dir / "source.patch").read_text()
    used = set()
    for line in tests.splitlines():
        if line.startswith("+") and not line.startswith("+++"):
            used |= set(re.findall(r"\b([A-Za-z_][A-Za-z0-9_]{3,})\b", line))
    defs = []
    current = None
    lines = source.splitlines()
    for i, line in enumerate(lines):
        if line.startswith("+++ b/"):
            current = line[6:]
        if not line.startswith("+") or line.startswith("+++"):
            continue
        m = re.match(r"\+\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?(?:const\s+)?(fn|struct|enum|const|static|type|trait)\s+([A-Za-z_][A-Za-z0-9_]*)", line)
        if m and m.group(2) in used:
            sig = [line[1:].strip()]
            j = i + 1
            while not re.search(r"[{;]\s*$", sig[-1]) and j < len(lines) and lines[j].startswith("+") and j - i < 12:
                sig.append(lines[j][1:].strip())
                j += 1
            text = " ".join(sig).rstrip("{").strip().replace("( ", "(").replace(", )", ")")
            defs.append((m.group(2), text, current))
    found = []
    for name, sig, path in defs:
        hits = subprocess.run(["git", "grep", "-q", "-w", "-e", name, task["parent"], "--", "crates/"],
                              cwd=REPO)
        if hits.returncode != 0:  # absent at the parent: new
            found.append({"name": name, "signature": sig, "file": path})
    seen, out = set(), []
    for item in found:
        if item["name"] not in seen:
            seen.add(item["name"])
            out.append(item)
    return out


def interface_text(task: dict) -> str:
    items = interface(task)
    if not items:
        return ""
    rows = "\n".join(f"{i['signature']}    // {i['file']}" for i in items)
    return ("\n\nThe tests that will check this change call these new items, so define them with "
            "these names and signatures:\n\n```rust\n" + rows + "\n```\n")


def grade(task: dict, change: str, slot: int = 0, limit: int = 1500) -> dict:
    """Grade `change` (a patch on the parent) on the build host."""
    out = TASKS / str(task["issue"])
    buf = io.BytesIO()
    with tarfile.open(fileobj=buf, mode="w") as tar:
        for name, data in (
            ("change.patch", change),
            ("tests.patch", (out / "tests.patch").read_text()),
            ("names.txt", "\n".join(task["test_names"]) + ("\n" if task["test_names"] else "")),
            ("targets.txt", "\n".join(task.get("test_targets", [])) + ("\n" if task.get("test_targets") else "")),
        ):
            raw = data.encode()
            info = tarfile.TarInfo(name)
            info.size = len(raw)
            tar.addfile(info, io.BytesIO(raw))
    proc = run(
        SSH + [f"AB_BUILD={os.environ.get('AB_BUILD', '')} bash ~/ab/bin/eval.sh {slot} {task['parent']} {task['package']} {limit}"],
        input=buf.getvalue(),
        check=False,
        timeout=limit * 2 + 600,
    )
    stdout = proc.stdout.decode(errors="replace")
    last = stdout.strip().splitlines()[-1] if stdout.strip() else ""
    try:
        return json.loads(last)
    except json.JSONDecodeError:
        return {"error": stdout[-1500:] + proc.stderr.decode(errors="replace")[-1500:]}


def validate(task: dict, slot: int = 0) -> dict:
    out = TASKS / str(task["issue"])
    on_fix = grade(task, (out / "source.patch").read_text(), slot)
    on_parent = grade(task, "", slot)
    verdict = {
        "on_fix": on_fix,
        "on_parent": on_parent,
        "usable": bool(
            task["test_names"]
            and on_fix.get("tests_pass")
            and not on_parent.get("tests_pass")
        ),
        "behavioral": bool(on_parent.get("tests_compiled")),
    }
    dump_json(out / "validation.json", verdict)
    return verdict


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("pairs", nargs="+", help="ISSUE:COMMIT")
    ap.add_argument("--validate", action="store_true")
    ap.add_argument("--slot", type=int, default=0)
    args = ap.parse_args()
    for pair in args.pairs:
        issue, commit = pair.split(":")
        try:
            task = prepare(int(issue), commit)
        except Exception as error:  # noqa: BLE001 - report and go on
            print(f"{issue}: {error}", file=sys.stderr)
            continue
        line = f"{issue} {task['package']} src={len(task['source_files'])} tests={task['test_names']}"
        if args.validate and (TASKS / issue / "validation.json").exists():
            v = load_json(TASKS / issue / "validation.json")
            print(f"{line} usable={v['usable']} (validated before)", flush=True)
            continue
        if args.validate and task["package"] and task["test_names"]:
            v = validate(task, args.slot)
            line += f" usable={v['usable']} behavioral={v['behavioral']}"
            line += f" fix={v['on_fix'].get('tests_pass')} parent={v['on_parent'].get('tests_pass')}"
        print(line, flush=True)


if __name__ == "__main__":
    main()
