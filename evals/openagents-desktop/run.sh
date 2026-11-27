#!/bin/sh
# Firm reason: 5 (thin shell runner dispatching the Coder Desktop eval tiers and the run comparison)
set -eu

# Coder Desktop evaluation runner (Tier 0, Tier 1, Compaction, and opt-in Tier 2, 3 and 4).
# Usage: evals/openagents-desktop/run.sh [--tier 0|1|2|3|4|compaction|all] [--filter TEXT] [--update-goldens] [--no-build]
#        evals/openagents-desktop/run.sh --compare BASE NEW     (two summary.json files or run folders)
#        evals/openagents-desktop/run.sh --trend [TIER]
# Tier 2 and Tier 4 call real models and cost money: they run only with --tier 2 or --tier 4,
# never with --tier all. Tier 3 builds and drives the real Coder.app on this Mac: only with --tier 3.

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
TIER="all"
FILTER=""
WORKER="oa-worker"
T3_ARGS=""
HERE="$ROOT/evals/openagents-desktop"

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
    --filter)
      if [ "$#" -lt 2 ]; then
        echo "error: --filter requires an argument" >&2
        exit 2
      fi
      FILTER="$2"
      shift 2
      ;;
    --worker)
      if [ "$#" -lt 2 ]; then
        echo "error: --worker requires an argument (oa-worker, mcworker or inky)" >&2
        exit 2
      fi
      WORKER="$2"
      shift 2
      ;;
    --worker=*)
      WORKER="${1#--worker=}"
      shift
      ;;
    --update-goldens)
      T3_ARGS="$T3_ARGS --update-goldens"
      shift
      ;;
    --no-build)
      T3_ARGS="$T3_ARGS --no-build"
      shift
      ;;
    --compare)
      if [ "$#" -lt 3 ]; then
        echo "error: --compare requires BASE and NEW (summary.json files or run folders)" >&2
        exit 2
      fi
      exec python3 "$HERE/compare.py" diff "$2" "$3"
      ;;
    --trend)
      if [ "$#" -ge 2 ]; then exec python3 "$HERE/compare.py" trend --tier "$2"; fi
      exec python3 "$HERE/compare.py" trend
      ;;
    -h|--help)
      echo "Usage: $0 [--tier 0|1|2|3|4|compaction|all] [--filter TEXT] [--update-goldens] [--no-build]"
      echo "       $0 --compare BASE NEW | --trend [TIER]"
      echo "  --tier 0           Run Tier 0 unit & wire-contract checks (page.coax, mcworker-wire.coax, tools_smoke.sh)"
      echo "  --tier 1           Run Tier 1 deterministic transcript replay + compaction suites"
      echo "  --tier compaction  Run deterministic & long-horizon compaction eval suite (C1-C12)"
      echo "  --tier 2           Run the Tier 2 live-agent suite on shipped Inkling 2 Mini (Psionic Metal 32K, \$0 local; opt-in)"
      echo "  --tier 3           Run the Tier 3 native suite: build OpenAgents.app, drive it on the mock worker, compare captures (macOS; opt-in)"
      echo "  --tier 4           Run Tier 4: golden trajectories and the 25-task benchmark slice (tier4/slice.json) on local Inkling 2 Mini (opt-in)"
      echo "  --tier all         Run Tier 0, Tier 1, and Compaction (default; never Tier 2, 3 or 4)"
      echo "  --filter TEXT      Tier 2, 3 and 4: run tasks or flows whose name contains TEXT (comma-separated)"
      echo "  --compare BASE NEW Compare two Tier 2 or Tier 4 runs (summary.json files or run folders)"
      echo "  --trend [TIER]     One row per kept run under results/: pass rate, cost a task, p95 wall, regressions"
      echo "  --update-goldens   Tier 3: write the captures as the new goldens (tier3/goldens/)"
      echo "  --no-build         Tier 3: reuse the last scratch build (results/tier3-build/)"
      exit 0
      ;;
    *)
      echo "error: unknown argument '$1'" >&2
      exit 2
      ;;
  esac
done

case "$TIER" in
  0|1|2|3|4|compaction|all) ;;
  *)
    echo "error: unsupported tier '$TIER' (supported: 0, 1, 2, 3, 4, compaction, all)" >&2
    exit 2
    ;;
esac

TS="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
TS_FILE="$(date -u +%Y%m%dT%H%M%SZ)"
SHA="$(git -C "$ROOT" rev-parse --short HEAD 2>/dev/null || echo nogit)"
RUN_ID="${TS}-${SHA}"
RESULTS_DIR="$ROOT/evals/openagents-desktop/results"
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
    "model": "live" if tier in (2, 4) else ("mock" if tier == 3 else "deterministic"),
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

# Also emit per-task records when running the Tier 2 live suite or Tier 4 (the same runner)
if (tier == 2 and case_name == "tier2_live_suite") or (tier == 4 and case_name == "tier4_suite"):
    for ln in text.splitlines():
        tm = re.match(r"^(ok  |FAIL|KNOWN) task (\S+): (.*?) \((\d+) ms, \$([0-9.]+), (\d+) attempts, (\d+) turns, (\d+) commands(?:, .*?)?\)(?: -- (.*))?$", ln)
        if tm:
            ok = tm.group(1).strip() == "ok"
            trec = {
                "run_id": run_id,
                "tier": tier,
                "case": f"task_{tm.group(2)}",
                "model": "inkling-2-mini",
                "passed": ok,
                "wall_ms": int(tm.group(4)),
                "cost_usd_micros": int(round(float(tm.group(5)) * 1e6)),
                "metrics": {"description": tm.group(3), "attempts": int(tm.group(6)), "turns": int(tm.group(7)),
                            "commands": int(tm.group(8)), "known_failure": tm.group(1) == "KNOWN"},
                "failure_reason": None if ok else (tm.group(9) or ln),
            }
            with open(out_path, "a", encoding="utf-8") as out:
                out.write(json.dumps(trec, separators=(",", ":")) + "\n")

