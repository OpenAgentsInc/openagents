"""The suite scheduler, driven with a fake launcher and a fake host."""

import json
import os
from pathlib import Path

import pytest

from tbench.panel import Task, TaskResources
from tbench.suite import (
    FAILED,
    FINISHED,
    PENDING,
    RUNNING,
    SKIPPED,
    Budget,
    Host,
    Scheduler,
    acquire_gpu_slot,
    gpu_slot_holder,
    inspect_job,
    job_name,
    read_status,
    status_lines,
    suite_dir,
)


def _task(task_id: str, cpus: int = 2, memory_gb: int = 4, gpus: int = 0) -> Task:
    return Task(
        id=task_id,
        path=f"tasks/{task_id}",
        set="tasks",
        role="",
        resources=TaskResources(cpus, memory_gb * 1024, 10240, gpus),
        agent_timeout_sec=28800,
        requires_gpu_runtime=gpus > 0,
    )


def _write_result(job_dir: Path, *, reward=None, exception=None) -> None:
    trial = job_dir / "trial__abc"
    trial.mkdir(parents=True, exist_ok=True)
    result = {
        "exception_info": {"exception_type": exception} if exception else None,
        "verifier_result": {"rewards": {"reward": reward}} if reward is not None else None,
    }
    (trial / "result.json").write_text(json.dumps(result))


class FakeLauncher:
    """Records starts; ``finish`` makes a job exit with a given outcome."""

    def __init__(self, jobs: Path, logs: Path):
        self.jobs = jobs
        self.logs = logs
        self.started: list[tuple[str, str]] = []
        self.live: set[int] = set()
        self.pids: dict[str, int] = {}
        self.interrupted: list[int] = []
        self.next_pid = 1000
        # Descriptors a "trial process" keeps, as a real child would.
        self.held: dict[str, int] = {}

    def start(self, trial, verb, hold=None):
        self.next_pid += 1
        self.started.append((trial.job, verb))
        self.live.add(self.next_pid)
        self.pids[trial.job] = self.next_pid
        if hold is not None:
            self.held[trial.job] = os.dup(hold)
        return self.next_pid

    def exit(self, job):
        self.live.discard(self.pids[job])
        held = self.held.pop(job, None)
        if held is not None:
            os.close(held)

    def alive(self, pid):
        return pid in self.live

    def interrupt(self, pid):
        self.interrupted.append(pid)

    def finish(self, job, **result):
        _write_result(self.jobs / job, **result)
        self.exit(job)


class FakeHost:
    def __init__(self, free_gb=200.0, gpu=False, lock=None):
        self.free_gb = free_gb
        self.gpu = gpu
        self.lock = lock
        self.images: set[str] = set()
        self.removed: list[str] = []
        self.cache_pruned = 0

    def build(self) -> Host:
        return Host(
            gpu_refusal=lambda task_id: None if self.gpu else f"{task_id} needs a GPU",
            free_disk_gb=lambda: self.free_gb,
            task_images=lambda task: sorted(
                name for name in self.images if name.startswith(f"{task.id}__")
            ),
            remove_images=self._remove,
            prune_build_cache=self._prune_cache,
            gpu_slot=lambda job: acquire_gpu_slot(self.lock, job),
        )

    def _remove(self, names):
        self.removed.extend(names)
        self.images.difference_update(names)
        return names

    def _prune_cache(self):
        self.cache_pruned += 1
        return True


def _scheduler(tmp_path, tasks, *, attempts=1, budget=None, host=None, running=None):
    jobs = tmp_path / "jobs"
    jobs.mkdir(parents=True, exist_ok=True)
    launcher = FakeLauncher(jobs, tmp_path / "logs")
    host = host or FakeHost()
    host.lock = host.lock or tmp_path / "gpu.lock"
    scheduler = Scheduler(
        profile="tb4",
        arm="arm",
        pin={"git_commit_id": "abc"},
        tasks=tasks,
        attempts=attempts,
        budget=budget or Budget(max_cpus=24, max_mem_gb=100),
        jobs_dir=jobs,
        directory=tmp_path / "suite",
        launcher=launcher,
        host_=host.build(),
        clock=lambda: 0.0,
        find_running=(running or {}).get,
        failed=tmp_path / "failed",
    )
    return scheduler, launcher, host


