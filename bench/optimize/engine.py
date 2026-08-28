"""Run the optimizer: fixture/dry seed eval, or upstream `gepa` when asked.

Does not reimplement Pareto search. `--dry-run` evaluates the seed once
through the metric adapter. `--engine gepa` imports upstream `gepa`.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from .adapter import GymAdapter, GymTask, HoldoutScreenError
from .emit import acceptance_argument, build_candidate, write_run
from .job import load_job
from .metric import DEV_SUITE_ID, HOLDOUT_SUITE_ID, score_job
from .surfaces import load_seed_candidate, load_suite_task_ids, repo_root_from

PACKAGE_DIR = Path(__file__).resolve().parent
DEFAULT_FIXTURE = PACKAGE_DIR / "fixtures" / "tb2-quick-job"


@dataclass
class OptimizeConfig:
    output: Path
    max_metric_calls: int = 1
    dry_run: bool = True
    live: bool = False
    engine: str = "seed"
    model: str = "openai/gpt-5.6-luna"
    lane: str = "proxy"
    dev_suite: str = DEV_SUITE_ID
    holdout_suite: str = HOLDOUT_SUITE_ID
    confirm_holdout: bool = False
    fixture_job: Path | None = None
    repo_root: Path | None = None
    reflection_lm: str | None = None


class OptimizeError(RuntimeError):
    pass


def _catalog_model(model: str) -> str:
    _, _, name = model.partition("/")
    return name or model


def _transfer_label(config: OptimizeConfig) -> dict[str, str]:
    if config.live:
        return {"modelFamily": _catalog_model(config.model), "lane": config.lane}
    return {"modelFamily": "fixture", "lane": "fixture"}


def _dev_tasks(root: Path, config: OptimizeConfig) -> list[GymTask]:
    suite_path = root / "bench" / "suites" / f"{config.dev_suite}.suite.json"
    if not suite_path.is_file():
        raise OptimizeError(f"dev suite manifest missing: {suite_path}")
    ids = load_suite_task_ids(suite_path)
    return [
        GymTask(task_id=task_id, suite=config.dev_suite, model=config.model, lane=config.lane)
        for task_id in ids
    ]


def run_optimize(config: OptimizeConfig) -> dict[str, Any]:
    if config.max_metric_calls < 1:
        raise OptimizeError("max_metric_calls must be >= 1")
    if config.live and config.dry_run:
        raise OptimizeError("choose --live or --dry-run, not both")
    if config.dev_suite == HOLDOUT_SUITE_ID and not config.confirm_holdout:
        raise HoldoutScreenError(
            "dev suite is tb2-cross-section; the optimizer must not screen on the holdout (ledger O3)"
        )
    if config.confirm_holdout:
        raise OptimizeError(
            "holdout confirmation is not implemented in this packet; do not run tb2-cross-section"
        )
    root = Path(config.repo_root or repo_root_from())
    seed = load_seed_candidate(root)
    fixture = Path(config.fixture_job or DEFAULT_FIXTURE)
    if not config.live and not fixture.is_dir():
        raise OptimizeError(f"fixture job missing: {fixture}")

    adapter = GymAdapter(
        fixture_job_dir=None if config.live else fixture,
        live=config.live,
        repo_root=root,
        model=config.model,
        lane=config.lane,
        jobs_parent=Path(config.output) / "jobs",
        max_metric_calls=config.max_metric_calls,
        allow_holdout_screen=False,
    )
    tasks = _dev_tasks(root, config)[: config.max_metric_calls]
    produced_by = f"bench.optimize:{config.engine}:max_metric_calls={config.max_metric_calls}"
    harbor = "live" if config.live else "fixture"
    residual = None
    gepa_used = False
    proposed = dict(seed)

    if config.engine == "gepa":
        if config.dry_run or not config.live:
            raise OptimizeError(
                "upstream gepa search needs --engine gepa --live; "
                "the dry-run path evaluates the seed through the metric without search"
            )
        proposed, gepa_used = _run_gepa(adapter, seed, tasks, config)
    else:
        adapter.evaluate(tasks, seed, capture_traces=True)

    job = adapter.job
    if job is None and not config.live:
        job = load_job(fixture)
    if job is None:
        raise OptimizeError("optimizer produced no Harbor job to score")
    job_score = score_job(
        job,
        suite=config.dev_suite,
        max_metric_calls=config.max_metric_calls,
        tasks=[task.task_id for task in tasks],
    )

    mutated = proposed != seed
    if mutated:
        disposition = "candidate_emitted"
        summary = "GEPA mutation of staged coder text surfaces, scored on the Gym metric."
        risk = (
            "Prompt-class text is family-specific (ledger O5). This packet does not land "
            "the diff; a later cycle applies it under the runbook."
        )
        parent = None
    else:
        disposition = "no_beat_incumbent"
        summary = (
            "Seed evaluation of staged surfaces from #122; no mutation beat the incumbent "
            f"within max_metric_calls={config.max_metric_calls}."
        )
        risk = (
            "This is the incumbent snapshot, not a proposed landing. Empty surfaces mean "
            "the optimizer did not produce a diff."
        )
        parent = None

    if not config.live:
        residual = "no live Harbor cycle in this packet"

    candidate = build_candidate(
        seed=seed,
        proposed=proposed,
        job_score=job_score,
        produced_by=produced_by,
        transfer_label=_transfer_label(config),
        parent=parent,
        summary=summary,
        risk=risk,
    )
    run = {
        "schema": "openagents.coder_optimizer_run.v1",
        "producedAt": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "disposition": disposition,
        "harbor": harbor,
        "engine": config.engine,
        "gepaImported": gepa_used,
        "maxMetricCalls": config.max_metric_calls,
        "metricCalls": job_score.metric_calls,
        "devSuite": config.dev_suite,
        "holdoutSuite": config.holdout_suite,
        "holdoutRan": False,
        "incumbentScore": job_score.mean_score,
        "bestScore": job_score.mean_score,
        "beatsIncumbent": mutated,
        "acceptance": acceptance_argument(
            suite=config.dev_suite,
            trials=max(1, job_score.metric_calls),
            harbor=harbor,
        ),
        "metric": job_score.as_dict(),
        "transferLabel": _transfer_label(config),
        "seedSurfaces": list(seed.keys()),
        "residual": residual,
        "writes": "output directory only; surfaces/coder was not modified",
    }
    write_run(Path(config.output), run=run, candidates=[candidate])
    return {"run": run, "candidate": candidate, "output": str(Path(config.output))}


def _run_gepa(
    adapter: GymAdapter,
    seed: dict[str, str],
    tasks: list[GymTask],
    config: OptimizeConfig,
) -> tuple[dict[str, str], bool]:
    try:
        from gepa import optimize
    except ImportError as error:
        raise OptimizeError(
            "gepa is not installed. In a venv: pip install -r bench/optimize/requirements.txt"
        ) from error
    if not config.reflection_lm:
        raise OptimizeError("--reflection-lm is required with --engine gepa")
    result = optimize(
        seed_candidate=seed,
        trainset=tasks,
        valset=tasks,
        adapter=adapter,
        reflection_lm=config.reflection_lm,
        max_metric_calls=config.max_metric_calls,
        display_progress_bar=False,
    )
    best = getattr(result, "best_candidate", None) or seed
    if not isinstance(best, dict):
        raise OptimizeError("gepa best_candidate was not a dict[str, str]")
    return best, True
