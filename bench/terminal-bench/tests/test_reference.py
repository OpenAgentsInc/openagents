"""The TB4 leaderboard reference, folded from a fake Hub."""

import json

from tbench.reference import fetch_reference, write_reference


class FakeHub:
    def __init__(self):
        self.rows = [
            {
                "id": "row-1",
                "rank": 1,
                "status": "display",
                "metadata": {
                    "agent_display": {"label": "Claude Code"},
                    "model_display": {"label": "Fable 5.1"},
                    "reasoning_effort": "max",
                    "date": "2026-09-01",
                },
                "metrics": {"successes": 2, "n_trials": 3, "total_cost_usd": 3.0},
            }
        ]
        self.trials_by_id = {
            "t1": self._trial("a", 1.0, "00:00:00", "00:10:00"),
            "t2": self._trial("a", 0.0, "00:00:00", "00:20:00", error="AgentTimeoutError"),
            "t3": self._trial("b", 1.0, "00:00:00", "00:05:00"),
        }

    @staticmethod
    def _trial(task, reward, start, end, error=None):
        return {
            "job_id": "job-1",
            "task_name": f"terminal-bench/{task}",
            "rewards": {"reward": reward},
            "exception_type": error,
            "agent_execution_started_at": f"2026-09-01T{start}+00:00",
            "agent_execution_finished_at": f"2026-09-01T{end}+00:00",
            "config": {
                "agent": {
                    "name": "claude-code",
                    "model_name": "anthropic/claude-fable-5-1",
                    "kwargs": {"version": "2.1.257"},
                    "env": {"ANTHROPIC_API_KEY": "${ANTHROPIC_API_KEY}"},
                }
            },
        }

    def leaderboard_rows(self, leaderboard_id):
        return self.rows

    def row_trial_ids(self, row_id):
        return list(self.trials_by_id)

    def trials(self, ids):
        return [self.trials_by_id[i] for i in ids]

    def job_tasks(self, job_id):
        return [
            # Matches the row: two trials of task a.
            {"task_name": "terminal-bench/a", "agent_name": "claude-code",
             "model_provider": "anthropic", "model_name": "claude-fable-5-1",
             "n_trials": 2, "cost_usd": 2.5},
            # Covers a retry the row dropped: three trials, so unknown.
            {"task_name": "terminal-bench/b", "agent_name": "claude-code",
             "model_provider": "anthropic", "model_name": "claude-fable-5-1",
             "n_trials": 2, "cost_usd": 1.0},
            # Another agent in the same job never counts.
            {"task_name": "terminal-bench/a", "agent_name": "codex",
             "model_provider": "openai", "model_name": "gpt-6-luna",
             "n_trials": 2, "cost_usd": 99.0},
        ]


def test_rows_fold_into_per_task_counts_and_costs(tmp_path):
    document = fetch_reference(FakeHub(), task_names=["a", "b", "c"], now="fixed")
    assert document["schema"] == "openagents.tbench.reference.v1"
    entry = document["entries"][0]
    assert (entry["agent"], entry["model"], entry["reasoning_effort"]) == (
        "Claude Code",
        "Fable 5.1",
        "max",
    )
    assert entry["harbor_model"] == "anthropic/claude-fable-5-1"
    a, b, c = (entry["tasks"][name] for name in ("a", "b", "c"))
    assert (a["successes"], a["trials"], a["errors"]) == (1, 2, 1)
    assert a["cost_usd"] == 2.5
    assert a["mean_agent_sec"] == 900.0
    assert (b["successes"], b["trials"], b["cost_usd"]) == (1, 1, None)
    assert (c["successes"], c["trials"]) == (0, 0)
    assert entry["per_task"]["consistent"] is True
    assert entry["per_task"]["cost_consistent"] is False
    assert entry["source_jobs"] == ["https://hub.harborframework.com/jobs/job-1"]
    path = write_reference(document, tmp_path / "ref.json")
    text = path.read_text()
    # Trial configs are read for the agent identity only, never stored.
    assert "API_KEY" not in text
    assert json.loads(text)["entries"][0]["rank"] == 1
