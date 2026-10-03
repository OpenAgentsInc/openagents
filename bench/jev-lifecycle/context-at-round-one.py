#!/usr/bin/env python3
"""Assemble source-bound candidate pools for the separate Jev lifecycle pilot."""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
import math
from pathlib import Path, PurePosixPath
import re
import subprocess
import time
import tomllib

SCHEMA = "openagents.jev-lifecycle.context.v1"
PACK_BYTES = 16 * 1024
STATE_BYTES = 28 * 1024
MAX_CANDIDATES = 32
MAX_FILES = 48
MAX_CATALOG = 64
UNIT_BYTES = 6000
SLICE_BYTES = 2400
ROLES = ("implementation", "test", "document")
KINDS = {"function_item", "struct_item", "enum_item", "const_item", "static_item", "type_item"}
STOP = set("the and for with from that this into when then have has are was were will should must can not only each any all code source file issue task fix use used run tests test".split())


def encoded(value):
    return json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(",", ":")).encode()


def digest(value):
    return hashlib.sha256(value).hexdigest()


def git(repo, *args):
    return subprocess.check_output(["git", "--literal-pathspecs", "-C", str(repo), *args])


def safe_path(value):
    path = PurePosixPath(value)
    if (not isinstance(value, str) or not value or len(value.encode()) > 1024
            or path.is_absolute() or ".." in path.parts or ".git" in path.parts
            or str(path) != value or any(ord(c) < 32 for c in value)):
        raise ValueError("Use a bounded repository-relative path")
    return value


def tree(repo, rev):
    if not re.fullmatch(r"[0-9a-f]{40}", rev):
        raise ValueError("Supply a full immutable source commit")
    if git(repo, "rev-parse", rev + "^{commit}").decode().strip() != rev:
        raise ValueError("The source commit did not resolve exactly")
    rows = {}
    for item in git(repo, "ls-tree", "-r", "-z", rev).split(b"\0"):
        if item:
            metadata, path = item.split(b"\t", 1)
            mode, kind, blob = metadata.decode().split()
            if mode in ("100644", "100755") and kind == "blob":
                rows[path.decode()] = blob
    return rows


def read_blob(repo, path, bindings):
    safe_path(path)
    if path not in bindings:
        raise ValueError("The source path is not a regular file at the pinned commit")
    blob = bindings[path]
    # Bound allocations before reading; source slices do not follow symlinks.
    size = int(git(repo, "cat-file", "-s", blob))
    if size > 2 * 1024 * 1024:
        raise ValueError("The source file exceeds the 2 MiB read bound")
    raw = git(repo, "cat-file", "blob", blob)
    if len(raw) != size:
        raise ValueError("The source blob size changed")
    raw.decode("utf-8")
    return raw


def slice_lines(lines, start, stop, limit):
    selected, size = [], 0
    for line in lines[start - 1:stop]:
        length = len(line.encode())
        if size + length > limit:
            break
        selected.append(line)
        size += length
    return "".join(selected), start + len(selected) - 1


def read_source(repo, rev, path, start_line=1, max_bytes=4096):
    """Read complete lines from a pinned regular file without a checkout."""
    started = time.monotonic()
    if type(start_line) is not int or start_line < 1 or type(max_bytes) is not int or not 1 <= max_bytes <= PACK_BYTES:
        raise ValueError("Invalid source read bounds")
    bindings = tree(repo, rev)
    raw = read_blob(repo, path, bindings)
    lines = raw.decode().splitlines(keepends=True)
    if start_line > max(1, len(lines)):
        raise ValueError("The requested line is outside the source file")
    text, end = slice_lines(lines, start_line, len(lines), max_bytes)
    return {"source_commit": rev, "path": path, "blob": bindings[path],
            "file_sha256": digest(raw), "file_bytes": len(raw), "total_lines": len(lines),
            "start_line": start_line, "end_line": end if text else None,
            "text": text, "source_sha256": digest(text.encode()),
            "truncated": start_line > 1 or end < len(lines),
            "reason": "line_exceeds_read_budget" if not text and lines else "bounded_source_read",
            "wall_s": time.monotonic() - started}


