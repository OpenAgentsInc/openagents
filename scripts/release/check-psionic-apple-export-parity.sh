#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-apple-export-parity.sh \
  --oracle-path <toolkit-exported.fmadapter> \
  --candidate-path <candidate.fmadapter> \
  [--candidate-expected success|failure] \
  [--bridge-base-url http://127.0.0.1:11435] \
  [--output-dir <path>]

Compares two Apple `.fmadapter` directories, writes a machine-readable parity
report, and then asks the live Swift bridge to load the oracle package and the
candidate package.

The gate passes only if:

- the oracle package loads successfully through the live bridge
- the candidate package outcome matches `--candidate-expected`

This script is the canonical export-validity gate for issue #3664. Package
shape or metadata alone are not treated as success.
EOF
}

bridge_base_url="http://127.0.0.1:11435"
candidate_expected="failure"
candidate_path=""
oracle_path=""
output_dir=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --oracle-path)
      oracle_path="$2"
      shift 2
      ;;
    --candidate-path)
      candidate_path="$2"
      shift 2
      ;;
    --candidate-expected)
      candidate_expected="$2"
      shift 2
      ;;
    --bridge-base-url)
      bridge_base_url="$2"
      shift 2
      ;;
    --output-dir)
      output_dir="$2"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -z "$oracle_path" || -z "$candidate_path" ]]; then
  echo "Both --oracle-path and --candidate-path are required." >&2
  usage >&2
  exit 1
fi

if [[ "$candidate_expected" != "success" && "$candidate_expected" != "failure" ]]; then
  echo "--candidate-expected must be 'success' or 'failure'." >&2
  exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repo_root}"

timestamp="$(date -u +"%Y%m%dT%H%M%SZ")"
output_dir="${output_dir:-$repo_root/output/apple-export-parity/$timestamp}"
mkdir -p "${output_dir}"

health_json="${output_dir}/bridge-health.json"
parity_json="${output_dir}/parity-report.json"
summary_json="${output_dir}/summary.json"
oracle_response_json="${output_dir}/oracle-load.json"
candidate_response_json="${output_dir}/candidate-load.json"

echo "==> checking bridge health"
curl -fsS "${bridge_base_url}/health" > "${health_json}"

echo "==> comparing package inventory and metadata"
python3 - "$oracle_path" "$candidate_path" "$parity_json" <<'PY'
import hashlib
import json
import os
import sys
from pathlib import Path

oracle_path = Path(sys.argv[1])
candidate_path = Path(sys.argv[2])
output_path = Path(sys.argv[3])


def file_digest(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def collect_package(path: Path) -> dict:
    files = {}
    for file_path in sorted(p for p in path.rglob("*") if p.is_file()):
        relative = file_path.relative_to(path).as_posix()
        files[relative] = {
            "byte_length": file_path.stat().st_size,
            "sha256": file_digest(file_path),
        }
    metadata_path = path / "metadata.json"
    metadata = {}
    if metadata_path.exists():
        metadata = json.loads(metadata_path.read_text())
    return {
        "path": str(path),
        "package_name": path.name,
        "files": files,
        "metadata": metadata,
    }


def flatten_json(value, prefix=""):
    if isinstance(value, dict):
        result = {}
        for key, child in value.items():
            child_prefix = f"{prefix}.{key}" if prefix else key
            result.update(flatten_json(child, child_prefix))
        return result
    if isinstance(value, list):
        return {prefix: value}
    return {prefix: value}


oracle = collect_package(oracle_path)
candidate = collect_package(candidate_path)

oracle_files = oracle["files"]
candidate_files = candidate["files"]
oracle_names = set(oracle_files)
candidate_names = set(candidate_files)
shared_names = sorted(oracle_names & candidate_names)

inventory = {
    "oracle_only_files": sorted(oracle_names - candidate_names),
    "candidate_only_files": sorted(candidate_names - oracle_names),
    "differing_files": {},
}

for relative in shared_names:
    oracle_entry = oracle_files[relative]
    candidate_entry = candidate_files[relative]
    if oracle_entry != candidate_entry:
        inventory["differing_files"][relative] = {
            "oracle": oracle_entry,
            "candidate": candidate_entry,
        }

oracle_metadata = flatten_json(oracle["metadata"])
candidate_metadata = flatten_json(candidate["metadata"])
metadata_keys = set(oracle_metadata) | set(candidate_metadata)
metadata = {
    "oracle_only_fields": {},
    "candidate_only_fields": {},
    "differing_fields": {},
}

for key in sorted(metadata_keys):
    if key not in candidate_metadata:
        metadata["oracle_only_fields"][key] = oracle_metadata[key]
    elif key not in oracle_metadata:
        metadata["candidate_only_fields"][key] = candidate_metadata[key]
    elif oracle_metadata[key] != candidate_metadata[key]:
        metadata["differing_fields"][key] = {
            "oracle": oracle_metadata[key],
            "candidate": candidate_metadata[key],
        }

report = {
    "oracle": {
        "path": oracle["path"],
        "package_name": oracle["package_name"],
        "file_count": len(oracle_files),
        "adapter_weights_bytes": oracle_files.get("adapter_weights.bin", {}).get("byte_length"),
        "adapter_weights_sha256": oracle_files.get("adapter_weights.bin", {}).get("sha256"),
        "metadata": oracle["metadata"],
    },
    "candidate": {
        "path": candidate["path"],
        "package_name": candidate["package_name"],
        "file_count": len(candidate_files),
        "adapter_weights_bytes": candidate_files.get("adapter_weights.bin", {}).get("byte_length"),
        "adapter_weights_sha256": candidate_files.get("adapter_weights.bin", {}).get("sha256"),
        "metadata": candidate["metadata"],
    },
    "inventory": inventory,
    "metadata_delta": metadata,
}

output_path.write_text(json.dumps(report, indent=2, sort_keys=True))
print(json.dumps({
    "oracle_adapter_weights_bytes": report["oracle"]["adapter_weights_bytes"],
    "candidate_adapter_weights_bytes": report["candidate"]["adapter_weights_bytes"],
    "oracle_only_files": inventory["oracle_only_files"],
    "candidate_only_files": inventory["candidate_only_files"],
    "differing_file_count": len(inventory["differing_files"]),
    "differing_metadata_field_count": len(metadata["differing_fields"]),
}, indent=2))
PY

load_adapter() {
  local package_path="$1"
  local requested_adapter_id="$2"
  local response_path="$3"
  local request_path
  request_path="$(mktemp)"
  python3 - "$package_path" "$requested_adapter_id" > "$request_path" <<'PY'
import json
import sys
payload = {
    "package_path": sys.argv[1],
    "requested_adapter_id": sys.argv[2],
}
print(json.dumps(payload))
PY
  local http_code
  http_code="$(
    curl -sS \
      -o "$response_path" \
      -w '%{http_code}' \
      -X POST \
      "${bridge_base_url}/v1/adapters/load" \
      -H 'content-type: application/json' \
      --data "@${request_path}"
  )"
  rm -f "$request_path"
  echo "$http_code"
}

