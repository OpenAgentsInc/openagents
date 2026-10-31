#!/usr/bin/env python3
"""Book-only checks: local links, navigation coverage, opted-in source excerpts and chapter status.

This uses only the standard library. It is never invoked by coax/build.sh or
by an app build. It does not claim to execute every illustrative fenced block.
"""
from pathlib import Path
import importlib.util
import json
import re
import sys
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[1]

def problems(root):
    errors = []
    summary = (root / "SUMMARY.md").read_text()
    targets = re.findall(r"\]\(([^)]+\.md)\)", summary)
    if len(targets) != len(set(targets)):
        errors.append("SUMMARY.md: duplicate page")
    listed = {Path(x).name for x in targets}
    pages = {p.name for p in root.glob("*.md") if p.name != "SUMMARY.md"}
    for page in sorted(pages - listed):
        errors.append(f"{page}: missing from SUMMARY.md")
    for file in root.glob("*.md"):
        text = file.read_text()
        # Fenced examples contain intentionally illustrative path names.
        prose = re.sub(r"^```[^\n]*\n.*?^```\s*$", "", text, flags=re.M | re.S)
        for target in re.findall(r"\]\(([^)]+)\)", prose):
            url = urlsplit(target)
            if url.scheme or url.netloc or not url.path:
                continue
            path = unquote(url.path)
            resolved = (file.parent / path).resolve()
            if not resolved.is_relative_to(root.resolve()) or not resolved.exists():
                errors.append(f"{file.name}: broken local link {target}")
        for source, listing in re.findall(
            r"<!-- checked-source: ([^\n]+) -->\n```[^\n]*\n(.*?)\n```", text, re.S
        ):
            path = (file.parent / source).resolve()
            if not path.is_relative_to(root.resolve()) or not path.is_file():
                errors.append(f"{file.name}: missing checked source {source}")
            elif listing not in path.read_text():
                errors.append(f"{file.name}: excerpt differs from {source}")
    spec = importlib.util.spec_from_file_location("coax_book_claims", Path(__file__).with_name("capability-claims.py"))
    claims = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(claims)
    errors.extend(claims.problems(root.resolve().parents[1]))
    errors.extend(registry_problems("coax", root.resolve().parents[1]))
    return errors


# Chapter status registry. Each book keeps a `chapters.json` next to its
# SUMMARY.md, one record per chapter. These checks fail when a chapter's status
# marker, its SUMMARY label, its README label or (for Psionic) its PLAN label
# disagrees with the registry. Every record keeps three facts apart:
# `source` (the implemented source the chapter shows or runs; empty for a
# design draft or planned chapter), `qualified_targets` (targets with recorded
# evidence for the chapter's profile, such as cpu, cuda, metal or wasm; a
# target with no recorded run is not listed) and `adopted_by` (files outside
# the book that use the chapter's code today). `psionic/tests/book.py` runs
# the same checks for Psionic and Aethyr.
BOOKS = {"coax": "coax/book", "psionic": "psionic/book", "aethyr": "aethyr/book"}
STATUSES = ("implemented", "partly implemented", "design draft", "planned")
TARGETS = ("cpu", "cuda", "metal", "wasm")
DRAFT = "> **Status: unimplemented — design draft.**"
PARTLY = "> **Status: partly implemented.**"
PLANNED = "> Note: This feature is planned and not available in `coax0`."
SUMMARY_LABEL = {"design draft": " — design draft", "partly implemented": " — partly implemented",
                 "planned": " — planned"}


def readme_label(status, profile):
    """The README status cell for a Psionic or Aethyr chapter, before any `(...)` note."""
    return {"implemented": f"Runnable: {profile}", "partly implemented": f"Partly implemented: {profile}",
            "design draft": "Unimplemented: design draft", "planned": "Planned"}[status]


def registry_load(book, root=REPO):
    return json.loads((Path(root) / BOOKS[book] / "chapters.json").read_text())