def public_task(value):
    """Copy only public task fields; never propagate evaluator configuration."""
    title = value.get("title", "")
    prompt = value.get("prompt", value.get("body", "")) or ""
    if not isinstance(title, str) or not isinstance(prompt, str) or not (title or prompt):
        raise ValueError("The public task needs a title or prompt")
    if len((title + prompt).encode()) > 16 * 1024:
        raise ValueError("The complete public task exceeds the 16 KiB bound")
    result = {"id": str(value.get("id", value.get("number", "task"))), "title": title, "prompt": prompt}
    for key in ("packages", "allowed_paths"):
        values = value.get(key, [])
        if not isinstance(values, list) or len(values) > 32 or any(not isinstance(v, str) for v in values):
            raise ValueError("Invalid public task scope")
        result[key] = values
    for name in result["packages"]:
        if not re.fullmatch(r"[A-Za-z0-9_-]{1,96}", name):
            raise ValueError("Invalid package name")
    result["allowed_paths"] = [safe_path(v.rstrip("/")) + "/" for v in result["allowed_paths"]]
    readings = value.get("required_public_readings", [])
    if not isinstance(readings, list) or len(readings) > 16:
        raise ValueError("Invalid public reading list")
    result["required_public_readings"] = [safe_path(v["path"]) for v in readings]
    return result


def terms(text):
    return set(re.findall(r"[a-z][a-z0-9]{2,}", re.sub(r"([a-z])([A-Z])", r"\1 \2", text).lower())) - STOP


def file_role(path):
    if not path.endswith(".rs"):
        return "document"
    pieces = PurePosixPath(path).parts
    return "test" if ("tests" in pieces or "benches" in pieces or pieces[-1] in ("tests.rs", "test.rs")) else "implementation"


def package_roots(repo, task, bindings):
    roots = {v.rstrip("/") for v in task["allowed_paths"]}
    missing = set(task["packages"])
    # Try conventional and explicitly scoped manifests first; use exact names.
    possible = {f"crates/{name}/Cargo.toml" for name in missing}
    possible.update(root + "/Cargo.toml" for root in roots)
    ordered = sorted(possible & bindings.keys())
    ordered += sorted(p for p in bindings if p.endswith("Cargo.toml") and p not in possible)
    for path in ordered:
        if not missing:
            break
        data = tomllib.loads(read_blob(repo, path, bindings).decode())
        name = data.get("package", {}).get("name")
        if name in missing:
            roots.add(str(PurePosixPath(path).parent))
            missing.remove(name)
    return sorted(roots), sorted(missing)


def within(path, root):
    return root == "." or path == root or path.startswith(root + "/")


def round_robin(groups):
    result = []
    queues = {role: list(groups.get(role, [])) for role in ROLES}
    while any(queues.values()):
        for role in ROLES:
            if queues[role]:
                result.append(queues[role].pop(0))
    return result


