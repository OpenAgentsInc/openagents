import argparse
import hashlib
import json
import os
import platform
import shutil
import tempfile
import time
from pathlib import Path
from typing import Dict, List, Optional

from . import HARNESS_VERSION
from .artifacts import ArtifactRecorder
from .fake_adapter import BenchmarkTimeout, run_fake_task
from .gcs import download_object, upload_directory
from .repo_adapter import is_repo_dataset, run_repo_task
from .schemas import (
    BenchmarkArtifactManifest,
    BenchmarkProofBundle,
    BenchmarkResult,
    BenchmarkTask,
    SchemaError,
    UsageReport,
    VerifierResult,
)
from .signature_routing import record_signature_selector_trace
from .terminal_bench import run_terminal_bench_task


def _safe_token(value: str) -> str:
    cleaned = []
    for char in value.strip()[:96]:
        if char.isalnum() or char in "-_.:/@+=#":
            cleaned.append(char)
        else:
            cleaned.append("_")
    token = "".join(cleaned).strip("_")
    return token or "unknown"


def _sha256_json(payload: Dict) -> str:
    encoded = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return "sha256:%s" % hashlib.sha256(encoded).hexdigest()


def _cloud_execution_closeout(
    *,
    run_id: str,
    task_run_id: str,
    task_id: str,
    dataset: str,
    dataset_version: str,
    status: str,
    artifact_digests: List[str],
    resource_usage_receipt_digest: str,
) -> Dict:
    emitted_at_ms = int(time.time() * 1000)
    payload = {
        "schemaVersion": "openagents.benchmark_cloud_closeout.v1",
        "closeoutId": "benchmark.cloud.closeout.%s.%s" % (_safe_token(task_run_id), emitted_at_ms),
        "runRef": run_id,
        "taskRunRef": task_run_id,
        "taskRef": task_id,
        "dataset": dataset,
        "datasetVersion": dataset_version,
        "status": status,
        "artifactDigests": artifact_digests,
        "resourceUsageReceiptDigest": resource_usage_receipt_digest,
        "redactionStatus": "local_redaction_applied",
        "claimState": "internal",
        "publicSafe": True,
        "walletAuthority": False,
        "payoutAuthority": False,
        "publicClaimAuthority": False,
        "authorityOwner": "omega",
        "executionEvidenceOnly": True,
        "emittedAtMs": emitted_at_ms,
    }
    payload["closeoutDigest"] = _sha256_json(payload)
    return payload


def _directory_bytes(path: Path) -> Optional[int]:
    if not path.exists():
        return None
    total = 0
    for child in path.rglob("*"):
        if child.is_file():
            total += child.stat().st_size
    return total


def _provider_lane() -> str:
    node_ref = os.environ.get("OA_NODE_ID", "benchmark-runner-local").lower()
    if "shc" in node_ref:
        return "shc"
    if "gcp" in node_ref or "gce" in node_ref:
        return "gcp"
    if "provider" in node_ref or "pylon" in node_ref:
        return "provider"
    if "local" in node_ref:
        return "local"
    return os.environ.get("OA_PROVIDER_LANE", "unknown")


