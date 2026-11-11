#!/usr/bin/env bash
# Deploys the room server (Orbit League's rooms, ol-room, the relay any game's rooms use,
# room-relay.coax, and the everglade rooms' world, room-everglade.coax with room-world.c) to Cloud Run as oa-room and
# routes /api/room/ and /api/playtest/rooms/ (the everglade instances, room-relay.coax rm_rooms)
# on openagents.com and staging.openagents.com to it, ahead of the API's /api/ rule.
# Then it runs, through the site's address, the load test (load.coax: four players,
# 60 Hz each, the same ball), the relay's check, its slow-socket check, and the
# everglade checks (two players share one world: sim-check.coax; the bridge: bridge-check.coax; a
# player whose network drops comes back where they stood, once: rejoin-check.coax) and load test (64 players in 16s,
# 20 s; spec section 11.3), each shard's health through /api/room/sK/health, and the fan-out check
# (fanout-check.coax: 17 guests through the API's directory fill one instance and spill into the next).
# Before any of that it runs the drift check on this machine (coax/web/everglade-drift.coax, #12778):
# the same scripted session offline and in a local room, on localhost and at 100 and 250 ms with
# jitter and loss, frame by frame, then the raid. It prints its table either way, and a failure
# stops the deploy before anything is built or routed.
#   coax/web/room/deploy.sh
# A deploy waits for no world event: when a shard's /health says one is running (a live instance
# with players and its raid under way), it stops before building, unless OA_ROOM_DEPLOY_FORCE=1
# (#12797). Shards are replaced one at a time, each healthy before the next, so at most one shard's
# rooms restart at once. Before its replacement each shard is drained (#12797, room-directory.coax
# rm_drain_ask): a signed POST /api/room/sK/drain, after which it seats nobody new and the directory
# places nobody on it; it waits up to OA_ROOM_DRAIN_WAIT seconds (300; 0 with OA_ROOM_DEPLOY_FORCE=1)
# for its world events to end, tells every page "restarting" (each sits again on another shard), and
# saves every seat; the deploy waits for its health to say "drain done". The new revision takes traffic
# only once its startup probe, GET /ready, answers: the room's start check held, the realm's world came
# from the API and a room's world built from it. OA_ROOM_DIR_GEN=1 (the default here) has each process
# name its generation in its directory reports, so the API refuses an old process's (migration 0133:
# the API must be deployed with it first).
#
# Everglade's instances run on shards (#12717): oa-room is shard 1 (with Orbit League's rooms and
# every other game's relay rooms), oa-room-2 to oa-room-$shards shards 2 to $shards, the same image with
# OA_ROOM_SHARD set. Each service runs one container at most, so a room is one room: its world lives
# in memory. The API's directory (coax/web/api/rooms.c) places each instance on a shard and the
# seat ticket names it; the load balancer sends /api/room/sK/ to shard K. None run while nobody
# plays (min 0); a WebSocket keeps one up, for an hour at most (the service's timeout), and the
# game joins again. Placement fills the lowest shard first (2 instances each, verse.room_shards.rooms_cap), so shard K runs only
# while more than 2(K-1) instances are open.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../../.." && pwd)"
cd "$root"
# A failure is confirmed by a second run: on a busy Mac its position probe fails by millimetres (#13137).
if "$here/../tool.sh" everglade-drift; then echo "room: the drift check holds"
elif echo "room: the drift check failed once (its table above): running it again to confirm" >&2 && "$here/../tool.sh" everglade-drift; then echo "room: the drift check holds on its second run"
else echo "room: the drift check fails (its table above): not deploying" >&2; exit 1
fi
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
project=openagentsgemini region=us-central1 service=oa-room map=coaxial-openagents-web-staging shards=6
# Production's rooms on the room VM (#13174, docs/deployment/room-vm.md): when the url map sends
# openagents.com's /api/room/ to the VM, a room deploy goes there (staging unit first, then the shards,
# each drained), and Cloud Run's services stay as they are, the rollback (vm-deploy.sh switch run).
if gcloud compute url-maps describe "$map" --global --project "$project" --format json 2>/dev/null | grep -q '/backendServices/oa-room-vm-s1"'; then
  echo "room: production's rooms run on the room VM: deploying there (coax/web/room/vm-deploy.sh)"
  "$here/vm-deploy.sh" deploy --staging && exec "$here/vm-deploy.sh" deploy
  exit 1
