"""CLI for the Gym GEPA lane.

Install (venv, not the host):

    python3 -m venv .venv-optimize
    .venv-optimize/bin/pip install -r bench/optimize/requirements.txt

Dry-run (stdlib only; no gepa, no Harbor):

    PYTHONPATH=bench python3 -m optimize --dry-run --max-metric-calls 1 \\
      --output bench/optimize/output/dry
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from .adapter import HoldoutScreenError
from .aggregate import DEFAULT_N_TRIALS
from .engine import OptimizeConfig, OptimizeError, run_optimize
from .metric import DEV_SUITE_ID, HOLDOUT_SUITE_ID
from .surfaces import repo_root_from


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="optimize",
        description=(
            "GEPA optimizer lane over the Gym. Emits openagents.coder_candidate.v1 "
            "artifacts. Does not land changes (ledger O1)."
        ),
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        default=True,
        help="Score a fixture Harbor job. Default. No Harbor, no Cloud Run.",
    )
    parser.add_argument(
        "--live",
        action="store_true",
        help="Invoke bench/run-suite.sh. Not used by the landing packet.",
    )
    parser.add_argument(
        "--max-metric-calls",
        type=int,
        default=1,
        help="Hard budget: one Harbor trial is one metric call (default 1).",
    )
    parser.add_argument(
        "--n-trials",
        type=int,
        default=DEFAULT_N_TRIALS,
        help=(
            f"Trials per A/B side (default {DEFAULT_N_TRIALS}). The emitted metric "
            "carries mean ± spread across trials, not one sample."
        ),
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="Directory for run.json and candidates/. Default: bench/optimize/output/<stamp>.",
    )
    parser.add_argument(
        "--model",
        default="openai/gpt-5.6-luna",
        help="Harbor model string. Transfer label uses the catalog id when --live.",
    )
    parser.add_argument(
        "--lane",
        default="proxy",
        choices=("proxy", "local"),
        help="Gym lane for a live run.",
    )
    parser.add_argument(
        "--dev-suite",
        default=DEV_SUITE_ID,
        help="Screening suite. Default tb2-quick. Must not be the holdout.",
    )
    parser.add_argument(
        "--holdout-suite",
        default=HOLDOUT_SUITE_ID,
        help="Named only. This lane does not run it.",
    )
    parser.add_argument(
        "--confirm-holdout",
        action="store_true",
        help="Refused in this packet: would run the holdout on a survivor.",
    )
    parser.add_argument(
        "--engine",
        default="seed",
        choices=("seed", "gepa"),
        help="seed: evaluate the incumbent once. gepa: upstream search (needs --live).",
    )
    parser.add_argument(
        "--reflection-lm",
        default=None,
        help="Reflection model for --engine gepa.",
    )
    parser.add_argument(
        "--fixture-job",
        type=Path,
        default=None,
        help="Harbor job directory with result.json. Default: package fixture.",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="Run the package's unit tests and exit.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.self_test:
        return _self_test()
    dry_run = not args.live
    try:
        root = repo_root_from()
    except FileNotFoundError as error:
        print(error, file=sys.stderr)
        return 2
    output = args.output
    if output is None:
        output = root / "bench" / "optimize" / "output" / "dry"
    try:
        result = run_optimize(
            OptimizeConfig(
                output=output,
                max_metric_calls=args.max_metric_calls,
                n_trials=args.n_trials,
                dry_run=dry_run,
                live=args.live,
                engine=args.engine,
                model=args.model,
                lane=args.lane,
                dev_suite=args.dev_suite,
                holdout_suite=args.holdout_suite,
                confirm_holdout=args.confirm_holdout,
                fixture_job=args.fixture_job,
                repo_root=root,
                reflection_lm=args.reflection_lm,
            )
        )
    except HoldoutScreenError as error:
        print(f"holdout-screen: {error}", file=sys.stderr)
        return 3
    except OptimizeError as error:
        print(error, file=sys.stderr)
        return 2
    run = result["run"]
    print(
        json.dumps(
            {
                "disposition": run["disposition"],
                "metricCalls": run["metricCalls"],
                "maxMetricCalls": run["maxMetricCalls"],
                "harbor": run["harbor"],
                "residual": run.get("residual"),
                "output": result["output"],
                "candidateId": result["candidate"]["candidateId"],
            },
            indent=2,
        )
    )
    return 0


def _self_test() -> int:
    import unittest

    suite = unittest.defaultTestLoader.discover(
        str(Path(__file__).resolve().parent / "tests"),
        pattern="test_*.py",
    )
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1