def _resource_usage_receipt(
    *,
    run_id: str,
    task_run_id: str,
    task_id: str,
    artifact_dir: Path,
    agent_slug: str,
    model: str,
    usage: UsageReport,
) -> Dict:
    emitted_at_ms = int(time.time() * 1000)
    disk_usage = shutil.disk_usage(artifact_dir)
    kvm_present = Path("/dev/kvm").exists()
    cgroup_mode = None
    if Path("/sys/fs/cgroup/cgroup.controllers").exists():
        cgroup_mode = "v2"
    elif Path("/sys/fs/cgroup").exists():
        cgroup_mode = "v1_or_hybrid"
    container_runtime = "docker" if Path("/.dockerenv").exists() else os.environ.get("container")
    log_bytes = sum(
        (artifact_dir / name).stat().st_size
        for name in ("events.jsonl", "commands.jsonl", "agent_stdout.log", "agent_stderr.log")
        if (artifact_dir / name).exists()
    )
    payload = {
        "schemaVersion": "openagents.resource_usage_receipt.v1",
        "receiptId": "resource.usage.%s.%s" % (_safe_token(task_run_id), emitted_at_ms),
        "runRef": run_id,
        "taskRunRef": task_run_id,
        "workroomId": task_run_id,
        "nodeRef": _safe_token(os.environ.get("OA_NODE_ID", "benchmark-runner-local")),
        "providerLane": _provider_lane(),
        "host": {
            "os": _safe_token(platform.platform()),
            "arch": _safe_token(platform.machine() or "unknown"),
            "cpu": _safe_token(platform.processor() or "unknown"),
            "logicalCpuCount": os.cpu_count() or 1,
            "physicalCpuCount": None,
            "memoryTotalBytes": None,
            "memoryAvailableBytes": None,
            "diskTotalBytes": disk_usage.total,
            "diskAvailableBytes": disk_usage.free,
            "acceleratorInventory": ["nvidia_gpu"] if Path("/dev/nvidia0").exists() else [],
            "virtualization": {
                "kvmPresent": kvm_present,
                "firecrackerCandidate": platform.system().lower() == "linux" and kvm_present,
                "containerRuntime": _safe_token(container_runtime) if container_runtime else None,
                "cgroupMode": cgroup_mode,
            },
        },
        "run": {
            "sandbox": _safe_token(os.environ.get("OA_BENCH_SANDBOX", "container_or_process")),
            "imageOrProfileDigest": _sha256_json({"runner": "py-bench-runner", "agent": agent_slug}),
            "workspaceDigest": _sha256_json({"artifactDir": str(artifact_dir), "taskId": task_id}),
            "wallTimeMs": usage.wall_time_ms,
            "exitCode": None,
            "timedOut": False,
            "workspaceBytes": _directory_bytes(artifact_dir),
            "artifactBytes": _directory_bytes(artifact_dir),
            "logBytes": log_bytes,
        },
        "modelUsage": [
            {
                "provider": _safe_token(os.environ.get("OA_MODEL_PROVIDER", "unknown")),
                "backend": _safe_token(agent_slug),
                "model": _safe_token(model),
                "mode": "benchmark_task",
                "accountRef": _safe_token(os.environ["OA_PROVIDER_ACCOUNT_REF"])
                if os.environ.get("OA_PROVIDER_ACCOUNT_REF")
                else None,
                "inputTokens": usage.input_tokens or None,
                "cachedInputTokens": None,
                "outputTokens": usage.output_tokens or None,
                "reasoningTokens": None,
                "totalTokens": (usage.input_tokens + usage.output_tokens)
                if usage.input_tokens or usage.output_tokens
                else None,
                "countSource": "parsed_from_stream"
                if usage.input_tokens or usage.output_tokens
                else "unavailable",
                "costMicrousd": int(usage.cost_usd * 1_000_000) if usage.cost_usd else None,
                "billingBasis": "adapter_reported" if usage.cost_usd else "unknown_or_subscription",
                "unavailableReason": None
                if usage.input_tokens or usage.output_tokens
                else "runner_adapter_did_not_report_model_tokens",
            }
        ],
        "emittedAtMs": emitted_at_ms,
    }
    payload["receiptDigest"] = _sha256_json(payload)
    return payload


def load_task(path: Path) -> BenchmarkTask:
    return BenchmarkTask.from_dict(json.loads(path.read_text(encoding="utf-8")))


def run_dataset_adapter(
    task: BenchmarkTask,
    recorder: ArtifactRecorder,
    agent_slug: str,
    model: str,
):
    if task.dataset == "terminal-bench":
        return run_terminal_bench_task(task, recorder, agent_slug, model)
    if is_repo_dataset(task.dataset):
        return run_repo_task(task, recorder, agent_slug, model)
    if task.dataset == "openagents-fake":
        return run_fake_task(task, recorder, agent_slug, model)
    raise ValueError("unsupported benchmark dataset: %s" % task.dataset)


