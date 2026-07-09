import shutil
import subprocess
from pathlib import Path
from typing import List


class GcsError(RuntimeError):
    pass


def is_gcs_uri(value: str) -> bool:
    return value.startswith("gs://")


def _run_gcloud(args: List[str]) -> None:
    if shutil.which("gcloud") is None:
        raise GcsError("gcloud is required for GCS-backed benchmark runs")
    completed = subprocess.run(["gcloud"] + args, check=False, text=True, capture_output=True)
    if completed.returncode != 0:
        raise GcsError(completed.stderr.strip() or completed.stdout.strip() or "gcloud command failed")


def download_object(uri: str, destination: Path) -> None:
    if not is_gcs_uri(uri):
        raise GcsError("expected GCS URI: %s" % uri)
    destination.parent.mkdir(parents=True, exist_ok=True)
    _run_gcloud(["storage", "cp", uri, str(destination)])


def upload_directory(source: Path, prefix: str) -> None:
    if not is_gcs_uri(prefix):
        raise GcsError("expected GCS artifact prefix: %s" % prefix)
    normalized_prefix = prefix.rstrip("/")
    for path in sorted(source.rglob("*")):
        if path.is_file():
            relative = path.relative_to(source).as_posix()
            _run_gcloud(["storage", "cp", str(path), "%s/%s" % (normalized_prefix, relative)])