parse_loaded_adapter_id() {
  python3 - "$1" <<'PY'
import json
import sys
with open(sys.argv[1], "r", encoding="utf-8") as handle:
    data = json.load(handle)
print(data["adapter"]["adapter"]["adapter_id"])
PY
}

summarize_failure_reason() {
  python3 - "$1" <<'PY'
import json
import sys
with open(sys.argv[1], "r", encoding="utf-8") as handle:
    data = json.load(handle)
error = data.get("error", {})
print(json.dumps({
    "type": error.get("type"),
    "message": error.get("message"),
    "failure_reason": error.get("failure_reason"),
    "debug_description": error.get("debug_description"),
}, indent=2))
PY
}

unload_adapter() {
  local adapter_id="$1"
  curl -fsS -X DELETE "${bridge_base_url}/v1/adapters/${adapter_id}" >/dev/null
}

oracle_adapter_id="openagents_oracle_${timestamp}_$$"
candidate_adapter_id="openagents_candidate_${timestamp}_$$"

echo "==> loading oracle package through live bridge"
oracle_http_code="$(load_adapter "$oracle_path" "$oracle_adapter_id" "$oracle_response_json")"
if [[ ! "$oracle_http_code" =~ ^2 ]]; then
  echo "Oracle package failed to load through the live bridge." >&2
  cat "$oracle_response_json" >&2
  exit 1
fi
oracle_loaded_id="$(parse_loaded_adapter_id "$oracle_response_json")"
unload_adapter "$oracle_loaded_id"

echo "==> loading candidate package through live bridge"
candidate_http_code="$(load_adapter "$candidate_path" "$candidate_adapter_id" "$candidate_response_json")"
candidate_result="failure"
candidate_failure_reason=""
if [[ "$candidate_http_code" =~ ^2 ]]; then
  candidate_result="success"
  candidate_loaded_id="$(parse_loaded_adapter_id "$candidate_response_json")"
  unload_adapter "$candidate_loaded_id"
else
  candidate_failure_reason="$(summarize_failure_reason "$candidate_response_json")"
fi

python3 - "$summary_json" "$oracle_path" "$candidate_path" "$candidate_expected" "$candidate_result" "$oracle_http_code" "$candidate_http_code" "$parity_json" "$health_json" <<'PY'
import json
import sys
summary = {
    "oracle_path": sys.argv[2],
    "candidate_path": sys.argv[3],
    "candidate_expected": sys.argv[4],
    "candidate_result": sys.argv[5],
    "oracle_http_code": sys.argv[6],
    "candidate_http_code": sys.argv[7],
    "parity_report_path": sys.argv[8],
    "bridge_health_path": sys.argv[9],
}
with open(sys.argv[1], "w", encoding="utf-8") as handle:
    json.dump(summary, handle, indent=2, sort_keys=True)
PY

if [[ "$candidate_result" != "$candidate_expected" ]]; then
  echo "Candidate package outcome mismatch." >&2
  echo "Expected: $candidate_expected" >&2
  echo "Actual:   $candidate_result" >&2
  if [[ -n "$candidate_failure_reason" ]]; then
    echo "$candidate_failure_reason" >&2
  else
    cat "$candidate_response_json" >&2
  fi
  exit 1
fi

echo
echo "Apple export parity gate passed."
echo "  oracle:    ${oracle_path}"
echo "  candidate: ${candidate_path}"
echo "  expected:  ${candidate_expected}"
echo "  result:    ${candidate_result}"
echo "  artifacts:"
echo "    - ${parity_json}"
echo "    - ${oracle_response_json}"
echo "    - ${candidate_response_json}"
echo "    - ${summary_json}"
