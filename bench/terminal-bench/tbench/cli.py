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

from . import UPSTREAM_GIT_URL, memcap, paths
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
    from . import suite, usage_limit

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
        order=args.order,
        allow_oversize=args.allow_oversize,
        max_claude_concurrent=args.max_claude_concurrent,
        usage_backoff_sec=args.usage_backoff_min * 60,
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
        providers=usage_limit.arm_providers(request.agent),
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
            holder = suite.gpu_slot_holder(suite.gpu_lock_path())
            print(
                "  host GPU slot: "
                + (f"held by {holder.get('job')}" if holder else "free")
            )
            slots = (status.get("budget") or {}).get("max_claude_concurrent") or 0
            if slots:
                holders = suite.claude_slot_holders(suite.claude_slot_dir(), slots)
                print(
                    f"  host Claude slots: {len(holders)}/{slots} held"
                    + (
                        " by " + ", ".join(h.get("job", "unknown") for h in holders)
                        if holders
                        else ""
                    )
                )
            import time

            for name, pause in sorted(suite.read_pauses(suite.pause_path()).items()):
                if float(pause.get("until") or 0) > time.time():
                    print(
                        f"  host pause: {name} until {pause.get('until_iso')} "
                        f"({pause.get('reason')})"
                    )
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
        scheduler.launcher.start = lambda trial, verb, hold=None: 0  # type: ignore[method-assign]
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
            memcap.scoped(
                [sys.executable, "-m", "tbench", *argv],
                memcap.cap(memcap.SCHEDULER_ENV, memcap.SCHEDULER_MAX),
            ),
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


def _experiment_spec(args: argparse.Namespace):
    """The experiment's spec: pinned on disk, or built from the flags."""
    from . import experiment

    pinned = experiment.load_spec(args.id)
    arm_args: dict[str, list[str]] = {}
    for pair in args.arm_kwarg or []:
        arm, _, kwarg = pair.partition(":")
        if not kwarg or "=" not in kwarg:
            raise RunError(f"--arm-kwarg wants ARM:key=value, got {pair!r}")
        arm_args.setdefault(arm, []).extend(["--agent-kwarg", kwarg])
    tasks = [t.strip() for part in args.tasks or [] for t in part.split(",") if t.strip()]
    stop = {
        "stop_early": args.stop_early,
        "stop_alpha": args.stop_alpha,
        "accept_pass_rate": args.accept_pass_rate,
    }
    prior = (
        experiment.Spec.from_pinned(
            pinned, args.quota_usd, recorded_status=experiment.read_status(args.id), **stop
        ) if pinned is not None else None
    )
    if prior is not None:
        stop = {
            "stop_early": prior.stop_early,
            "stop_alpha": prior.stop_alpha,
            "accept_pass_rate": prior.accept_pass_rate,
        }
    else:
        stop["stop_early"] = True if args.stop_early is None else args.stop_early
        stop["stop_alpha"] = 0.05 if args.stop_alpha is None else args.stop_alpha
    if pinned is not None and not (args.profile or args.arm or tasks):
        spec = prior
    else:
        if not (args.profile and args.arm and tasks):
            raise RunError(
                f"experiment {args.id} isn't pinned yet; give --profile, --arm "
                "(twice or more), and --tasks"
            )
        arms: list[str] = []
        arm_profiles: dict[str, str] = {}
        for given in args.arm:
            name, _, profile = given.partition("=")
            arms.append(name)
            if profile:
                arm_profiles[name] = profile
        spec = experiment.Spec(
            id=args.id,
            profile=args.profile,
            arms=arms,
            tasks=tasks,
            attempts=args.attempts,
            arm_args=arm_args,
            quota_usd=args.quota_usd,
            arm_profiles=arm_profiles,
            **stop,
        )
    spec.validate()
    for name in spec.arm_args:
        if name not in spec.arms:
            raise RunError(f"--arm-kwarg names {name!r}, which isn't an arm")
    return spec


