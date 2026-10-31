#!/usr/bin/env python3
"""Book tooling: closed example inventory, exact source listings, links and runs.

One checker serves the Psionic and Aethyr books (`--book psionic|aethyr`).
Each book keeps chapters as chNNN-slug.md, an examples.json inventory of its
examples/*.coax programs, and a status marker on every design draft.
"""
import argparse
import importlib.util
from concurrent.futures import ThreadPoolExecutor
import os
import json
from pathlib import Path
import re
import subprocess
from urllib.parse import unquote, urlparse

ROOT = Path(__file__).resolve().parents[2]
BOOKS = {
    # book directory, example programs, project README, least source listings
    "psionic": ("psionic/book", "psionic/examples", "psionic/README.md", 3),
    "aethyr": ("aethyr/book", "aethyr/examples", "aethyr/README.md", 0),
}
# Aethyr's engine-core tasks each own one chapter, named in its status block.
TASKS = {"aethyr": "apps/openagents-web/projects/verse/work/engine-core.json"}
DRAFT = "> **Status: unimplemented — design draft.**"


def path(name):
    result = (ROOT / name).resolve()
    result.relative_to(ROOT)
    assert result.is_file(), f"missing file: {name}"
    return result


def check(name="psionic"):
    book, programs, readme, least = BOOKS[name]
    spec = importlib.util.spec_from_file_location("coax_book_claims", ROOT / "coax/book/capability-claims.py")
    claims = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(claims)
    errors = claims.problems(ROOT)
    assert not errors, "reviewed capability claims: " + "; ".join(errors)
    manifest = json.loads(path(f"{book}/examples.json").read_text())
    assert manifest["schema"] == 1
    examples = manifest["examples"]
    assert examples or least == 0, "empty example inventory"
    sources = [item["source"] for item in examples]
    assert len(sources) == len(set(sources)), "duplicate example"
    actual = {str(p.relative_to(ROOT)) for p in (ROOT / programs).glob("*.coax")}
    assert set(sources) == actual, "example inventory does not cover every program"
    linked = set()
    listings = 0
    docs = [ROOT / readme, *sorted((ROOT / book).glob("*.md"))]
    chapters(name, book)
    pattern = r"<!-- source: ([^ ]+) ([^ ]+) -->\n```jai\n(.*?)\n```"
    for doc in docs:
        text = doc.read_text()
        assert text.endswith("\n") and text.count("```") % 2 == 0, f"bad structure: {doc}"
        in_block = False
        for line in text.splitlines():
            if line.startswith("```"):
                if not in_block:
                    # A project README may also use plain sh fences.
                    fences = ("```jai", "```shell", "```text") + (("```sh",) if doc.name == "README.md" and doc.parent.name != "book" else ())
                    assert line in fences, f"book fence must be jai, shell or text: {doc}: {line}"
                else:
                    assert line == "```", f"bad closing fence: {doc}: {line}"
                in_block = not in_block
        for label, href in re.findall(r"\[([^\]]+)\]\(([^)]+)\)", text):
            parsed = urlparse(href)
            if not parsed.scheme and parsed.path:
                target = (doc.parent / unquote(parsed.path)).resolve()
                target.relative_to(ROOT)
                assert target.exists(), f"broken link in {doc}: {href}"
                linked.add(target)
        found = list(re.finditer(pattern, text, re.DOTALL))
        assert len(found) == text.count("<!-- source:"), f"malformed listing: {doc}"
        for match in found:
            source, tag, listing = match.groups()
            data = path(source).read_text()
            begin, end = f"// book:begin {tag}\n", f"// book:end {tag}"
            assert data.count(begin) == data.count(end) == 1, f"bad source markers: {source} {tag}"
            expected = data.split(begin)[1].split(end)[0].rstrip("\n")
            assert listing == expected, f"listing drift: {doc.name} -> {source} {tag}"
            listings += 1
    assert listings >= least, "required library/example listings missing"
    for item in examples:
        source = path(item["source"])
        chapter = path(item["chapter"])
        path(item["stdout"])
        assert source in linked and chapter in linked, f"unlinked example/chapter: {source}"
        assert type(item["exit_status"]) is int and 0 <= item["exit_status"] <= 255
        assert item["modes"] == ["debug", "release"], "both build modes are required"
    print(f"{name} book structure: {len(examples)} programs, {listings} source-linked listings, local links pass")
    return examples