# Also emit per-flow records when running the Tier 3 native suite
if tier == 3 and case_name == "tier3_native_suite":
    for ln in text.splitlines():
        fm = re.match(r"^(ok  |FAIL) flow (\S+): (.*?) \((\d+) ms, (\d+) captures\)(?: -- (.*))?$", ln)
        if fm:
            ok = fm.group(1).strip() == "ok"
            frec = {
                "run_id": run_id,
                "tier": 3,
                "case": f"flow_{fm.group(2)}",
                "model": "mock",
                "passed": ok,
                "wall_ms": int(fm.group(4)),
                "cost_usd_micros": 0,
                "metrics": {"description": fm.group(3), "captures": int(fm.group(5))},
                "failure_reason": None if ok else (fm.group(6) or ln),
            }
            with open(out_path, "a", encoding="utf-8") as out:
                out.write(json.dumps(frec, separators=(",", ":")) + "\n")

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
  run_case 0 "page_coax_tests" coax/bin/coax test apps/openagents-desktop/page.coax --library
  run_case 0 "oa_worker_tests" coax/bin/coax test openagents/runtime/oa-worker.coax --library
  run_case 0 "openagents_core_wire_tests" coax/bin/coax test openagents/core/wire.coax --library
  run_case 0 "exa_policy_tests" coax/bin/coax test coax/web/api/exa-policy.coax --library
  run_case 0 "router_policy_tests" coax/bin/coax test coax/lib/agent/router/policy.coax --library
  run_case 0 "mcworker_wire_tests" coax/bin/coax test coax/lib/agent/engine/mcworker-wire.coax --library
  run_case 0 "microcoder_tools_smoke" bash coax/lib/microcoder/tools_smoke.sh
fi

if [ "$TIER" = "1" ] || [ "$TIER" = "all" ]; then
  run_case 1 "tier1_fixtures_sync" python3 evals/openagents-desktop/tier1/gen_fixtures.py
  run_case 1 "tier1_replay_suite" coax/bin/coax test evals/openagents-desktop/tier1/replay_test.coax --library
  run_case 1 "tier1_host_scenarios" python3 evals/openagents-desktop/tier1/run_scenarios.py
  run_case 1 "tier1_followups" python3 evals/openagents-desktop/followups/run_followups.py
fi

if [ "$TIER" = "1" ] || [ "$TIER" = "compaction" ] || [ "$TIER" = "all" ]; then
  run_case 1 "compaction_suite" sh evals/openagents-desktop/compaction/run.sh
fi

# Tier 2 is opt-in: real models, real cost. Never part of --tier all.
if [ "$TIER" = "2" ]; then
  if [ -n "$FILTER" ]; then
    run_case 2 "tier2_live_suite" env BROWSER=/usr/bin/true python3 evals/openagents-desktop/tier2/run_tasks.py --worker "$WORKER" --filter "$FILTER"
  else
    run_case 2 "tier2_live_suite" env BROWSER=/usr/bin/true python3 evals/openagents-desktop/tier2/run_tasks.py --worker "$WORKER"
  fi
fi

# Tier 4 is opt-in: real models, about $1.20 a run (tier4/slice.json: 4 golden trajectories and
# the 25-task benchmark slice, each capped by its own chat budget). Never part of --tier all.
# The run is compared with tier4/baseline/summary.json (report in the run folder's compare.md).
if [ "$TIER" = "4" ]; then
  T4_OUT="$RESULTS_DIR/tier4-${TS_FILE}-${SHA}"
  if [ -n "$FILTER" ]; then
    run_case 4 "tier4_suite" env BROWSER=/usr/bin/true python3 evals/openagents-desktop/tier2/run_tasks.py --worker "$WORKER" --manifest evals/openagents-desktop/tier4/slice.json --out "$T4_OUT" --filter "$FILTER"
  else
    run_case 4 "tier4_suite" env BROWSER=/usr/bin/true python3 evals/openagents-desktop/tier2/run_tasks.py --worker "$WORKER" --manifest evals/openagents-desktop/tier4/slice.json --out "$T4_OUT"
  fi
  if [ -f "$T4_OUT/summary.json" ] && [ -f "$HERE/tier4/baseline/summary.json" ]; then
    python3 "$HERE/compare.py" diff "$HERE/tier4/baseline/summary.json" "$T4_OUT/summary.json" --out "$T4_OUT/compare.md" > /dev/null || true
    echo "Baseline comparison: $T4_OUT/compare.md"
  fi
fi

# Tier 3 is opt-in: it builds Coder.app (minutes) and opens its window, parked behind every other
# window. It uses the mock worker, so it costs nothing. Never part of --tier all.
if [ "$TIER" = "3" ]; then
  if [ -n "$FILTER" ]; then
    run_case 3 "tier3_native_suite" env BROWSER=/usr/bin/true python3 evals/openagents-desktop/tier3/run_ui.py --filter "$FILTER" $T3_ARGS
  else
    run_case 3 "tier3_native_suite" env BROWSER=/usr/bin/true python3 evals/openagents-desktop/tier3/run_ui.py $T3_ARGS
  fi
fi

echo "Results written to: $OUT_JSONL"
echo "Summary: $PASS_COUNT passed, $FAIL_COUNT failed"

if [ "$FAIL_COUNT" -ne 0 ]; then
  exit 1
fi