def _experiment_credentials(
    arm_agents: dict, providers: dict, *, allow_login: bool, strict: bool
) -> tuple[str | None, list[str]]:
    """Choose the Claude credential and check every arm's, by name only.

    Returns the credential source and warnings. Raises ``RunError`` when
    ``strict`` and a credential is missing. A token's value is set in this
    process's environment for the trials and never printed.
    """
    from . import credentials, suite
    from .runner import RunRefused, _check_credentials

    warnings: list[str] = []
    source = None

    def refuse(message: str) -> None:
        if strict:
            raise RunError(message)
        warnings.append(message)

    claude = any("anthropic" in p for p in providers.values())
    if claude or any(_subscription(agent, providers[arm]) for arm, agent in arm_agents.items()):
        status = credentials.setup_token_status()
        if status["usable"]:
            os.environ["CLAUDE_CODE_OAUTH_TOKEN"] = credentials.read_setup_token()
            source = credentials.SOURCE_SETUP_TOKEN
        elif allow_login:
            token = credentials.login_token(suite.CLAUDE_CREDENTIALS)
            if not token:
                refuse("no Claude login to fall back to; " + credentials.SETUP_TOKEN_HINT)
            else:
                os.environ["CLAUDE_CODE_OAUTH_TOKEN"] = token
                source = credentials.SOURCE_LOGIN
                warnings.append(
                    f"{status['problem']}; running on the expiring Claude login, so "
                    "a login refresh can revoke running trials' token. To avoid "
                    "that, " + credentials.SETUP_TOKEN_HINT
                )
        elif not claude:
            # A Codex-only arm signs in through the subscription mode, which
            # names the Claude token too; without one it runs on what it has.
            warnings.append(f"{status['problem']}; the Codex-only arms run without it")
        else:
            refuse(
                f"{status['problem']}. Claude trials in an experiment use the "
                f"long-lived token: {credentials.SETUP_TOKEN_HINT}. "
                "--allow-login-token runs on the expiring login instead."
            )
    # The Coder One arms' door and Jev keys, from the host's usual files.
    credentials.fill_host_credentials(os.environ)
    for arm, agent in arm_agents.items():
        mode = "subscription-oauth" if source and _subscription(agent, providers[arm]) else None
        try:
            _check_credentials(agent, mode)
        except RunRefused as exc:
            refuse(f"arm {arm}: {exc}")
    return source, warnings


def _subscription(agent, providers: frozenset[str]) -> bool:
    """Whether an arm signs in through the subscription mode: it has one and
    draws on Claude, or on Codex, whose ChatGPT sign-in only that mode forwards."""
    return "subscription-oauth" in agent.auth_modes and bool(
        providers & {"anthropic", "openai"}
    )


def _with_arm_policy(agent, flags: list[str]):
    """The profile as one arm runs it: an arm's ``policy=`` kwarg replaces
    the profile's, so its providers are the ones its own manifest draws on."""
    import dataclasses

    policy = None
    for flag, value in zip(flags, flags[1:]):
        if flag == "--agent-kwarg" and value.startswith("policy="):
            policy = value.partition("=")[2]
    if policy is None:
        return agent
    return dataclasses.replace(agent, kwargs={**dict(agent.kwargs or {}), "policy": policy})


def _experiment_scheduler(args: argparse.Namespace, *, dry_run: bool = False):
    from . import credentials, experiment, suite, usage_limit

    spec = _experiment_spec(args)
    agents = load_agents()
    unknown = [spec.profile_of(arm) for arm in spec.arms if spec.profile_of(arm) not in agents]
    if unknown:
        raise RunError(
            f"unknown arms {', '.join(unknown)}; known: {', '.join(sorted(agents))}"
        )
    profile = load_job_profile(spec.profile)
    panel = load_panel(catalog=profile.catalog)
    tasks = {task_id: panel.task(task_id) for task_id in spec.tasks}
    excluded = [task for task in tasks.values() if task.excluded]
    if excluded:
        raise RunError(
            "excluded tasks can't run: "
            + "; ".join(f"{t.id}: {t.excluded_reason_text}" for t in excluded)
        )
    checkout = panel.checkout()
    if not (checkout / ".git").exists() and not dry_run:
        raise RunError(f"no task checkout at {checkout}; run `tbench tasks checkout` first")
    arm_agents = {arm: agents[spec.profile_of(arm)] for arm in spec.arms}
    providers = {
        arm: usage_limit.arm_providers(_with_arm_policy(agent, spec.arm_args.get(arm, [])))
        for arm, agent in arm_agents.items()
    }
    source, warnings = _experiment_credentials(
        arm_agents, providers, allow_login=args.allow_login_token, strict=not dry_run
    )
    for warning in warnings:
        print(f"experiment: warning: {warning}", file=sys.stderr)
    arm_args = {arm: list(flags) for arm, flags in spec.arm_args.items()}
    if source is not None:
        for arm, agent in arm_agents.items():
            if _subscription(agent, providers[arm]):
                arm_args.setdefault(arm, []).extend(["--auth-mode", "subscription-oauth"])
    directory = experiment.experiment_dir(spec.id)
    if not dry_run:
        experiment.pin(spec, directory)
    budget = suite.Budget(
        max_cpus=args.max_cpus,
        max_mem_gb=args.max_mem_gb,
        min_free_disk_gb=args.min_free_disk_gb,
        max_concurrent=args.max_concurrent,
        max_gpus=args.max_gpus,
        order="listed",
        max_claude_concurrent=args.max_claude_concurrent,
        usage_backoff_sec=args.usage_backoff_min * 60,
    )
    launcher = suite.Launcher(
        profile=spec.profile,
        arm=spec.arms[0],
        logs=directory / "logs",
        arm_args=arm_args,
        arm_profiles=spec.arm_profiles,
        # On the long-lived token, a trial never switches to the login.
        login_fallback=source != credentials.SOURCE_SETUP_TOKEN,
    )
    host_ = suite.Host.docker(checkout)
    if dry_run:
        host_.remove_images = lambda names: []
        host_.prune_build_cache = lambda: False
    return experiment.ExperimentScheduler(
        spec=spec,
        tasks=tasks,
        arm_providers=providers,
        credential_source=source,
        budget=budget,
        jobs_dir=paths.jobs_dir(),
        directory=directory,
        launcher=launcher,
        host_=host_,
        pin_={
            "git_url": panel.git_url,
            "git_commit_id": panel.git_commit_id,
            "ref": panel.ref,
            "catalog": panel.catalog,
        },
    )