fi
# no world event under way on any shard (#12797); a shard that doesn't answer has none
for k in $(seq 1 "$shards"); do
  h="$(curl -s -m 20 "https://openagents.com/api/room/s$k/health" || true)"
  running="$(sed -nE 's/.*, ([0-9]+) world events running.*/\1/p' <<<"$h")"
  if [[ "${running:-0}" -gt 0 && "${OA_ROOM_DEPLOY_FORCE:-0}" != 1 ]]; then
    echo "room: shard $k has $running world events running: not deploying (OA_ROOM_DEPLOY_FORCE=1 deploys anyway)" >&2; exit 1
  fi
done
g() { gcloud "$@" --project "$project" --quiet; }
# Compare every revision currently receiving traffic, including a pinned rollback.
# This read-only gate runs before ticket keys, drains or a new image are requested.
coax="${COAX:-$root/coax/bin/coax}"
compat_dir="$(mktemp -d)"
if COAX="$coax" coax/web/tool.sh compat-room "$coax" "$project" "$region" "$service" "$shards" "$compat_dir"; then
  rm -rf "$compat_dir"
else rm -rf "$compat_dir"; echo "room: compatibility regression against production: not deploying" >&2; exit 1
fi
build_commit="$(git rev-parse HEAD)"

# The checks and the drains sign with the seat-ticket key, which stays in this process's environment and
# is never printed or put in an argument list.
OA_ROOM_TICKET_KEY="$(g secrets versions access latest --secret oa-room-ticket-key)"; export OA_ROOM_TICKET_KEY
drain_wait="${OA_ROOM_DRAIN_WAIT:-300}"; [[ "${OA_ROOM_DEPLOY_FORCE:-0}" == 1 ]] && drain_wait=0
# Drains shard K (#12797): asks it, signed, then waits for its health to say "drain done". A shard that
# doesn't answer the ask (none running behind it) has nothing to drain.
drain() {
  local k="$1" at mac code h
  at="$(python3 -I -c 'import time; print(int(time.time() * 1000))')"
  mac="$(python3 -I -c 'import hashlib, hmac, os, sys; print(hmac.new(os.environ["OA_ROOM_TICKET_KEY"].encode(), sys.argv[1].encode(), hashlib.sha256).hexdigest())' "drain $k $at $drain_wait")"
  code="$(curl -s -o /dev/null -w '%{http_code}' -m 30 -X POST -H "X-Room-At: $at" -H "X-Room-Wait: $drain_wait" -H "X-Room-Mac: $mac" \
    "https://openagents.com/api/room/s$k/drain" || true)"
  if [[ "$code" == 403 ]]; then echo "room: shard $k refused the drain (its key or clock): stopping" >&2; return 1; fi
  if [[ "$code" != 200 ]]; then echo "room: shard $k: nothing to drain (answered $code)"; return 0; fi
  local until=$((SECONDS + drain_wait + 60))
  while (( SECONDS < until )); do
    h="$(curl -s -m 20 "https://openagents.com/api/room/s$k/health" || true)"
    if [[ "$h" == *"drain done"* || "$h" != ok* ]]; then echo "room: shard $k drained"; return 0; fi
    sleep 2
  done
  echo "room: shard $k didn't finish its drain in $((drain_wait + 60)) s: stopping before replacing it" >&2; return 1
}
stage="$(mktemp -d)"; trap 'rm -rf "$stage"' EXIT
# the room's C sources and coax/web/compat (stage.sh: the VM's deploy, vm-deploy.sh, stages the same)
COAX="${COAX:-coax/bin/coax}" "$here/stage.sh" "$stage" >/dev/null
cp "$here/Dockerfile" "$stage/"
image="$region-docker.pkg.dev/$project/cloud-run-source-deploy/$service:$(git rev-parse --short HEAD)-$(date +%Y%m%d-%H%M%S)"
gcloud builds submit "$stage" --project "$project" --region "$region" --tag "$image" --quiet \
  --service-account "projects/$project/serviceAccounts/oa-mvp-automation@$project.iam.gserviceaccount.com" \
  --default-buckets-behavior REGIONAL_USER_OWNED_BUCKET > "$stage/build.log" 2>&1 || { grep -E "error|undefined reference" "$stage/build.log" | head -30; exit 1; }
