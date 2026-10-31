#!/usr/bin/env python3
"""Materialize only the changed sample constructor; retain every check.

This tooling writes a new private directory and never runs a compiler. The
other five exact product bodies carry their initial zero-survivor evidence.
"""
from pathlib import Path
import argparse
import hashlib
import importlib.util
import json
import re

ROOT = Path(__file__).resolve().parents[3]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    out = args.output.resolve()
    if out.exists():
        parser.error("choose a new directory; previous evidence is retained")
    helper = ROOT / "coax/tests/spatial_mutation_scope.py"
    spec = importlib.util.spec_from_file_location("spatial_scope", helper)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    source = ROOT / "psionic/lib/robotics/panda-owned-sample.coax"
    text = source.read_text()
    fixture = ROOT / "psionic/fixtures/robotics/panda-owned-sample"
    old = (fixture / "development-initial-source.txt").read_text()
    digest = lambda value: hashlib.sha256(value.encode()).hexdigest()
    unchanged = {}
    for name in ("pcs_status", "pcs_rates", "pcs_velocity", "pcs_pack", "pcs_forward_raw"):
        first, last = module.procedure_span(text, name)
        a, b = module.procedure_span(old, name)
        if text[first:last] != old[a:b]:
            raise ValueError("prior product body changed: " + name)
        unchanged[name] = digest(text[first:last])
    first, last = module.procedure_span(text, "pcs_forward")
    dependency = text[:first] + text[last:]
    for match in list(re.finditer(r'#load "([^"]+)";', dependency))[::-1]:
        path = source.parent / match.group(1)
        dependency = dependency[:match.start()] + '#load "' + str(path) + '";' + dependency[match.end():]
    entry = '#load "dependency.coax";\n#authority;\n' + text[first:last] + '\n'
    out.mkdir(parents=True, exist_ok=False)
    (out / "root.coax").write_text(entry)
    (out / "dependency.coax").write_text(dependency)
    record = dict(schema="psionic.panda-owned-sample.materialized-scope.v1",
                  source_sha256=digest(text), root_sha256=digest(entry),
                  dependency_sha256=digest(dependency),
                  unchanged_body_sha256=unchanged,
                  scope="Exact pcs_forward only; every original check retained. No check filtering, waiver, compiler execution or old procedure mutation rerun.")
    (out / "scope.json").write_text(json.dumps(record, indent=2) + "\n")
    print(out / "root.coax")


if __name__ == "__main__":
    main()
