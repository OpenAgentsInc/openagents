#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: it runs the Coax checks below in order and reads the service's numbers)
# Everglade multiplayer's proof on production (GitHub #12711, spec sections 11 and 13), rerun after a
# release from the repository's root:
#   coax/web/room/everglade-proof.sh [all|smoke|latency|load] [--players 64] [--seconds 60] [--url wss://openagents.com]
#   smoke    the two-browser smoke, desktop then phone (coax/web/everglade-pair.coax: two headless
#            Chromes in one pair-<hex> room: bodies, names, move latency, smoothness, the jump, a cast,
#            a kill and WANTED)
#   latency  two bots, 30 s, 5 clock pings a second each (t MS): the round trip's p50, p95 and p99
#   load     PLAYERS bots (default 64, 16 a room, two rooms a shard as the directory packs them,
#            --shards 6) for SECONDS (default 60) over the real socket protocol (load.coax everglade);
#            halfway it reads GET /api/playtest/rooms/pair-load-STAMP (the rooms the run fills, with
#            their shards) and every shard's /api/room/sK/health (its everglade tick p50 and p95); after,
#            each oa-room shard's instances, CPU and memory from Cloud Monitoring and its tick lines;
#            then fanout-check.coax (17 guests through the API's directory fill one instance and spill)
# The smoke's smoothness is judged for #12731: on start and stop (the trials) under 1 correction a
# second (re-runs that moved you more than 5 cm) and no snap past 0.5 m, and the jump pose check.
# Bots are tagged as tests: rooms pair-load-*, seat-ticket ids p-load-*, names "Load K", and the
# browsers carry the verse-automation mark pair-check-*. The run is bounded: --seconds at most 300 and
# --players at most 256; it never changes the service (its max instances stay as deployed).
# The seat-ticket key comes from Secret Manager into this process's environment and is never printed.
# Output goes to coax/web/build/everglade-proof/STAMP/ (one file a step) and a summary on stdout.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; root="$(cd "$here/../../.." && pwd)"; cd "$root"
what="${1:-all}"; [[ $# -gt 0 ]] && shift
players=64 seconds=60 url=wss://openagents.com shards=6
while (($#)); do
  case "$1" in
    --players) players="$2"; shift 2 ;;
    --seconds) seconds="$2"; shift 2 ;;
    --url) url="$2"; shift 2 ;;
    --shards) shards="$2"; shift 2 ;;
    *) echo "everglade-proof: unknown option $1" >&2; exit 2 ;;
  esac
done
((players >= 2 && players <= 256 && seconds >= 10 && seconds <= 300)) || { echo "everglade-proof: --players 2..256, --seconds 10..300" >&2; exit 2; }
https="${url/wss:/https:}"; https="${https/ws:/http:}"
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-/Users/christopherdavid/work/.secrets/gcloud-sa-config}"
project=openagentsgemini
stamp="$(( $(date +%s) % 100000 ))"
out="coax/web/build/everglade-proof/$stamp"; mkdir -p "$out"
tool() { coax/web/tool.sh "$@"; }
key() {
  [[ -n "${OA_ROOM_TICKET_KEY:-}" ]] && return
  OA_ROOM_TICKET_KEY="$(gcloud secrets versions access latest --secret oa-room-ticket-key --project "$project" --quiet)"; export OA_ROOM_TICKET_KEY
}
status=0
echo "everglade-proof $what, $url, results in $out"
for svc in oa-room oa-room-2 oa-room-3 oa-room-4 oa-room-5 oa-room-6; do
  echo "$svc: $(gcloud run services describe "$svc" --region us-central1 --project "$project" --quiet --format 'value(status.latestReadyRevisionName,spec.template.metadata.annotations."autoscaling.knative.dev/maxScale")' 2>/dev/null | tr '\t' ' ' | sed 's/ / max instances /')"
done

