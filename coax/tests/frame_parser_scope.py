#!/usr/bin/env python3
"""Materialize the exact M3 frame-parser mutation scope, retaining every check.

This writes a private snapshot only. Run the printed root with the compiler's
--root-only mutation option; it does not install or qualify a compiler.
"""
from pathlib import Path
import argparse
import hashlib
import json
import re

PROCEDURES = ("PS_struct", "PS_physical", "PS_frame", "PS_frame_word")


def procedure_span(text, name):
    match = re.search(r"^" + re.escape(name) + r"\s*::", text, re.M)
    if not match:
        raise ValueError("missing procedure: " + name)
    i = text.index("{", match.start())
    depth = block = 0
    quoted = line = escaped = False
    while i < len(text):
        c, following = text[i], text[i + 1:i + 2]
        if line:
            line = c != "\n"
        elif quoted:
            if escaped:
                escaped = False
            elif c == "\\":
                escaped = True
            elif c == '"':
                quoted = False
        elif block:
            if c == "/" and following == "*":
                block += 1
                i += 1
            elif c == "*" and following == "/":
                block -= 1
                i += 1
        elif c == "/" and following == "/":
            line = True
            i += 1
        elif c == "/" and following == "*":
            block = 1
            i += 1
        elif c == '"':
            quoted = True
        elif c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                return match.start(), i + 1
        i += 1
    raise ValueError("unterminated procedure: " + name)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path, help="new private output directory")
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--procedures", help="comma-separated subset of the declared changed scope")
    args = parser.parse_args()
    root, output = args.root.resolve(), args.output.resolve()
    names = tuple(args.procedures.split(",")) if args.procedures else PROCEDURES
    if not names or len(set(names)) != len(names) or any(name not in PROCEDURES for name in names):
        parser.error("procedures must be a nonempty, unique subset of the declared scope")
    if output.exists():
        parser.error("output already exists; preserve earlier snapshots and choose a new directory")
    source = root / "coax/lib/compiler/parser.coax"
    text = source.read_text()
    spans = [(name, *procedure_span(text, name)) for name in names]
    dependency = text
    for _, first, last in sorted(spans, key=lambda row: row[1], reverse=True):
        dependency = dependency[:first] + dependency[last:]
    entry = '#load "dependency.coax";\n#authority;\n'
    entry += "\n\n".join(text[first:last] for _, first, last in spans) + "\n"
    output.mkdir(parents=True)
    (output / "dependency.coax").write_text(dependency)
    (output / "root.coax").write_text(entry)
    digest = lambda value: hashlib.sha256(value.encode()).hexdigest()
    receipt = {
        "schema": "coax-changed-procedure-mutation-scope-v1",
        "original": str(source), "source_sha256": digest(text),
        "root": str(output / "root.coax"), "root_sha256": digest(entry),
        "dependency_sha256": digest(dependency), "procedure_count": len(spans),
        "procedures": [{"name": name, "first_line": text[:first].count("\n") + 1,
                        "sha256": digest(text[first:last])} for name, first, last in spans],
        "transformation": "Exact selected procedure bodies are root source. Every other body and every original check remain in dependency source. No load path or check changes. No waiver or check filtering.",
    }
    (output / "scope.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(output / "root.coax")


if __name__ == "__main__":
    main()