def cmd_experiment(args: argparse.Namespace) -> int:
    """Run, plan, inspect, or stop a targeted experiment."""
    import signal as signals

    from . import experiment, suite

    directory = experiment.experiment_dir(args.id)
    if args.experiment_command == "status":
        status = experiment.read_status(args.id)
        if status is None:
            print(f"experiment: no status under {directory}", file=sys.stderr)
            return 1
        if args.json:
            print(json.dumps(status, indent=2))
        else:
            for line in experiment.status_lines(status):
                print(line)
        return 0
    if args.experiment_command == "stop":
        try:
            pid = int((directory / "lock").read_text().strip())
        except (OSError, ValueError):
            print("experiment: no scheduler pid recorded", file=sys.stderr)
            return 1
        if not suite.pid_alive(pid):
            print(f"experiment: scheduler {pid} isn't running")
            return 0
        os.kill(pid, signals.SIGTERM)
        print(f"experiment: asked scheduler {pid} to stop; a restart resumes its trials")
        return 0
    try:
        scheduler = _experiment_scheduler(args, dry_run=args.experiment_command == "plan")
    except (RunError, KeyError, ValueError, experiment.ExperimentError) as exc:
        print(f"experiment: {exc}", file=sys.stderr)
        return 1
    if args.experiment_command == "plan":
        spec = scheduler.spec
        print(
            f"experiment {spec.id}: {len(spec.arms)} arms × {len(spec.tasks)} tasks × "
            f"{spec.attempts} attempts = {len(scheduler.trials)} trials, interleaved; "
            f"Claude credential {scheduler.credential_source or 'none needed'}; "
            + (
                f"Claude quota budget ${spec.quota_usd:.2f}"
                if spec.quota_usd is not None
                else "no Claude quota budget"
            )
            + "; "
            + (
                f"early stopping on (alpha {spec.stop_alpha}"
                + (
                    f", acceptance bar {100 * spec.accept_pass_rate:.0f}%"
                    if spec.accept_pass_rate is not None
                    else ""
                )
                + ")"
                if spec.stop_early
                else "early stopping off"
            )
        )
        for index, trial in enumerate(scheduler.trials, 1):
            print(f"{index:>4}  r{trial.attempt}  {trial.task.id:<32} {trial.arm}")
        print("plan: nothing was started")
        return 0
    if args.detach:
        directory.mkdir(parents=True, exist_ok=True)
        argv = [a for a in sys.argv[1:] if a != "--detach"]
        log = (directory / "scheduler.log").open("ab")
        process = subprocess.Popen(
            memcap.scoped(
                [sys.executable, "-m", "tbench", *argv],
                memcap.cap(memcap.SCHEDULER_ENV, memcap.SCHEDULER_MAX),
            ),
            stdout=log,
            stderr=subprocess.STDOUT,
            stdin=subprocess.DEVNULL,
            start_new_session=True,
            cwd=paths.PACKAGE_DIR,
        )
        print(
            f"experiment: scheduler {process.pid} runs in the background; "
            f"status {directory / 'status.json'}, log {directory / 'scheduler.log'}"
        )
        return 0
    try:
        with suite.suite_lock(scheduler.directory):
            for signum in (signals.SIGINT, signals.SIGTERM, signals.SIGHUP):
                signals.signal(signum, lambda *_: scheduler.stop())
            status = scheduler.run(interval=args.interval)
    except suite.SuiteError as exc:
        print(f"experiment: {exc}", file=sys.stderr)
        return 1
    for line in experiment.status_lines(status):
        print(line)
    return 0