# Shard K's service: oa-room for 1, oa-room-K after. 1 vCPU and 512 MiB each: one process's 30 Hz tick
# fits about three full rooms on it (the directory's rooms_cap), and more vCPU would not speed one thread.
# --no-cpu-throttling: the tick, the room-control poll and the HP posts run between requests too; with
# Cloud Run's default the process froze once the last socket closed, and a leaving player's HP never
# reached the API (#12715). Billed while an instance is up; min 0 still scales to none when idle.
svc() { if [[ "$1" == 1 ]]; then echo "$service"; else echo "$service-$1"; fi; }
# one shard at a time, each answering its health before the next is replaced (#12797)
for k in $(seq 1 "$shards"); do
  drain "$k" || exit 1
  out="$(g run deploy "$(svc "$k")" --image "$image" --region "$region" \
    --labels "coax-commit=$build_commit" \
    --allow-unauthenticated --port 8080 --cpu 1 --memory 512Mi \
    --startup-probe "httpGet.path=/ready,httpGet.port=8080,periodSeconds=2,timeoutSeconds=2,failureThreshold=60" \
    --min-instances 0 --max-instances 1 --concurrency 1000 --timeout 3600 --session-affinity --no-cpu-throttling \
    --set-secrets OA_ROOM_TICKET_KEY=oa-room-ticket-key:latest \
    --set-env-vars "OA_ROOM_TICKET_REQUIRED=${OA_ROOM_TICKET_REQUIRED:-1},OA_ROOM_SHARD=$k,OA_ROOM_DIR_GEN=${OA_ROOM_DIR_GEN:-1}" 2>&1)" ||
    { sed "s/^/shard $k: /" <<<"$out" >&2; echo "room: shard $k's deploy failed: stopping before the next shard" >&2; exit 1; }
  { grep -E "ERROR|revision" <<<"$out" || true; } | sed "s/^/shard $k: /"
  # a rollback pins traffic to a named revision (the runbook's update-traffic --to-revisions), and a
  # deploy after it would leave the new revision at 0 % with every check passing on the old one (#12936)
  g run services update-traffic "$(svc "$k")" --region "$region" --to-latest >/dev/null 2>&1 ||
    { echo "room: shard $k's traffic didn't move to its new revision: stopping before the next shard" >&2; exit 1; }
  echo "room: shard $k serves $(g run services describe "$(svc "$k")" --region "$region" --format 'value(status.latestReadyRevisionName)') (latest)"
  url="$(g run services describe "$(svc "$k")" --region "$region" --format 'value(status.url)')"
  [[ "$(curl -s -m 20 "$url/health")" == ok*"shard $k,"* ]] || { echo "room: $url/health is not shard $k's: stopping before the next shard" >&2; exit 1; }
  echo "room: $url/health ok"
done
for k in $(seq 1 "$shards"); do
  url="$(g run services describe "$(svc "$k")" --region "$region" --format 'value(status.url)')"
  # The load balancer: a serverless backend for each (a WebSocket lives as long as the service's timeout).
  s="$(svc "$k")"
  g compute network-endpoint-groups describe "$s-neg" --region "$region" >/dev/null 2>&1 ||
    g compute network-endpoint-groups create "$s-neg" --region "$region" --network-endpoint-type serverless --cloud-run-service "$s"
  if ! g compute backend-services describe "$s" --global >/dev/null 2>&1; then
    g compute backend-services create "$s" --global --load-balancing-scheme EXTERNAL_MANAGED --protocol HTTP
    g compute backend-services add-backend "$s" --global --network-endpoint-group "$s-neg" --network-endpoint-group-region "$region"
  fi
