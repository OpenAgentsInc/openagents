#!/usr/bin/env bash
# Moves playtest.openagents.com to openagents.com, and back (pt-cutover-plan,
# docs/verse/playtest-cutover.md). Only the URL map of the playtest's own load
# balancer changes: its IP, DNS, certificate, backend and the Cloud Run
# service aethyr-demo stay as they are, so going back is one import.
#
#   playtest-switch.sh show                 the map that serves now
#   playtest-switch.sh switch [--to PATH]   every old address moves for good (301) to openagents.com: OWNER STEP
#   playtest-switch.sh rollback             the old demo serves again
#   playtest-switch.sh smoke old|moved      checks what the host answers
#
# Rehearsal (or anything that isn't the real one): --map, --old, --host, --ip
# name a throwaway load balancer, its backend, and the hostname and address
# to test (curl --resolve, so no DNS is needed). Without them it's the real
# playtest.openagents.com.
set -euo pipefail
project=openagentsgemini
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
map=aethyr-demo-lb old=aethyr-demo-web api=oa-api host=playtest.openagents.com ip=34.110.190.11 to=/verse insecure=""
verb="${1:-}"; [[ $# -gt 0 ]] && shift
what="${1:-}"; [[ "$verb" == smoke && $# -gt 0 ]] && shift
while [[ $# -gt 0 ]]; do
  case "$1" in
    --map) map="$2"; shift 2 ;; --old) old="$2"; shift 2 ;; --api) api="$2"; shift 2 ;;
    --host) host="$2"; shift 2 ;; --ip) ip="$2"; shift 2 ;; --to) to="$2"; shift 2 ;;
    --insecure) insecure=-k; shift ;;
    *) echo "playtest-switch: unknown $1" >&2; exit 2 ;;
  esac
done
[[ "$to" =~ ^/[a-z0-9/_-]*$ ]] || { echo "playtest-switch: --to is a path on openagents.com" >&2; exit 2; }
B="https://www.googleapis.com/compute/v1/projects/$project/global/backendServices"
g() { gcloud "$@" --project "$project" --quiet; }
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT

# The old map, exactly as it is today: everything to the demo's backend.
old_map() { printf 'name: %s\ndefaultService: %s/%s\n' "$map" "$B" "$old"; }
# The moved map. /haunt.html (a shared world, ?visit=ID) goes to the API,
# which answers 301 to /realm/haunted-house#ID; /api/ still reaches the old
# service, so tabs already open keep recording until the last import; the
# dashboard pages move to /admin/analytics; everything else to $to.
moved_map() {
  cat <<EOF
name: $map
defaultService: $B/$old
hostRules:
- hosts: ['*']
  pathMatcher: moved
pathMatchers:
- name: moved
  defaultUrlRedirect: {hostRedirect: openagents.com, pathRedirect: '$to?from=playtest', httpsRedirect: true, stripQuery: true, redirectResponseCode: MOVED_PERMANENTLY_DEFAULT}
  routeRules:
  - priority: 1
    matchRules: [{fullPathMatch: /haunt.html}]
    service: $B/$api
    routeAction: {urlRewrite: {pathPrefixRewrite: /api/playtest/old-link}}
  - priority: 2
    matchRules: [{prefixMatch: /api/}]
    service: $B/$old
  - priority: 3
    matchRules: [{fullPathMatch: /analytics.html}, {fullPathMatch: /feedback.html}]
    urlRedirect: {hostRedirect: openagents.com, pathRedirect: /admin/analytics, httpsRedirect: true, stripQuery: true, redirectResponseCode: MOVED_PERMANENTLY_DEFAULT}
EOF
}
put() {   # the map in $1 becomes the live one
  g compute url-maps validate --global --load-balancing-scheme EXTERNAL_MANAGED --source "$1" --format 'value(result.loadSucceeded)' 2>&1 | grep -qi true ||
    { echo "playtest-switch: the map doesn't validate" >&2; g compute url-maps validate --global --source "$1" >&2; exit 1; }
  g compute url-maps import "$map" --global --source "$1" >/dev/null
}
# The edge writes the default port into a redirect it makes (https://openagents.com:443/verse); browsers drop it.
ask() { curl -s $insecure -o /dev/null --resolve "$host:443:$ip" -w '%{http_code} %{redirect_url}' "https://$host$1" | sed 's#^\([0-9]* https://[^/:]*\):443/#\1/#'; }
smoke() {   # old|moved: every address answers as it should. The edge's front ends take up a new map
            # at different moments over a few minutes: it reports the first clean pass and waits up to
            # 8 minutes for five clean passes in a row (settled).
  local want="$1" fails=0 deadline=$((SECONDS + 480)) ok=0 first=""
  local -a checks
  if [[ "$want" == old ]]; then
    checks=("/|200 " "/haunt.html|200 " "/kiki/index.html|200 " "/api/config|200 ")
  else
    checks=("/|301 https://openagents.com$to?from=playtest"
            "/kiki/index.html|301 https://openagents.com$to?from=playtest"
            "/home.html?x=1|301 https://openagents.com$to?from=playtest"
            "/haunt.html|301 https://openagents.com/realm/haunted-house?from=playtest"
            "/haunt.html?visit=planet-06258408&planet=forest|301 https://openagents.com/realm/haunted-house?from=playtest#planet-06258408"
            "/analytics.html|301 https://openagents.com/admin/analytics"
            "/api/config|200 ")
  fi
  while :; do
    fails=0
    for c in "${checks[@]}"; do [[ "$(ask "${c%%|*}")" == "${c#*|}" ]] || fails=$((fails + 1)); done
    if [[ $fails -eq 0 ]]; then
      [[ -n "$first" ]] || { first=$SECONDS; echo "  first clean pass after ${first}s"; }
      ok=$((ok + 1)); [[ $ok -ge 5 ]] && break
    else ok=0; fi
    [[ $SECONDS -lt $deadline ]] || break
    sleep 2
  done
  for c in "${checks[@]}"; do
    got="$(ask "${c%%|*}")"; [[ "$got" == "${c#*|}" ]] && echo "  ok    ${c%%|*}  $got" || echo "  FAIL  ${c%%|*}  $got (want ${c#*|})"
  done
  [[ $ok -ge 5 ]] || { echo "smoke $want: FAIL" >&2; return 1; }
  echo "smoke $want: settled after ${SECONDS}s"
}

case "$verb" in
  show) g compute url-maps export "$map" --global ;;
  switch)
    g compute url-maps export "$map" --global --destination "$tmp/before.yaml"
    grep -q "hostRules" "$tmp/before.yaml" && { echo "playtest-switch: $map already moved"; exit 0; }
    SECONDS=0; moved_map > "$tmp/moved.yaml"; put "$tmp/moved.yaml"; echo "switch: $map moved to openagents.com$to (map in ${SECONDS}s)"
    SECONDS=0; smoke moved || { echo "switch: smoke failed, rolling back" >&2; old_map > "$tmp/old.yaml"; put "$tmp/old.yaml"; SECONDS=0; smoke old; exit 1; } ;;
  rollback) SECONDS=0; old_map > "$tmp/old.yaml"; put "$tmp/old.yaml"; echo "rollback: $map serves $old again (map in ${SECONDS}s)"; SECONDS=0; smoke old ;;
  smoke) [[ "$what" == old || "$what" == moved ]] || { echo "smoke old|moved" >&2; exit 2; }; SECONDS=0; smoke "$what" ;;
  *) sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'; exit 2 ;;
esac