def add_experiment_parser(sub) -> None:
    experiment_parser = sub.add_parser(
        "experiment",
        help="run a targeted experiment: several arms, repeated, interleaved",
    )
    experiment_sub = experiment_parser.add_subparsers(
        dest="experiment_command", required=True
    )
    for name, helptext in (
        ("run", "schedule every trial; resumable and safe to restart"),
        ("plan", "print the interleaved schedule and check credentials; start nothing"),
        ("status", "print an experiment's status file"),
        ("stop", "stop a running experiment; a restart resumes it"),
    ):
        p = experiment_sub.add_parser(name, help=helptext)
        p.add_argument("--id", required=True, help="experiment id, such as v7-vs-cc")
        if name == "status":
            p.add_argument("--json", action="store_true", help="print JSON")
        if name not in ("run", "plan"):
            continue
        p.add_argument("--profile", help="job profile id (pinned on the first run)")
        p.add_argument(
            "--arm",
            action="append",
            help="agent profile id, or NAME=PROFILE to run a profile as a separately "
            "named arm (with --arm-kwarg NAME:key=value); give two or more",
        )
        p.add_argument(
            "--tasks", action="append", help="task ids, comma-separated or repeated"
        )
        p.add_argument(
            "--attempts", type=int, default=3, help="attempts per task per arm (default 3)"
        )
        p.add_argument(
            "--arm-kwarg", action="append", help="ARM:key=value adapter kwarg for one arm"
        )
        p.add_argument(
            "--quota-usd",
            type=float,
            help="Claude quota budget, as the list-price value Claude Code reports; "
            "no Claude trial starts once it's used",
        )
        p.add_argument(
            "--stop-early",
            action=argparse.BooleanOptionalAction,
            default=None,
            help="after every graded trial, stop an arm that is dominated, decided, "
            "undecidable, or can't reach --accept-pass-rate, and end the experiment "
            "when every candidate arm has stopped (default on for new experiments; --no-stop-early runs "
            "every planned attempt)",
        )
        p.add_argument(
            "--stop-alpha",
            type=float,
            default=None,
            help="the stopping rule's two-sided exact McNemar level (default 0.05)",
        )
        p.add_argument(
            "--accept-pass-rate",
            type=float,
            help="the acceptance bar: stop a candidate arm that can't reach this pass "
            "rate even if every open attempt passes (0 to 1; no bar by default)",
        )
        p.add_argument(
            "--allow-login-token",
            action="store_true",
            help="run Claude trials on the expiring login when there is no "
            "long-lived token in ~/.openagents/claude-setup-token",
        )
        p.add_argument("--max-cpus", type=float, default=24)
        p.add_argument("--max-mem-gb", type=float, default=100)
        p.add_argument("--min-free-disk-gb", type=float, default=40)
        p.add_argument("--max-concurrent", type=int, help="cap on trials at once")
        p.add_argument("--max-gpus", type=int, default=1)
        p.add_argument(
            "--max-claude-concurrent",
            type=int,
            default=3,
            help="Claude trials at once across the host (default 3)",
        )
        p.add_argument("--usage-backoff-min", type=float, default=30.0)
        p.add_argument("--interval", type=float, default=15.0)
        if name == "run":
            p.add_argument(
                "--detach",
                action="store_true",
                help="run the scheduler in the background and return",
            )
    experiment_parser.set_defaults(func=cmd_experiment)


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


def cmd_try(args: argparse.Namespace) -> int:
    """Run one arm on a few tasks and report the loop time."""
    from .tryout import prepare, run_try

    try:
        request = _load(args.profile, args.arm)
        assert request is not None
        wanted = [t.strip() for part in args.task for t in part.split(",") if t.strip()]
        request.tasks = request.panel.select(wanted)
    except (RunError, KeyError, ValueError) as exc:
        print(f"try: {exc}", file=sys.stderr)
        return 1
    request.auth_mode = args.auth_mode
    kwargs = {}
    for pair in args.agent_kwarg or []:
        key, _, value = pair.partition("=")
        if not value:
            print(f"try: --agent-kwarg wants key=value, got {pair!r}", file=sys.stderr)
            return 1
        kwargs[key] = value
    request.agent_kwargs = kwargs or None
    prepare(
        request,
        attempts=args.attempts,
        concurrency=args.concurrency,
        warm=not args.cold,
        name=args.job_name,
    )
    _job_dir, _summary, code = run_try(
        request, interval=args.interval, retain=not args.no_retain
    )
    return code


