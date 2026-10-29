#!/usr/bin/env python3
"""Writes verse/icecream/fixtures/prefixes.coax: the first steps of a few
Rust-recorded Ice Cream Man traces as Coax strings, for verse/icecream/trace.coax
to replay under coax test. The traces are the Rust build's own text
(~/work/aethyr-demo/tools/icecream-trace), cut after STEPS step lines.

  python3 verse/icecream/tools/prefixes.py DIR_WITH_TRACES
"""
import pathlib
import sys

PREFIXES = [("FUZZ_ARMED", "fuzz-armed", 150), ("TOUR", "tour", 150), ("ESCAPE", "escape", 120)]


def prefix(path, steps):
    out, taken, header = [], 0, True
    for line in path.read_text().splitlines():
        out.append(line)
        if header:
            header = not line.startswith("driver ")
            continue
        if line != "keep":
            taken += 1
        if taken == steps:
            break
    return "\\n".join(out) + "\\n"


def main():
    src = pathlib.Path(sys.argv[1])
    here = pathlib.Path(__file__).resolve().parent.parent
    lines = [
        "// The first steps of Rust-recorded Ice Cream Man traces (tools/icecream-trace in",
        "// ~/work/aethyr-demo), written by verse/icecream/tools/prefixes.py. Do not edit.",
        "",
    ]
    for name, trace, steps in PREFIXES:
        text = prefix(src / f"{trace}.trace", steps)
        lines.append(f"TRACE_{name} :: \"{text}\";")
    (here / "fixtures" / "prefixes.coax").write_text("\n".join(lines) + "\n")


main()