def test_job_names_follow_the_runbook():
    assert job_name("tb4", "arm", "cad-model", 1) == "tb4--arm--cad-model"
    assert job_name("tb4", "arm", "cad-model", 3) == "tb4--arm--cad-model-3"


def test_trials_pack_into_the_cpu_budget_largest_first(tmp_path):
    tasks = [_task("small-a"), _task("big", cpus=16, memory_gb=32), _task("mid", cpus=8)]
    scheduler, launcher, _ = _scheduler(tmp_path, tasks)
    scheduler.reconcile()
    started = scheduler.launch_ready()
    assert [t.job for t in started] == ["tb4--arm--big", "tb4--arm--mid"]
    assert scheduler.status()["in_use"]["cpus"] == 24
    launcher.finish("tb4--arm--big", reward=1.0)
    scheduler.poll()
    assert [t.job for t in scheduler.launch_ready()] == ["tb4--arm--small-a"]


def test_a_task_bigger_than_the_budget_is_skipped_by_default(tmp_path):
    tasks = [_task("huge", cpus=32), _task("fat", memory_gb=200), _task("small")]
    scheduler, launcher, _ = _scheduler(tmp_path, tasks)
    scheduler.reconcile()
    skipped = {t.task.id: t.reason for t in scheduler.trials if t.state == SKIPPED}
    assert sorted(skipped) == ["fat", "huge"]
    assert "32 CPUs over the 24-CPU budget" in skipped["huge"]
    assert "--allow-oversize" in skipped["huge"]
    assert "200 GiB over the 100 GiB budget" in skipped["fat"]
    assert [t.job for t in scheduler.launch_ready()] == ["tb4--arm--small"]


def test_allow_oversize_runs_a_task_bigger_than_the_budget_alone(tmp_path):
    tasks = [_task("huge", cpus=32), _task("small")]
    budget = Budget(max_cpus=24, max_mem_gb=100, allow_oversize=True)
    scheduler, launcher, _ = _scheduler(tmp_path, tasks, budget=budget)
    scheduler.reconcile()
    assert [t.job for t in scheduler.launch_ready()] == ["tb4--arm--huge"]
    assert scheduler.launch_ready() == []
    launcher.finish("tb4--arm--huge", reward=1.0)
    scheduler.poll()
    assert [t.job for t in scheduler.launch_ready()] == ["tb4--arm--small"]


def test_order_smallest_and_listed(tmp_path):
    tasks = [_task("mid", cpus=8), _task("big", cpus=16), _task("s1", cpus=2), _task("s2", cpus=2)]
    budget = Budget(max_cpus=16, max_mem_gb=100, order="smallest")
    scheduler, _, _ = _scheduler(tmp_path / "s", tasks, budget=budget)
    scheduler.reconcile()
    # Smallest first packs 2 + 2 + 8 into 16 CPUs; the 16-CPU task waits.
    assert [t.task.id for t in scheduler.launch_ready()] == ["s1", "s2", "mid"]

    budget = Budget(max_cpus=16, max_mem_gb=100, order="listed")
    scheduler, _, _ = _scheduler(tmp_path / "l", tasks, budget=budget)
    scheduler.reconcile()
    # Listed order: mid (8) first, big (16) doesn't fit beside it, then s1, s2.
    assert [t.task.id for t in scheduler.launch_ready()] == ["mid", "s1", "s2"]

    budget = Budget(max_cpus=16, max_mem_gb=100)
    scheduler, _, _ = _scheduler(tmp_path / "d", tasks, budget=budget)
    scheduler.reconcile()
    # The default stays largest first: the 16-CPU task takes the budget.
    assert [t.task.id for t in scheduler.launch_ready()] == ["big"]
    assert scheduler.status()["budget"]["order"] == "largest"


def test_an_unknown_order_is_refused():
    with pytest.raises(ValueError, match="order must be one of"):
        Budget(order="random")


