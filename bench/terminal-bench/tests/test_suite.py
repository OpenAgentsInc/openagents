"""The suite scheduler, driven with a fake launcher and a fake host."""

import json
from pathlib import Path

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

    def start(self, trial, verb):
        self.next_pid += 1
        self.started.append((trial.job, verb))
        self.live.add(self.next_pid)
        self.pids[trial.job] = self.next_pid
        return self.next_pid

    def alive(self, pid):
        return pid in self.live

    def interrupt(self, pid):
        self.interrupted.append(pid)

    def finish(self, job, **result):
        _write_result(self.jobs / job, **result)
        self.live.discard(self.pids[job])


class FakeHost:
    def __init__(self, free_gb=200.0, gpu=False):
        self.free_gb = free_gb
        self.gpu = gpu
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


def test_a_task_bigger_than_the_budget_runs_alone(tmp_path):
    tasks = [_task("huge", cpus=32), _task("small")]
    scheduler, launcher, _ = _scheduler(tmp_path, tasks)
    scheduler.reconcile()
    assert [t.job for t in scheduler.launch_ready()] == ["tb4--arm--huge"]
    assert scheduler.launch_ready() == []


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
