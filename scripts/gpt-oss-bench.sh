#!/bin/bash
#
# Benchmark GPT-OSS llama-server latency for raw completions.
# Usage:
#   scripts/gpt-oss-bench.sh 10
#
set -euo pipefail

RUNS="${1:-5}"
BASE_URL="${GPT_OSS_URL:-http://localhost:8000}"
MODEL="${GPT_OSS_MODEL:-gpt-oss-20b}"
MAX_TOKENS="${GPT_OSS_MAX_TOKENS:-8}"
TEMP="${GPT_OSS_TEMPERATURE:-0}"
PROMPT="${GPT_OSS_PROMPT:-1+1=}"
SLEEP_SECS="${GPT_OSS_SLEEP_SECS:-0.2}"

if ! curl -4 -s "${BASE_URL}/health" | grep -q '"status"'; then
    echo "Server not healthy at ${BASE_URL}" >&2
    exit 1
fi

latencies=()
for _ in $(seq 1 "$RUNS"); do
    ms=$(python3 - <<PY
import json, subprocess, time
payload = json.dumps({
    "model": "${MODEL}",
    "prompt": "${PROMPT}",
    "max_tokens": int("${MAX_TOKENS}"),
    "temperature": float("${TEMP}"),
})
start = time.perf_counter()
subprocess.check_output([
    "curl","-4","-s","${BASE_URL}/v1/completions",
    "-H","Content-Type: application/json",
    "-d", payload,
])
end = time.perf_counter()
print(int((end - start) * 1000))
PY
)
    latencies+=("$ms")
    echo "${ms}ms"
    sleep "$SLEEP_SECS"
done

printf '%s\n' "${latencies[@]}" | python3 -c '
import statistics
import sys

lat = [int(line) for line in sys.stdin if line.strip()]
lat.sort()
print(f"runs: {len(lat)}")
print(f"min: {min(lat)}ms")
print(f"p50: {statistics.median(lat)}ms")
if len(lat) > 1:
    p95 = lat[int(len(lat) * 0.95) - 1]
    print(f"p95: {p95}ms")
print(f"max: {max(lat)}ms")
'