def test_a_gpu_budget_of_zero_skips_gpu_tasks_even_with_oversize(tmp_path):
    # The claude-code-opus launch: --max-gpus 0 --max-cpus 8 on a host with
    # a GPU, and a 1-GPU, 16-CPU task that used to start alone.
    tasks = [_task("jax-speedrun-gpu", cpus=16, gpus=1), _task("cpu", cpus=4)]
    for allow in (False, True):
        budget = Budget(max_cpus=8, max_mem_gb=100, max_gpus=0, allow_oversize=allow)
        scheduler, launcher, _ = _scheduler(
            tmp_path / str(allow), tasks, budget=budget, host=FakeHost(gpu=True)
        )
        scheduler.reconcile()
        gpu = scheduler.trials[0]
        assert gpu.state == SKIPPED
        assert "GPU budget is 0" in gpu.reason
        assert [t.task.id for t in scheduler.launch_ready()] == ["cpu"]
        launcher.finish("tb4--arm--cpu", reward=1.0)
        scheduler.poll()
        assert scheduler.launch_ready() == []
        assert [job for job, _ in launcher.started] == ["tb4--arm--cpu"]


def test_the_host_wide_gpu_slot_holds_a_second_scheduler_back(tmp_path):
    lock = tmp_path / "gpu.lock"
    tasks = [_task("gpu", gpus=1)]
    first, first_launcher, _ = _scheduler(
        tmp_path / "a", tasks, host=FakeHost(gpu=True, lock=lock)
    )
    second, second_launcher, _ = _scheduler(
        tmp_path / "b", tasks, host=FakeHost(gpu=True, lock=lock)
    )
    first.reconcile()
    second.reconcile()
    assert [t.job for t in first.launch_ready()] == ["tb4--arm--gpu"]
    assert gpu_slot_holder(lock)["job"] == "tb4--arm--gpu"
    # The other arm's scheduler has a free GPU budget, but the host's slot
    # is taken for as long as the first trial's process lives.
    assert second.launch_ready() == []
    assert "host-wide GPU slot" in second.trials[0].reason
    first_launcher.finish("tb4--arm--gpu", reward=1.0)
    first.poll()
    assert gpu_slot_holder(lock) is None
    assert [t.job for t in second.launch_ready()] == ["tb4--arm--gpu"]
    assert second.trials[0].reason is None
    second_launcher.exit("tb4--arm--gpu")


def test_a_cpu_trial_never_waits_for_the_gpu_slot(tmp_path):
    lock = tmp_path / "gpu.lock"
    held = acquire_gpu_slot(lock, "elsewhere")
    try:
        scheduler, _, _ = _scheduler(
            tmp_path,
            [_task("gpu", cpus=4, gpus=1), _task("cpu"), _task("cpu-2")],
            attempts=1,
            host=FakeHost(gpu=True, lock=lock),
            budget=Budget(max_cpus=24, max_mem_gb=100, max_concurrent=1),
        )
        scheduler.clock = lambda: 1.0
        scheduler.reconcile()
        assert [t.task.id for t in scheduler.launch_ready()] == ["cpu"]
        scheduler.budget = Budget(max_cpus=24, max_mem_gb=100)
        # Hours later the GPU trial still waits on another suite's slot, but
        # that wait doesn't reserve the budget: the backfill continues.
        scheduler.clock = lambda: 10_000.0
        assert [t.task.id for t in scheduler.launch_ready()] == ["cpu-2"]
    finally:
        os.close(held)


def test_a_restart_skips_finished_trials_and_adopts_running_ones(tmp_path):
    tasks = [_task("done"), _task("live"), _task("interrupted"), _task("new")]
    jobs = tmp_path / "jobs"
    _write_result(jobs / "tb4--arm--done", reward=0.0)
    _write_result(jobs / "tb4--arm--interrupted", exception="CancelledError")
    scheduler, launcher, _ = _scheduler(
        tmp_path, tasks, running={"tb4--arm--live": 4242}
    )
    scheduler.reconcile()
    states = {t.task.id: t.state for t in scheduler.trials}
    assert states == {
        "done": FINISHED,
        "live": RUNNING,
        "interrupted": PENDING,
        "new": PENDING,
    }
    scheduler.launch_ready()
    assert sorted(launcher.started) == [
        ("tb4--arm--interrupted", "resume"),
        ("tb4--arm--new", "run"),
    ]


