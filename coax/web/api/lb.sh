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
#
# Private projects (private.c, docs/private-projects.md):
#   lb.sh private-route SLUG   /projects/SLUG and /projects/SLUG/* on both hosts -> the API
#                              (the slug comes from whoever runs it, never from this repository)
#   lb.sh private-pages        any other /projects/* that the site doesn't have answers with the
#                              site's locked private page (/projects/private/), 404, so a slug
#                              nobody routed looks like one you may not see
set -euo pipefail
project=openagentsgemini region=us-central1 map=coaxial-openagents-web-staging
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
g() { gcloud "$@" --project "$project" --quiet; }
if [[ "${1:-}" == private-route || "${1:-}" == private-pages ]]; then
  what="$1" slug="${2:-}"
  [[ "$what" == private-pages || "$slug" =~ ^[a-z0-9][a-z0-9-]{0,63}$ ]] || { echo "lb.sh: private-route SLUG (lowercase letters, digits, -)" >&2; exit 2; }
  [[ "$slug" != private ]] || { echo "lb.sh: 'private' is the site's locked page" >&2; exit 2; }
  tmp="$(mktemp)"; trap 'rm -f "$tmp"' EXIT
  g compute url-maps describe "$map" --global --format json > "$tmp"
  python3 -I - "$tmp" "$project" "$what" "$slug" <<'PY'
import json, sys
path, project, what, slug = sys.argv[1:]
m = json.load(open(path))
base = f"https://www.googleapis.com/compute/v1/projects/{project}/global/"
api = {"production": "oa-api", "staging": "oa-api-staging"}
site = {"production": "coaxial-openagents-web", "staging": "coaxial-openagents-web-staging"}
changed = False
def site_rule(rules):   # the production rule that sends /projects/ to the site's bucket, if any
    return next((r for r in rules if r.get("service", "").endswith("/" + site["production"])
                 and any(x.get("prefixMatch") == "/projects/" for x in r.get("matchRules", []))), None)
def room_before(rules, limit):   # a free priority below limit (everyone's spread out when there is none)
    used = {r["priority"] for r in rules}
    if limit is None: return max(used | {0}) + 10
    free = [p for p in range(limit - 1, -1, -1) if p not in used]
    if not free:
        for r in rules: r["priority"] = r["priority"] * 10 + 10
        return limit * 10 + 9
    return free[0]
for pm in m["pathMatchers"]:
    name = pm["name"]
    if name not in api: continue
    rules = pm.setdefault("routeRules", [])
    if what == "private-route":
        want = [{"fullPathMatch": f"/projects/{slug}"}, {"prefixMatch": f"/projects/{slug}/"}]
        if any(r.get("matchRules") == want for r in rules): continue
        anchor = next((r for r in rules if r.get("customErrorResponsePolicy") and any(x.get("prefixMatch") == "/projects/" for x in r["matchRules"])), None) or site_rule(rules)
        p = room_before(rules, anchor["priority"] if anchor else None)
        rules.append({"priority": p, "matchRules": want, "service": base + "backendServices/" + api[name]})
    else:
        if any(r.get("customErrorResponsePolicy") for r in rules): continue
        anchor = site_rule(rules)
        p = room_before(rules, anchor["priority"] if anchor else None)
        bucket = base + "backendBuckets/" + site[name]
        rules.append({"priority": p, "matchRules": [{"prefixMatch": "/projects/"}], "service": bucket,
                      "customErrorResponsePolicy": {"errorService": bucket, "errorResponseRules": [
                          {"matchResponseCodes": ["404"], "path": "/projects/private/index.html", "overrideResponseCode": 404}]}})
    rules.sort(key=lambda r: r["priority"])
    changed = True
for k in ("kind", "creationTimestamp", "id", "selfLink"): m.pop(k, None)
json.dump(m, open(path, "w"))
print(f"url map: {what} " + ("added" if changed else "already there"))
PY
  g compute url-maps validate --global --source "$tmp" --format 'value(result.loadSucceeded)' 2>&1 | grep -qi true ||
    { echo "lb.sh: the new url map doesn't validate" >&2; g compute url-maps validate --global --source "$tmp" >&2; exit 1; }
  g compute url-maps import "$map" --global --source "$tmp" 2>&1 | grep -E "ERROR" && exit 1
  exit 0
fi
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
# The path rules (the map read as JSON, which import takes as YAML): /api/ in each host's matcher, at the next
# free priority, and /admin/analytics, /signin and the app's universal-link file to the site in production.
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
# /admin/analytics (admin.coax) and /signin (private.coax page 2) are Coaxial
# pages: in production they go to the site's bucket with the other Coax pages,
# not to the coder service; so does /.well-known/apple-app-site-association.
for pm in m["pathMatchers"]:
    if pm["name"] != "production": continue
    for rule in pm.get("routeRules", []):
        if not rule.get("service", "").endswith("/coaxial-openagents-web"): continue
        if not any(x.get("prefixMatch") == "/build/" for x in rule.get("matchRules", [])): continue   # the Coax files' rule
        have = {r.get("fullPathMatch") or r.get("prefixMatch") for r in rule.get("matchRules", [])}
        for match in ({"fullPathMatch": "/admin/analytics"}, {"prefixMatch": "/admin/analytics/"},
                      {"fullPathMatch": "/signin"}, {"prefixMatch": "/signin/"},
                      {"fullPathMatch": "/.well-known/apple-app-site-association"}):
            if next(iter(match.values())) not in have:
                rule["matchRules"].append(match); changed = True
                print("url map: production " + next(iter(match.values())) + " to the site")
for k in ("kind", "creationTimestamp", "id", "selfLink"): m.pop(k, None)   # read-only: import refuses them
json.dump(m, open(path, "w"))
print("url map: /api/ rules added" if changed else "url map: /api/ rules already there")
PY
g compute url-maps import "$map" --global --source "$tmp" 2>&1 | grep -E "ERROR" && exit 1 || true