def _error_result(
    run_id: str,
    task_run_id: str,
    task_id: str,
    dataset: str,
    dataset_version: str,
    agent_slug: str,
    model: str,
    status: str,
    error: str,
    started_at: float,
) -> BenchmarkResult:
    return BenchmarkResult(
        run_id=run_id,
        task_run_id=task_run_id,
        task_id=task_id,
        dataset=dataset,
        dataset_version=dataset_version,
        agent_slug=agent_slug,
        model=model,
        harness_version=HARNESS_VERSION,
        status=status,
        verifier=VerifierResult(exit_code=None, passed=False, score=0.0, summary=error),
        usage=UsageReport(wall_time_ms=int((time.monotonic() - started_at) * 1000)),
        artifacts=[],
        error=error,
    )


def run_task(
    run_id: str,
    task_run_id: str,
    task_spec: Path,
    artifact_dir: Path,
    agent_slug: str,
    model: str,
) -> BenchmarkResult:
    started_at = time.monotonic()
    recorder = ArtifactRecorder(artifact_dir)
    recorder.event("task_started", {"runId": run_id, "taskRunId": task_run_id})

    task_id = "unknown"
    dataset = "unknown"
    dataset_version = "unknown"
    proof_provider = "unknown"
    proof_retry_policy = "single_attempt"
    proof_timeout_seconds = None
    proof_task_selector = "unknown"

    try:
        task = load_task(task_spec)
        task_id = task.id
        dataset = task.dataset
        dataset_version = task.version
        proof_provider = str(task.metadata.get("provider") or agent_slug)
        proof_retry_policy = str(task.metadata.get("retryPolicy") or "single_attempt")
        if isinstance(task.limits.get("timeoutSeconds"), int):
            proof_timeout_seconds = int(task.limits["timeoutSeconds"])
        proof_task_selector = str(
            task.metadata.get("terminalBenchTaskId")
            or task.metadata.get("sweBenchInstanceId")
            or task.metadata.get("customTaskId")
            or task.id
        )
        recorder.event(
            "task_loaded",
            {
                "taskId": task.id,
                "dataset": task.dataset,
                "datasetVersion": task.version,
            },
        )
        record_signature_selector_trace(task, recorder, agent_slug)
        status, verifier, agent_steps, shell_commands = run_dataset_adapter(task, recorder, agent_slug, model)
        result = BenchmarkResult(
            run_id=run_id,
            task_run_id=task_run_id,
            task_id=task.id,
            dataset=task.dataset,
            dataset_version=task.version,
            agent_slug=agent_slug,
            model=model,
            harness_version=HARNESS_VERSION,
            status=status,
            verifier=verifier,
            usage=UsageReport(
                wall_time_ms=int((time.monotonic() - started_at) * 1000),
                agent_steps=agent_steps,
                shell_commands=shell_commands,
            ),
            artifacts=[],
        )
    except BenchmarkTimeout as exc:
        result = _error_result(
            run_id,
            task_run_id,
            task_id,
            dataset,
            dataset_version,
            agent_slug,
            model,
            "timeout",
            str(exc),
            started_at,
        )
    except (SchemaError, json.JSONDecodeError) as exc:
        recorder.event("task_spec_invalid", {"error": str(exc)})
        result = _error_result(
            run_id,
            task_run_id,
            task_id,
            dataset,
            dataset_version,
            agent_slug,
            model,
            "error",
            "invalid task spec: %s" % exc,
            started_at,
        )
    except Exception as exc:
        recorder.event("task_error", {"error": str(exc)})
        result = _error_result(
            run_id,
            task_run_id,
            task_id,
            dataset,
            dataset_version,
            agent_slug,
            model,
            "error",
            str(exc),
            started_at,
        )
    finally:
        recorder.event("task_finished", {"taskId": task_id})

    metadata = {
        "runId": run_id,
        "taskRunId": task_run_id,
        "taskId": result.task_id,
        "dataset": result.dataset,
        "datasetVersion": result.dataset_version,
        "agentSlug": agent_slug,
        "model": model,
        "harnessVersion": HARNESS_VERSION,
    }
    recorder.json("metadata.json", metadata)
    resource_receipt = _resource_usage_receipt(
        run_id=run_id,
        task_run_id=task_run_id,
        task_id=result.task_id,
        artifact_dir=artifact_dir,
        agent_slug=agent_slug,
        model=model,
        usage=result.usage,
    )
    recorder.json("resource_usage_receipt.json", resource_receipt)
    result = BenchmarkResult(
        run_id=result.run_id,
        task_run_id=result.task_run_id,
        task_id=result.task_id,
        dataset=result.dataset,
        dataset_version=result.dataset_version,
        agent_slug=result.agent_slug,
        model=result.model,
        harness_version=result.harness_version,
        status=result.status,
        verifier=result.verifier,
        usage=result.usage,
        artifacts=recorder.collect(exclude={"artifact_manifest.json", "proof_bundle.json", "result.json"}),
        error=result.error,
    )
    recorder.json("result.json", result.to_dict())
    pre_closeout_artifacts = recorder.collect(exclude={"artifact_manifest.json", "proof_bundle.json"})
    closeout = _cloud_execution_closeout(
        run_id=result.run_id,
        task_run_id=result.task_run_id,
        task_id=result.task_id,
        dataset=result.dataset,
        dataset_version=result.dataset_version,
        status=result.status,
        artifact_digests=[artifact.sha256 for artifact in pre_closeout_artifacts],
        resource_usage_receipt_digest=resource_receipt["receiptDigest"],
    )
    recorder.json("cloud_execution_closeout.json", closeout)
    manifest_artifacts = recorder.collect(exclude={"artifact_manifest.json", "proof_bundle.json"})
    recorder.json(
        "artifact_manifest.json",
        BenchmarkArtifactManifest(
            run_id=result.run_id,
            task_run_id=result.task_run_id,
            artifacts=manifest_artifacts,
        ).to_dict(),
    )
    recorder.json(
        "proof_bundle.json",
        BenchmarkProofBundle(
            run_id=result.run_id,
            task_run_id=result.task_run_id,
            task_id=result.task_id,
            dataset=result.dataset,
            dataset_version=result.dataset_version,
            agent_slug=result.agent_slug,
            model=result.model,
            harness_version=result.harness_version,
            status=result.status,
            artifact_count=len(manifest_artifacts),
            artifact_digests=[artifact.sha256 for artifact in manifest_artifacts],
            redaction_status="local_fake_redaction_applied",
            claim_state="internal",
            provider=proof_provider,
            retry_policy=proof_retry_policy,
            timeout_seconds=proof_timeout_seconds,
            task_selector=proof_task_selector,
            resource_usage_receipt_digest=resource_receipt["receiptDigest"],
            cloud_execution_closeout_digest=closeout["closeoutDigest"],
        ).to_dict(),
    )
    return result


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Run one normalized OpenAgents benchmark task")
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--task-run-id", required=True)
    task_spec_group = parser.add_mutually_exclusive_group(required=True)
    task_spec_group.add_argument("--task-spec")
    task_spec_group.add_argument("--task-spec-gcs")
    parser.add_argument("--artifact-dir")
    parser.add_argument("--artifact-prefix")
    parser.add_argument("--agent", required=True)
    parser.add_argument("--model", required=True)
    return parser


def main(argv: Optional[List[str]] = None) -> int:
    args = build_parser().parse_args(argv)
    with tempfile.TemporaryDirectory(prefix="oa-bench-runner-") as tmp:
        tmp_path = Path(tmp)
        task_spec = Path(args.task_spec) if args.task_spec else tmp_path / "task.json"
        artifact_dir = Path(args.artifact_dir) if args.artifact_dir else tmp_path / "artifacts"

        if args.task_spec_gcs:
            download_object(args.task_spec_gcs, task_spec)

        result = run_task(
            run_id=args.run_id,
            task_run_id=args.task_run_id,
            task_spec=task_spec,
            artifact_dir=artifact_dir,
            agent_slug=args.agent,
            model=args.model,
        )

        if args.artifact_prefix:
            upload_directory(artifact_dir, args.artifact_prefix)

    return 0 if result.status in {"passed", "failed", "timeout", "error"} else 1


if __name__ == "__main__":
    raise SystemExit(main())