done
tmp="$stage/map.json"
g compute url-maps describe "$map" --global --format json > "$tmp"
python3 -I - "$tmp" "$project" "$service" "$shards" <<'PY'
import json, sys
path, project, service, shards = sys.argv[1:]
m = json.load(open(path))
base = f"https://www.googleapis.com/compute/v1/projects/{project}/global/backendServices/"
def has(r, prefix): return any(x.get("prefixMatch") == prefix for x in r.get("matchRules", []))
changed = []
for pm in m["pathMatchers"]:
    if pm["name"] not in ("production", "staging"): continue
    rules = pm.setdefault("routeRules", [])
    # /api/playtest/rooms/ is the API's now (its instance directory, coax/web/api/rooms.c): the old rule goes
    before = len(rules)
    rules[:] = [r for r in rules if not has(r, "/api/playtest/rooms/")]
    if len(rules) != before: changed.append("/api/playtest/rooms/ removed")
    # each shard's prefix before /api/room/ (shard 1's), and /api/room/ before /api/
    want = [(f"/api/room/s{k}/", service if k == 1 else f"{service}-{k}") for k in range(1, int(shards) + 1)] + [("/api/room/", service)]
    for prefix, svc in want:
        if any(has(r, prefix) for r in rules): continue
        anchor = next((r for r in rules if has(r, "/api/room/")), None) if prefix != "/api/room/" else None
        anchor = anchor or next((r for r in rules if has(r, "/api/")), None)
        used = {r["priority"] for r in rules}
        limit = anchor["priority"] if anchor else max(used | {0}) + 10
        free = [p for p in range(limit - 1, -1, -1) if p not in used]
        if not free:
            for r in rules: r["priority"] = r["priority"] * 10 + 10
            anchor = (next((r for r in rules if has(r, "/api/room/")), None) if prefix != "/api/room/" else None) or next((r for r in rules if has(r, "/api/")), None)
            p = (anchor["priority"] - 1) if anchor else max(r["priority"] for r in rules) + 10
        else:
            p = free[0]
        rules.append({"priority": p, "matchRules": [{"prefixMatch": prefix}], "service": base + svc})
        rules.sort(key=lambda r: r["priority"])
        changed.append(prefix + " added")
for k in ("kind", "creationTimestamp", "id", "selfLink"): m.pop(k, None)
json.dump(m, open(path, "w"))
print("url map: " + (", ".join(sorted(set(changed))) if changed else "as it was"))
PY
g compute url-maps validate --global --source "$tmp" --format 'value(result.loadSucceeded)' 2>&1 | grep -qi true ||
  { echo "room: the new url map doesn't validate" >&2; exit 1; }
g compute url-maps import "$map" --global --source "$tmp" 2>&1 | grep -E "ERROR" && exit 1
# The checks join everglade rooms, which need seat tickets (room-ticket.coax): they sign their own
# with the key (above).
# load.coax signs its own tickets for /api/room/relay, so every room it opens lands on shard 1:
# it loads one shard with 32 players (two rooms, the directory's rooms_cap), and fanout-check.coax
# is the load across shards through the directory.
# Every shard answers its own health through the site's address (the load balancer's /api/room/sK/).
shards_ok() { for k in $(seq 1 "$shards"); do [[ "$(curl -s -m 20 "https://openagents.com/api/room/s$k/health")" == ok*"shard $k,"* ]] || return 1; done; }
# Through the site's address (the rule takes a minute or two to reach every edge).
for try in 1 2 3 4 5 6 7 8 9 10; do
  "$here/../tool.sh" "$here/load.coax" wss://openagents.com 4 10 && "$here/../tool.sh" "$here/relay-check.coax" wss://openagents.com &&
    "$here/../tool.sh" "$here/relay-check.coax" wss://openagents.com slow && "$here/../tool.sh" "$here/sim-check.coax" wss://openagents.com && "$here/../tool.sh" "$here/bridge-check.coax" wss://openagents.com && "$here/../tool.sh" "$here/rejoin-check.coax" wss://openagents.com && "$here/../tool.sh" "$here/load.coax" everglade wss://openagents.com 32 20 &&
    [[ "$(curl -s -m 20 https://openagents.com/api/playtest/rooms/everglade)" == '{"realm":"everglade"'* ]] &&
    shards_ok &&
    "$here/../tool.sh" "$here/fanout-check.coax" https://openagents.com 17 5 &&
    curl -s -m 20 https://openagents.com/api/room/health | tee /dev/stderr | awk '{ for (i = 1; i <= NF; i++) if ($i == "p95") v = $(i + 1) } END { exit !(v != "" && v >= 0 && v < 50000) }' && { echo "room: deployed $image behind wss://openagents.com/api/room/"; exit 0; }
  sleep 30
done
echo "room: the load test through openagents.com failed" >&2; exit 1
