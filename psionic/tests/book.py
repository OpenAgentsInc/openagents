#!/usr/bin/env python3
"""Book tooling: closed example inventory, exact source listings, links and runs."""
import argparse
import json
from pathlib import Path
import re
import subprocess
from urllib.parse import unquote, urlparse

ROOT = Path(__file__).resolve().parents[2]


def path(name):
    result = (ROOT / name).resolve()
    result.relative_to(ROOT)
    assert result.is_file(), f"missing file: {name}"
    return result


def check():
    manifest = json.loads(path("psionic/book/examples.json").read_text())
    assert manifest["schema"] == 1
    examples = manifest["examples"]
    assert examples, "empty example inventory"
    sources = [item["source"] for item in examples]
    assert len(sources) == len(set(sources)), "duplicate example"
    actual = {str(p.relative_to(ROOT)) for p in (ROOT / "psionic/examples").glob("*.coax")}
    assert set(sources) == actual, "example inventory does not cover every program"
    linked = set()
    listings = 0
    docs = [ROOT / "psionic/README.md", *sorted((ROOT / "psionic/book").glob("*.md"))]
    pattern = r"<!-- source: ([^ ]+) ([^ ]+) -->\n```coax\n(.*?)\n```"
    for doc in docs:
        text = doc.read_text()
        assert text.endswith("\n") and text.count("```") % 2 == 0, f"bad structure: {doc}"
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
    assert listings >= 3, "required library/example listings missing"
    for item in examples:
        source = path(item["source"])
        chapter = path(item["chapter"])
        path(item["stdout"])
        assert source in linked and chapter in linked, f"unlinked example/chapter: {source}"
        assert type(item["exit_status"]) is int and 0 <= item["exit_status"] <= 255
        assert item["modes"] == ["debug", "release"], "both build modes are required"
    print(f"book structure: {len(examples)} programs, {listings} source-linked listings, local links pass")
    return examples


def run(compiler, examples):
    for item in examples:
        for mode in item["modes"]:
            cmd = [compiler, "run", str(path(item["source"]))]
            if mode == "release":
                cmd.append("--release")
            result = subprocess.run(cmd, cwd=ROOT, capture_output=True, timeout=60)
            assert result.returncode == item["exit_status"], f"exit status: {cmd}\n{result.stderr.decode()}"
            assert not result.stderr, f"unexpected diagnostics: {cmd}\n{result.stderr.decode()}"
            assert result.stdout == path(item["stdout"]).read_bytes(), f"transcript drift: {cmd}\n{result.stdout.decode()}"
    print(f"book execution: {len(examples) * 2} runs match output and exit status")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--coax", help="also compile and run every example")
    args = parser.parse_args()
    examples = check()
    if args.coax:
        run(str(Path(args.coax).resolve()), examples)


if __name__ == "__main__":
    main()
