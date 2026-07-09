import hashlib
import json
import re
import time
from pathlib import Path
from typing import Any, Dict, Iterable, List, Optional

from .schemas import ArtifactRef


SECRET_VALUE_RE = re.compile(
    r"(?i)\b(token|secret|password|api[_-]?key|authorization|bearer)\b\s*[:=]\s*([^\s,;]+)"
)
SECRET_MARKER_RE = re.compile(
    r"(?i)(sk-[a-z0-9_-]{12,}|xox[baprs]-[a-z0-9-]{12,}|gh[pousr]_[a-z0-9_]{12,}|"
    r"wallet[_ -]?seed|private[_ -]?key|refresh[_ -]?token|access[_ -]?token|bearer\s+[a-z0-9._-]{12,})"
)


def redact_text(value: str) -> str:
    redacted = SECRET_VALUE_RE.sub(lambda match: "%s=<redacted>" % match.group(1), value)
    return SECRET_MARKER_RE.sub("<redacted-secret>", redacted)


def redact_json(value: Any) -> Any:
    if isinstance(value, str):
        return redact_text(value)
    if isinstance(value, list):
        return [redact_json(item) for item in value]
    if isinstance(value, dict):
        return {key: redact_json(item) for key, item in value.items()}
    return value


def contains_secret_material(value: str) -> bool:
    return SECRET_MARKER_RE.search(value) is not None or SECRET_VALUE_RE.search(value) is not None


def write_json(path: Path, payload: Dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(redact_json(payload), indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_jsonl(path: Path, rows: Iterable[Dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(json.dumps(row, sort_keys=True) + "\n")


def artifact_kind(path: Path) -> str:
    name = path.name
    if name == "result.json":
        return "result"
    if name == "metadata.json":
        return "metadata"
    if name == "resource_usage_receipt.json":
        return "resource_usage_receipt"
    if name == "events.jsonl":
        return "events"
    if name == "commands.jsonl":
        return "commands"
    if name == "transcript.md":
        return "transcript"
    if name.endswith(".diff"):
        return "diff"
    if "verifier" in name:
        return "verifier_log"
    if "agent" in name:
        return "agent_log"
    return "artifact"


class ArtifactRecorder:
    def __init__(self, artifact_dir: Path):
        self.artifact_dir = artifact_dir
        self.artifact_dir.mkdir(parents=True, exist_ok=True)
        self.events_path = self.artifact_dir / "events.jsonl"
        self._events: List[Dict[str, Any]] = []

    def event(self, event_type: str, payload: Optional[Dict[str, Any]] = None) -> None:
        event = {
            "type": event_type,
            "source": "runner",
            "createdAt": int(time.time() * 1000),
            "payload": payload or {},
        }
        self._events.append(event)
        write_jsonl(self.events_path, self._events)

    def text(self, name: str, value: str) -> None:
        path = self.artifact_dir / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(redact_text(value), encoding="utf-8")

    def json(self, name: str, value: Dict[str, Any]) -> None:
        path = self.artifact_dir / name
        write_json(path, value)

    def jsonl(self, name: str, rows: Iterable[Dict[str, Any]]) -> None:
        path = self.artifact_dir / name
        write_jsonl(path, rows)

    def collect(self, exclude: Iterable[str] = ()) -> List[ArtifactRef]:
        excluded = set(exclude)
        artifacts: List[ArtifactRef] = []
        for path in sorted(self.artifact_dir.rglob("*")):
            if not path.is_file() or path.name in excluded:
                continue
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            relative = path.relative_to(self.artifact_dir).as_posix()
            artifacts.append(
                ArtifactRef(
                    path=relative,
                    kind=artifact_kind(path),
                    bytes=path.stat().st_size,
                    sha256="sha256:%s" % digest,
                )
            )
        return artifacts
