#!/usr/bin/env python3
"""Proves `panel.py` scores the way `crates/gym/src/calibrate.rs` scores.

The fixtures and the asserted values are lifted from that file's own unit
tests. A baseline computed against a different scorer than the doors it is
compared to would be measuring its scorer.

    python3 check_panel.py
"""

from __future__ import annotations

import math
import sys

from panel import Observation, score


def _obs(pairs: list[tuple[float, bool]]) -> list[Observation]:
    return [Observation(raw=raw, correct=correct) for raw, correct in pairs]


CASES = [
    # perfect_confidence_that_is_always_wrong_scores_as_badly_as_it_should
    (
        "perfect confidence that is always wrong",
        _obs([(1.0, False), (1.0, False)]),
        {"accuracy": 0.0, "ece": 1.0, "brier": 1.0, "confident_errors": 2},
    ),
    # a_well_calibrated_set_scores_near_zero_error
    (
        "a well calibrated set",
        _obs([(0.75, True)] * 6 + [(0.75, False)] * 2),
        {"accuracy": 0.75, "ece": 0.0, "brier": 0.1875, "confident_errors": 0},
    ),
    # An empty set scores as all zeroes rather than dividing by zero.
    ("no observations", [], {"accuracy": 0.0, "ece": 0.0, "brier": 0.0, "confident_errors": 0}),
]


def main() -> int:
    failures = 0
    for name, observations, expected in CASES:
        got = score(observations)
        for field, want in expected.items():
            have = getattr(got, field)
            ok = have == want if isinstance(want, int) and not isinstance(want, bool) else math.isclose(
                have, want, abs_tol=1e-9
            )
            mark = "ok" if ok else "FAILED"
            if not ok:
                failures += 1
                print(f"{mark}: {name}: {field} was {have}, the Rust test asserts {want}")
        if not failures:
            print(f"ok: {name}")

    # The log loss of an item reported at zero is finite, not an infinity.
    floored = score(_obs([(0.0, True)]))
    if not math.isfinite(floored.nll):
        print("FAILED: a reported zero must clamp rather than diverge")
        failures += 1
    else:
        print("ok: a reported zero clamps rather than diverges")

    print(f"\n{'all fixtures agree' if failures == 0 else f'{failures} disagreements'}")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