def test_a_setup_timeout_moves_aside_and_retries_once(tmp_path):
    scheduler, launcher, _ = _scheduler(tmp_path, [_task("flaky")])
    scheduler.reconcile()
    scheduler.launch_ready()
    launcher.finish("tb4--arm--flaky", exception="AgentSetupTimeoutError")
    scheduler.poll()
    trial = scheduler.trials[0]
    assert trial.state == PENDING and trial.retries == 1
    moved = list((tmp_path / "failed").iterdir())
    assert [p.name.split("-setup-timeout-")[0] for p in moved] == ["tb4--arm--flaky"]
    assert not (tmp_path / "jobs" / "tb4--arm--flaky").exists()
    scheduler.launch_ready()
    launcher.finish("tb4--arm--flaky", exception="AgentSetupTimeoutError")
    scheduler.poll()
    assert trial.state == FAILED
    assert "twice" in trial.reason


def test_a_restart_counts_earlier_setup_timeout_moves(tmp_path):
    failed = tmp_path / "failed" / "tb4--arm--flaky-setup-timeout-1"
    failed.mkdir(parents=True)
    _write_result(tmp_path / "jobs" / "tb4--arm--flaky", exception="AgentSetupTimeoutError")
    scheduler, _, _ = _scheduler(tmp_path, [_task("flaky")])
    scheduler.reconcile()
    assert scheduler.trials[0].state == FAILED


def test_gpu_tasks_skip_without_a_gpu_and_take_one_slot_with_one(tmp_path):
    tasks = [_task("gpu-a", gpus=1), _task("gpu-b", gpus=1), _task("cpu")]
    scheduler, _, _ = _scheduler(tmp_path, tasks)
    scheduler.reconcile()
    skipped = [t for t in scheduler.trials if t.state == SKIPPED]
    assert [t.task.id for t in skipped] == ["gpu-a", "gpu-b"]
    assert "needs a GPU" in skipped[0].reason
    scheduler, launcher, _ = _scheduler(tmp_path / "g", tasks, host=FakeHost(gpu=True))
    scheduler.reconcile()
    started = [t.task.id for t in scheduler.launch_ready()]
    assert sorted(started) == ["cpu", "gpu-a"]
    launcher.finish("tb4--arm--gpu-a", reward=1.0)
    scheduler.poll()
    assert [t.task.id for t in scheduler.launch_ready()] == ["gpu-b"]


def test_later_attempts_wait_for_the_first_attempts_image(tmp_path):
    scheduler, launcher, host = _scheduler(tmp_path, [_task("t")], attempts=3)
    scheduler.reconcile()
    assert [t.job for t in scheduler.launch_ready()] == ["tb4--arm--t"]
    host.images.add("t__abc__env-main:latest")
    assert [t.job for t in scheduler.launch_ready()] == [
        "tb4--arm--t-2",
        "tb4--arm--t-3",
    ]


def test_no_trial_starts_under_the_disk_floor(tmp_path):
    host = FakeHost(free_gb=30)
    scheduler, launcher, _ = _scheduler(tmp_path, [_task("t")], host=host)
    scheduler.reconcile()
    assert scheduler.launch_ready() == []
    assert launcher.started == []
    assert host.cache_pruned == 1
    host.free_gb = 100
    assert len(scheduler.launch_ready()) == 1


def test_a_finished_task_is_pruned_near_the_floor(tmp_path):
    host = FakeHost(free_gb=200)
    scheduler, launcher, _ = _scheduler(tmp_path, [_task("a"), _task("b")], attempts=1, host=host)
    scheduler.reconcile()
    scheduler.launch_ready()
    host.images.update({"a__x__env-main:latest", "b__y__env-main:latest"})
    launcher.finish("tb4--arm--a", reward=1.0)
    scheduler.poll()
    assert host.removed == []  # plenty of disk: images stay for reuse
    host.free_gb = 50  # within the 20 GiB margin of the 40 GiB floor
    launcher.finish("tb4--arm--b", reward=0.0)
    scheduler.poll()
    assert sorted(host.removed) == ["a__x__env-main:latest", "b__y__env-main:latest"]
    assert [p["task"] for p in scheduler.status()["pruned"]] == ["a", "b"]


