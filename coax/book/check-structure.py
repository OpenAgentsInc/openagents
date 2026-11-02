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
# the same checks for Psionic, Aethyr, Nostr, Coder and Bitcoin.
BOOKS = {"coax": "coax/book", "psionic": "psionic/book", "aethyr": "aethyr/book", "nostr": "nostr/book",
         "coder": "coder/book", "bitcoin": "bitcoin/book"}
# A moved chapter keeps its number so the numbering stays 1..N; its title names
# where it went and it has no file here.
# A written chapter (Coder's Part VII) surveys another project: its profile
# names the pinned revision it read and its source cites that project's files
# as `owner/repo:path`; it builds nothing, so it has no target or adoption.
STATUSES = ("implemented", "partly implemented", "design draft", "planned", "moved", "written")
TARGETS = ("cpu", "cuda", "metal", "wasm")
# Coder's chapters run as programs on a person's machine; a book may qualify
# its own extra targets.
BOOK_TARGETS = {"coder": ("macos-arm64",)}
EXTERNAL = re.compile(r"[\w.-]+/[\w.-]+:[^\s:]+")
DRAFT = "> **Status: unimplemented — design draft.**"
PARTLY = "> **Status: partly implemented.**"
PLANNED = "> Note: This feature is planned and not available in `coax0`."
# Books other than Coax mark a planned chapter with this line instead.
PLANNED_OTHER = "> **Status: planned.**"
WRITTEN = "> **Status: written.**"
SUMMARY_LABEL = {"design draft": " — design draft", "partly implemented": " — partly implemented",
                 "planned": " — planned", "written": " — written"}


def readme_label(status, profile, book=None):
    """The README status cell for a Psionic, Aethyr, Nostr, Coder or Bitcoin chapter, before any `(...)` note."""
    implemented = "Implemented" if book == "coder" else "Runnable"
    return {"implemented": f"{implemented}: {profile}", "partly implemented": f"Partly implemented: {profile}",
            "design draft": "Unimplemented: design draft", "planned": "Planned",
            "written": f"Written: {profile}"}[status]


def registry_load(book, root=REPO):
    return json.loads((Path(root) / BOOKS[book] / "chapters.json").read_text())


def dependency_problems(registries):
    """Minimum teaching prerequisites form a DAG, including cross-book edges.

    A forward chapter reference is fine. A later integration belongs in prose,
    rather than creating a cycle in the prerequisites for its own foundation.
    Unknown dependencies are diagnosed separately by registry_problems.
    """
    graph = {f"{book}:{c['number']}": [f"{book}:{d}" if type(d) is int else d
                                      for d in c["depends_on"]]
             for book, registry in registries.items() for c in registry["chapters"]}
    active, done, path, errors = set(), set(), [], []

    def visit(node):
        if node in active:
            cycle = path[path.index(node):] + [node]
            errors.append("chapter prerequisites contain a cycle: " + " -> ".join(cycle)
                          + "; keep later integrations in chapter/plan prose")
            return
        if node in done or node not in graph:
            return
        active.add(node)
        path.append(node)
        for dep in graph[node]:
            visit(dep)
        path.pop()
        active.remove(node)
        done.add(node)

    for node in graph:
        visit(node)
    return errors