def assemble(repo, rev, index, task, *, state_bytes=STATE_BYTES, max_candidates=MAX_CANDIDATES):
    """Build a deterministic pool before either lexical or semantic packing."""
    started = time.monotonic()
    if not 4096 <= state_bytes <= 64 * 1024 or not 1 <= max_candidates <= 48:
        raise ValueError("Invalid candidate pool bounds")
    if index.get("commit") != rev or index.get("syntax", {}).get("extractor_version") != "briefing-lab-rust-v1":
        raise ValueError("Use the syntax index for the exact source commit")
    if task.get("source_commit", rev) != rev:
        raise ValueError("The public task and source commit differ")
    task = public_task(task)
    if len(encoded({"task": task, "candidates": []})) > state_bytes:
        raise ValueError("The complete public task exceeds the state budget")
    bindings = tree(repo, rev)
    request = task["title"] + "\n" + task["prompt"]
    query, title = terms(request), terms(task["title"])
    roots, missing_packages = package_roots(repo, task, bindings)
    entries = {entry["path"]: entry for entry in index["files"]}
    if len(entries) != len(index["files"]):
        raise ValueError("The source index has duplicate paths")
    explicit = {p for p in entries if p in request} | set(task["required_public_readings"])
    # Docs or root Cargo anchors do not admit an entire workspace.
    anchored_roots = set(roots)
    for path in explicit:
        if path.endswith(".rs"):
            parent = PurePosixPath(path).parent
            while str(parent) != ".":
                if str(parent / "Cargo.toml") in bindings:
                    anchored_roots.add(str(parent))
                    break
                parent = parent.parent
    names = set(re.findall(r"`([A-Za-z_][A-Za-z_0-9:]*)`", request))

    def overlap(text):
        found = terms(text) & query
        return sum(3 if term in title else 1 for term in sorted(found))

    files = []
    for path, entry in entries.items():
        rust = path.endswith(".rs") and bool(entry.get("syntax"))
        document = path in explicit and path.endswith((".md", ".toml"))
        if not (rust or document) or path not in bindings:
            continue
        scoped = any(within(path, root) for root in anchored_roots)
        if not scoped and path not in explicit:
            continue
        if entry.get("blob") != bindings[path]:
            raise ValueError("The index path-to-blob binding differs from the source")
        score = 1000 * int(path in explicit) + 10 * overlap(path) + overlap(" ".join(entry.get("terms", [])))
        files.append({"path": path, "entry": entry, "role": file_role(path), "score": score,
                      "reasons": (["explicit_file"] if path in explicit else []) + (["package_scope"] if scoped else [])})
    # Required readings may not belong to the text index; retain explicit absence.
    missing_paths = sorted(path for path in explicit if path not in {f["path"] for f in files})
    by_role = {role: sorted((f for f in files if f["role"] == role), key=lambda f: (-f["score"], f["path"])) for role in ROLES}
    ordered_files = round_robin(by_role)
    admitted_files, file_omissions = ordered_files[:MAX_FILES], ordered_files[MAX_FILES:]
    units, oversized, parse_errors, unrepresentable = [], [], 0, 0
    catalog_reasons = {f["path"]: "file_admission_budget" for f in file_omissions}
    for f in admitted_files:
        path, entry = f["path"], f["entry"]
        raw = read_blob(repo, path, bindings)
        if len(raw) != entry.get("size") or digest(raw) != entry.get("sha256"):
            raise ValueError("The indexed source bytes differ from the pinned blob")
        lines = raw.decode().splitlines(keepends=True)
        declarations = entry.get("syntax", {}).get("declarations", []) if path.endswith(".rs") else [{"kind": "document", "name": PurePosixPath(path).name, "qualified_name": path, "declaration": {"start_line": 1, "end_line": len(lines)}}]
        for declaration in declarations:
            if declaration.get("parse_has_error"):
                parse_errors += 1
                continue
            if declaration["kind"] not in KINDS | {"document"}:
                continue
            start, end = (declaration["declaration"][k] for k in ("start_line", "end_line"))
            if not 1 <= start <= end <= len(lines):
                raise ValueError("The indexed declaration range is invalid")
            while start > 1 and lines[start - 2].lstrip().startswith(("#[", "///", "//!")):
                start -= 1
            body = "".join(lines[start - 1:end])
            role = f["role"]
            if role == "implementation" and ("tests::" in declaration["qualified_name"] or re.search(r"#\[(?:\w+::)?test(?:\]|\()", body[:500])):
                role = "test"
            score = f["score"] + 10 * overlap(declaration["qualified_name"]) + overlap(body) + 100 * int(declaration["name"] in names or declaration["qualified_name"] in names)
            full_start, full_end = start, end
            completeness = "complete_declaration" if role != "document" else "complete_file"
            if len(body.encode()) > UNIT_BYTES:
                oversized.append({"path": path, "name": declaration["qualified_name"], "start_line": start, "end_line": end, "bytes": len(body.encode())})
                # Select a line window by public query overlap, never by a solution.
                anchor = min(range(start, end + 1), key=lambda n: (-overlap(lines[n - 1]), n))
                start = max(full_start, anchor - 2)
                body, end = slice_lines(lines, start, full_end, SLICE_BYTES)
                completeness = "partial_declaration" if role != "document" else "partial_file"
                if not body:
                    unrepresentable += 1
                    end = None
                    completeness = "targeted_read_only"
            units.append({"path": path, "blob": bindings[path], "file_sha256": digest(raw),
                          "name": declaration["qualified_name"], "role": role, "kind": declaration["kind"],
                          "start_line": start, "end_line": end, "full_start_line": full_start, "full_end_line": full_end,
                          "text": body, "source_sha256": digest(body.encode()), "completeness": completeness,
                          "admission_reasons": f["reasons"] + (["oversized_declaration_read_pointer"] if completeness.startswith("partial") or completeness == "targeted_read_only" else []),
                          "baseline_score": score})
        if not any(u["path"] == path for u in units):
            catalog_reasons[path] = "no_eligible_declaration"
    # Give each file's strongest unit a turn before its lower-ranked units.
    groups = {}
    for role in ROLES:
        ordered = sorted((u for u in units if u["role"] == role), key=lambda u: (-u["baseline_score"], u["path"], u["full_start_line"], u["name"]))
        first, rest, seen = [], [], set()
        for row in ordered:
            (rest if row["path"] in seen else first).append(row)
            seen.add(row["path"])
        groups[role] = first + rest
    pool, omitted = [], []
    for row in round_robin(groups):
        candidate = {"id": f"c{len(pool) + 1:02d}", **row}
        reason = "candidate_count_budget" if len(pool) >= max_candidates else "candidate_state_budget"
        if len(pool) < max_candidates and len(encoded({"task": task, "candidates": pool + [candidate]})) <= state_bytes:
            pool.append(candidate)
        else:
            omitted.append({"path": row["path"], "name": row["name"], "start_line": row["full_start_line"], "end_line": row["full_end_line"], "reason": reason})
            catalog_reasons.setdefault(row["path"], reason)
    for row in pool:
        if row["completeness"] != "complete_declaration" and row["completeness"] != "complete_file":
            catalog_reasons.setdefault(row["path"], "partial_unit_needs_targeted_read")
    catalog = []
    for f in ordered_files:
        path = f["path"]
        if path not in catalog_reasons:
            continue
        pending = next((r for r in omitted if r["path"] == path), None)
        partial = next((r for r in pool if r["path"] == path and r["completeness"].startswith("partial")), None)
        suggested = pending["start_line"] if pending else partial["full_start_line"] if partial else 1
        catalog.append({"id": f"f{len(catalog) + 1:02d}", "path": path, "blob": f["entry"]["blob"],
                        "file_sha256": f["entry"]["sha256"], "bytes": f["entry"]["size"], "role": f["role"],
                        "reason": catalog_reasons[path], "baseline_score": f["score"], "suggested_start_line": suggested})
    coverage = {"scoped_files": len(files), "files_read": len(admitted_files), "file_omissions": len(file_omissions),
                "eligible_units": len(units), "candidate_units": len(pool), "candidate_roles": dict(Counter(r["role"] for r in pool)),
                "omitted_units": len(omitted), "omission_reasons": dict(Counter(r["reason"] for r in omitted)),
                "oversized_units": len(oversized), "targeted_read_only_units": unrepresentable, "parse_error_units": parse_errors,
                "catalog_total": len(catalog), "catalog_omitted": max(0, len(catalog) - MAX_CATALOG),
                "missing_packages": missing_packages, "unavailable_explicit_paths": missing_paths,
                "complete_dependency_coverage_claimed": False}
    result = {"schema": SCHEMA, "source_commit": rev, "task": task, "candidates": pool,
              "catalog": catalog[:MAX_CATALOG], "coverage": coverage, "oversized": oversized,
              "omissions": omitted, "limits": {"candidate_state_bytes": state_bytes, "max_candidates": max_candidates,
              "max_files": MAX_FILES, "pack_bytes": PACK_BYTES, "unit_bytes": UNIT_BYTES, "slice_bytes": SLICE_BYTES},
              "instruction_policy": "Optional source evidence only; supply complete applicable instructions separately.",
              "index_sha256": digest(encoded(index)), "candidate_pool_sha256": digest(encoded(pool)),
              "candidate_state_bytes": len(encoded({"task": task, "candidates": pool}))}
    result["wall_s"] = time.monotonic() - started
    return result


