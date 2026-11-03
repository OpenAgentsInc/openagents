#!/usr/bin/env python3
"""Materialize an exact changed-procedure mutation scope, retaining every check.

This writes a private snapshot only. Run the printed root with the compiler's
--root-only mutation option; it does not install or qualify a compiler.
"""
from pathlib import Path
import argparse

from spatial_mutation_scope import materialize

PROCEDURES = (
    "CK_bind", "CK_unit_constructor", "CK_unit_form", "CK_unit_intern",
    "CK_unit_admit", "CK_unit_compose", "CK_unit_type", "CK_unit_target",
    "CK_quantity_derived", "CK_quantity_conversion", "CK_quantity_convert",
    "CK_binary", "CK_resolve", "CK_call", "CK_builtin_name", "CK_quantity_binary",
)


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