if [[ "$what" == all || "$what" == smoke ]]; then
  for v in desktop phone; do
    flag=(); [[ "$v" == phone ]] && flag=(--phone)
    tool everglade-pair "$https/worlds/everglade" "${flag[@]}" --shots "$out/smoke-$v" > "$out/smoke-$v.txt" 2>&1 || status=1
    echo "smoke $v: $(tail -1 "$out/smoke-$v.txt")"; grep -E '^(ok|FAIL|skip)' "$out/smoke-$v.txt" | sed 's/^/  /'
    python3 -I - "$out/smoke-$v.txt" <<'PY' || status=1
import json, sys
line = next((l for l in open(sys.argv[1]) if l.startswith("{")), None)
if not line: print("  smoothness: no numbers"); sys.exit(1)
j = json.loads(line); bad = []
for side in "ab":
    ph = j[side]["phases"]
    for name in ("trials", "walk", "after_jump", "walk_b"):
        p = ph.get(name) or {}
        if p.get("you_snaps", 0) > 0: bad.append(f"{side} {name}: {p['you_snaps']} snaps")
        if name == "trials" and (p.get("corrections_per_s") or 0) >= 1: bad.append(f"{side} trials: {p['corrections_per_s']} corrections/s")
    w = ph.get("walk") if side == "b" else ph.get("walk_b")
    t = ph.get("trials") or {}
    print(f"  smoothness {side}: fps {w.get('fps')}, frame p50 {w.get('frame_ms_p50')} p99 {w.get('frame_ms_p99')} ms, over 33 ms {w.get('long_33')}, Coax p99 {w.get('coax_ms_p99')} ms;"
          f" start/stop corrections {t.get('corrections_per_s')}/s, re-run move p99 {t.get('rerun_move_m_p99')} m, snaps {t.get('you_snaps')};"
          f" remote {w.get('remote_speed_med')} m/s, stalls {w.get('remote_stalls')} lurches {w.get('remote_lurches')} of {w.get('remote_moving_frames')} frames")
print("  smoothness (#12731): " + ("holds" if not bad else "fails: " + "; ".join(bad)))
sys.exit(1 if bad else 0)
PY
  done
fi

if [[ "$what" == all || "$what" == latency ]]; then
  key
  tool coax/web/room/load.coax everglade "$url" 2 30 "$((stamp + 1))" 5 > "$out/latency.txt" 2>&1 || status=1
  echo "latency: $(tail -1 "$out/latency.txt") $(grep -o '"ping_rtt_ms":[0-9-]*,"ping_rtt_p95_ms":[0-9-]*,"ping_rtt_p99_ms":[0-9-]*,"pings":[0-9]*' "$out/latency.txt" | head -1)"
fi

if [[ "$what" == all || "$what" == load ]]; then
  key
  curl -s -m 20 "$https/api/room/health" > "$out/health-before.txt" || true
  t0="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  tool coax/web/room/load.coax everglade "$url" "$players" "$seconds" "$stamp" 1 "$shards" > "$out/load.txt" 2>&1 &
  pid=$!
  sleep "$((seconds / 2 + 5))"
  curl -s -m 20 "$https/api/playtest/rooms/pair-load-$stamp" > "$out/rooms-mid.json" || true
  for k in 1 2 3 4 5 6; do echo "shard $k: $(curl -s -m 20 "$https/api/room/s$k/health")"; done > "$out/health-mid.txt" || true
  wait "$pid" || status=1
  curl -s -m 20 "$https/api/room/health" > "$out/health-after.txt" || true
  sleep 90   # Cloud Monitoring's points come a minute or so late
  t1="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  token="$(gcloud auth print-access-token)"
  for m in container/instance_count container/cpu/utilizations container/memory/utilizations; do
    curl -s -m 30 -H "Authorization: Bearer $token" -G "https://monitoring.googleapis.com/v3/projects/$project/timeSeries" \
      --data-urlencode "filter=metric.type=\"run.googleapis.com/$m\" AND resource.labels.service_name=starts_with(\"oa-room\")" \
      --data-urlencode "interval.startTime=$t0" --data-urlencode "interval.endTime=$t1" > "$out/metric-${m//\//-}.json" || true
  done
  gcloud logging read "resource.type=cloud_run_revision AND resource.labels.service_name:oa-room AND textPayload:\"ticks in\" AND timestamp>=\"$t0\"" \
    --project "$project" --limit 30 --format 'value(timestamp,resource.labels.service_name,textPayload)' > "$out/ticks.txt" 2>/dev/null || true
  python3 -I - "$out" "$players" <<'PY'
