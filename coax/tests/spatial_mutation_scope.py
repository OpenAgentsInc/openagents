#!/usr/bin/env python3
"""Materialize an exact changed-procedure mutation scope, retaining every check.

This writes a private snapshot only. Run the printed root with the compiler's
--root-only mutation option; it does not install or qualify a compiler.
"""
from pathlib import Path
import argparse
import hashlib
import json
import re

PROCEDURES = (
    "CK_spatial_call",
    "CK_spatial_token",
    "CK_spatial_mutation_candidate",
    "CK_prepare_spatial_mutations",
)


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


def checker_sources(root):
    """The checker root and its parts in load order (#12646: a root plus
    lib/compiler/checker/*.coax). Each part is a contiguous run of the old
    single file, so a procedure lives in exactly one of them."""
    source = root / "coax/lib/compiler/checker.coax"
    text = source.read_text()
    parts = re.findall(r'^#load "(checker/[^"]+)";$', text, re.M)
    return source, text, [(part, (source.parent / part).read_text()) for part in parts]


def materialize(root, output, names, kind="changed-procedure"):
    """Write OUTPUT/root.coax (the selected bodies), OUTPUT/dependency.coax (the
    checker root) and OUTPUT/checker/ (every part, selected bodies removed)."""
    source, text, parts = checker_sources(root)
    original = dict(parts)
    located = []
    for name in names:
        found = []
        for part, body in parts:
            try:
                found.append((part, *procedure_span(body, name)))
            except ValueError:
                pass
        if len(found) != 1:
            raise ValueError("procedure must be in exactly one checker part: " + name)
        located.append((name, *found[0]))
    content = dict(parts)
    for part, _ in parts:
        spans = sorted((row for row in located if row[1] == part), key=lambda row: row[2], reverse=True)
        for _, _, first, last in spans:
            content[part] = content[part][:first] + content[part][last:]
    # The parts stay relative (they are copied below); every policy module
    # load becomes absolute.
    dependency = re.sub(r'^#load "((?!checker/)[^"]+)";$',
                        lambda m: '#load "' + str(source.parent / m.group(1)) + '";', text, flags=re.M)
    entry = '#load "dependency.coax";\n#authority;\n'
    entry += "\n\n".join(original[part][first:last] for _, part, first, last in located) + "\n"
    output.mkdir(parents=True)
    for part, body in content.items():
        (output / part).parent.mkdir(parents=True, exist_ok=True)
        (output / part).write_text(body)
    (output / "dependency.coax").write_text(dependency)
    (output / "root.coax").write_text(entry)
    digest = lambda value: hashlib.sha256(value.encode()).hexdigest()
    receipt = {
        "schema": "coax-" + kind + "-mutation-scope-v2",
        "original": str(source), "source_sha256": digest(text),
        "part_sha256": {part: digest(body) for part, body in parts},
        "root": str(output / "root.coax"), "root_sha256": digest(entry),
        "dependency_sha256": digest(dependency),
        "dependency_part_sha256": {part: digest(body) for part, body in content.items()},
        "procedure_count": len(located),
        "procedures": [{"name": name, "part": part,
                        "first_line": original[part][:first].count("\n") + 1,
                        "sha256": digest(original[part][first:last])}
                       for name, part, first, last in located],
        "transformation": "Exact selected procedure bodies are root source. Every other body and every original check remain in dependency source: the checker root and its parts. Only the policy module load paths become absolute. No waiver or check filtering.",
    }
    (output / "scope.json").write_text(json.dumps(receipt, indent=2) + "\n")
    return output / "root.coax"


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
    print(materialize(root, output, names))


if __name__ == "__main__":
    main()
