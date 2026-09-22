"""Panel loading and selection behavior."""

import pytest

from tbench.panel import load_panel


@pytest.fixture()
def panel():
    return load_panel()


def test_panel_loads_all_tasks(panel):
    ids = {task.id for task in panel.tasks}
    assert ids == {
        "fix-git",
        "build-cython-ext",
        "fix-code-vulnerability",
        "cancel-async-tasks",
        "headless-terminal",
        "vllm-deepseek-streaming",
        "batched-eval-parity",
        "math-eval-grader",
        "git-leak-recovery",
        "log-summary-date-ranges",
        "sqlite-db-truncate",
    }


def test_upstream_pin(panel):
    assert panel.git_commit_id == "3b5caaa4863d64dda7f0957bf4fc2d4f019202d4"
    assert "terminal-bench" in panel.git_url


def test_math_eval_grader_stays_excluded(panel):
    task = panel.task("math-eval-grader")
    assert task.excluded
    assert task.resources.gpus == 1
    with pytest.raises(ValueError, match="excluded"):
        panel.select(["math-eval-grader"])


def test_runnable_keeps_exclusion_visible(panel):
    runnable, excluded = panel.runnable(["fix-git", "math-eval-grader"])
    assert [t.id for t in runnable] == ["fix-git"]
    assert [t.id for t in excluded] == ["math-eval-grader"]


def test_smoke_tasks_have_resource_pins(panel):
    for task_id in ("fix-git", "build-cython-ext"):
        task = panel.task(task_id)
        assert task.resources.cpus >= 1
        assert task.resources.memory_mb > 0
        assert task.agent_timeout_sec > 0
        assert "smoke" in task.profiles


def test_fix_git_records_amd64_only(panel):
    task = panel.task("fix-git")
    assert task.image_arches == ("amd64",)
    assert task.images == ("alexgshaw/fix-git:20260403",)
