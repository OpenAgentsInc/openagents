"""Print the numbers a results-page row needs, for one or more jobs.

Usage, from ``bench/terminal-bench``::

    python3 tools/trial_metrics.py smoke--coder-one--fix-git smoke--claude-code-opus--fix-git

For every trial in each job under ``~/.openagents/terminal-bench/jobs/``, it
prints the reward, the exception type, Harbor's agent and total time, and
Harbor's usage and cost. For a Coder One trial it adds the episode outcome,
calls by kind, the generation, Jev, and delegate components, the exact Jev
cost, and the time spent in each kind of step. For a Codex trial on a GPT-6
model, whose cost Harbor does not report, it prices the tokens at OpenAI's
standard list prices (``GPT6_PRICES``), labeled as a manual estimate.
"""

from __future__ import annotations

import datetime
import glob
import json
import os
import sys

JOBS = os.path.expanduser("~/.openagents/terminal-bench/jobs")

# Jev's published rate, dollars per million input tokens; output is free.
JEV_PER_MILLION = 0.042

# OpenAI standard pricing, short context, dollars per million tokens:
# (input, cached input, output). Supplied by the operator on 2026-09-22.
GPT6_PRICES = {
    "gpt-6-astra": (10.00, 1.00, 50.00),
    "gpt-6-sol": (2.00, 0.20, 10.00),
    "gpt-6-luna": (0.10, 0.01, 0.50),
}


def _seconds(phase: dict | None) -> float | None:
    if not phase or not phase.get("started_at") or not phase.get("finished_at"):
        return None
    parse = lambda s: datetime.datetime.fromisoformat(s.replace("Z", "+00:00"))  # noqa: E731
    return round((parse(phase["finished_at"]) - parse(phase["started_at"])).total_seconds(), 1)


def gpt6_cost(model: str, tokens_in: int, cached: int, tokens_out: int) -> float | None:
    """Uncached input, cached input, and output, each at its rate."""
    rates = GPT6_PRICES.get(model)
    if rates is None or tokens_in is None or cached is None or tokens_out is None:
        return None
    rate_in, rate_cached, rate_out = rates
    return ((tokens_in - cached) * rate_in + cached * rate_cached + tokens_out * rate_out) / 1e6


def trial(path: str) -> None:
    job = os.path.basename(os.path.dirname(path.rstrip("/")))
    name = os.path.basename(path.rstrip("/"))
    try:
        with open(os.path.join(path, "result.json")) as handle:
            result = json.load(handle)
    except OSError:
        print(f"{job} {name}: no result.json yet")
        return
    agent = result.get("agent_result") or {}
    reward = ((result.get("verifier_result") or {}).get("rewards") or {}).get("reward")
    exception = (result.get("exception_info") or {}).get("exception_type")
    print(
        f"{job} {name} reward={reward} exception={exception} "
        f"agent={_seconds(result.get('agent_execution'))}s total={_seconds(result)}s "
        f"harbor_cost={agent.get('cost_usd')} in={agent.get('n_input_tokens')} "
        f"cached={agent.get('n_cache_tokens')} out={agent.get('n_output_tokens')}"
    )

    episode = os.path.join(path, "agent", "episode")
    usage_path = os.path.join(episode, "evaluation", "usage.json")
    if os.path.exists(usage_path):
        usage = json.load(open(usage_path))
        manifest = json.load(open(os.path.join(episode, "manifest.json")))
        parts = usage["components"]
        jev = parts["jev"]
        print(f"  outcome={manifest.get('outcome')} steps={manifest.get('steps')} calls={usage['calls']}")
        print(f"  total_cost={usage['cost'].get('amount_usd')}")
        print(f"  generation={json.dumps(parts['generation'])}")
        exact = None if jev.get("input_tokens") is None else jev["input_tokens"] * JEV_PER_MILLION / 1e6
        print(f"  jev requests={jev['requests']} input_tokens={jev['input_tokens']} cost_exact={exact}")
        if "delegate" in parts:
            print(f"  delegate={json.dumps(parts['delegate'])[:800]}")
        trajectory = json.load(open(os.path.join(episode, "trajectory.atif.json")))
        kinds: dict[str, list[int]] = {}
        prompts: list[int] = []
        for step in trajectory["steps"]:
            calls = [call["function_name"] for call in step.get("tool_calls") or []]
            prompt = (step.get("metrics") or {}).get("prompt_tokens")
            kind = calls[0] if calls else ("generation" if step["source"] == "agent" and prompt is not None else None)
            if kind is None:
                continue
            if kind == "generation":
                prompts.append(prompt)
            count, ms = kinds.get(kind, [0, 0])
            kinds[kind] = [count + 1, ms + ((step.get("extra") or {}).get("duration_ms") or 0)]
        print(f"  time by kind (count, ms)={kinds}")
        if prompts:
            print(f"  generation prompt tokens mean={round(sum(prompts) / len(prompts))} min={min(prompts)} max={max(prompts)}")
        return

    trajectory_path = os.path.join(path, "agent", "trajectory.json")
    try:
        trajectory = json.load(open(trajectory_path))
    except (OSError, json.JSONDecodeError) as error:
        print(f"  no readable trajectory: {error}")
        return
    steps = trajectory.get("steps", [])
    model = (trajectory.get("agent") or {}).get("model_name")
    tool_calls = sum(len(step.get("tool_calls") or []) for step in steps)
    agent_steps = sum(1 for step in steps if step.get("source") == "agent")
    print(f"  model={model} agent_steps={agent_steps} tool_calls={tool_calls}")
    manual = gpt6_cost(model or "", agent.get("n_input_tokens"), agent.get("n_cache_tokens"), agent.get("n_output_tokens"))
    if manual is not None:
        print(f"  manual_cost={manual:.6f} (OpenAI standard list price, not Harbor)")


def main(jobs: list[str]) -> None:
    if not jobs:
        print(__doc__)
        return
    for job in jobs:
        paths = sorted(glob.glob(os.path.join(JOBS, job, "*__*/")))
        if not paths:
            print(f"{job}: no trials under {os.path.join(JOBS, job)}")
        for path in paths:
            trial(path)


if __name__ == "__main__":
    main(sys.argv[1:])
