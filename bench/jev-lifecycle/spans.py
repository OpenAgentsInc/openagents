#!/usr/bin/env python3
"""Select pinned source pointers before spending a source-text budget."""
from __future__ import annotations

import argparse
from collections import Counter
import json
import math
from pathlib import Path
import re
import time

import context

SCHEMA = "openagents.jev-lifecycle.spans.v1"
CAPS = {"implementation": 64, "test": 16, "document": 8}
STATE_BYTES = 64 * 1024
PACK_BYTES = 16 * 1024
CONTRACT_BYTES = 3072
MAX_READ_POINTERS = 24
MAX_SIGNATURE_FILES = 48
SIGNATURE_BYTES = 512
COMPLETE_BYTES = 6000
SLICE_BYTES = 2400


def clauses(task):
    spans = re.split(r"(?<=[.!?])\s+(?=[A-Z])", task["prompt"].strip())
    if not 1 <= len(spans) <= 24:
        raise ValueError("The complete task must contain at most 24 sentence clauses")
    return {f"r{i + 1:02d}": text for i, text in enumerate(spans)}


def lexical(text, row):
    query = context.terms(text)
    return 4 * len(query & context.terms(row["name"])) + len(query & context.terms(row["path"]))


def identity(value):
    return context.digest(context.encoded({k: v for k, v in value.items() if k not in ("wall_s", "catalog_sha256")}))


def validate(value):
    if value.get("schema") != SCHEMA or value.get("catalog_sha256") != identity(value):
        raise ValueError("The span catalog changed")
    ids = [r["id"] for r in value["candidates"]]
    if len(ids) != len(set(ids)) or len(ids) > sum(CAPS.values()):
        raise ValueError("Invalid catalog candidate identities")



def signature_metadata(raw, declaration):
    """Classify a bounded signature; visibility and operation tiers are heuristics."""
    span = declaration.get("signature")
    outer = declaration["declaration"]
    if not span or not 0 <= outer["start_byte"] <= span["start_byte"] < span["end_byte"] <= outer["end_byte"] <= len(raw):
        raise ValueError("The indexed signature range is invalid")
    fragment = raw[span["start_byte"]:span["end_byte"]].decode()
    public = bool(re.match(r"pub\s", fragment))
    name = declaration["qualified_name"].split("::")[-1]
    constructor = name in ("new", "default", "with_capacity") or name.startswith(("from_", "with_"))
    parameters = fragment.split("(", 1)[-1].rsplit(")", 1)[0]
    parameter = bool(re.search(r"\b(?!self\b)[A-Za-z_]\w*\s*:", parameters))
    operation = (parameter or "async fn" in fragment or "&mut self" in parameters
                 or bool(re.search(r"(?:^|,)\s*(?:mut\s+)?self\s*(?:,|$)", parameters))
                 or bool(re.search(r"->\s*(?:[\w:]+::)?Result\s*<", fragment)))
    tier = 2 if public and operation and not constructor else 1 if public and not constructor else 0
    bounded = fragment.encode()[:SIGNATURE_BYTES].decode("utf-8", errors="ignore")
    return {"signature": bounded, "signature_complete": bounded == fragment,
            "signature_sha256": context.digest(fragment.encode()), "public_api_tier": tier}