def pack(context, scores=None, budget=PACK_BYTES):
    """Pack the same candidate pool under either deterministic or supplied ranks."""
    started = time.monotonic()
    if type(budget) is not int or not 512 <= budget <= PACK_BYTES:
        raise ValueError("The pack budget must be between 512 and 16384 bytes")
    rows = context["candidates"]
    if context.get("candidate_pool_sha256") != digest(encoded(rows)):
        raise ValueError("The candidate pool changed")
    if scores is not None and (set(scores) != {r["id"] for r in rows} or any(type(x) not in (int, float) or not math.isfinite(x) for x in scores.values())):
        raise ValueError("Supply one finite score for every candidate ID")
    ordered = sorted(rows, key=lambda r: (-(scores[r["id"]] if scores is not None else r["baseline_score"]), -r["baseline_score"], r["path"], r["start_line"], r["id"]))
    payload = ("## Prepared source evidence\n\n" + f"Source commit: `{context['source_commit']}`. "
               "Selections may omit requirements, dependencies, fixtures, and callers. Partial units are labeled; inspect their full spans before relying on completeness. Complete applicable instructions are supplied separately.\n")
    selected, omissions = [], []
    for row in ordered:
        if any(row["path"] == r["path"] and row["end_line"] is not None and r["end_line"] is not None and not (row["end_line"] < r["start_line"] or row["start_line"] > r["end_line"]) for r in selected):
            omissions.append({"id": row["id"], "reason": "overlapping_selected_span"})
            continue
        mark = "`" * max(3, 1 + max((len(m.group()) for m in re.finditer(r"`+", row["text"])), default=0))
        block = (f"\n### {row['id']} {row['path']}:{row['start_line']}-{row['end_line']} — {row['name']}\n\n"
                 f"Role: {row['role']}; {row['completeness']}. Full unit: {row['full_start_line']}-{row['full_end_line']}. "
                 f"Blob: `{row['blob']}`. File SHA-256: `{row['file_sha256']}`.\n\n{mark}text\n{row['text']}"
                 + ("" if row["text"].endswith("\n") else "\n") + f"{mark}\n")
        if len((payload + block).encode()) > budget:
            omissions.append({"id": row["id"], "reason": "pack_byte_budget"})
            continue
        payload += block
        selected.append(row)
    return {"text": payload, "selected_ids": [r["id"] for r in selected], "omissions": omissions,
            "payload_bytes": len(payload.encode()), "sha256": digest(payload.encode()),
            "candidate_pool_sha256": context["candidate_pool_sha256"], "budget": budget,
            "policy": "supplied_scores" if scores is not None else "deterministic", "wall_s": time.monotonic() - started}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("repo", "rev", "index", "task-manifest", "output"):
        parser.add_argument("--" + name, required=True)
    parser.add_argument("--task-id")
    parser.add_argument("--state-bytes", type=int, default=STATE_BYTES)
    args = parser.parse_args()
    source = json.loads(Path(args.task_manifest).read_text())
    if "tasks" in source:
        matches = [t for t in source["tasks"] if args.task_id is None or t.get("id") == args.task_id]
        if len(matches) != 1:
            raise ValueError("Choose exactly one public task")
        source = matches[0]
    context = assemble(args.repo, args.rev, json.loads(Path(args.index).read_text()), source, state_bytes=args.state_bytes)
    baseline = pack(context)
    output = Path(args.output)
    output.mkdir(parents=True, exist_ok=False)
    (output / "context.json").write_bytes(encoded(context) + b"\n")
    (output / "baseline.md").write_bytes(baseline["text"].encode())
    (output / "baseline-pack.json").write_bytes(encoded({k: v for k, v in baseline.items() if k != "text"}) + b"\n")
    print(json.dumps({"candidate_units": len(context["candidates"]), "candidate_state_bytes": context["candidate_state_bytes"], "payload_bytes": baseline["payload_bytes"], "wall_s": context["wall_s"] + baseline["wall_s"]}))


if __name__ == "__main__":
    main()
