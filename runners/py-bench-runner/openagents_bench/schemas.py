from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional


JsonDict = Dict[str, Any]


class SchemaError(ValueError):
    """Raised when a benchmark envelope does not match the normalized contract."""


def _require_string(data: JsonDict, key: str) -> str:
    value = data.get(key)
    if not isinstance(value, str) or not value:
        raise SchemaError("missing or invalid string field: %s" % key)
    return value


def _optional_string(data: JsonDict, key: str) -> Optional[str]:
    value = data.get(key)
    if value is None:
        return None
    if not isinstance(value, str) or not value:
        raise SchemaError("invalid string field: %s" % key)
    return value


def _dict_field(data: JsonDict, key: str) -> JsonDict:
    value = data.get(key, {})
    if not isinstance(value, dict):
        raise SchemaError("invalid object field: %s" % key)
    return dict(value)


@dataclass(frozen=True)
class BenchmarkTask:
    id: str
    dataset: str
    version: str
    instruction: str
    environment: JsonDict = field(default_factory=dict)
    verifier: JsonDict = field(default_factory=dict)
    limits: JsonDict = field(default_factory=dict)
    metadata: JsonDict = field(default_factory=dict)
    repo_url: Optional[str] = None
    base_commit: Optional[str] = None

    @staticmethod
    def from_dict(data: JsonDict) -> "BenchmarkTask":
        if not isinstance(data, dict):
            raise SchemaError("task spec must be an object")

        return BenchmarkTask(
            id=_require_string(data, "id"),
            dataset=_require_string(data, "dataset"),
            version=_require_string(data, "version"),
            instruction=_require_string(data, "instruction"),
            repo_url=_optional_string(data, "repo_url"),
            base_commit=_optional_string(data, "base_commit"),
            environment=_dict_field(data, "environment"),
            verifier=_dict_field(data, "verifier"),
            limits=_dict_field(data, "limits"),
            metadata=_dict_field(data, "metadata"),
        )


@dataclass(frozen=True)
class VerifierResult:
    exit_code: Optional[int]
    passed: bool
    score: float
    summary: str

    def to_dict(self) -> JsonDict:
        return {
            "exitCode": self.exit_code,
            "passed": self.passed,
            "score": self.score,
            "summary": self.summary,
        }


@dataclass(frozen=True)
class UsageReport:
    wall_time_ms: int
    agent_steps: int = 0
    shell_commands: int = 0
    input_tokens: int = 0
    output_tokens: int = 0
    cost_usd: float = 0.0

    def to_dict(self) -> JsonDict:
        return {
            "wallTimeMs": self.wall_time_ms,
            "agentSteps": self.agent_steps,
            "shellCommands": self.shell_commands,
            "inputTokens": self.input_tokens,
            "outputTokens": self.output_tokens,
            "costUsd": self.cost_usd,
        }


@dataclass(frozen=True)
class ArtifactRef:
    path: str
    kind: str
    bytes: int
    sha256: str

    def to_dict(self) -> JsonDict:
        return {
            "path": self.path,
            "kind": self.kind,
            "bytes": self.bytes,
            "sha256": self.sha256,
        }


@dataclass(frozen=True)
class BenchmarkResult:
    run_id: str
    task_run_id: str
    task_id: str
    dataset: str
    dataset_version: str
    agent_slug: str
    model: str
    harness_version: str
    status: str
    verifier: VerifierResult
    usage: UsageReport
    artifacts: List[ArtifactRef]
    error: Optional[str] = None

    def to_dict(self) -> JsonDict:
        return {
            "runId": self.run_id,
            "taskRunId": self.task_run_id,
            "taskId": self.task_id,
            "dataset": self.dataset,
            "datasetVersion": self.dataset_version,
            "agentSlug": self.agent_slug,
            "model": self.model,
            "harnessVersion": self.harness_version,
            "status": self.status,
            "verifier": self.verifier.to_dict(),
            "usage": self.usage.to_dict(),
            "artifacts": [artifact.to_dict() for artifact in self.artifacts],
            "error": self.error,
        }


@dataclass(frozen=True)
class BenchmarkArtifactManifest:
    run_id: str
    task_run_id: str
    artifacts: List[ArtifactRef]

    def to_dict(self) -> JsonDict:
        return {
            "runId": self.run_id,
            "taskRunId": self.task_run_id,
            "artifacts": [artifact.to_dict() for artifact in self.artifacts],
        }


@dataclass(frozen=True)
class BenchmarkProofBundle:
    run_id: str
    task_run_id: str
    task_id: str
    dataset: str
    dataset_version: str
    agent_slug: str
    model: str
    harness_version: str
    status: str
    artifact_count: int
    artifact_digests: List[str]
    redaction_status: str
    claim_state: str
    provider: str
    retry_policy: str
    timeout_seconds: Optional[int]
    task_selector: str
    resource_usage_receipt_digest: Optional[str] = None
    cloud_execution_closeout_digest: Optional[str] = None
    wallet_authority: bool = False
    payout_authority: bool = False
    public_claim_authority: bool = False

    def to_dict(self) -> JsonDict:
        return {
            "runId": self.run_id,
            "taskRunId": self.task_run_id,
            "taskId": self.task_id,
            "dataset": self.dataset,
            "datasetVersion": self.dataset_version,
            "agentSlug": self.agent_slug,
            "model": self.model,
            "harnessVersion": self.harness_version,
            "status": self.status,
            "artifactCount": self.artifact_count,
            "artifactDigests": self.artifact_digests,
            "redactionStatus": self.redaction_status,
            "claimState": self.claim_state,
            "provider": self.provider,
            "retryPolicy": self.retry_policy,
            "timeoutSeconds": self.timeout_seconds,
            "taskSelector": self.task_selector,
            "resourceUsageReceiptDigest": self.resource_usage_receipt_digest,
            "cloudExecutionCloseoutDigest": self.cloud_execution_closeout_digest,
            "walletAuthority": self.wallet_authority,
            "payoutAuthority": self.payout_authority,
            "publicClaimAuthority": self.public_claim_authority,
        }
