#!/usr/bin/env bash
# Deploys the room server (Orbit League's rooms, ol-room, the relay any game's rooms use,
# room-relay.coax, and the everglade rooms' world, room-everglade.coax with room-world.c) to Cloud Run as oa-room and
# routes /api/room/ and /api/playtest/rooms/ (the everglade instances, room-relay.coax rm_rooms)
# on openagents.com and staging.openagents.com to it, ahead of the API's /api/ rule.
# Then it runs, through the site's address, the load test (load.coax: four players,
# 60 Hz each, the same ball), the relay's check, its slow-socket check, and the
# everglade check (two players share one world: sim-check.coax) and load test (64 players in 16s,
# 20 s; spec section 11.3), each shard's health through /api/room/sK/health, and the fan-out check
# (fanout-check.coax: 17 guests through the API's directory fill one instance and spill into the next).
#   coax/web/room/deploy.sh
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
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
project=openagentsgemini region=us-central1 service=oa-room map=coaxial-openagents-web-staging shards=6
g() { gcloud "$@" --project "$project" --quiet; }
stage="$(mktemp -d)"; trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/src"
coax="${COAX:-coax/bin/coax}"
"$coax" c coax/web/room/room.coax --library --header "$stage/src/room.h" > "$stage/src/room.c"
# the everglade rooms' world (room-world.c): the city's units, as build.sh links them
for u in apps/openagents-web/everglade.coax apps/openagents-web/everglade-step.coax verse/grid.coax verse/everglade/river.coax verse/everglade/guard-guide-policy.coax; do
  n="$(basename "$u" .coax)"
  "$coax" c "$u" --library --header "$stage/src/$n.h" > "$stage/src/$n.c"
done
cp "$here/room-host.c" "$here/room-world.c" "$here/room-world.h" verse/voice-host/ws.c verse/voice-host/ws.h "$stage/src/"
python3 "$here/room-catalog.py" apps/openagents-web/realms/everglade/assets/everglade.pack "$stage/src/room-catalog.c"
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
for k in $(seq 1 "$shards"); do
  g run deploy "$(svc "$k")" --image "$image" --region "$region" \
    --allow-unauthenticated --port 8080 --cpu 1 --memory 512Mi \
    --min-instances 0 --max-instances 1 --concurrency 1000 --timeout 3600 --session-affinity --no-cpu-throttling \
    --set-secrets OA_ROOM_TICKET_KEY=oa-room-ticket-key:latest \
    --set-env-vars "OA_ROOM_TICKET_REQUIRED=${OA_ROOM_TICKET_REQUIRED:-1},OA_ROOM_SHARD=$k" 2>&1 | grep -E "ERROR|revision" | sed "s/^/shard $k: /" &
done
wait
for k in $(seq 1 "$shards"); do
  url="$(g run services describe "$(svc "$k")" --region "$region" --format 'value(status.url)')"
  [[ "$(curl -s -m 20 "$url/health")" == ok*"shard $k,"* ]] || { echo "room: $url/health is not shard $k's" >&2; exit 1; }
  echo "room: $url/health ok"
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
# with the key, which stays in this process's environment and is never printed.
OA_ROOM_TICKET_KEY="$(g secrets versions access latest --secret oa-room-ticket-key)"; export OA_ROOM_TICKET_KEY
# load.coax signs its own tickets for /api/room/relay, so every room it opens lands on shard 1:
# it loads one shard with 32 players (two rooms, the directory's rooms_cap), and fanout-check.coax
# is the load across shards through the directory.
# Every shard answers its own health through the site's address (the load balancer's /api/room/sK/).
shards_ok() { for k in $(seq 1 "$shards"); do [[ "$(curl -s -m 20 "https://openagents.com/api/room/s$k/health")" == ok*"shard $k,"* ]] || return 1; done; }
# Through the site's address (the rule takes a minute or two to reach every edge).
for try in 1 2 3 4 5 6 7 8 9 10; do
  "$here/../tool.sh" "$here/load.coax" wss://openagents.com 4 10 && "$here/../tool.sh" "$here/relay-check.coax" wss://openagents.com &&
    "$here/../tool.sh" "$here/relay-check.coax" wss://openagents.com slow && "$here/../tool.sh" "$here/sim-check.coax" wss://openagents.com && "$here/../tool.sh" "$here/bridge-check.coax" wss://openagents.com && "$here/../tool.sh" "$here/load.coax" everglade wss://openagents.com 32 20 &&
    [[ "$(curl -s -m 20 https://openagents.com/api/playtest/rooms/everglade)" == '{"realm":"everglade"'* ]] &&
    shards_ok &&
    "$here/../tool.sh" "$here/fanout-check.coax" https://openagents.com 17 5 &&
    curl -s -m 20 https://openagents.com/api/room/health | tee /dev/stderr | awk '{ for (i = 1; i <= NF; i++) if ($i == "p95") v = $(i + 1) } END { exit !(v != "" && v >= 0 && v < 50000) }' && { echo "room: deployed $image behind wss://openagents.com/api/room/"; exit 0; }
  sleep 30
done
echo "room: the load test through openagents.com failed" >&2; exit 1