def _job_or_trial(name: str) -> Path | None:
    path = Path(name)
    if not path.exists():
        path = paths.jobs_dir() / name
    return path if path.exists() else None


def cmd_looptime(args: argparse.Namespace) -> int:
    """Print where each trial's time went, for jobs or trial directories."""
    from . import looptime

    rows = []
    for name in args.job:
        path = _job_or_trial(name)
        if path is None:
            print(f"looptime: no job or trial at {name}", file=sys.stderr)
            return 1
        if (path / "result.json").is_file() and (path / "agent").is_dir():
            rows.append(looptime.trial_looptime(path))
        else:
            rows.extend(looptime.job_looptimes(path))
    if args.json:
        print(json.dumps({"totals": looptime.totals(rows), "trials": rows}, indent=2))
    else:
        print(looptime.render(rows))
    return 0


def cmd_replay(args: argparse.Namespace) -> int:
    """Rerun checks, the repair brief, or the verifier on trials' workspaces."""
    import time

    from . import replay

    try:
        trials = [replay.find_trial(ref) for ref in args.trial or []]
        if args.failing:
            trials += replay.failing_trials(args.failing, args.limit)
    except replay.ReplayError as exc:
        print(f"replay: {exc}", file=sys.stderr)
        return 1
    if not trials:
        print("replay: name trials, or --failing MATCH to pick them", file=sys.stderr)
        return 1
    artifact = Path(args.artifact).expanduser() if args.artifact else None
    policy = Path(args.policy).expanduser() if args.policy else None
    if policy is not None and not policy.is_file():
        print(f"replay: no policy manifest at {policy}", file=sys.stderr)
        return 1
    started = time.monotonic()
    echo = (lambda _line: None) if args.json else print
    results = replay.replay_many(
        trials, args.stage, artifact=artifact, jobs=args.jobs, echo=echo, policy=policy
    )
    summary = replay.summary(results, time.monotonic() - started)
    if args.json:
        print(
            json.dumps(
                {"summary": summary, "replays": [r.record() for r in results]},
                indent=2,
            )
        )
    else:
        print(
            f"\nreplay: {summary['replays']} {args.stage} replays in "
            f"{summary['wall_sec']} s, {summary['errors']} errors"
            + (
                f"; checks detected {summary['failing_detected']} of "
                f"{summary['failing']} failing trials and flagged "
                f"{summary['passing_flagged']} of {summary['passing']} passing"
                if args.stage != "verify"
                else ""
            )
        )
        for result in results:
            if result.out:
                print(f"  {result.trial}: {result.out / 'replay.json'}")
    return 1 if summary["errors"] else 0


def cmd_candidate_preflight(args: argparse.Namespace) -> int:
    from .candidate_capture import preflight
    result = preflight(Path(args.task).expanduser())
    print(json.dumps(result, indent=2))
    return 0 if result["supported"] else 1


def cmd_cohort(args: argparse.Namespace) -> int:
    from . import cohort, paths
    output = Path(args.output).expanduser().resolve()
    checkout = Path(__file__).resolve().parents[3]
    try:
        spec = json.loads(Path(args.spec).expanduser().read_text())
        cohort.validate(spec)
        if output.is_relative_to(checkout):
            raise cohort.CohortError("the durable ledger must stay outside the source checkout")
        if args.action == "plan":
            from .candidate_capture import preflight
            coverage = {row['id']: preflight(Path(row['task_path']).expanduser()) for row in spec['schedule']}
            print(json.dumps({"spec": spec, "identity": cohort.identity(checkout, spec), "coverage": coverage,
                             "cost_rule": spec["cost_rule"],
                             "launches": len(spec["schedule"]), "inference_started": False}, indent=2))
            return 0 if all(row['supported'] for row in coverage.values()) else 1
        # Reporting reads the original pin and records an inspection epoch;
        # it never relabels old runs with today's source identity.
        if args.action == "report":
            first = json.loads((output / "ledger.jsonl").read_text().splitlines()[0])
            pin = first["identity"]
        else:
            try:
                pin = cohort.identity(checkout, spec)
            except (cohort.CohortError, OSError, ValueError, KeyError):
                ledger = output / "ledger.jsonl"
                if ledger.exists():
                    original = json.loads(ledger.read_text().splitlines()[0])
                    with cohort.Journal(output, original["spec"], original["identity"]) as journal:
                        journal.append("deviation", reason="restart identity could not be verified; no launch",
                                       attempted_spec_digest=cohort.digest(spec))
                        journal.report()
                raise
        with cohort.Journal(output, spec, pin) as journal:
            report = (cohort.run(journal, paths.jobs_dir(), checkout) if args.action == "run" else journal.report())
        print(json.dumps(report, indent=2))
        return 0 if report["complete"] and report["accounting_complete"] else 1
    except (cohort.CohortError, OSError, ValueError, KeyError) as error:
        print(f"cohort: {error}", file=sys.stderr)
        return 2


