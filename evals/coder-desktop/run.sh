#!/bin/sh
# Firm reason: 5 (thin shell runner dispatching Tier 0, Tier 1, and compaction evals)
set -eu

# Coder Desktop evaluation runner (Tier 0, Tier 1, and Compaction).
# Usage: evals/coder-desktop/run.sh [--tier 0|1|compaction|all]

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
TIER="all"

while [ "$#" -gt 0 ]; do
  case "$1" in
    --tier)
      if [ "$#" -lt 2 ]; then
        echo "error: --tier requires an argument (0, 1, or all)" >&2
        exit 2
      fi
      TIER="$2"
      shift 2
      ;;
    --tier=*)
      TIER="${1#--tier=}"
      shift
      ;;
    -h|--help)
      echo "Usage: $0 [--tier 0|1|compaction|all]"
      echo "  --tier 0           Run Tier 0 unit & wire-contract checks (page.coax, mcworker-wire.coax, tools_smoke.sh)"
      echo "  --tier 1           Run Tier 1 deterministic transcript replay + compaction suites"
      echo "  --tier compaction  Run deterministic & long-horizon compaction eval suite (C1-C12)"
      echo "  --tier all         Run Tier 0, Tier 1, and Compaction (default)"
      exit 0
      ;;
    *)
      echo "error: unknown argument '$1'" >&2
      exit 2
      ;;
  esac
done

case "$TIER" in
  0|1|compaction|all) ;;
  *)
    echo "error: unsupported tier '$TIER' (supported: 0, 1, compaction, all)" >&2
    exit 2
    ;;
esac

TS="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
TS_FILE="$(date -u +%Y%m%dT%H%M%SZ)"
SHA="$(git -C "$ROOT" rev-parse --short HEAD 2>/dev/null || echo nogit)"
RUN_ID="${TS}-${SHA}"
RESULTS_DIR="$ROOT/evals/coder-desktop/results"
mkdir -p "$RESULTS_DIR"
OUT_JSONL="$RESULTS_DIR/${TS_FILE}-${SHA}.jsonl"
: > "$OUT_JSONL"

PASS_COUNT=0
FAIL_COUNT=0

