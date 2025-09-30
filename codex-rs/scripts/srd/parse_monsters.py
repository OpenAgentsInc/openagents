#!/usr/bin/env python3
"""
Parse SRD Monsters text (extracted with pdftotext -layout)
and split into per-monster Markdown files under docs/srd/07-monsters/a-z.

Usage:
  python3 scripts/srd/parse_monsters.py docs/srd/.tmp/monsters_254_343.txt

This is heuristic and meant to accelerate transcription. It preserves
raw blocks as a fenced code block when exact parsing fails.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

SRC = Path("docs/srd/07-monsters/a-z")


def slugify(name: str) -> str:
    return (
        name.lower()
        .strip()
        .replace("—", "-")
        .replace("—", "-")
        .replace("’", "'")
        .replace("/", "-")
        .replace(",", "")
        .replace("(", "-")
        .replace(")", "")
        .replace("  ", " ")
        .replace(" ", "-")
    )


def ensure_letter_dir(letter: str) -> Path:
    d = SRC / letter.upper()
    d.mkdir(parents=True, exist_ok=True)
    readme = d / "README.md"
    if not readme.exists():
        readme.write_text(f"# {letter.upper()} Monsters\n\n", encoding="utf-8")
    return d


def is_probable_name(line: str) -> bool:
    # Title Case word(s), not too long, no trailing punctuation
    return bool(re.match(r"^[A-Z][A-Za-z'()\- ]{1,60}$", line.strip()))


def split_entries(lines: list[str]) -> list[tuple[int, int]]:
    """Return (start,end) indices for blocks that look like stat blocks.
    Heuristic: a block contains a line with 'Armor Class' and likely starts
    1-2 lines before with the monster name.
    """
    indices: list[tuple[int, int]] = []
    i = 0
    n = len(lines)
    while i < n:
        if "Armor Class" in lines[i]:
            window = "\n".join(lines[i : min(n, i + 12)])
            if ("Hit Points" not in window) or ("Speed" not in window):
                i += 1
                continue
            # backtrack to find the first non-empty line above
            start = i
            for j in range(i - 1, max(-1, i - 5), -1):
                if lines[j].strip():
                    start = j
            # extend until a blank line gap followed by a new probable name + Armor Class later
            end = i + 1
            while end < n:
                if (
                    end + 2 < n
                    and not lines[end].strip()
                    and is_probable_name(lines[end + 1])
                    and ("Armor Class" in lines[end + 2] or "Hit Points" in lines[end + 2])
                ):
                    break
                end += 1
            indices.append((start, end))
            i = end
        else:
            i += 1
    return indices


def main() -> None:
    if len(sys.argv) < 2:
        print("Usage: parse_monsters.py <monsters_text_file>", file=sys.stderr)
        sys.exit(2)
    src_txt = Path(sys.argv[1])
    text = src_txt.read_text(encoding="utf-8", errors="ignore")
    # Normalize weird hyphenation artifacts
    text = text.replace("\u00ad", "")
    lines = text.splitlines()
    entries = split_entries(lines)
    print(f"Detected {len(entries)} probable stat blocks.")

    for (start, end) in entries:
        block = [l.rstrip() for l in lines[start:end]]
        # Guess name: first non-empty line of block
        name = next((l.strip() for l in block if l.strip()), "Unknown Creature")
        letter = name[:1].upper() if name else "_"
        out_dir = ensure_letter_dir(letter)
        out_path = out_dir / f"{slugify(name)}.md"
        if out_path.exists():
            # Skip to avoid clobbering manual edits
            continue
        md = [f"# {name}", "", "<!-- Auto-extracted from SRD 5.2.1 PDF; needs review -->", "", "```text"]
        md.extend(block)
        md.append("```")
        out_path.write_text("\n".join(md) + "\n", encoding="utf-8")
        print(f"Wrote {out_path}")


if __name__ == "__main__":
    main()