def cmd_candidates(args: argparse.Namespace) -> int:
    """Grade all retained candidates from completed trials."""
    from . import candidates, replay

    try:
        trials = [replay.find_trial(ref) for ref in args.trial]
        report = candidates.batch(trials, Path(args.output).expanduser(), jobs=args.jobs,
                                  deduplicate=args.deduplicate)
    except (OSError, ValueError, replay.ReplayError) as error:
        print(f"candidates: {error}", file=sys.stderr)
        return 1
    print(json.dumps({key: report[key] for key in
                     ("wall_seconds", "verifier_executions", "reused_grades", "invalid_candidates", "errors", "oracle")}, indent=2))
    return 1 if report["errors"] or report["invalid_candidates"] else 0


def cmd_verify(args: argparse.Namespace) -> int:
    """Run a task's verifier on a candidate directory or a trial."""
    import tempfile
    import time

    from . import replay
    from .panel import load_panel

    started = time.monotonic()
    try:
        if args.trial:
            result = replay.replay(replay.find_trial(args.trial), "verify")
            print(replay.line(result))
            print(f"verify: {result.out / 'replay.json'}")
            return 1 if result.error else 0
        panel = load_panel(catalog=args.catalog)
        task = panel.checkout() / panel.task(args.task).path
        out = replay.replays_dir() / (
            f"{args.task}--candidate--{time.strftime('%Y%m%dT%H%M%S')}"
        )
        with tempfile.TemporaryDirectory(prefix="tbench-verify-") as scratch:
            workspace = replay.candidate_workspace(
                Path(args.candidate).expanduser(), args.mount, Path(scratch)
            )
            result = replay.run_verifier(task, workspace, out)
    except (replay.ReplayError, KeyError, ValueError) as exc:
        print(f"verify: {exc}", file=sys.stderr)
        return 1
    result["seconds"] = round(time.monotonic() - started, 1)
    out.mkdir(parents=True, exist_ok=True)
    (out / "verify.json").write_text(json.dumps(result, indent=2) + "\n")
    print(
        f"verify {args.task}: reward {result['reward']} in {result['seconds']} s"
        + (f" ({result['exception']})" if result.get("exception") else "")
        + f"; verifier image {result.get('image_cache') or '?'}"
    )
    print(f"verify: {result['trial_dir']}")
    return 0 if result["reward"] is not None else 1


def cmd_images(args: argparse.Namespace) -> int:
    """List or remove the task images tbench.warm_docker keeps."""
    from .warm_docker import remove_warm_images, warm_images

    if args.images_command == "prune":
        removed = remove_warm_images(args.match)
        for reference in removed:
            print(f"removed {reference}")
        print(f"images: removed {len(removed)} kept images")
        return 0
    images = warm_images()
    for image in images:
        print(f"{image['reference']:<72} {image['size']:>10}  {image['created']}")
    print(f"images: {len(images)} kept task images")
    return 0


