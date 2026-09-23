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
from .panel import catalog_names, load_panel
from .results import TrialPaths, load_trial_results
from .retain import MAX_FILE_BYTES, MAX_TRIAL_BYTES, retain_jobs
from .runner import RunError, RunRequest, collect, materialize, resume, run


def _load(profile_id: str | None, agent_id: str | None) -> RunRequest | None:
    agents = load_agents()
    if agent_id is None or profile_id is None:
        return None
    profile = load_job_profile(profile_id)
    panel = load_panel(catalog=profile.catalog)
    agent = agents.get(agent_id)
    if agent is None:
        raise RunError(
            f"unknown agent {agent_id!r}; known: {', '.join(sorted(agents))}"
        )
    try:
        tasks = panel.select(profile.task_ids)
    except (KeyError, ValueError) as exc:
        raise RunError(str(exc)) from exc
    checkout = panel.checkout()
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


def checkout_panel(panel) -> Path:
    """Clone or pin one panel's upstream checkout at its commit."""
    checkout = panel.checkout()
    commit = panel.git_commit_id
    checkout.parent.mkdir(parents=True, exist_ok=True)
    if not (checkout / ".git").exists():
        subprocess.run(
            [
                "git",
                "clone",
                "--filter=blob:none",
                "--no-checkout",
                panel.git_url or UPSTREAM_GIT_URL,
                str(checkout),
            ],
            check=True,
        )
    subprocess.run(
        ["git", "-C", str(checkout), "fetch", "origin", commit],
        check=True,
    )
    subprocess.run(
        ["git", "-C", str(checkout), "checkout", "--detach", commit],
        check=True,
    )
    return checkout


