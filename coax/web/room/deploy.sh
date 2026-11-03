#!/usr/bin/env bash
# Deploys the room server (Orbit League's rooms, ol-room, the relay any game's rooms use,
# room-relay.coax, and the everglade rooms' world, room-everglade.coax with room-world.c) to Cloud Run as oa-room and
# routes /api/room/ and /api/playtest/rooms/ (the everglade instances, room-relay.coax rm_rooms)
# on openagents.com and staging.openagents.com to it, ahead of the API's /api/ rule.
# Then it runs, through the site's address, the load test (load.coax: four players,
# 60 Hz each, the same ball), the relay's check, its slow-socket check, and the
# everglade check (two players share one world: sim-check.coax) and load test (64 players in 16s,
# 20 s; spec section 11.3).
#   coax/web/room/deploy.sh
#
# One instance at most, so a room is one room: its rooms live in memory. None
# while nobody plays (min 0); a WebSocket keeps one up, for an hour at most
# (the service's timeout), and the game joins again.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../../.." && pwd)"
cd "$root"
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
project=openagentsgemini region=us-central1 service=oa-room map=coaxial-openagents-web-staging
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
cp "$here/Dockerfile" "$stage/"
image="$region-docker.pkg.dev/$project/cloud-run-source-deploy/$service:$(git rev-parse --short HEAD)-$(date +%Y%m%d-%H%M%S)"
gcloud builds submit "$stage" --project "$project" --region "$region" --tag "$image" --quiet \
  --service-account "projects/$project/serviceAccounts/oa-mvp-automation@$project.iam.gserviceaccount.com" \
  --default-buckets-behavior REGIONAL_USER_OWNED_BUCKET > "$stage/build.log" 2>&1 || { grep -E "error|undefined reference" "$stage/build.log" | head -30; exit 1; }
g run deploy "$service" --image "$image" --region "$region" \
  --allow-unauthenticated --port 8080 --cpu 1 --memory 512Mi \
  --min-instances 0 --max-instances 1 --concurrency 1000 --timeout 3600 --session-affinity 2>&1 | grep -E "ERROR|revision" || true
url="$(g run services describe "$service" --region "$region" --format 'value(status.url)')"
[[ "$(curl -s -m 20 "$url/health")" == ok* ]] || { echo "room: $url/health is not ok" >&2; exit 1; }
echo "room: $url/health ok"
# The load balancer: a serverless backend (a WebSocket lives as long as the service's timeout), and the rule.
g compute network-endpoint-groups describe "$service-neg" --region "$region" >/dev/null 2>&1 ||
  g compute network-endpoint-groups create "$service-neg" --region "$region" --network-endpoint-type serverless --cloud-run-service "$service"
if ! g compute backend-services describe "$service" --global >/dev/null 2>&1; then
  g compute backend-services create "$service" --global --load-balancing-scheme EXTERNAL_MANAGED --protocol HTTP
  g compute backend-services add-backend "$service" --global --network-endpoint-group "$service-neg" --network-endpoint-group-region "$region"
fi
tmp="$stage/map.json"
g compute url-maps describe "$map" --global --format json > "$tmp"
python3 -I - "$tmp" "$project" "$service" <<'PY'
import json, sys
path, project, service = sys.argv[1:]
m = json.load(open(path))
backend = f"https://www.googleapis.com/compute/v1/projects/{project}/global/backendServices/{service}"
changed = False
for pm in m["pathMatchers"]:
    if pm["name"] not in ("production", "staging"): continue
    rules = pm.setdefault("routeRules", [])
    for prefix in ("/api/room/", "/api/playtest/rooms/"):
        if any(any(x.get("prefixMatch") == prefix for x in r.get("matchRules", [])) for r in rules): continue
        api = next((r for r in rules if any(x.get("prefixMatch") == "/api/" for x in r.get("matchRules", []))), None)
        used = {r["priority"] for r in rules}
        limit = api["priority"] if api else max(used | {0}) + 10
        free = [p for p in range(limit - 1, -1, -1) if p not in used]
        if not free:
            for r in rules: r["priority"] = r["priority"] * 10 + 10
            api = next((r for r in rules if any(x.get("prefixMatch") == "/api/" for x in r.get("matchRules", []))), None)
            p = (api["priority"] - 1) if api else max(r["priority"] for r in rules) + 10
        else:
            p = free[0]
        rules.append({"priority": p, "matchRules": [{"prefixMatch": prefix}], "service": backend})
        rules.sort(key=lambda r: r["priority"])
        changed = True
for k in ("kind", "creationTimestamp", "id", "selfLink"): m.pop(k, None)
json.dump(m, open(path, "w"))
print("url map: /api/room/ and /api/playtest/rooms/ " + ("added" if changed else "already there"))
PY
g compute url-maps validate --global --source "$tmp" --format 'value(result.loadSucceeded)' 2>&1 | grep -qi true ||
  { echo "room: the new url map doesn't validate" >&2; exit 1; }
g compute url-maps import "$map" --global --source "$tmp" 2>&1 | grep -E "ERROR" && exit 1
# Through the site's address (the rule takes a minute or two to reach every edge).
for try in 1 2 3 4 5 6 7 8 9 10; do
  "$here/../tool.sh" "$here/load.coax" wss://openagents.com 4 10 && "$here/../tool.sh" "$here/relay-check.coax" wss://openagents.com &&
    "$here/../tool.sh" "$here/relay-check.coax" wss://openagents.com slow && "$here/../tool.sh" "$here/sim-check.coax" wss://openagents.com && "$here/../tool.sh" "$here/load.coax" everglade wss://openagents.com 64 20 &&
    [[ "$(curl -s -m 20 https://openagents.com/api/playtest/rooms/everglade)" == '{"realm":"everglade"'* ]] &&
    curl -s -m 20 https://openagents.com/api/room/health | tee /dev/stderr | awk '{ for (i = 1; i <= NF; i++) if ($i == "p95") v = $(i + 1) } END { exit !(v != "" && v >= 0 && v < 50000) }' && { echo "room: deployed $image behind wss://openagents.com/api/room/"; exit 0; }
  sleep 30
done
echo "room: the load test through openagents.com failed" >&2; exit 1
