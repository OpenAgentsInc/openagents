#!/usr/bin/env python3
"""Book-only checks: local links, navigation coverage and opted-in source excerpts.

This uses only the standard library. It is never invoked by coax/build.sh or
by an app build. It does not claim to execute every illustrative fenced block.
"""
from pathlib import Path
import re
import sys
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parent

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
    return errors

if __name__ == "__main__":
    errors = problems(ROOT)
    for error in errors:
        print(error, file=sys.stderr)
    if errors:
        sys.exit(1)
    print("book navigation, local links and checked excerpts pass")