import json, sys, glob, os
out, players = sys.argv[1], int(sys.argv[2])
t = open(f"{out}/load.txt").read().splitlines()
run = next((json.loads(l) for l in t if l.startswith("{")), None)
print(f"load {players} players: {t[-1] if t else 'no output'}")
if run:
    rooms = [r for r in run["rooms"] if r]
    k = lambda f: [r[f] for r in rooms]
    print(f"  rooms {len(rooms)}, sockets open {sum(k('open'))}/{players}, still open {sum(k('still_open'))}, blind {sum(k('blind'))}, D dropped {sum(k('d_dropped'))}, refused {sum(k('refused'))}, sends failed {sum(k('sends_failed'))}")
    print(f"  slowest D rate {min(k('slowest_d_hz'))} Hz; input-to-ACK p50 {max(k('ack_p50_ms'))} p95 {max(k('ack_p95_ms'))} p99 {max(k('ack_p99_ms'))} ms (worst room); ping round trip p50 {max(k('ping_rtt_ms'))} p95 {max(k('ping_rtt_p95_ms'))} p99 {max(k('ping_rtt_p99_ms'))} ms over {sum(k('pings'))} pings")
try:
    r = json.load(open(f"{out}/rooms-mid.json"))
    print(f"  rooms list mid-run: {len(r['rooms'])} instances, " + ", ".join(f"{x['code']} {x['players']} (shard {x.get('shard', '?')})" for x in r["rooms"]) + f"; join {r['join']}")
except Exception as e:
    print(f"  rooms list mid-run: unreadable ({e})")
for h in ("before", "mid", "after"):
    for l in open(f'{out}/health-{h}.txt').read().strip().splitlines(): print(f"  health {h}: {l}")
def series(name):
    try: return json.load(open(f"{out}/metric-{name}.json")).get("timeSeries", [])
    except Exception: return []
inst = {}
for s in series("container-instance_count"):
    if s["metric"]["labels"].get("state") != "active": continue
    svc = s["resource"]["labels"]["service_name"]
    inst[svc] = max([inst.get(svc, 0)] + [int(p["value"]["int64Value"]) for p in s["points"]])
print("  Cloud Run instances active (max): " + (", ".join(f"{k} {v}" for k, v in sorted(inst.items())) or "n/a") + f"; {sum(inst.values())} in all")
for name, label in (("container-cpu-utilizations", "CPU"), ("container-memory-utilizations", "memory")):
    per = {}
    for s in series(name):
        svc = s["resource"]["labels"]["service_name"]
        for p in s["points"]:
            d = p["value"].get("distributionValue", {})
            if d.get("count"): per[svc] = max(per.get(svc, 0), float(d.get("mean", 0)))
    print(f"  {label} (per-minute mean, max): " + (", ".join(f"{k} {v*100:.0f}%" for k, v in sorted(per.items())) or "n/a"))
ticks = open(f"{out}/ticks.txt").read().strip().splitlines()
print("  host tick lines (60 Hz host; 3600 a minute keeps up): " + (" | ".join(l.split("\t")[1] + " " + l.split("\t")[-1].split(", ")[0].replace("room: ", "") + " " + l.split("\t")[-1].split(", ")[1] for l in ticks[:12] if "players" in l and not ", 0 players" in l) if ticks else "none in the window"))
PY
  tool coax/web/room/fanout-check.coax "$https" 17 10 > "$out/fanout.txt" 2>&1 || status=1
  echo "  fanout-check (17 guests through the directory): $(tail -1 "$out/fanout.txt")"
fi
echo "everglade-proof: $( ((status == 0)) && echo holds || echo fails)"
exit "$status"
