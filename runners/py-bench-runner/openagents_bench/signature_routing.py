from typing import Any, Dict, List, Optional

from .artifacts import ArtifactRecorder
from .schemas import BenchmarkTask


SIGNATURE_ROUTING_SCHEMA_VERSION = "openagents.benchmark_signature_routing.v1"
PROBE_CODEX_AGENT_SLUGS = {"probe-codex", "probe-codex-signatures", "openagents-probe-codex"}

SIGNATURE_PLAYBOOKS: Dict[str, Dict[str, Any]] = {
    "benchmark.runner_supervisor": {
        "expectedReward": 1.0,
        "playbook": [
            "Treat missing closeout as a runner failure, not an agent failure.",
            "Emit periodic heartbeats while the verifier is running.",
            "If a verifier result exists but the parent job stalls, finalize artifacts from the partial result before continuing the queue.",
            "Record terminal-state repair evidence before retrying or skipping a task.",
        ],
        "evidence": ["runner-heartbeats.jsonl", "terminal-state-repair.json", "artifact-finalization.txt"],
    },
    "coding.gcode_parser_guard": {
        "expectedReward": 0.5,
        "playbook": [
            "Build a small parser instead of regex-only extraction.",
            "Separate comments, modal commands, motion commands, tool commands, units, and coordinates.",
            "Write golden examples that cover comments, numeric normalization, and multi-command lines before closeout.",
            "Preserve a parser-normalization report so hidden-case misses are diagnosable.",
        ],
        "evidence": ["gcode-golden-cases.txt", "parser-normalization-report.txt"],
    },
    "coding.python_package_index": {
        "expectedReward": 1.0,
        "playbook": [
            "Implement the PEP 503 Simple Repository API shape first.",
            "Normalize package names and expose predictable /simple/<normalized-name>/ links.",
            "Serve package files with stable hrefs and hashes when available.",
            "Run a local pip install smoke against the served index before calling the task complete.",
        ],
        "evidence": ["simple-index-tree.txt", "pip-install-smoke.txt", "package-index-service.log"],
    },
    "coding.query_optimizer_workflow": {
        "expectedReward": 0.8,
        "playbook": [
            "Capture baseline query results before changing indexes or SQL.",
            "Use EXPLAIN or EXPLAIN ANALYZE to compare candidate plans.",
            "Reject any optimization that changes result equivalence.",
            "Preserve before/after query plans and timing evidence for the accepted candidate.",
        ],
        "evidence": ["query-plan-before.txt", "query-plan-after.txt", "result-equivalence.txt"],
    },
    "coding.service_readiness": {
        "expectedReward": 1.0,
        "playbook": [
            "Identify the expected bind host, port, path, and protocol before editing service config.",
            "Start the service under a supervised process or foreground command that leaves logs.",
            "Probe the endpoint locally with curl or an equivalent socket check before verifier closeout.",
            "If the first probe fails, inspect service logs and retry the minimal config fix before declaring done.",
        ],
        "evidence": ["service-readiness.json", "service-logs.txt", "health-checks.txt"],
    },
    "coding.sqlite_wal_recovery": {
        "expectedReward": 1.0,
        "playbook": [
            "Before opening SQLite, copy the database, WAL, and SHM files as a matched set into a recovery directory.",
            "Open only the copied database so SQLite cannot delete or checkpoint unreadable original sidecars.",
            "Run PRAGMA integrity_check and preserve the output.",
            "Checkpoint or recover after the sidecars are safely copied, then record row counts or data digests.",
        ],
        "evidence": ["sqlite-integrity.txt", "wal-recovery-report.txt", "recovered-data-digest.txt"],
    },
    "coding.xss_sanitizer_policy": {
        "expectedReward": 1.0,
        "playbook": [
            "Preserve benign HTML semantics before adding blocking rules.",
            "Block executable vectors including script tags, event-handler attributes, javascript: URLs, and dangerous SVG/MathML paths.",
            "Create allowed-case and blocked-case fixtures and run them before closeout.",
            "Do not copy benchmark-local payload secrets into artifacts; summarize policy classes instead.",
        ],
        "evidence": ["sanitizer-allowed-cases.txt", "sanitizer-blocked-cases.txt", "xss-policy-report.txt"],
    },
}


def is_probe_signature_agent(agent_slug: str) -> bool:
    return agent_slug in PROBE_CODEX_AGENT_SLUGS


def signature_routing_metadata(task: BenchmarkTask) -> Optional[Dict[str, Any]]:
    value = task.metadata.get("signatureRouting")
    if value is None:
        return None
    if not isinstance(value, dict):
        raise ValueError("metadata.signatureRouting must be an object")
    return value


def _string_list(value: Any, field_name: str) -> List[str]:
    if value is None:
        return []
    if not isinstance(value, list) or not all(isinstance(item, str) and item for item in value):
        raise ValueError("metadata.signatureRouting.%s must be a string array" % field_name)
    return list(value)


