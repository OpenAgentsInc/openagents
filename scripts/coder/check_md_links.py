#!/usr/bin/env python3
"""Check that the relative links in Markdown files point at files that exist.

    check_md_links.py FILE.md...

Used by `coder solver` as the `links:docs` check for a change that edits
Markdown. Web links (http, https, mailto) and in-page anchors are not
fetched; a relative link's `#anchor` part is ignored. Exits 1 and prints
`error: FILE:LINE: missing TARGET` for each link whose file is missing.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

LINK = re.compile(r"\[[^\]]*\]\(\s*<?([^)\s>]+)>?(?:\s+\"[^\"]*\")?\s*\)")
FENCE = re.compile(r"^\s*(```|~~~)")


def missing(path: Path) -> list[str]:
    found = []
    fenced = False
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if FENCE.match(line):
            fenced = not fenced
            continue
        if fenced:
            continue
        for target in LINK.findall(line):
            if re.match(r"^[a-zA-Z][a-zA-Z0-9+.-]*:", target) or target.startswith("#"):
                continue
            file = target.split("#", 1)[0].split("?", 1)[0]
            if not file:
                continue
            resolved = (path.parent / file) if not file.startswith("/") else Path(file.lstrip("/"))
            if not resolved.exists():
                found.append(f"error: {path}:{number}: missing {target}")
    return found


def main() -> int:
    errors = []
    for name in sys.argv[1:]:
        path = Path(name)
        if path.suffix.lower() == ".md" and path.is_file():
            errors.extend(missing(path))
    for error in errors:
        print(error)
    print(f"{len(sys.argv) - 1} file(s) checked, {len(errors)} missing link(s)")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
