import json
from datetime import datetime, timezone

from tbench.agents import load_agents
from tbench.jobconfig import load_job_profile
from tbench.panel import load_panel
from tbench.runner import RunRequest
from tbench.tryout import WARM_ENVIRONMENT, job_name, prepare, run_try

from test_looptime import write_trial


def _request(tmp_path):
    panel = load_panel()
    return RunRequest(
        panel=panel,
        profile=load_job_profile("smoke"),
        agent=load_agents()["nop"],
        tasks=panel.select(["fix-git", "build-cython-ext"]),
        jobs_dir=tmp_path,
    )


def test_prepare_sets_attempts_concurrency_and_kept_images(tmp_path):
    request = prepare(_request(tmp_path), attempts=3, concurrency=None, warm=True)
    assert request.profile.n_attempts == 3
    # Every trial at once unless capped.
    assert request.profile.n_concurrent_trials == 6
    assert request.profile.environment["import_path"] == WARM_ENVIRONMENT
    assert request.job_name.startswith("try--nop--")
    capped = prepare(_request(tmp_path), attempts=1, concurrency=5, warm=False, name="x")
    assert capped.profile.n_concurrent_trials == 2
    assert "import_path" not in capped.profile.environment
    assert capped.job_name == "x"
    # The checked profile itself is untouched.
    assert load_job_profile("smoke").n_attempts == 1


def test_job_names_carry_the_arm_and_the_time():
    now = datetime(2026, 9, 23, 16, 2, 1, tzinfo=timezone.utc)
    assert job_name("nop", now) == "try--nop--20260923T160201Z"


def test_a_try_prints_the_table_and_writes_its_loop_time(tmp_path):
    request = prepare(_request(tmp_path), attempts=1, concurrency=None, warm=True, name="try--t")
    job = tmp_path / "try--t"

    def fake_run(req):
        write_trial(job / "fix-git__1")
        (job / "tbench").mkdir(parents=True)
        return job

    lines = []
    job_dir, summary, code = run_try(
        request, interval=60, retain=False, echo=lines.append, runner=fake_run
    )
    assert code == 0 and job_dir == job
    assert summary["totals"]["passed"] == 1
    assert summary["environment"] == WARM_ENVIRONMENT
    written = json.loads((job / "tbench" / "looptime.json").read_text())
    assert written["trials"][0]["seconds"]["executor"] == 600.0
    text = "\n".join(lines)
    assert "kept task images" in text and "fix-git__1" in text