def registry_problems(book, root=REPO):
    root = Path(root).resolve()
    directory = root / BOOKS[book]
    where = f"{BOOKS[book]}/chapters.json"
    try:
        registry = registry_load(book, root)
    except (OSError, ValueError) as exc:
        return [f"{where}: {exc}"]
    errors = []
    if registry.get("schema") != 1 or registry.get("book") != book:
        return [f"{where}: expected schema 1 for book {book}"]
    chapters = registry.get("chapters")
    if not isinstance(chapters, list) or not chapters:
        return [f"{where}: no chapters"]
    numbers = [c.get("number") for c in chapters]
    if numbers != list(range(1, len(chapters) + 1)):
        errors.append(f"{where}: chapter numbers must be 1..{len(chapters)} in order")
    others = {b: {c["number"] for c in registry_load(b, root)["chapters"]} for b in BOOKS if b != book}
    by_file = {}
    for c in chapters:
        n, status, name = c.get("number"), c.get("status"), c.get("file")
        label = f"{where}: chapter {n}"
        if set(c) != {"number", "title", "file", "status", "profile", "depends_on", "source",
                      "qualified_targets", "adopted_by"}:
            errors.append(f"{label}: unexpected or missing fields")
            continue
        if status not in STATUSES:
            errors.append(f"{label}: status must be one of {', '.join(STATUSES)}")
            continue
        if not isinstance(c["title"], str) or not c["title"].strip():
            errors.append(f"{label}: missing title")
        if name is None:
            if status != "planned":
                errors.append(f"{label}: only a planned chapter may have no file")
        elif not (directory / name).is_file():
            errors.append(f"{label}: missing file {name}")
        else:
            by_file[name] = c
        running = status in ("implemented", "partly implemented")
        if running != (isinstance(c["profile"], str) and bool(c["profile"].strip())):
            errors.append(f"{label}: an implemented or partly implemented chapter, and only one, names its profile")
        for dep in c["depends_on"]:
            if isinstance(dep, int):
                if dep == n or dep not in numbers:
                    errors.append(f"{label}: unknown dependency {dep}")
            elif not (isinstance(dep, str) and re.fullmatch(r"(coax|psionic|aethyr):\d+", dep)
                      and dep.split(":")[0] != book and int(dep.split(":")[1]) in others[dep.split(":")[0]]):
                errors.append(f"{label}: unknown dependency {dep!r} (use N or book:N)")
        for field in ("source", "adopted_by"):
            for item in c[field]:
                if not isinstance(item, str) or not (root / item).is_file():
                    errors.append(f"{label}: {field} names a missing file {item!r}")
        if any(t not in TARGETS for t in c["qualified_targets"]) or len(set(c["qualified_targets"])) != len(c["qualified_targets"]):
            errors.append(f"{label}: qualified_targets must be distinct names from {', '.join(TARGETS)}")
        if not running and (c["source"] or c["qualified_targets"] or c["adopted_by"]):
            errors.append(f"{label}: a {status} chapter has no implemented source, qualified target or adoption")
        if running and not c["qualified_targets"]:
            errors.append(f"{label}: an implemented chapter names at least one qualified target")
    files = {p.name for p in directory.glob("ch*.md")}
    if files != set(by_file):
        errors.append(f"{where}: chapter files and registry differ: {sorted(files ^ set(by_file))}")

    # Chapter headings and status markers.
    for name, c in by_file.items():
        lines = (directory / name).read_text().splitlines()
        text = "\n".join(lines)
        heading = re.fullmatch(r"# (\d+)\. (.+)", lines[0])
        if not heading or int(heading.group(1)) != c["number"] or heading.group(2) != c["title"]:
            errors.append(f"{name}: heading must be '# {c['number']}. {c['title']}' from the registry")
        third = lines[2] if len(lines) > 2 else ""
        want = {"design draft": DRAFT, "partly implemented": PARTLY, "planned": PLANNED}.get(c["status"])
        if want and not third.startswith(want):
            errors.append(f"{name}: registry status is {c['status']}; line 3 must start with {want}")
        # A chapter may quote another status's marker in its prose or code; only its opening block counts.
        opening = "\n".join(lines[:12])
        for status, marker in (("design draft", DRAFT), ("partly implemented", PARTLY), ("planned", PLANNED)):
            if c["status"] != status and marker in opening:
                errors.append(f"{name}: has the {status} marker but the registry says {c['status']}")
        # Every example and source listing the chapter shows is implemented source.
        shown = set(re.findall(r"<!-- source: ([^ ]+) [^ ]+ -->", text))
        shown |= {BOOKS[book] + "/" + s.strip() for s in re.findall(r"<!-- checked-source: ([^\n]+) -->", text)}
        missing = shown - set(c["source"])
        if missing:
            errors.append(f"{name}: registry source lacks listed files {sorted(missing)}")
    manifest = directory / "examples.json"
    if manifest.is_file():
        for item in json.loads(manifest.read_text())["examples"]:
            c = by_file.get(Path(item["chapter"]).name)
            if c and item["source"] not in c["source"]:
                errors.append(f"{where}: chapter {c['number']} source lacks its example {item['source']}")

    # SUMMARY labels.
    summary = (directory / "SUMMARY.md").read_text()
    listed = {}
    for title, name, rest in re.findall(r"^[*-] \[\d+\. ([^\]]+)\]\((ch[^)]+\.md)\)(.*)$", summary, re.M):
        listed[name] = (title, rest)
    for name, c in by_file.items():
        if name not in listed:
            errors.append(f"SUMMARY.md: {name} is not listed")
            continue
        title, rest = listed[name]
        if title != c["title"]:
            errors.append(f"SUMMARY.md: {name} title differs from the registry")
        label = SUMMARY_LABEL.get(c["status"])
        if label and not rest.startswith(label):
            errors.append(f"SUMMARY.md: {name} needs '{label.strip()}' after its link")
        for status, other in SUMMARY_LABEL.items():
            if status != c["status"] and other.strip() in rest:
                errors.append(f"SUMMARY.md: {name} is labelled {status} but the registry says {c['status']}")

    # README labels.
    readme = (directory / "README.md").read_text()
    if book == "coax":
        # Contents at a glance: `[N. Title](file)` then ` (planned)` or ` (partly implemented)`.
        found = {name: rest for name, rest in re.findall(r"\[\d+\. [^\]]+\]\((ch[^)]+\.md)\)( \([a-z ]+\))?", readme)}
        for name, c in by_file.items():
            if name not in found:
                errors.append(f"README.md: {name} is missing from the contents")
                continue
            want = "" if c["status"] == "implemented" else f" ({c['status']})"
            if found[name] != want:
                errors.append(f"README.md: {name} needs{want or ' no status label'} after its link")
    else:
        cells = {}
        for name, cell in re.findall(r"^\| \[\d+\. [^\]]+\]\((ch[^)]+\.md)\) \| ([^|]*) \|", readme, re.M):
            cells.setdefault(name, []).append(cell.strip())
        for name, c in by_file.items():
            if len(cells.get(name, [])) != 1:
                errors.append(f"README.md: {name} needs exactly one status-table row of its own")
                continue
            cell, want = cells[name][0], readme_label(c["status"], c["profile"])
            if cell != want and not cell.startswith(want + " ("):
                errors.append(f"README.md: {name} status must be '{want}' (optionally followed by ' (...)'), not '{cell}'")
    if book == "psionic":
        plan = (directory / "PLAN.md").read_text()
        for name, label in re.findall(r"\]\((ch[^)]+\.md)\) — ([^|]+)\|", plan):
            c = by_file.get(name)
            if c and (label.startswith("design draft") != (c["status"] == "design draft")):
                errors.append(f"PLAN.md: {name} is labelled '{label.strip()[:40]}' but the registry says {c['status']}")
    return errors



if __name__ == "__main__":
    if sys.argv[1:] == ["--registry"]:
        # Check all three registries and print their status counts.
        failed = False
        for book in sorted(BOOKS):
            errors = registry_problems(book)
            for error in errors:
                print(error, file=sys.stderr)
            failed |= bool(errors)
            counts = {s: 0 for s in STATUSES}
            for c in registry_load(book)["chapters"]:
                counts[c["status"]] += 1
            print(f"{book} chapter registry: " + ", ".join(f"{v} {k}" for k, v in counts.items()))
        sys.exit(1 if failed else 0)
    errors = problems(ROOT)
    for error in errors:
        print(error, file=sys.stderr)
    if errors:
        sys.exit(1)
    print("book navigation, local links, checked excerpts and the chapter registry pass")