def catalog(repo, rev, index, task):
    """Build metadata from pinned signatures; no function bodies enter selection state."""
    started = time.monotonic()
    if index.get("commit") != rev or index.get("syntax", {}).get("extractor_version") != "briefing-lab-rust-v1":
        raise ValueError("Use the syntax index for the exact source commit")
    if task.get("source_commit", rev) != rev:
        raise ValueError("The task and source commit differ")
    task = context.public_task(task)
    split = clauses(task)
    bindings = context.tree(repo, rev)
    roots, missing_packages = context.package_roots(repo, task, bindings)
    entries = {r["path"]: r for r in index["files"]}
    if len(entries) != len(index["files"]):
        raise ValueError("The syntax index has duplicate paths")
    request = task["title"] + "\n" + task["prompt"]
    explicit = {p for p in entries if p in request} | set(task["required_public_readings"])
    rows, omissions = [], []
    signature_paths = [p for p in entries if p.endswith(".rs") and (p in explicit or any(context.within(p, root) for root in roots))]
    signature_paths.sort(key=lambda p: (-int(p in explicit), -len(context.terms(p) & context.terms(request)), p))
    omitted_files = signature_paths[MAX_SIGNATURE_FILES:]
    signature_paths = signature_paths[:MAX_SIGNATURE_FILES]
    for path in signature_paths:
        if bindings.get(path) != entries[path].get("blob"):
            raise ValueError("The index path-to-blob binding differs from the source")
    signature_blobs = context.read_blobs(repo, signature_paths, bindings)
    for path, raw in signature_blobs.items():
        if len(raw) != entries[path]["size"] or context.digest(raw) != entries[path]["sha256"]:
            raise ValueError("The signature file bytes differ from the pinned index")
    omissions.extend({"path": p, "reason": "signature_file_limit"} for p in omitted_files)
    for path, entry in sorted(entries.items()):
        rust = path in signature_blobs
        document = path in explicit and path.endswith((".md", ".toml"))
        if not (rust or document):
            continue
        context.safe_path(path)
        if bindings.get(path) != entry.get("blob"):
            raise ValueError("The index path-to-blob binding differs from the source")
        if not re.fullmatch(r"[0-9a-f]{64}", entry.get("sha256", "")):
            raise ValueError("The index has an invalid file digest")
        declarations = entry.get("syntax", {}).get("declarations", []) if rust else [{"kind": "document", "qualified_name": path, "declaration": {"start_line": 1, "end_line": entry["line_count"], "start_byte": 0, "end_byte": entry["size"]}}]
        for decl in declarations:
            if decl["kind"] not in ("function_item", "document"):
                continue
            span = decl["declaration"]
            if decl.get("parse_has_error"):
                omissions.append({"path": path, "name": decl["qualified_name"], "reason": "syntax_parse_error"})
                continue
            if not (1 <= span["start_line"] <= span["end_line"] <= entry["line_count"] and 0 <= span["start_byte"] < span["end_byte"] <= entry["size"]):
                raise ValueError("The indexed source range is invalid")
            role = "document" if document else "test" if context.file_role(path) == "test" or "tests::" in decl["qualified_name"] else "implementation"
            row = {"path": path, "blob": entry["blob"], "file_sha256": entry["sha256"],
                   "file_bytes": entry["size"], "file_lines": entry["line_count"], "name": decl["qualified_name"],
                   "kind": decl["kind"], "role": role, "start_line": span["start_line"], "end_line": span["end_line"],
                   "start_byte": span["start_byte"], "end_byte": span["end_byte"], "unit_bytes": span["end_byte"] - span["start_byte"],
                   "explicit_file": path in explicit}
            row.update(signature_metadata(signature_blobs[path], decl) if rust else {"public_api_tier": 0})
            row["baseline_score"] = lexical(request, row)
            rows.append(row)
    def diverse(ranked):
        first, rest, seen = [], [], set()
        for row in ranked:
            (rest if row["path"] in seen else first).append(row)
            seen.add(row["path"])
        return first + rest

    selected = []
    for role, cap in CAPS.items():
        ranked = sorted((r for r in rows if r["role"] == role), key=lambda r: (-r["baseline_score"], -int(r["explicit_file"]), r["path"], r["start_line"], r["name"]))
        # Reserve half the implementation slots for visible operations; retain
        # lexical discovery of internal helpers in the other half.
        reserved = diverse([r for r in ranked if r["public_api_tier"] == 2])[:32] if role == "implementation" else []
        ordered = reserved + diverse([r for r in ranked if r not in reserved])
        selected.extend(ordered[:cap])
        omissions.extend({"path": r["path"], "name": r["name"], "reason": "role_catalog_limit"} for r in ordered[cap:])
    for i, row in enumerate(selected):
        row["id"] = f"s{i + 1:02d}"
    result = {"schema": SCHEMA, "source_commit": rev, "task": task, "clauses": split,
              "candidates": selected, "omissions": omissions, "index_sha256": context.digest(context.encoded(index)),
              "coverage": {"eligible_units": len(rows), "admitted_units": len(selected), "roles": dict(Counter(r["role"] for r in selected)),
                           "omitted_units": len(omissions), "missing_packages": missing_packages,
                           "missing_explicit_paths": sorted(p for p in explicit if p not in {r["path"] for r in rows}),
                           "body_text_used_for_ranking": False, "signature_files_read": len(signature_blobs), "signature_bytes_in_state": sum(len(r.get("signature", "").encode()) for r in selected), "complete_dependency_coverage_claimed": False},
              "limits": {"role_caps": CAPS, "state_bytes": STATE_BYTES, "pack_bytes": PACK_BYTES,
                         "contract_bytes": CONTRACT_BYTES, "read_pointers": MAX_READ_POINTERS,
                         "complete_unit_bytes": COMPLETE_BYTES, "partial_unit_bytes": SLICE_BYTES, "signature_files": MAX_SIGNATURE_FILES, "signature_bytes": SIGNATURE_BYTES, "public_operation_reservation": 32}}
    result["catalog_sha256"] = identity(result)
    if len(context.encoded(state(result))) > STATE_BYTES:
        raise ValueError("The metadata state exceeds 64 KiB; do not silently truncate it")
    result["wall_s"] = time.monotonic() - started
    return result


