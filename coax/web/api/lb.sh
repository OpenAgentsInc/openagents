#!/usr/bin/env bash
# One time: puts the API services behind the openagents.com load balancer
# (coaxial-openagents-web-staging). Safe to run again: it makes only what is
# missing and leaves every other route as it is.
#   /api/* on staging.openagents.com -> Cloud Run oa-api-staging
#   /api/* on openagents.com         -> Cloud Run oa-api
# The backends pass the load balancer's view of the visitor to the service
# as headers (geography, RTT, TLS, the client's address in X-Forwarded-For),
# which pt-capture records. Every other path keeps its rule (the coder
# service still gets what nothing else matches).
set -euo pipefail
project=openagentsgemini region=us-central1 map=coaxial-openagents-web-staging
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
g() { gcloud "$@" --project "$project" --quiet; }
headers=(
  "X-Client-Region:{client_region}" "X-Client-Subdivision:{client_region_subdivision}"
  "X-Client-City:{client_city}" "X-Client-Latlong:{client_city_lat_long}"
  "X-Client-Rtt:{client_rtt_msec}" "X-Client-Tls:{tls_version}" "X-Client-Device:{device_request_type}"
)
for svc in oa-api-staging oa-api; do
  g compute network-endpoint-groups describe "$svc-neg" --region "$region" >/dev/null 2>&1 ||
    g compute network-endpoint-groups create "$svc-neg" --region "$region" --network-endpoint-type serverless --cloud-run-service "$svc"
  if ! g compute backend-services describe "$svc" --global >/dev/null 2>&1; then
    g compute backend-services create "$svc" --global --load-balancing-scheme EXTERNAL_MANAGED --protocol HTTP --enable-logging --logging-sample-rate 1
    g compute backend-services add-backend "$svc" --global --network-endpoint-group "$svc-neg" --network-endpoint-group-region "$region"
  fi
  args=(); for h in "${headers[@]}"; do args+=(--custom-request-header "$h"); done
  g compute backend-services update "$svc" --global "${args[@]}" >/dev/null
done
# The path rules (the map read as JSON, which import takes as YAML): /api/ in each host's matcher, at the next free priority.
tmp="$(mktemp)"; trap 'rm -f "$tmp"' EXIT
g compute url-maps describe "$map" --global --format json > "$tmp"
python3 -I - "$tmp" "$project" <<'PY'
import json, sys
path, project = sys.argv[1:]
m = json.load(open(path))
want = {"production": "oa-api", "staging": "oa-api-staging"}
changed = False
for pm in m["pathMatchers"]:
    svc = want.get(pm["name"])
    if not svc: continue
    rules = pm.setdefault("routeRules", [])
    if any(any(r.get("prefixMatch") == "/api/" for r in rule.get("matchRules", [])) for rule in rules): continue
    rules.append({"priority": max([r["priority"] for r in rules] or [0]) + 1, "matchRules": [{"prefixMatch": "/api/"}],
                  "service": f"https://www.googleapis.com/compute/v1/projects/{project}/global/backendServices/{svc}"})
    changed = True
for k in ("kind", "creationTimestamp", "id", "selfLink"): m.pop(k, None)   # read-only: import refuses them
json.dump(m, open(path, "w"))
print("url map: /api/ rules added" if changed else "url map: /api/ rules already there")
PY
g compute url-maps import "$map" --global --source "$tmp" 2>&1 | grep -E "ERROR" && exit 1 || true