run_case() {
  case_tier="$1"
  case_name="$2"
  shift 2
  log_file="$(mktemp)"
  start_ms="$(python3 -c 'import time; print(int(time.time()*1000))')"
  if (cd "$ROOT" && "$@") >"$log_file" 2>&1; then
    rc=0
  else
    rc=$?
  fi
  end_ms="$(python3 -c 'import time; print(int(time.time()*1000))')"
  wall_ms=$((end_ms - start_ms))

  python3 - "$OUT_JSONL" "$RUN_ID" "$case_tier" "$case_name" "$rc" "$wall_ms" "$log_file" <<'PY'
import json, re, sys

out_path, run_id, tier_s, case_name, rc_s, wall_ms_s, log_path = sys.argv[1:8]
rc = int(rc_s)
wall_ms = int(wall_ms_s)
tier = int(tier_s)
with open(log_path, "r", encoding="utf-8", errors="replace") as f:
    text = f.read()

metrics = {"exit_code": rc}
m = re.search(r"(\d+) checks, (\d+) passed, (\d+) failed", text)
if m:
    metrics["checks_total"] = int(m.group(1))
    metrics["checks_passed"] = int(m.group(2))
    metrics["checks_failed"] = int(m.group(3))

failure_reason = None
if rc != 0:
    lines = [ln for ln in text.splitlines() if ln.strip()]
    failure_reason = "\n".join(lines[-8:]) if lines else f"exit code {rc}"

rec = {
    "run_id": run_id,
    "tier": tier,
    "case": case_name,
    "model": "deterministic",
    "passed": rc == 0,
    "wall_ms": wall_ms,
    "cost_usd_micros": 0,
    "metrics": metrics,
    "failure_reason": failure_reason,
}
with open(out_path, "a", encoding="utf-8") as out:
    out.write(json.dumps(rec, separators=(",", ":")) + "\n")

# Also emit per-fixture records when running Tier 1 replay_test.coax
if tier == 1 and case_name == "tier1_replay_suite":
    for ln in text.splitlines():
        fm = re.match(r"^(ok|FAIL)\s+#test tier1 fixture (\d+): (.*?) \(.*?(\d+) ms\)$", ln)
        if fm:
            ok = fm.group(1) == "ok"
            fnum = fm.group(2)
            fdesc = fm.group(3)
            fms = int(fm.group(4))
            frec = {
                "run_id": run_id,
                "tier": 1,
                "case": f"fixture_{fnum}",
                "model": "deterministic",
                "passed": ok,
                "wall_ms": fms,
                "cost_usd_micros": 0,
                "metrics": {"description": fdesc},
                "failure_reason": None if ok else ln,
            }
            with open(out_path, "a", encoding="utf-8") as out:
                out.write(json.dumps(frec, separators=(",", ":")) + "\n")

# Also emit per-scenario records when running Tier 1 host_driver + mock_worker scenarios
if tier == 1 and case_name == "tier1_host_scenarios":
    for ln in text.splitlines():
        sm = re.match(r"^(ok|FAIL)\s+scenario\s+(\S+):\s+(.*?)\s+\((\d+)\s+ms(?:,\s+max_v=(\d+),\s+max_frame=([0-9.]+)ms)?\)$", ln)
        if sm:
            ok = sm.group(1) == "ok"
            sname = sm.group(2)
            sdesc = sm.group(3)
            sms = int(sm.group(4))
            smetrics = {"description": sdesc}
            if sm.group(5) is not None:
                smetrics["max_vertices"] = int(sm.group(5))
            if sm.group(6) is not None:
                smetrics["max_frame_ms"] = float(sm.group(6))
            srec = {
                "run_id": run_id,
                "tier": 1,
                "case": f"scenario_{sname}",
                "model": "deterministic",
                "passed": ok,
                "wall_ms": sms,
                "cost_usd_micros": 0,
                "metrics": smetrics,
                "failure_reason": None if ok else ln,
            }
            with open(out_path, "a", encoding="utf-8") as out:
                out.write(json.dumps(srec, separators=(",", ":")) + "\n")

# Also emit per-scenario records when running compaction_suite
if case_name == "compaction_suite":
    for ln in text.splitlines():
        cm = re.match(r"^(ok|FAIL)\s+#test compaction (C\d+): (.*?) \(.*?(\d+) ms\)$", ln)
        if cm:
            ok = cm.group(1) == "ok"
            cid = cm.group(2)
            cdesc = cm.group(3)
            cms = int(cm.group(4))
            crec = {
                "run_id": run_id,
                "tier": 1,
                "case": f"compaction_{cid}",
                "model": "deterministic",
                "passed": ok,
                "wall_ms": cms,
                "cost_usd_micros": 0,
                "metrics": {"description": cdesc},
                "failure_reason": None if ok else ln,
            }
            with open(out_path, "a", encoding="utf-8") as out:
                out.write(json.dumps(crec, separators=(",", ":")) + "\n")
PY

  if [ "$rc" -eq 0 ]; then
    PASS_COUNT=$((PASS_COUNT + 1))
    summary="$(grep -E '^[0-9]+ checks,|^smoke: ok' "$log_file" | tail -n 1 || true)"
    if [ -n "$summary" ]; then
      echo "[PASS] tier $case_tier | $case_name (${wall_ms} ms) — $summary"
    else
      echo "[PASS] tier $case_tier | $case_name (${wall_ms} ms)"
    fi
  else
    FAIL_COUNT=$((FAIL_COUNT + 1))
    echo "[FAIL] tier $case_tier | $case_name (${wall_ms} ms, exit $rc)" >&2
    tail -n 20 "$log_file" >&2
  fi
  rm -f "$log_file"
}

if [ "$TIER" = "0" ] || [ "$TIER" = "all" ]; then
  run_case 0 "page_coax_tests" coax/bin/coax test apps/coder-desktop/page.coax --library
  run_case 0 "mcworker_wire_tests" coax/bin/coax test coax/lib/agent/engine/mcworker-wire.coax --library
  run_case 0 "microcoder_tools_smoke" sh coax/lib/microcoder/tools_smoke.sh
fi

if [ "$TIER" = "1" ] || [ "$TIER" = "all" ]; then
  run_case 1 "tier1_fixtures_sync" python3 evals/coder-desktop/tier1/gen_fixtures.py
  run_case 1 "tier1_replay_suite" coax/bin/coax test evals/coder-desktop/tier1/replay_test.coax --library
  run_case 1 "tier1_host_scenarios" python3 evals/coder-desktop/tier1/run_scenarios.py
fi

if [ "$TIER" = "1" ] || [ "$TIER" = "compaction" ] || [ "$TIER" = "all" ]; then
  run_case 1 "compaction_suite" sh evals/coder-desktop/compaction/run.sh
fi

echo "Results written to: $OUT_JSONL"
echo "Summary: $PASS_COUNT passed, $FAIL_COUNT failed"

if [ "$FAIL_COUNT" -ne 0 ]; then
  exit 1
fi