def state(value):
    validate(value)
    keep = ("id", "path", "name", "kind", "role", "start_line", "end_line", "unit_bytes", "signature", "signature_complete")
    return {"task": value["task"], "source_commit": value["source_commit"], "clauses": value["clauses"],
            "catalog": [{k: row[k] for k in keep if k in row} for row in value["candidates"]],
            "evidence_limit": "Names, bounded signatures and source spans only; metadata suggests where to read and does not establish behavior."}


def questions(value, *, include_scores=False):
    validate(value)
    options = {r["id"]: r["name"] + " (" + r["role"] + ")" for r in value["candidates"]}
    options["none"] = "No catalog pointer is a useful next source read for this clause"
    result = {key: {"type": "choice", "instructions": f"Which source pointer in state.catalog should be read first to understand or implement this exact task clause: {text} Select the implementation or check the clause actually concerns, using the scoped names and paths. A test name does not show the implementation. For procedural or scope instructions with no useful source pointer, use none. Metadata is a reading lead, not proof of behavior.", "criteria": options} for key, text in value["clauses"].items()}
    if include_scores:
        for row in value["candidates"]:
            result["rank_" + row["id"]] = {"type": "score", "instructions": f"How useful is reading source pointer {row['id']} in state.catalog for state.task? Judge the scoped name, signature, role and path; source bodies are not shown. Do not treat metadata as proof that requirements are satisfied.", "criteria": ["Unrelated source", "Possibly related background", "Useful dependency or regression lead", "Direct implementation or contract lead"]}
    return result


def deterministic_choices(value):
    validate(value)
    return {key: (max(value["candidates"], key=lambda r: (lexical(text, r), r["baseline_score"], -int(r["id"][1:])))['id'] if value["candidates"] and max(lexical(text, r) for r in value["candidates"]) > 0 else "none") for key, text in value["clauses"].items()}


def ordered_pointers(value, choices=None, scores=None):
    validate(value)
    chosen = deterministic_choices(value) if choices is None else choices
    ids = {r["id"] for r in value["candidates"]}
    if set(chosen) != set(value["clauses"]) or any(v not in ids | {"none"} for v in chosen.values()):
        raise ValueError("Supply one valid pointer or none for every exact clause")
    if scores is not None and (set(scores) != ids or any(type(x) not in (int, float) or not math.isfinite(x) for x in scores.values())):
        raise ValueError("Supply one finite score for each catalog pointer")
    votes = Counter(chosen.values())
    rows = sorted(value["candidates"], key=lambda r: (-votes[r["id"]], -(scores[r["id"]] if scores is not None else r["baseline_score"]), -r["baseline_score"], r["id"]))
    return rows, chosen


def render(row, text, start, end, complete):
    mark = "`" * max(3, 1 + max((len(m.group()) for m in re.finditer(r"`+", text)), default=0))
    return (f"\n### {row['id']} {row['path']}:{start}-{end} — {row['name']}\n\n"
            f"{row['role']}; {complete}. Full span: {row['start_line']}-{row['end_line']}. "
            f"Blob: `{row['blob']}`. File SHA-256: `{row['file_sha256']}`.\n\n{mark}text\n{text}"
            + ("" if text.endswith("\n") else "\n") + f"{mark}\n")