def cmd_tasks(args: argparse.Namespace) -> int:
    if args.tasks_command == "checkout":
        # Every pinned ref: the panel's own, then each catalog's, each in
        # its own directory so the panel's pin never moves.
        names = [None, *catalog_names()]
        if args.catalog:
            names = [None if args.catalog == "panel" else args.catalog]
        for name in names:
            panel = load_panel(catalog=name)
            checkout = checkout_panel(panel)
            print(
                f"{name or 'panel'}: upstream checkout at {checkout} "
                f"({panel.git_commit_id[:12]}"
                + (f", {panel.ref}" if panel.ref else "")
                + ")"
            )
        return 0

    catalog = None
    ids: set[str] = set()
    if args.profile:
        profile = load_job_profile(args.profile)
        catalog = profile.catalog
        ids = set(profile.task_ids)
    panel = load_panel(catalog=catalog)
    for task in panel.tasks:
        if ids and task.id not in ids:
            continue
        flags = []
        if task.excluded:
            flags.append("excluded")
        if task.requires_gpu_runtime:
            flags.append("needs a GPU")
        res = task.peak_resources
        where = task.base_image or (
            ", ".join(task.images) if task.images else "no image pin"
        )
        print(
            f"{task.id:<32} {res.cpus:>2} CPU {res.memory_mb // 1024:>3} GiB "
            f"{task.agent_timeout_sec:>6}s  {where}"
            + (f" [{'; '.join(flags)}]" if flags else "")
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


def cmd_collect(args: argparse.Namespace) -> int:
    """Rewrite one job's attempt records and manifests from its trials."""
    job_dir = Path(args.job)
    if not job_dir.is_dir():
        job_dir = paths.jobs_dir() / args.job
    context_path = TrialPaths(job_dir).context_path
    try:
        context = json.loads(context_path.read_text())
    except (OSError, json.JSONDecodeError):
        print(
            f"collect: no readable {context_path}; run or resume the job "
            "through tbench first",
            file=sys.stderr,
        )
        return 1
    try:
        request = _load(context["profile_id"], context["agent_id"])
    except RunError as exc:
        print(f"collect: {exc}", file=sys.stderr)
        return 1
    assert request is not None
    request.auth_mode = context.get("auth_mode")
    written = collect(job_dir, request)
    print(f"collect: wrote {len(written) // 2} attempt records under {job_dir}")
    return 0


def cmd_retain(args: argparse.Namespace) -> int:
    """Copy jobs' evidence closures into the retained traces."""
    retained, errors = retain_jobs(
        args.job,
        traces_dir=Path(args.traces_dir) if args.traces_dir else None,
        trials=args.trial or None,
        max_file_bytes=args.max_file_bytes,
        max_trial_bytes=args.max_trial_bytes,
        dry_run=args.dry_run,
    )
    total = 0
    for item in retained:
        total += item.retained_bytes
        scan = item.record["credential_scan"]
        print(
            f"{item.job}/{item.trial}: {len(item.record['files'])} files, "
            f"{item.retained_bytes} bytes, {len(item.missing)} missing, "
            f"credential scan clean over {len(scan['credentials_checked'])} "
            "values"
        )
        for missing in item.missing:
            print(
                f"  missing {missing['kind']}: {missing['reference']} "
                f"({missing['reason']})"
            )
        for entry in item.record["files"]:
            if entry["digest"] == "mismatch":
                print(f"  digest mismatch: {entry['path']}")
    for error in errors:
        print(f"retain: {error}", file=sys.stderr)
    verb = "checked" if args.dry_run else "retained"
    print(f"retain: {verb} {len(retained)} trials, {total} bytes")
    return 1 if errors else 0


def cmd_toolchain(args: argparse.Namespace) -> int:
    """Build or list the prebuilt toolchain layers outside any trial."""
    from .toolchain import (
        ToolchainError,
        cache_root,
        ensure_layer,
        layers_for,
    )

    if args.toolchain_command == "check":
        from .toolchain import check_image

        profile = load_job_profile(args.profile)
        panel = load_panel(catalog=profile.catalog)
        images = args.image or sorted(
            {
                task.base_image
                for task in panel.tasks
                if task.id in profile.task_ids and task.base_image
            }
        )
        failures = 0
        for image in images:
            result = check_image(image, args.executor, args.version)
            failures += 0 if result["ok"] else 1
            print(
                f"{'ok  ' if result['ok'] else 'FAIL'} {image}: "
                f"{result['platform'] or '?'}, {result['detail']}"
            )
        print(f"toolchain check: {len(images) - failures}/{len(images)} images ok")
        return 1 if failures else 0
    if args.toolchain_command == "list":
        root = cache_root()
        for manifest in sorted(root.glob("*/layer.json")):
            layer = json.loads(manifest.read_text())
            print(
                f"{layer['key']:<40} {layer['bytes']:>12} bytes  "
                f"{layer['tree_sha256'][:12]}  built {layer['built_at']}"
            )
        return 0
    try:
        for spec in layers_for(args.executor, args.version, args.platform):
            layer = ensure_layer(spec)
            print(
                f"{spec.key}: {layer.cache}, {layer.build_ms} ms, "
                f"{layer.manifest['bytes']} bytes, "
                f"tree {layer.manifest['tree_sha256'][:12]}"
            )
    except ToolchainError as exc:
        print(f"toolchain: {exc}", file=sys.stderr)
        return 1
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


def _suite_scheduler(args: argparse.Namespace, *, dry_run: bool = False):
    from . import suite

    request = _load(args.profile, args.agent)
    assert request is not None
    panel = request.panel
    tasks = list(panel.tasks) if not request.profile.task_ids else [
        panel.task(task_id) for task_id in request.profile.task_ids
    ]
    if args.tasks:
        wanted = [t.strip() for part in args.tasks for t in part.split(",") if t.strip()]
        tasks = [panel.task(task_id) for task_id in wanted]
    excluded = [task for task in tasks if task.excluded]
    if excluded:
        raise RunError(
            "excluded tasks can't run: "
            + "; ".join(f"{t.id}: {t.excluded_reason_text}" for t in excluded)
        )
    checkout = panel.checkout()
    if not (checkout / ".git").exists() and not dry_run:
        raise RunError(
            f"no task checkout at {checkout}; run `tbench tasks checkout` first"
        )
    directory = suite.suite_dir(request.profile.id, request.agent.id)
    extra: list[str] = []
    if args.auth_mode:
        extra += ["--auth-mode", args.auth_mode]
    for pair in args.agent_kwarg or []:
        extra += ["--agent-kwarg", pair]
    budget = suite.Budget(
        max_cpus=args.max_cpus,
        max_mem_gb=args.max_mem_gb,
        min_free_disk_gb=args.min_free_disk_gb,
        prune_margin_gb=args.prune_margin_gb,
        max_concurrent=args.max_concurrent,
        max_gpus=args.max_gpus,
    )
    launcher = suite.Launcher(
        profile=request.profile.id,
        arm=request.agent.id,
        logs=directory / "logs",
        extra_args=extra,
    )
    host_ = suite.Host.docker(checkout)
    if dry_run:
        host_.remove_images = lambda names: []
        host_.prune_build_cache = lambda: False
    return suite.Scheduler(
        profile=request.profile.id,
        arm=request.agent.id,
        pin={
            "git_url": panel.git_url,
            "git_commit_id": panel.git_commit_id,
            "ref": panel.ref,
            "catalog": panel.catalog,
        },
        tasks=tasks,
        attempts=args.attempts,
        budget=budget,
        jobs_dir=paths.jobs_dir(),
        directory=directory,
        launcher=launcher,
        host_=host_,
    )


def cmd_suite(args: argparse.Namespace) -> int:
    """Run, plan, inspect, or stop a whole-suite schedule."""
    import signal as signals

    from . import suite

    if args.suite_command == "status":
        status = suite.read_status(args.profile, args.agent)
        if status is None:
            print(
                f"suite: no status for {args.profile} / {args.agent} under "
                f"{suite.suite_dir(args.profile, args.agent)}",
                file=sys.stderr,
            )
            return 1
        if args.json:
            print(json.dumps(status, indent=2))
        else:
            for line in suite.status_lines(status):
                print(line)
        return 0
    if args.suite_command == "stop":
        lock = suite.suite_dir(args.profile, args.agent) / "lock"
        try:
            pid = int(lock.read_text().strip())
        except (OSError, ValueError):
            print("suite: no scheduler pid recorded", file=sys.stderr)
            return 1
        if not suite.pid_alive(pid):
            print(f"suite: scheduler {pid} isn't running")
            return 0
        os.kill(pid, signals.SIGTERM)
        print(
            f"suite: asked scheduler {pid} to stop; it interrupts its trials "
            "and exits once they have cleaned up"
        )
        return 0
    try:
        scheduler = _suite_scheduler(args, dry_run=args.suite_command == "plan")
    except (RunError, KeyError, ValueError) as exc:
        print(f"suite: {exc}", file=sys.stderr)
        return 1
    if args.suite_command == "plan":
        scheduler.launcher.start = lambda trial, verb: 0  # type: ignore[method-assign]
        scheduler.echo = False
        scheduler.reconcile()
        started = scheduler.launch_ready()
        status = scheduler.status()
        print(
            " · ".join(f"{k} {v}" for k, v in sorted(status["counts"].items()))
            + f" · free disk {status['free_disk_gb']} GiB"
        )
        for trial in scheduler.trials:
            if trial.state == suite.SKIPPED:
                print(f"skip  {trial.job}: {trial.reason}")
        for trial in started:
            print(
                f"start {trial.job} ({trial.cpus} CPUs, {trial.memory_gb:g} GiB"
                + (f", {trial.gpus} GPU" if trial.gpus else "")
                + ")"
            )
        print(
            f"plan: the first wave starts {len(started)} trials; "
            "nothing was started"
        )
        return 0
    if args.detach:
        directory = suite.suite_dir(scheduler.profile, scheduler.arm)
        directory.mkdir(parents=True, exist_ok=True)
        argv = [a for a in sys.argv[1:] if a != "--detach"]
        log = (directory / "scheduler.log").open("ab")
        process = subprocess.Popen(
            [sys.executable, "-m", "tbench", *argv],
            stdout=log,
            stderr=subprocess.STDOUT,
            stdin=subprocess.DEVNULL,
            start_new_session=True,
            cwd=paths.PACKAGE_DIR,
        )
        print(
            f"suite: scheduler {process.pid} runs in the background; "
            f"status {directory / 'status.json'}, log {directory / 'scheduler.log'}"
        )
        return 0
    try:
        with suite.suite_lock(scheduler.directory):
            for signum in (signals.SIGINT, signals.SIGTERM, signals.SIGHUP):
                signals.signal(signum, lambda *_: scheduler.stop())
            status = scheduler.run(interval=args.interval)
    except suite.SuiteError as exc:
        print(f"suite: {exc}", file=sys.stderr)
        return 1
    for line in suite.status_lines(status):
        print(line)
    return 0


def cmd_reference(args: argparse.Namespace) -> int:
    """Fetch the public TB4 leaderboard's per-task results."""
    from .reference import HarborHubReader, fetch_reference, write_reference

    panel = load_panel(catalog="tb4")
    document = fetch_reference(
        HarborHubReader(), task_names=[task.id for task in panel.tasks]
    )
    path = write_reference(document, Path(args.out) if args.out else None)
    for entry in document["entries"]:
        check = entry["per_task"]
        print(
            f"{entry['rank']!s:>3} {entry['agent'] or '?'} / {entry['model'] or '?'}"
            f" ({entry['reasoning_effort']}): {entry['metrics']['successes']}/"
            f"{entry['metrics']['n_trials']}, per task "
            f"{check['successes_counted']}/{check['trials_counted']}"
            + ("" if check["consistent"] else " [differs from the row's metrics]")
        )
    print(f"wrote {path}")
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
    checkout_parser = tasks_sub.add_parser(
        "checkout",
        help="clone or pin every upstream task checkout (panel and catalogs)",
    )
    checkout_parser.add_argument(
        "--catalog", help="pin only this one: panel, or a catalog such as tb4"
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

    collect_parser = sub.add_parser(
        "collect",
        help="rewrite one job's attempt records from its trial results",
    )
    collect_parser.add_argument("job", help="job name or job dir path")
    collect_parser.set_defaults(func=cmd_collect)

    retain_parser = sub.add_parser(
        "retain",
        help="copy jobs' full evidence closures into the retained traces",
    )
    retain_parser.add_argument(
        "job", nargs="+", help="job names or job dir paths"
    )
    retain_parser.add_argument(
        "--trial", action="append", help="retain only these trials"
    )
    retain_parser.add_argument("--traces-dir", help="where retained traces go")
    retain_parser.add_argument(
        "--max-file-bytes",
        type=int,
        default=MAX_FILE_BYTES,
        help="report, don't copy, a file above this size",
    )
    retain_parser.add_argument(
        "--max-trial-bytes",
        type=int,
        default=MAX_TRIAL_BYTES,
        help="report files past this total per trial",
    )
    retain_parser.add_argument(
        "--dry-run",
        action="store_true",
        help="copy to scratch and scan without writing",
    )
    retain_parser.set_defaults(func=cmd_retain)

    toolchain_parser = sub.add_parser(
        "toolchain", help="build or list prebuilt agent toolchain layers"
    )
    toolchain_sub = toolchain_parser.add_subparsers(
        dest="toolchain_command", required=True
    )
    build = toolchain_sub.add_parser(
        "build", help="build the layers one executor needs, or reuse them"
    )
    build.add_argument(
        "--executor", required=True, choices=("codex", "claude-code")
    )
    build.add_argument("--version", required=True, help="the pinned CLI version")
    build.add_argument(
        "--platform",
        default="linux-x64",
        help="linux-x64, linux-arm64, or a -musl variant",
    )
    toolchain_sub.add_parser("list", help="list the cached layers")
    check = toolchain_sub.add_parser(
        "check",
        help="place the layers in each base image of a profile and run them",
    )
    check.add_argument("--profile", required=True, help="job profile id")
    check.add_argument(
        "--executor", required=True, choices=("codex", "claude-code")
    )
    check.add_argument("--version", required=True, help="the pinned CLI version")
    check.add_argument(
        "--image", action="append", help="check only these base images"
    )
    toolchain_parser.set_defaults(func=cmd_toolchain)

    suite_parser = sub.add_parser(
        "suite",
        help="run a profile's whole task suite within the host's budgets",
    )
    suite_sub = suite_parser.add_subparsers(dest="suite_command", required=True)
    for name, helptext in (
        ("run", "schedule every trial; resumable and safe to restart"),
        ("plan", "show what a run would start first, without starting it"),
        ("status", "print a suite's status file"),
        ("stop", "stop a running scheduler; its trials cancel cleanly"),
    ):
        p = suite_sub.add_parser(name, help=helptext)
        p.add_argument("--profile", required=True, help="job profile id")
        p.add_argument("--agent", required=True, help="agent profile id")
        if name == "status":
            p.add_argument("--json", action="store_true", help="print JSON")
        if name not in ("run", "plan"):
            continue
        p.add_argument(
            "--attempts", type=int, default=1, help="trials per task (default 1)"
        )
        p.add_argument(
            "--tasks",
            action="append",
            help="only these task ids, comma-separated or repeated",
        )
        p.add_argument("--auth-mode", help="pick one configured auth mode")
        p.add_argument(
            "--agent-kwarg", action="append", help="key=value adapter kwargs"
        )
        p.add_argument("--max-cpus", type=float, default=24)
        p.add_argument("--max-mem-gb", type=float, default=100)
        p.add_argument("--min-free-disk-gb", type=float, default=40)
        p.add_argument(
            "--prune-margin-gb",
            type=float,
            default=20,
            help="prune a finished task's images within this much of the floor",
        )
        p.add_argument("--max-concurrent", type=int, help="cap on trials at once")
        p.add_argument(
            "--max-gpus", type=int, default=1, help="GPU trials at once (default 1)"
        )
        p.add_argument(
            "--interval", type=float, default=15.0, help="seconds between polls"
        )
        if name == "run":
            p.add_argument(
                "--detach",
                action="store_true",
                help="run the scheduler in the background and return",
            )
    suite_parser.set_defaults(func=cmd_suite)

    reference_parser = sub.add_parser(
        "reference",
        help="fetch the TB4 leaderboard's per-task results from the Harbor Hub",
    )
    reference_parser.add_argument(
        "--out", help="where to write (default reference/tb4-leaderboard.json)"
    )
    reference_parser.set_defaults(func=cmd_reference)

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
