"""The single entry point: ``tbench``.

``uv run tbench <command>`` covers the whole benchmark surface: doctor,
tasks, run, resume, inspect, compare. Every subcommand resolves the
checked profiles, so an invocation that reaches Harbor has already been
through the pin checks.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

from . import UPSTREAM_GIT_URL, paths
from .agents import load_agents
from .compare import SMALL_SAMPLE_LABEL, compare, render_table
from .doctor import run_doctor
from .jobconfig import list_job_profiles, load_job_profile
from .panel import load_panel
from .results import TrialPaths, load_trial_results
from .runner import RunError, RunRequest, collect, materialize, resume, run


def _load(profile_id: str | None, agent_id: str | None) -> RunRequest | None:
    panel = load_panel()
    agents = load_agents()
    if agent_id is None or profile_id is None:
        return None
    profile = load_job_profile(profile_id)
    agent = agents.get(agent_id)
    if agent is None:
        raise RunError(
            f"unknown agent {agent_id!r}; known: {', '.join(sorted(agents))}"
        )
    try:
        tasks = panel.select(profile.task_ids)
    except (KeyError, ValueError) as exc:
        raise RunError(str(exc)) from exc
    checkout = paths.upstream_checkout()
    if not (checkout / ".git").exists():
        checkout = None
    return RunRequest(
        panel=panel,
        profile=profile,
        agent=agent,
        tasks=tasks,
        checkout=checkout,
    )


def cmd_doctor(args: argparse.Namespace) -> int:
    panel = load_panel()
    agents = load_agents()
    selected = (
        [agents[name] for name in args.agent if name in agents]
        if args.agent
        else list(agents.values())
    )
    report = run_doctor(panel, selected, online=args.smoke)
    for check in report.checks:
        print(check.line())
    print()
    if report.ok:
        print(f"doctor: {report.worst()}")
        return 0 if report.worst() == "pass" else 0
    print("doctor: blockers found")
    return 1


def cmd_tasks(args: argparse.Namespace) -> int:
    if args.tasks_command == "checkout":
        checkout = paths.upstream_checkout()
        commit = load_panel().git_commit_id
        checkout.parent.mkdir(parents=True, exist_ok=True)
        if not (checkout / ".git").exists():
            subprocess.run(
                [
                    "git",
                    "clone",
                    "--filter=blob:none",
                    UPSTREAM_GIT_URL,
                    str(checkout),
                ],
                check=True,
            )
        subprocess.run(
            ["git", "-C", str(checkout), "fetch", "origin", commit],
            check=True,
        )
        subprocess.run(
            ["git", "-C", str(checkout), "checkout", commit],
            check=True,
        )
        print(f"upstream checkout at {checkout} ({commit[:12]})")
        return 0

    panel = load_panel()
    ids = set()
    if args.profile:
        ids = set(load_job_profile(args.profile).task_ids)
    for task in panel.tasks:
        if ids and task.id not in ids:
            continue
        excluded = " [excluded]" if task.excluded else ""
        images = ", ".join(task.images) if task.images else "no image pin"
        print(
            f"{task.id:<32} {task.path:<42} {images}{excluded}"
        )
    return 0


def cmd_profiles(args: argparse.Namespace) -> int:
    for profile_id, description in list_job_profiles().items():
        print(f"{profile_id:<24} {description}")
    return 0


def _request(args: argparse.Namespace) -> RunRequest:
    request = _load(args.profile, args.agent)
    assert request is not None
    if getattr(args, "task", None):
        try:
            request.tasks = request.panel.select(args.task)
        except (KeyError, ValueError) as exc:
            raise RunError(str(exc)) from exc
        # A narrowed task set is a different job, not a resume of the
        # profile's; give it a distinct deterministic name.
        request.job_name = args.job_name or (
            f"{request.profile.id}--{request.agent.id}--"
            + "_".join(task.id for task in request.tasks)
        )
    request_kwargs = {}
    for pair in args.agent_kwarg or []:
        key, _, value = pair.partition("=")
        if not value:
            raise RunError(f"--agent-kwarg wants key=value, got {pair!r}")
        request_kwargs[key] = value
    request.agent_kwargs = request_kwargs or None
    request.auth_mode = args.auth_mode
    if args.job_name:
        request.job_name = args.job_name
    return request


def cmd_run(args: argparse.Namespace) -> int:
    try:
        job_dir = run(_request(args))
    except RunError as exc:
        print(f"run: {exc}", file=sys.stderr)
        return 1
    print(f"job dir: {job_dir}")
    return 0


def cmd_resume(args: argparse.Namespace) -> int:
    try:
        job_dir = resume(_request(args))
    except RunError as exc:
        print(f"resume: {exc}", file=sys.stderr)
        return 1
    print(f"job dir: {job_dir}")
    return 0


def cmd_inspect(args: argparse.Namespace) -> int:
    job_dir = Path(args.job)
    if not job_dir.is_dir():
        candidate = paths.jobs_dir() / args.job
        if candidate.is_dir():
            job_dir = candidate
        else:
            print(f"inspect: no job dir at {job_dir}", file=sys.stderr)
            return 1
    results = load_trial_results(job_dir)
    if not results:
        print(f"{job_dir}: no trial results yet")
        return 0
    for trial_dir, result in results:
        verifier = result.get("verifier_result") or {}
        rewards = verifier.get("rewards") or {}
        exception = result.get("exception_info")
        status = (
            f"exception:{exception.get('exception_type')}"
            if exception
            else "ok"
        )
        print(
            f"{trial_dir.name:<48} "
            f"reward={rewards.get('reward', '?')} {status} "
            f"agent={result.get('agent_execution', {}).get('finished_at', '?')}"
        )
    report = TrialPaths(job_dir).report_path
    if report.exists():
        print(f"\nreport: {report}")
    return 0


def cmd_compare(args: argparse.Namespace) -> int:
    jobs_dir = Path(args.jobs_dir or paths.jobs_dir())
    report = compare(
        jobs_dir,
        arms=args.arm or None,
        label=args.label or SMALL_SAMPLE_LABEL,
    )
    out = Path(args.out) if args.out else jobs_dir / "tbench-report.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, indent=2) + "\n")
    print(render_table(report))
    print(f"\nwrote {out}")
    return 0


def cmd_materialize(args: argparse.Namespace) -> int:
    """Write the resolved job config without starting Harbor."""
    try:
        job_dir, config = materialize(_request(args))
    except RunError as exc:
        print(f"materialize: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(config, indent=2))
    print(f"# wrote {TrialPaths(job_dir).config_path}", file=sys.stderr)
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="tbench",
        description=(
            "Pinned local Terminal-Bench runs: doctor, run, resume, "
            "inspect, compare. One concurrent trial by default; fresh "
            "task environments every attempt."
        ),
    )
    sub = parser.add_subparsers(dest="command", required=True)

    doctor = sub.add_parser("doctor", help="read-only preflight checks")
    doctor.add_argument(
        "--smoke",
        action="store_true",
        help="include checks that reach the registry and network",
    )
    doctor.add_argument(
        "--agent",
        action="append",
        help="check credentials for these arms only",
    )
    doctor.set_defaults(func=cmd_doctor)

    tasks = sub.add_parser("tasks", help="panel inspection and checkout")
    tasks_sub = tasks.add_subparsers(dest="tasks_command")
    tasks_sub.add_parser(
        "checkout", help="clone or pin the upstream task checkout"
    )
    tasks_list = tasks_sub.add_parser("list", help="list the task panel")
    tasks_list.add_argument("--profile", help="filter to a job profile")
    tasks.set_defaults(func=cmd_tasks)

    profiles = sub.add_parser("profiles", help="list job profiles")
    profiles.set_defaults(func=cmd_profiles)

    def add_run_args(p: argparse.ArgumentParser) -> None:
        p.add_argument("--profile", required=True, help="job profile id")
        p.add_argument("--agent", required=True, help="agent profile id")
        p.add_argument("--auth-mode", help="pick one configured auth mode")
        p.add_argument(
            "--agent-kwarg",
            action="append",
            help="key=value adapter kwargs (e.g. artifact_sha256=...)",
        )
        p.add_argument(
            "--task",
            action="append",
            help="narrow the profile to these task ids",
        )
        p.add_argument("--job-name", help="override the deterministic job name")

    for name, func, helptext in (
        ("run", cmd_run, "materialize and run a pinned job"),
        ("resume", cmd_resume, "resume an existing job dir"),
        (
            "materialize",
            cmd_materialize,
            "write the resolved job config without running",
        ),
    ):
        p = sub.add_parser(name, help=helptext)
        add_run_args(p)
        p.set_defaults(func=func)

    inspect = sub.add_parser("inspect", help="show one job's trial results")
    inspect.add_argument("job", help="job name or job dir path")
    inspect.set_defaults(func=cmd_inspect)

    cmp_parser = sub.add_parser(
        "compare", help="fold attempts into a comparison report"
    )
    cmp_parser.add_argument("--jobs-dir", help="jobs dir to scan")
    cmp_parser.add_argument("--arm", action="append", help="arms to include")
    cmp_parser.add_argument("--label", help="report label")
    cmp_parser.add_argument("--out", help="where to write the report JSON")
    cmp_parser.set_defaults(func=cmd_compare)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    return int(args.func(args) or 0)


if __name__ == "__main__":
    sys.exit(main())