def chapters(name, book):
    """Chapter files, headings, contents and plan agree, and numbers have no gaps."""
    files = {}
    for doc in (ROOT / book).glob("ch*.md"):
        # Chapter files carry three-digit numbers so they list in reading order.
        assert re.fullmatch(r"ch\d{3}-[a-z0-9-]+\.md", doc.name), f"chapter file needs a three-digit number: {doc.name}"
        number = int(doc.name[2:5])
        assert number not in files, f"two chapter files numbered {number}"
        files[number] = doc
        lines = doc.read_text().splitlines()
        title = re.fullmatch(r"# (?:(\d+)\.|Chapter (\d+):) .+", lines[0])
        assert title and int(title.group(1) or title.group(2)) == number, f"heading does not match the file number: {doc.name}"
        draft = len(lines) > 2 and lines[2].startswith(DRAFT)
        assert draft or "<!-- source:" in doc.read_text() or DRAFT not in doc.read_text(), f"misplaced status marker: {doc.name}"
        if draft:
            assert "<!-- source:" not in doc.read_text(), f"a design draft has source-linked listings: {doc.name}"
    summary = (ROOT / book / "SUMMARY.md").read_text()
    listed = re.findall(r"^- \[(\d+)\. [^\]]+\]\((ch\d{3}-[a-z0-9-]+\.md)\)", summary, re.M)
    names = [f for _, f in listed]
    assert len(names) == len(set(names)), f"{book}/SUMMARY.md lists a chapter twice"
    for number, f in listed:
        assert int(number) == int(f[2:5]), f"SUMMARY number does not match {f}"
    assert set(names) == {p.name for p in files.values()}, f"{book}/SUMMARY.md does not list exactly the chapter files"
    assert [int(n) for n, _ in listed] == sorted(int(n) for n, _ in listed), f"{book}/SUMMARY.md is out of order"
    plan = ROOT / book / "PLAN.md"
    text = plan.read_text() if plan.is_file() else ""
    # A plan that tables its chapters, written or not, numbers them 1..N once each.
    planned = [int(n) for n in re.findall(r"^\| (?:\[|\*\*)?(\d+)\. ", text, re.M)]
    planned += [int(n) for n in re.findall(r"^\| \d+ \| (?:\[|\*\*)(\d+)\. ", text, re.M)]
    if planned:
        assert sorted(planned) == list(range(1, len(planned) + 1)), f"{book}/PLAN.md chapter numbers are not 1..N once each"
        assert set(files) <= set(planned), f"{book}/PLAN.md is missing a written chapter"
    else:
        assert sorted(files) == list(range(1, len(files) + 1)), f"{book} chapter numbers have a gap"
    if name in TASKS:
        tasks = [t["slug"] for t in json.loads(path(TASKS[name]).read_text())["tasks"]]
        status = {}
        for number, doc in files.items():
            block = []
            for line in doc.read_text().splitlines()[2:]:
                if not line.startswith(">"):
                    break
                block.append(line)
            status[number] = " ".join(block)
        for task in tasks:
            owners = [n for n, block in status.items() if f"`{task}`" in block]
            assert len(owners) == 1, f"engine-core task {task} needs exactly one chapter, has {owners}"


def run_one(compiler, item, mode):
    cmd = [compiler, "run", str(path(item["source"]))]
    if mode == "release":
        cmd.append("--release")
    result = subprocess.run(cmd, cwd=ROOT, capture_output=True, timeout=60)
    assert result.returncode == item["exit_status"], f"exit status: {cmd}\n{result.stderr.decode()}"
    assert not result.stderr, f"unexpected diagnostics: {cmd}\n{result.stderr.decode()}"
    assert result.stdout == path(item["stdout"]).read_bytes(), f"transcript drift: {cmd}\n{result.stdout.decode()}"


def run(compiler, examples, jobs=1):
    runs = [(item, mode) for item in examples for mode in item["modes"]]
    with ThreadPoolExecutor(max_workers=max(1, jobs)) as pool:
        for future in [pool.submit(run_one, compiler, item, mode) for item, mode in runs]:
            future.result()  # in inventory order: the first failure is reported
    print(f"book execution: {len(examples) * 2} runs match output and exit status")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--book", choices=sorted(BOOKS), default="psionic")
    parser.add_argument("--coax", help="also compile and run every example")
    parser.add_argument("--jobs", type=int, default=os.cpu_count() or 1, help="examples run at once")
    args = parser.parse_args()
    examples = check(args.book)
    if args.coax:
        run(str(Path(args.coax).resolve()), examples, args.jobs)


if __name__ == "__main__":
    main()
