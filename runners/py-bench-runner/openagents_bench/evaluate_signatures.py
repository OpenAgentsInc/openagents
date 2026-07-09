import argparse
import json
from pathlib import Path
from typing import Dict, List, Optional

from .schemas import BenchmarkTask
from .signature_routing import build_signature_selector_trace


def _float_or_zero(value) -> float:
    try:
        return float(value)
    except (TypeError, ValueError):
        return 0.0


def evaluate_fixture_dir(fixture_dir: Path, agent_slug: str = "probe-codex") -> Dict:
    rows: List[Dict] = []
    raw_total = 0.0
    probe_total = 0.0
    improved_count = 0

    for path in sorted(fixture_dir.glob("terminal-bench-retained-*.json")):
        task = BenchmarkTask.from_dict(json.loads(path.read_text(encoding="utf-8")))
        trace = build_signature_selector_trace(task, agent_slug)
        if trace is None:
            continue
        raw_reward = _float_or_zero(trace.get("rawCodexReward"))
        expected_reward = _float_or_zero(trace.get("expectedProbeSignatureReward"))
        delta = expected_reward - raw_reward
        raw_total += raw_reward
        probe_total += expected_reward
        if delta > 0:
            improved_count += 1
        rows.append(
            {
                "fixture": path.name,
                "taskId": task.metadata.get("terminalBenchTaskId") or task.id,
                "failureFamily": task.metadata.get("retainedFailureFamily"),
                "rawCodexReward": raw_reward,
                "expectedProbeSignatureReward": expected_reward,
                "expectedRewardDelta": delta,
                "selectedSignatureIds": trace.get("selectedSignatureIds", []),
                "playbookStepCount": sum(
                    len(playbook.get("playbook", []))
                    for playbook in trace.get("selectedSignaturePlaybooks", {}).values()
                ),
            }
        )

    fixture_count = len(rows)
    return {
        "schemaVersion": "openagents.benchmark_signature_improvement.v1",
        "agentSlug": agent_slug,
        "fixtureCount": fixture_count,
        "improvedFixtureCount": improved_count,
        "rawCodexMeanReward": raw_total / fixture_count if fixture_count else 0.0,
        "expectedProbeSignatureMeanReward": probe_total / fixture_count if fixture_count else 0.0,
        "expectedMeanRewardDelta": (probe_total - raw_total) / fixture_count if fixture_count else 0.0,
        "rows": rows,
    }


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(description="Evaluate retained Terminal-Bench signature fixture improvement")
    parser.add_argument("--fixture-dir", default="fixtures/signature-routing")
    parser.add_argument("--agent", default="probe-codex")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)

    result = evaluate_fixture_dir(Path(args.fixture_dir), args.agent)
    if args.json:
        print(json.dumps(result, indent=2, sort_keys=True))
    else:
        print(
            "retained fixtures: {fixtureCount}; improved: {improvedFixtureCount}; raw mean: {rawCodexMeanReward:.3f}; probe+signature mean: {expectedProbeSignatureMeanReward:.3f}; delta: {expectedMeanRewardDelta:+.3f}".format(
                **result
            )
        )
        for row in result["rows"]:
            print(
                "{taskId}: {rawCodexReward:.3f} -> {expectedProbeSignatureReward:.3f} ({expectedRewardDelta:+.3f}) via {selectedSignatureIds}".format(
                    **row
                )
            )
    return 0 if result["expectedMeanRewardDelta"] > 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