def test_a_process_that_exits_without_a_result_is_resumed_then_failed(tmp_path):
    scheduler, launcher, _ = _scheduler(tmp_path, [_task("crashy")])
    scheduler.reconcile()
    for _ in range(3):
        scheduler.launch_ready()
        (tmp_path / "jobs" / "tb4--arm--crashy" / "trial__abc").mkdir(parents=True, exist_ok=True)
        (tmp_path / "jobs" / "tb4--arm--crashy" / "trial__abc" / "trial.log").write_text("")
        launcher.live.discard(launcher.pids["tb4--arm--crashy"])
        scheduler.poll()
    assert scheduler.trials[0].state == FAILED
    assert [verb for _, verb in launcher.started] == ["run", "resume", "resume"]


def test_stop_interrupts_running_trials_and_leaves_them_resumable(tmp_path):
    scheduler, launcher, _ = _scheduler(tmp_path, [_task("t")])
    scheduler.reconcile()
    scheduler.launch_ready()
    scheduler.stop()
    assert launcher.interrupted == [launcher.pids["tb4--arm--t"]]
    assert scheduler.launch_ready() == []
    _write_result(tmp_path / "jobs" / "tb4--arm--t", exception="CancelledError")
    launcher.live.clear()
    scheduler.poll()
    assert scheduler.trials[0].state == PENDING
    assert scheduler.done()


def test_the_status_file_is_written_and_summarized(tmp_path, monkeypatch):
    monkeypatch.setenv("TBENCH_STATE_DIR", str(tmp_path / "state"))
    scheduler, launcher, _ = _scheduler(tmp_path, [_task("t", cpus=4)])
    scheduler.directory = suite_dir("tb4", "arm")
    scheduler.reconcile()
    scheduler.launch_ready()
    launcher.finish("tb4--arm--t", reward=1.0)
    status = scheduler.run(interval=0, sleep=lambda _: None)
    assert status["state"] == "done"
    written = read_status("tb4", "arm")
    assert written["counts"] == {"finished": 1}
    assert written["passes"] == 1
    assert "passes 1/1 graded" in "\n".join(status_lines(written))


def test_inspect_job_reads_refusals(tmp_path):
    refusals = tmp_path / "job" / "tbench" / "refusals"
    refusals.mkdir(parents=True)
    (refusals / "1.json").write_text(json.dumps({"reason": "no credentials"}))
    state = inspect_job(tmp_path / "job")
    assert (state.kind, state.reason) == ("refused", "no credentials")
    assert inspect_job(tmp_path / "missing").kind == "new"


def test_each_job_gets_the_current_claude_token(tmp_path):
    from tbench.suite import fresh_environment

    creds = tmp_path / "creds.json"
    creds.write_text('{"claudeAiOauth": {"accessToken": "fresh"}}')
    env = fresh_environment({"CLAUDE_CODE_OAUTH_TOKEN": "stale", "X": "1"}, creds)
    assert env == {"CLAUDE_CODE_OAUTH_TOKEN": "fresh", "X": "1"}
    # No token in the scheduler's environment: nothing is added.
    assert fresh_environment({"X": "1"}, creds) == {"X": "1"}
    # An unreadable file keeps the scheduler's token.
    creds.write_text("not json")
    assert fresh_environment({"CLAUDE_CODE_OAUTH_TOKEN": "stale"}, creds) == {
        "CLAUDE_CODE_OAUTH_TOKEN": "stale"
    }


def test_a_long_lived_setup_token_wins_over_the_refreshed_login(tmp_path, monkeypatch):
    import tbench.suite as suite

    creds = tmp_path / "creds.json"
    creds.write_text('{"claudeAiOauth": {"accessToken": "refreshed"}}')
    setup = tmp_path / "setup-token"
    monkeypatch.setattr(suite, "CLAUDE_SETUP_TOKEN", setup)
    assert suite.fresh_environment({"CLAUDE_CODE_OAUTH_TOKEN": "x"}, creds)[
        "CLAUDE_CODE_OAUTH_TOKEN"
    ] == "refreshed"
    setup.write_text("long-lived\n")
    assert suite.fresh_environment({"CLAUDE_CODE_OAUTH_TOKEN": "x"}, creds)[
        "CLAUDE_CODE_OAUTH_TOKEN"
    ] == "long-lived"