def adoption_problems(consumers, root):
    """A consumer is a repository file, not a teaching or verification artifact.

    Reviewers still establish the actual source-to-consumer relationship and
    scope. A valid path alone cannot prove runtime adoption or deployment.
    """
    root = Path(root).resolve()
    errors, seen = [], set()
    for name in consumers:
        if not isinstance(name, str):
            errors.append(f"adopted_by requires repository-relative file names: {name!r}")
            continue
        p = Path(name)
        if p.is_absolute() or ".." in p.parts or not (root / p).resolve().is_relative_to(root):
            errors.append(f"adopted_by must stay inside the repository: {name!r}")
        elif not (root / p).is_file():
            errors.append(f"adopted_by names a missing file: {name!r}")
        elif any(part in ("book", "tests", "examples", "fixtures") for part in p.parts):
            errors.append(f"adopted_by names teaching/verification evidence, not a consumer: {name!r}")
        if name in seen:
            errors.append(f"adopted_by names a duplicate consumer: {name!r}")
        seen.add(name)
    return errors


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
            if status not in ("planned", "moved"):
                errors.append(f"{label}: only a planned or moved chapter may have no file")
        elif status == "moved":
            errors.append(f"{label}: a moved chapter has no file in this book")
        elif not (directory / name).is_file():
            errors.append(f"{label}: missing file {name}")
        else:
            by_file[name] = c
        running = status in ("implemented", "partly implemented")
        written = status == "written"
        if (running or written) != (isinstance(c["profile"], str) and bool(c["profile"].strip())):
            errors.append(f"{label}: an implemented, partly implemented or written chapter, and only one, names its profile")
        for dep in c["depends_on"]:
            if isinstance(dep, int):
                if dep == n or dep not in numbers:
                    errors.append(f"{label}: unknown dependency {dep}")
            elif not (isinstance(dep, str) and re.fullmatch(r"(coax|psionic|aethyr|nostr|coder|bitcoin):\d+", dep)
                      and dep.split(":")[0] != book and int(dep.split(":")[1]) in others[dep.split(":")[0]]):
                errors.append(f"{label}: unknown dependency {dep!r} (use N or book:N)")
        for field in ("source", "adopted_by"):
            for item in c[field]:
                cited = written and field == "source" and isinstance(item, str) and EXTERNAL.fullmatch(item)
                if not cited and (not isinstance(item, str) or not (root / item).is_file()):
                    errors.append(f"{label}: {field} names a missing file {item!r}")
        errors.extend(f"{label}: {error}" for error in adoption_problems(c["adopted_by"], root))
        targets = TARGETS + BOOK_TARGETS.get(book, ())
        if any(t not in targets for t in c["qualified_targets"]) or len(set(c["qualified_targets"])) != len(c["qualified_targets"]):
            errors.append(f"{label}: qualified_targets must be distinct names from {', '.join(targets)}")
        if written and (c["qualified_targets"] or c["adopted_by"]):
            errors.append(f"{label}: a written chapter has no qualified target or adoption")
        elif not running and not written and (c["source"] or c["qualified_targets"] or c["adopted_by"]):
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
        want = {"design draft": DRAFT, "partly implemented": PARTLY, "written": WRITTEN,
                "planned": PLANNED if book == "coax" else PLANNED_OTHER}.get(c["status"])
        if want and not third.startswith(want):
            errors.append(f"{name}: registry status is {c['status']}; line 3 must start with {want}")
        # A chapter may quote another status's marker in its prose or code; only its opening block counts.
        opening = "\n".join(lines[:12])
        for status, marker in (("design draft", DRAFT), ("partly implemented", PARTLY), ("planned", PLANNED),
                               ("written", WRITTEN)):
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
            cell, want = cells[name][0], readme_label(c["status"], c["profile"], book)
            if cell != want and not cell.startswith(want + " ("):
                errors.append(f"README.md: {name} status must be '{want}' (optionally followed by ' (...)'), not '{cell}'")
    if book == "psionic":
        plan = (directory / "PLAN.md").read_text()
        for name, label in re.findall(r"\]\((ch[^)]+\.md)\) — ([^|]+)\|", plan):
            c = by_file.get(name)
            if c and (label.startswith("design draft") != (c["status"] == "design draft")):
                errors.append(f"PLAN.md: {name} is labelled '{label.strip()[:40]}' but the registry says {c['status']}")
    errors.extend(dependency_problems({b: registry_load(b, root) for b in BOOKS}))
    return errors



if __name__ == "__main__":
    if sys.argv[1:] == ["--registry"]:
        # Check every registry and print its status counts.
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