def build_signature_selector_trace(task: BenchmarkTask, agent_slug: str) -> Optional[Dict[str, Any]]:
    routing = signature_routing_metadata(task)
    if routing is None:
        return None

    selection_enabled = is_probe_signature_agent(agent_slug)
    expected_signature_ids = _string_list(routing.get("expectedSignatureIds"), "expectedSignatureIds")
    candidate_signature_ids = _string_list(routing.get("candidateSignatureIds"), "candidateSignatureIds")
    forbidden_signature_ids = _string_list(routing.get("forbiddenSignatureIds"), "forbiddenSignatureIds")
    selected_signature_ids = expected_signature_ids if selection_enabled else []
    selected_forbidden = sorted(set(selected_signature_ids).intersection(forbidden_signature_ids))
    selected_playbooks = {
        signature_id: SIGNATURE_PLAYBOOKS[signature_id]
        for signature_id in selected_signature_ids
        if signature_id in SIGNATURE_PLAYBOOKS
    }
    expected_probe_reward = None
    if selected_playbooks:
        expected_probe_reward = max(float(playbook.get("expectedReward", 0.0)) for playbook in selected_playbooks.values())
    raw_reward = task.metadata.get("rawCodexReward")

    return {
        "schemaVersion": SIGNATURE_ROUTING_SCHEMA_VERSION,
        "taskId": task.id,
        "terminalBenchTaskId": task.metadata.get("terminalBenchTaskId") or task.id,
        "dataset": task.dataset,
        "datasetVersion": task.version,
        "agentSlug": agent_slug,
        "selectionEnabled": selection_enabled,
        "selectorMode": "fixture_expected" if selection_enabled else "baseline_disabled",
        "retainedFailureFamily": task.metadata.get("retainedFailureFamily"),
        "preservedRunRef": task.metadata.get("preservedRunRef"),
        "sourceDoc": task.metadata.get("sourceDoc"),
        "taskChecksum": task.metadata.get("taskChecksum"),
        "rawCodexReward": task.metadata.get("rawCodexReward"),
        "rawCodexVerifier": task.metadata.get("rawCodexVerifier"),
        "candidateSignatureIds": candidate_signature_ids,
        "expectedSignatureIds": expected_signature_ids,
        "selectedSignatureIds": selected_signature_ids,
        "selectedSignaturePlaybooks": selected_playbooks,
        "forbiddenSignatureIds": forbidden_signature_ids,
        "selectedForbiddenSignatureIds": selected_forbidden,
        "failureFingerprints": _string_list(routing.get("failureFingerprints"), "failureFingerprints"),
        "requiredEvidence": _string_list(routing.get("requiredEvidence"), "requiredEvidence"),
        "closeoutArtifacts": _string_list(routing.get("closeoutArtifacts"), "closeoutArtifacts"),
        "routingPassed": selection_enabled and selected_signature_ids == expected_signature_ids and not selected_forbidden,
        "rawCodexReward": raw_reward,
        "expectedProbeSignatureReward": expected_probe_reward,
        "expectedRewardDelta": _reward_delta(raw_reward, expected_probe_reward),
        "publicClaimState": "internal_regression_fixture",
    }


def record_signature_selector_trace(
    task: BenchmarkTask,
    recorder: ArtifactRecorder,
    agent_slug: str,
) -> Optional[Dict[str, Any]]:
    trace = build_signature_selector_trace(task, agent_slug)
    if trace is None:
        return None

    recorder.json("signature_selector_trace.json", trace)
    recorder.event(
        "signature_selector_trace_recorded",
        {
            "taskId": trace["taskId"],
            "selectionEnabled": trace["selectionEnabled"],
            "selectedSignatureIds": trace["selectedSignatureIds"],
        },
    )
    return trace


def signature_prompt_addendum(task: BenchmarkTask, agent_slug: str) -> str:
    trace = build_signature_selector_trace(task, agent_slug)
    if trace is None or not trace["selectionEnabled"]:
        return ""

    lines = [
        "Probe signature context:",
        "Selected signatures: %s" % ", ".join(trace["selectedSignatureIds"]),
    ]
    if trace["failureFingerprints"]:
        lines.append("Failure fingerprints: %s" % ", ".join(trace["failureFingerprints"]))
    if trace["requiredEvidence"]:
        lines.append("Required evidence: %s" % ", ".join(trace["requiredEvidence"]))
    if trace["closeoutArtifacts"]:
        lines.append("Closeout artifacts: %s" % ", ".join(trace["closeoutArtifacts"]))
    if trace["selectedSignaturePlaybooks"]:
        lines.append("Signature playbooks:")
        for signature_id, playbook in trace["selectedSignaturePlaybooks"].items():
            lines.append("- %s:" % signature_id)
            for step in playbook.get("playbook", []):
                lines.append("  - %s" % step)
            evidence = playbook.get("evidence", [])
            if evidence:
                lines.append("  - Evidence files: %s" % ", ".join(evidence))
    if trace["expectedRewardDelta"] is not None:
        lines.append(
            "Retained-fixture target: raw Codex reward %s -> expected signature reward %s (delta %+0.2f)."
            % (trace["rawCodexReward"], trace["expectedProbeSignatureReward"], trace["expectedRewardDelta"])
        )
    lines.append("Do not expose hidden verifier details or benchmark-local secrets.")
    return "\n".join(lines)


def _reward_delta(raw_reward: Any, expected_probe_reward: Any) -> Optional[float]:
    if raw_reward is None or expected_probe_reward is None:
        return None
    try:
        return float(expected_probe_reward) - float(raw_reward)
    except (TypeError, ValueError):
        return None