def cmd_envstart(args: argparse.Namespace) -> int:
    """Start and stop a task's environments with no agent or verifier."""
    from . import envstart
    from .panel import load_panel

    agent = load_agents()[args.arm]
    hosts = [] if args.open else list(agent.extra_allowed_hosts)
    panel = load_panel(catalog=args.catalog)
    task_dirs = [panel.checkout() / panel.task(task).path for task in args.task]
    artifact = Path(args.artifact).expanduser() if args.artifact else None
    report = envstart.run(
        task_dirs,
        modes=args.mode or ["harbor", "warm"],
        repeat=args.repeat,
        allowed_hosts=hosts,
        probe=args.probe,
        artifact=artifact,
        verifier=not args.no_verifier,
    )
    failed = [row for row in report["rows"] if row.get("error")]
    print(f"envstart: {len(report['rows'])} startups, {len(failed)} failed; {report['path']}")
    return 1 if failed else 0


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
            "--max-gpus",
            type=int,
            default=1,
            help="GPU trials at once (default 1); 0 skips every GPU task",
        )
        p.add_argument(
            "--order",
            choices=("largest", "smallest", "listed"),
            default="largest",
            help="start pending trials largest first (default), smallest "
            "first, or in the profile's task order",
        )
        p.add_argument(
            "--allow-oversize",
            action="store_true",
            help="run a task larger than the whole budget alone; by default "
            "it is skipped with the reason recorded",
        )
        p.add_argument(
            "--max-claude-concurrent",
            type=int,
            default=2,
            help="trials at once whose arm runs Claude, across every suite on "
            "the host, since they share one subscription (default 2); 0 turns "
            "the cap off",
        )
        p.add_argument(
            "--usage-backoff-min",
            type=float,
            default=30.0,
            help="minutes a provider pauses after a usage limit that states no "
            "reset time (default 30)",
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

    add_experiment_parser(sub)
    cohort_parser = sub.add_parser("cohort", help="run or report a frozen cohort with durable spend holds")
    cohort_parser.add_argument("action", choices=("run", "report", "plan"))
    cohort_parser.add_argument("--spec", required=True, help="frozen cohort JSON specification")
    cohort_parser.add_argument("--output", required=True, help="durable ledger directory outside the checkout")
    cohort_parser.set_defaults(func=cmd_cohort)
    coverage = sub.add_parser("candidate-preflight", help="check per-session artifact capture coverage without inference")
    coverage.add_argument("task", help="task directory")
    coverage.set_defaults(func=cmd_candidate_preflight)

    reference_parser = sub.add_parser(
        "reference",
        help="fetch the TB4 leaderboard's per-task results from the Harbor Hub",
    )
    reference_parser.add_argument(
        "--out", help="where to write (default reference/tb4-leaderboard.json)"
    )
    reference_parser.set_defaults(func=cmd_reference)

    try_parser = sub.add_parser(
        "try",
        help="run one arm on a few tasks from kept images and report loop time",
    )
    try_parser.add_argument("--arm", required=True, help="agent profile id")
    try_parser.add_argument(
        "--task",
        action="append",
        required=True,
        help="task ids, comma-separated or repeated",
    )
    try_parser.add_argument(
        "--attempts", type=int, default=1, help="trials per task (default 1)"
    )
    try_parser.add_argument(
        "--profile", default="tb4", help="job profile the tasks come from (default tb4)"
    )
    try_parser.add_argument(
        "--concurrency",
        type=int,
        help="trials at once (default: all of them); keep Claude arms at 3 or fewer",
    )
    try_parser.add_argument("--auth-mode", help="pick one configured auth mode")
    try_parser.add_argument(
        "--agent-kwarg", action="append", help="key=value adapter kwargs"
    )
    try_parser.add_argument("--job-name", help="override the try--<arm>--<stamp> name")
    try_parser.add_argument(
        "--cold",
        action="store_true",
        help="build task images fresh instead of reusing kept ones",
    )
    try_parser.add_argument(
        "--no-retain",
        action="store_true",
        help="don't copy the evidence into the retained traces",
    )
    try_parser.add_argument(
        "--interval", type=float, default=10.0, help="seconds between table polls"
    )
    try_parser.set_defaults(func=cmd_try)

    looptime_parser = sub.add_parser(
        "looptime", help="where each trial's time went: setup, agent, checks, verifier"
    )
    looptime_parser.add_argument(
        "job", nargs="+", help="job names, job dirs, or trial dirs"
    )
    looptime_parser.add_argument("--json", action="store_true", help="print JSON")
    looptime_parser.set_defaults(func=cmd_looptime)

    replay_parser = sub.add_parser(
        "replay",
        help="rerun checks, the repair brief, or the verifier on trials' "
        "workspaces, with no model call",
    )
    replay_parser.add_argument(
        "trial", nargs="*", help="trial dirs, <job>/<trial>, or trial names"
    )
    replay_parser.add_argument(
        "--stage", choices=("checks", "repair", "verify"), default="checks"
    )
    replay_parser.add_argument(
        "--artifact",
        help="the Coder One build to run the checks with (default: the trial's own)",
    )
    replay_parser.add_argument(
        "--policy",
        help="a policy manifest whose check options the replay runs instead of the "
        "trial's own (for example crates/coder-one/policies/tunable-v7.json)",
    )
    replay_parser.add_argument(
        "--failing",
        metavar="MATCH",
        help="also replay graded Coder One trials with reward 0 from jobs whose "
        "name contains MATCH",
    )
    replay_parser.add_argument(
        "--limit", type=int, default=10, help="most --failing trials (default 10)"
    )
    replay_parser.add_argument(
        "--jobs", type=int, default=4, help="replays at once (default 4)"
    )
    replay_parser.add_argument("--json", action="store_true", help="print JSON")
    replay_parser.set_defaults(func=cmd_replay)

    candidates_parser = sub.add_parser(
        "candidates", help="grade retained sequential candidates after trials finish"
    )
    candidates_parser.add_argument("trial", nargs="+", help="trial paths or names")
    candidates_parser.add_argument("--output", required=True, help="a new output directory outside the trials")
    candidates_parser.add_argument("--jobs", type=int, default=2, help="verifiers at once, 1 to 8 (default 2)")
    candidates_parser.add_argument("--deduplicate", action="store_true", help="reuse identical inputs within this batch; assumes a deterministic verifier")
    candidates_parser.set_defaults(func=cmd_candidates)

    verify_parser = sub.add_parser(
        "verify", help="run a task's verifier on a candidate directory or a trial"
    )
    verify_parser.add_argument("--task", help="task id")
    verify_parser.add_argument(
        "--candidate", help="a directory holding what --mount should hold"
    )
    verify_parser.add_argument(
        "--mount", default="/app", help="where the candidate goes (default /app)"
    )
    verify_parser.add_argument(
        "--trial", help="verify this trial's workspace instead"
    )
    verify_parser.add_argument(
        "--catalog", default="tb4", help="the task catalog (default tb4)"
    )
    verify_parser.set_defaults(func=cmd_verify)

    images_parser = sub.add_parser(
        "images", help="list or remove the task images kept between trials"
    )
    images_sub = images_parser.add_subparsers(dest="images_command", required=True)
    images_sub.add_parser("list", help="list the kept task images")
    prune_parser = images_sub.add_parser("prune", help="remove kept task images")
    prune_parser.add_argument(
        "--match", help="remove only images whose reference contains this text"
    )
    images_parser.set_defaults(func=cmd_images)

    envstart_parser = sub.add_parser(
        "envstart",
        help="time a task's environment startups with no agent, verifier, or model",
    )
    envstart_parser.add_argument("task", nargs="+", help="task names in the catalog")
    envstart_parser.add_argument(
        "--mode",
        action="append",
        choices=("harbor", "warm"),
        help="harbor's own start and stop, or kept images and a short stop "
        "(default: both, harbor first)",
    )
    envstart_parser.add_argument(
        "--repeat", type=int, default=1, help="startups per task and mode (default 1)"
    )
    envstart_parser.add_argument(
        "--arm",
        default="coder-one-microluna-v15",
        help="the arm whose allowed hosts the agent phase gets",
    )
    envstart_parser.add_argument(
        "--open", action="store_true", help="skip the allowlist; the agent phase stays public"
    )
    envstart_parser.add_argument(
        "--probe", action="store_true", help="check the allowlist from inside the environment"
    )
    envstart_parser.add_argument(
        "--artifact", help="also time a Coder One binary's upload and digest check"
    )
    envstart_parser.add_argument(
        "--no-verifier", action="store_true", help="skip the separate verifier environment"
    )
    envstart_parser.add_argument(
        "--catalog", default="tb4", help="the task catalog (default tb4)"
    )
    envstart_parser.set_defaults(func=cmd_envstart)

    cmp_parser = sub.add_parser(
        "compare", help="fold attempts into a comparison report"
    )
    cmp_parser.add_argument("--jobs-dir", help="jobs dir to scan")
    cmp_parser.add_argument("--arm", action="append", help="arms to include")
    cmp_parser.add_argument("--label", help="report label")
    cmp_parser.add_argument("--out", help="where to write the report JSON")
    cmp_parser.set_defaults(func=cmd_compare)
    return parser


# Commands that read results and start nothing. Each caps its own memory,
# so an input too large to hold raises MemoryError instead of taking the
# host (#9596). Commands that start trials or containers don't: children
# would inherit the cap.
ANALYSIS_COMMANDS = frozenset({"inspect", "compare", "reference", "looptime", "profiles"})


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    if args.command in ANALYSIS_COMMANDS:
        memcap.limit_self(memcap.cap(memcap.ANALYSIS_ENV, memcap.ANALYSIS_MAX))
    return int(args.func(args) or 0)


if __name__ == "__main__":
    sys.exit(main())
