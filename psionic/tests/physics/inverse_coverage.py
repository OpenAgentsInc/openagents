#!/usr/bin/env python3
"""Firm reason2: new closed-transcript coverage and independent state-word gate.

Initial qualification reuses the prior passing numerical calculation. Its
numeric function ASTs must be unchanged; only entry-point coverage is hardened.
No native code, model fitting, original solver gate or old mutation is rerun.
"""
import argparse
import ast
import copy
import hashlib
import json
from pathlib import Path
import struct
import inverse_reference as reference


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def numeric_identity(path):
    names = {"random_word", "uniform", "step", "data_for", "objective", "fit", "grid", "forecast"}
    tree = ast.parse(path.read_text())
    functions = [node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name in names]
    assert {node.name for node in functions} == names
    return hashlib.sha256("\n".join(ast.dump(node, include_attributes=False) for node in functions).encode()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("prefix", type=Path)
    parser.add_argument("--prior-numeric-source", type=Path)
    args = parser.parse_args()
    gradients = json.loads(Path(str(args.prefix) + ".gradients.json").read_text())
    fits = json.loads(Path(str(args.prefix) + ".fits.json").read_text())
    reference.check_transcripts(gradients, fits)
    rejected = 0
    bad_cases = [(gradients[:-1], fits), (gradients[:-1] + gradients[:1], fits),
                 (gradients, fits[:-1]), (gradients, fits[:-1] + fits[:1])]
    missing_fd = copy.deepcopy(gradients)
    missing_fd[0]["fd"].pop()
    bad_cases.append((missing_fd, fits))
    for bad_gradients, bad_fits in bad_cases:
        try:
            reference.check_transcripts(bad_gradients, bad_fits)
        except AssertionError:
            rejected += 1
    assert rejected == 5
    experiment = json.loads(reference.EXPERIMENT.read_text())
    expected = []
    points = [(1, .9), (2.4, .55), (3.5, .2), (.5, .1), (4, 1)]
    for mode in range(7):
        data = reference.data_for(mode, experiment)
        for k, c in points:
            for episode in range(3):
                state = data["initial"][episode][:] + [0.0] * 4
                for tick in range(24):
                    state = reference.step(state, k, c, data["forces"][episode * 24 + tick])
                    expected.extend(state[:2])
    raw = Path(str(args.prefix) + ".states.f64").read_bytes()
    assert len(raw) == 5040 * 8
    actual = struct.unpack("=5040d", raw)
    error = max(abs(a - b) for a, b in zip(actual, expected))
    assert error <= 1e-11, error
    record = dict(schema="psionic-inverse-coverage-v1", gradient_records=35,
                  closed_mode_point_pairs=35, closed_fd_pairs=252,
                  fit_records=7, negative_coverage_cases=rejected,
                  independent_state_values=5040, state_max_absolute=error,
                  current_oracle_sha256=sha(Path(reference.__file__)),
                  coverage_oracle_sha256=sha(Path(__file__)),
                  numeric_function_ast_sha256=numeric_identity(Path(reference.__file__)))
    if args.prior_numeric_source:
        prior = json.loads(Path(str(args.prefix) + ".reference.json").read_text())
        assert sha(args.prior_numeric_source) == prior["identity"]["oracle_sha256"]
        assert numeric_identity(args.prior_numeric_source) == record["numeric_function_ast_sha256"]
        for suffix, expected_sha in prior["payload_sha256"].items():
            assert sha(Path(str(args.prefix) + "." + suffix)) == expected_sha
        record["reused_numeric_reference_sha256"] = sha(Path(str(args.prefix) + ".reference.json"))
        record["reused_numeric_oracle_sha256"] = sha(args.prior_numeric_source)
        record["numerical_algorithms_unchanged"] = True
    Path(str(args.prefix) + ".coverage.json").write_text(json.dumps(record, indent=2) + "\n")
    print(f"inverse coverage: 35 gradients,7 fits,252 FD pairs,5 negative cases;5040 independent states max {error:.9g}")


if __name__ == "__main__":
    main()