def pack(repo, value, *, choices=None, scores=None):
    """Materialize bounded selected pointers; both policies share the same renderer."""
    started = time.monotonic()
    ordered, chosen = ordered_pointers(value, choices, scores)
    bindings = context.tree(repo, value["source_commit"])
    documents = sorted((r for r in value["candidates"] if r["role"] == "document"), key=lambda r: r["path"])
    source_rows = [r for r in ordered if r["role"] != "document"]
    read_rows = source_rows[:MAX_READ_POINTERS]
    pending = documents + read_rows
    for row in pending:
        if bindings.get(row["path"]) != row["blob"]:
            raise ValueError("The catalog does not match the pinned source blob")
    blobs = context.read_blobs(repo, [r["path"] for r in pending], bindings)
    for row in pending:
        raw = blobs[row["path"]]
        if len(raw) != row["file_bytes"] or context.digest(raw) != row["file_sha256"]:
            raise ValueError("The catalog file bytes differ from pinned source")
    payload = ("## Prepared source spans\n\n" + f"Source commit: `{value['source_commit']}`. "
               "Complete applicable instructions are supplied separately. Partial evidence and omissions are explicit; a selected span does not prove requirement or dependency coverage.\n")
    selected, omissions = [], [{"id": r["id"], "reason": "read_pointer_limit"} for r in source_rows[MAX_READ_POINTERS:]]
    doc_allowance = CONTRACT_BYTES // max(1, len(documents))
    for row in pending:
        lines = blobs[row["path"]].decode().splitlines(keepends=True)
        start, end = row["start_line"], row["end_line"]
        if len(lines) != row["file_lines"] or not 1 <= start <= end <= len(lines):
            raise ValueError("The catalog line range is invalid")
        if row["role"] != "document":
            while start > 1 and lines[start - 2].lstrip().startswith(("#[", "///", "//!")):
                start -= 1
        text = "".join(lines[start - 1:end])
        complete = "complete_file" if row["role"] == "document" else "complete_declaration"
        if row["role"] == "document":
            # Each explicit contract has the same fixed rendered allowance in both arms.
            allowance = max(0, doc_allowance - len(render(row, "", start, end, "partial_file").encode()) - 32)
            if len(render(row, text, start, end, complete).encode()) > doc_allowance:
                text, end = context.slice_lines(lines, start, end, allowance)
                complete = "partial_file"
        elif len(text.encode()) > COMPLETE_BYTES:
            text, end = context.slice_lines(lines, start, end, SLICE_BYTES)
            complete = "partial_declaration"
        if not text:
            omissions.append({"id": row["id"], "reason": "unit_line_exceeds_slice_budget"})
            continue
        if any(row["path"] == s["path"] and start <= s["end_line"] and end >= s["start_line"] for s in selected):
            omissions.append({"id": row["id"], "reason": "overlapping_selected_span"})
            continue
        block = render(row, text, start, end, complete)
        if row["role"] == "document" and len(block.encode()) > doc_allowance:
            omissions.append({"id": row["id"], "reason": "contract_render_budget"})
            continue
        if len((payload + block).encode()) > PACK_BYTES:
            omissions.append({"id": row["id"], "reason": "pack_byte_budget"})
            continue
        payload += block
        selected.append({**row, "start_line": start, "end_line": end, "full_start_line": row["start_line"], "full_end_line": row["end_line"],
                         "completeness": complete, "source_sha256": context.digest(text.encode()), "source_bytes": len(text.encode())})
    return {"text": payload, "sha256": context.digest(payload.encode()), "payload_bytes": len(payload.encode()),
            "catalog_sha256": value["catalog_sha256"], "policy": "deterministic_metadata" if choices is None and scores is None else "supplied_metadata_judgments",
            "choices": chosen, "selected": selected, "selected_ids": [r["id"] for r in selected], "omissions": omissions,
            "read_files": len(blobs), "read_pointers": len(pending), "wall_s": time.monotonic() - started}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("repo", "rev", "index", "task-manifest", "task-id", "output"):
        parser.add_argument("--" + name, required=True)
    parser.add_argument("--answers", type=Path)
    parser.add_argument("--include-scores", action="store_true")
    args = parser.parse_args()
    started = time.monotonic()
    task = next(t for t in json.loads(Path(args.task_manifest).read_text())["tasks"] if t["id"] == args.task_id)
    value = catalog(args.repo, args.rev, json.loads(Path(args.index).read_text()), task)
    output = Path(args.output); output.mkdir(parents=True, exist_ok=False)
    baseline = pack(args.repo, value)
    artifacts = {"catalog": value, "state": state(value), "questions": questions(value, include_scores=args.include_scores), "deterministic-pack": baseline}
    if args.answers:
        answers = json.loads(args.answers.read_text())
        answers = answers.get("answers", answers)
        choices = {key: answers[key]["choice"] for key in value["clauses"]}
        scores = {r["id"]: answers["rank_" + r["id"]]["score"] for r in value["candidates"]} if args.include_scores else None
        artifacts["jev-pack"] = pack(args.repo, value, choices=choices, scores=scores)
    for name, artifact in artifacts.items():
        (output / (name + ".json")).write_text(json.dumps(artifact, indent=2, ensure_ascii=False) + "\n")
        if "text" in artifact:
            (output / (name + ".md")).write_text(artifact["text"])
    print(json.dumps({"candidates": len(value["candidates"]), "catalog_wall_s": value["wall_s"], "total_wall_s": time.monotonic() - started, "model_calls": 0}))


if __name__ == "__main__":
    main()
