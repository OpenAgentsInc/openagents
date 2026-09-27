"""One row per #9746 attempt, read from its Harbor job directory.

Run on the host that holds the jobs:

    python3 summarize.py <job> [<job>...] > attempts.json

The delegate estimate prices the stream's per-message usage and Claude
Code's thinking-token estimate at Fable 5.1 list prices. It is a lower
bound: the deadline cut each session before its result event, so the
in-flight call and the visible output tokens are not counted.
"""
import json, sys, datetime as d
from pathlib import Path

JOBS = Path.home() / ".openagents/terminal-bench/jobs"
# Fable 5.1 list prices per million tokens (docs/terminal-bench/measurement.md).
P = {"input": 10.0, "w5m": 12.5, "w1h": 20.0, "read": 0.25, "output": 50.0}


def ts(s):
    return d.datetime.fromisoformat(s.replace("Z", "+00:00"))


def row(job):
    T = next(p for p in (JOBS / job).iterdir() if p.is_dir() and p.name.startswith("sound-change-cascade__"))
    r = json.loads((T / "result.json").read_text())
    ep = T / "agent/episode"
    u = json.loads((ep / "evaluation/usage.json").read_text())
    m = json.loads((ep / "manifest.json").read_text())
    a = json.loads((ep / "trajectory.atif.json").read_text())
    dl = (m.get("delegate") or {}).get("delegation") or {}
    steps = a["steps"]
    first_agent = next(s["timestamp"] for s in steps if s.get("source") == "agent")
    deleg = next(s for s in steps for c in (s.get("tool_calls") or []) if c.get("function_name") == "delegate")
    d_end = ts(deleg["timestamp"])
    d_ms = dl.get("milliseconds")
    d_start = d_end - d.timedelta(milliseconds=d_ms)
    ae = r["agent_execution"]
    shells = sum(1 for s in steps for c in (s.get("tool_calls") or []) if c.get("function_name") == "shell")
    jevs = [c for s in steps for c in (s.get("tool_calls") or []) if c.get("function_name", "").startswith("jev")]
    # Delegate stream: distinct assistant messages and the CLI's thinking estimate.
    msgs, think, result = {}, 0, None
    for line in (ep / "artifacts/delegate-1.stream.jsonl").read_text().splitlines():
        try:
            e = json.loads(line)
        except ValueError:
            continue
        if e.get("type") == "assistant":
            msgs[e["message"]["id"]] = e["message"].get("usage") or {}
        elif e.get("subtype") == "thinking_tokens":
            think += e.get("estimated_tokens_delta") or 0
        elif e.get("type") == "result":
            result = e
    tok = {"input": 0, "w5m": 0, "w1h": 0, "read": 0, "output_reported": 0}
    for us in msgs.values():
        tok["input"] += us.get("input_tokens") or 0
        cc = us.get("cache_creation") or {}
        tok["w5m"] += cc.get("ephemeral_5m_input_tokens") or 0
        tok["w1h"] += cc.get("ephemeral_1h_input_tokens") or 0
        tok["read"] += us.get("cache_read_input_tokens") or 0
        tok["output_reported"] += us.get("output_tokens") or 0
    est_in = (tok["input"] * P["input"] + tok["w5m"] * P["w5m"] + tok["w1h"] * P["w1h"] + tok["read"] * P["read"]) / 1e6
    est_think = think * P["output"] / 1e6
    comp = u["components"]
    gen, jev = comp["generation"]["cost_usd"], comp["jev"]["cost_usd"]
    return {
        "job": job, "trial": T.name, "trial_id": r.get("id"),
        "reward": ((r.get("verifier_result") or {}).get("rewards") or {}).get("reward"),
        "exception": r.get("exception_info"),
        "trial_seconds": round((ts(r["finished_at"]) - ts(r["started_at"])).total_seconds(), 1),
        "phases_sec": {k: round((ts(r[k]["finished_at"]) - ts(r[k]["started_at"])).total_seconds(), 1)
                       for k in ("environment_setup", "agent_setup", "agent_execution", "verifier")},
        "agent_sec": {
            "before_first_step": round((ts(first_agent) - ts(ae["started_at"])).total_seconds(), 1),
            "explore_and_briefing": round((d_start - ts(first_agent)).total_seconds(), 1),
            "delegate": round(d_ms / 1000, 1),
            "after_delegate": round((ts(ae["finished_at"]) - d_end).total_seconds(), 1),
        },
        "explore": {"steps": m.get("steps"), "shell_commands": shells, "jev_calls": len(jevs),
                    "generations": (u.get("calls") or {}).get("generation"),
                    "generation_tokens_in_out": [comp["generation"].get("input_tokens"), comp["generation"].get("output_tokens")],
                    "jev_input_tokens": comp["jev"].get("input_tokens"), "jev_requests": comp["jev"].get("requests")},
        "delegate": {"status": dl.get("status"), "deadline_sec": (m.get("delegate") or {}).get("deadline_sec"),
                     "effort": (m.get("delegate") or {}).get("effort"), "model": (m.get("delegate") or {}).get("model"),
                     "api_calls": dl.get("api_calls"), "num_turns": dl.get("num_turns"),
                     "total_cost_usd": dl.get("total_cost_usd"), "cost_provenance": comp["delegate"].get("cost_provenance"),
                     "briefing_chars": (dl.get("briefing") or {}).get("chars"),
                     "briefing_sha256": (dl.get("briefing") or {}).get("sha256"),
                     "stream_tokens": tok, "thinking_estimate": think,
                     "estimate_usd": {"input_side": round(est_in, 4), "thinking": round(est_think, 4),
                                      "total_lower_bound": round(est_in + est_think, 4)},
                     "result_event": None if result is None else {k: result.get(k) for k in ("subtype", "is_error", "num_turns", "total_cost_usd", "duration_ms", "duration_api_ms")}},
        "cost": {"generation_usd": gen, "jev_usd": jev, "delegate_usd": comp["delegate"].get("cost_usd"),
                 "total_usd": u["cost"].get("amount_usd"), "known_lower_bound_usd": u["cost"].get("lower_bound_usd")},
    }


print(json.dumps([row(j) for j in sys.argv[1:]], indent=1, default=str))
