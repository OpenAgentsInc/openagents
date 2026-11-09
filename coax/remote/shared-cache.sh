#!/usr/bin/env bash
# Firm reason: 5. The build cache every host shares (docs/verse/remote-builds.md,
# #13119): curl to Cloud Storage's XML API, python3 and openssl to mint a token,
# gcloud for the reports. One bucket, gs://openagentsgemini-coax-build-cache,
# behind each host's disk caches: wasm.sh's unit cache (~/.cache/coax-wasm) and
# the compiler's check cache (~/.cache/coax-check). Its objects go 14 days after
# they are written (the bucket's lifecycle rule).
#
#   shared-cache.sh get NAME FILE       NAME into FILE, whole or not at all: 0 there, 1 not there (or off)
#   shared-cache.sh put NAME FILE       FILE as NAME unless NAME is there: 0 written, 2 already there, 1 failed (or off)
#   shared-cache.sh replace NAME FILE   FILE as NAME, over what is there
#   shared-cache.sh log NAME            records one compile of NAME (report counts them)
#   shared-cache.sh token               a token for this bucket alone, minted from the automation
#                                       account's key (the Mac: remote.sh sends one with each job)
#   shared-cache.sh summary FILE        one line from a job's stats file (host-job.sh's report)
#   shared-cache.sh report [DAY]        compiles per key on DAY (UTC, default today): keys compiled
#                                       more than once, across every host
#   shared-cache.sh publish-pages COMMIT MARKER
#                                       on a host, from a tree's root: the files under
#                                       apps/openagents-web the build wrote after MARKER that Git
#                                       doesn't track, as COMMIT's pages (pages.sh)
#
# Settings (environment):
#   COAX_SHARED_CACHE          the bucket (default openagentsgemini-coax-build-cache); 0 turns the tier off
#   COAX_SHARED_CACHE_TOKEN    the bearer token; with none, get, put and log do nothing
#   COAX_SHARED_CACHE_STATS    a file wasm.sh appends each step's result to (host-job.sh sets it)
#   COAX_SHARED_CACHE_KEY      the service account key token mints from
#                              (default ~/work/.secrets/gcp-mvp-automation.json)
set -uo pipefail
export LC_ALL=C
bucket="${COAX_SHARED_CACHE:-openagentsgemini-coax-build-cache}"
api="https://storage.googleapis.com"
token="${COAX_SHARED_CACHE_TOKEN:-}"
on() { [[ "$bucket" != 0 && -n "$token" ]]; }
fetch() {   # curl args...: the HTTP status, or 000
  curl -sS -o /dev/null -w '%{http_code}' --max-time 300 --retry 2 --retry-delay 1 -H "Authorization: Bearer $token" "$@" 2> /dev/null || true
}

get() {   # name file
  on || return 1
  local tmp code; tmp="$(mktemp "$(dirname "$2")/.shared.XXXXXXXX")" || return 1
  code="$(curl -sS -o "$tmp" -w '%{http_code}' --max-time 300 --retry 2 --retry-delay 1 -H "Authorization: Bearer $token" "$api/$bucket/$1" 2> /dev/null || true)"
  if [[ "$code" == 200 ]]; then mv -f "$tmp" "$2"; return 0; fi
  rm -f "$tmp"; return 1
}
put() {   # name file [replace]
  on || return 1
  local code guard=(-H 'x-goog-if-generation-match: 0')
  [[ "${3:-}" != replace ]] || guard=()
  code="$(fetch -X PUT ${guard[@]+"${guard[@]}"} -H 'Content-Type: application/octet-stream' --data-binary @"$2" "$api/$bucket/$1")"
  case "$code" in 200) return 0 ;; 412) return 2 ;; *) return 1 ;; esac
}
log() {   # name: one empty object per compile, under the UTC day
  on || return 1
  fetch -X PUT -H 'Content-Length: 0' --data-binary '' "$api/$bucket/log/$(date -u +%F)/$1/$(hostname -s 2> /dev/null || hostname).$$.$(date +%s)" > /dev/null
}

# A token for the bucket alone: an hour-long token for the automation account
# (a JWT signed with its key, so it is always fresh), exchanged at Google's STS
# for one bounded to this bucket's objects (a credential access boundary). A
# host holding it can read and write this bucket and nothing else.
token() {
  local key="${COAX_SHARED_CACHE_KEY:-$HOME/work/.secrets/gcp-mvp-automation.json}" cache="${OA_REMOTE_STATE:-$HOME/.openagents/remote}/shared-token"
  [[ "$bucket" != 0 ]] || return 1
  # Reused for 15 minutes, so a job always starts with 45 or more.
  if [[ -s "$cache" && -n "$(find "$cache" -mmin -15 2> /dev/null)" ]]; then cat "$cache"; return 0; fi
  local base=""
  if [[ -f "$key" ]]; then
    base="$(python3 -I - "$key" <<'PY'
import base64, json, os, subprocess, sys, tempfile, time, urllib.parse, urllib.request
k = json.load(open(sys.argv[1]))
b = lambda d: base64.urlsafe_b64encode(d).rstrip(b"=")
now = int(time.time())
signing = b(json.dumps({"alg": "RS256", "typ": "JWT"}).encode()) + b"." + b(json.dumps({
    "iss": k["client_email"], "scope": "https://www.googleapis.com/auth/devstorage.read_write",
    "aud": k["token_uri"], "iat": now, "exp": now + 3600}).encode())
with tempfile.NamedTemporaryFile() as data:
    data.write(signing); data.flush()
    sig = subprocess.run(["openssl", "dgst", "-sha256", "-sign", "/dev/stdin", data.name],
                         input=k["private_key"].encode(), capture_output=True, check=True).stdout
body = urllib.parse.urlencode({"grant_type": "urn:ietf:params:oauth:grant-type:jwt-bearer",
                               "assertion": (signing + b"." + b(sig)).decode()}).encode()
print(json.load(urllib.request.urlopen(k["token_uri"], body, timeout=30))["access_token"])
PY
)" || base=""
  fi
  [[ -n "$base" ]] || base="$(gcloud auth print-access-token 2> /dev/null)" || return 1
  local scoped
  scoped="$(BUCKET="$bucket" BASE="$base" python3 -I - <<'PY'
import json, os, urllib.parse, urllib.request
boundary = {"accessBoundary": {"accessBoundaryRules": [{
    "availablePermissions": ["inRole:roles/storage.objectAdmin"],
    "availableResource": "//storage.googleapis.com/projects/_/buckets/" + os.environ["BUCKET"]}]}}
body = urllib.parse.urlencode({
    "grant_type": "urn:ietf:params:oauth:grant-type:token-exchange",
    "subject_token_type": "urn:ietf:params:oauth:token-type:access_token",
    "requested_token_type": "urn:ietf:params:oauth:token-type:access_token",
    "subject_token": os.environ["BASE"], "options": json.dumps(boundary)}).encode()
print(json.load(urllib.request.urlopen("https://sts.googleapis.com/v1/token", body, timeout=30))["access_token"])
PY
)" || return 1
  [[ -n "$scoped" ]] || return 1
  mkdir -p "$(dirname "$cache")"
  (umask 077; printf '%s\n' "$scoped" > "$cache.$$" && mv -f "$cache.$$" "$cache")
  printf '%s\n' "$scoped"
}

# A job's stats file: "STEP RESULT MICROSECONDS" a line, from wasm.sh's steps
# (coax_to_c, clang, link: hit is the host's disk, shared the bucket, miss a
# compile, dup a compile whose result another host had written meanwhile, off
# no cache) and "checks shared|miss" for the check records of a compiled unit.
summary() {
  [[ -s "$1" ]] || { echo "cache: no cached steps"; return 0; }
  awk '
    $1 == "checks" { if ($2 == "shared") cs++; next }
    $1 == "wasm_opt" { next }
    { n++; s = $3 / 1e6 }
    $2 == "hit" { disk++ }
    $2 == "shared" { shared++ }
    $2 == "miss" || $2 == "dup" || $2 == "off" { built++; secs += s; if ($1 == "coax_to_c") { c++; csecs += s } }
    $2 == "dup" { dup++ }
    $2 == "prebuilt" { pre++ }
    END {
      hit = disk + shared
      printf "cache %d/%d hit (%d%%: disk %d, shared %d), %d compiled (%d Coax to C, %.0f s), %d duplicate%s",
        hit, n - pre, (n - pre) ? 100 * hit / (n - pre) : 0, disk, shared, built, c, csecs, dup, (dup == 1 ? "" : "s")
      if (cs) printf ", check records from the bucket for %d unit%s", cs, (cs == 1 ? "" : "s")
      if (pre) printf ", %d prebuilt", pre
      print ""
    }' "$1"
}

report() {   # [day]: compiles per key, from the log objects (gcloud: the Mac)
  local day="${1:-$(date -u +%F)}"
  export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
  gcloud storage ls "gs://$bucket/log/$day/**" 2> /dev/null | awk -F/ -v day="$day" '
    { key = $(NF - 1); n[key]++; host[key "|" $NF]++ }
    END {
      for (k in n) { keys++; total += n[k]; if (n[k] > 1) { more++; extra += n[k] - 1 }
        split(k, p, "."); kind[p[2]]++; if (n[k] > 1) kmore[p[2]]++ }
      printf "%s: %d compiles of %d keys; %d keys compiled more than once (%d extra compiles)\n", day, total, keys, more, extra
      for (e in kind) printf "  .%s: %d keys, %d more than once\n", e, kind[e], kmore[e]
    }'
}

publish_pages() {   # commit marker: run from the tree's root after build.sh
  local commit="$1" marker="$2" tmp
  on || { echo "pages: no token for the shared store" >&2; return 1; }
  tmp="$(mktemp -d)"
  # The build's own files: written after the marker, not tracked by Git (so a
  # fetch never overwrites a tracked file in someone's checkout), not work dirs.
  git ls-files apps/openagents-web | sort > "$tmp/tracked"
  find apps/openagents-web \( -name node_modules -o \( -path 'apps/openagents-web/build/.c/*' -type d \) \) -prune -o -type f -newer "$marker" -print |
    sort | comm -23 - "$tmp/tracked" > "$tmp/files"
  [[ -s "$tmp/files" ]] || { echo "pages: the build wrote no files" >&2; rm -rf "$tmp"; return 1; }
  tar -czf "$tmp/pages.tar.gz" -T "$tmp/files" || { rm -rf "$tmp"; return 1; }
  { echo coax-pages-1
    echo "commit $commit"
    echo "tree $(git rev-parse "$commit^{tree}")"
    echo "tar $(sha256sum < "$tmp/pages.tar.gz" | cut -c1-64) $(wc -c < "$tmp/pages.tar.gz" | tr -d ' ')"
    echo "files $(wc -l < "$tmp/files" | tr -d ' ')"
    echo "host $(hostname -s) $(date -u +%FT%TZ)"; } > "$tmp/MANIFEST"
  put "pages/$commit/pages.tar.gz" "$tmp/pages.tar.gz" replace && put "pages/$commit/MANIFEST" "$tmp/MANIFEST" replace ||
    { echo "pages: could not write to gs://$bucket/pages/$commit/" >&2; rm -rf "$tmp"; return 1; }
  echo "pages: ${commit:0:10}: $(wc -l < "$tmp/files" | tr -d ' ') files, $(( $(wc -c < "$tmp/pages.tar.gz") / 1000000 )) MB, in gs://$bucket/pages/$commit/" >&2
  rm -rf "$tmp"
}

case "${1:-}" in
  get) get "$2" "$3" ;;
  put) put "$2" "$3" ;;
  replace) put "$2" "$3" replace ;;
  log) log "$2" ;;
  token) token ;;
  summary) summary "$2" ;;
  report) report "${2:-}" ;;
  publish-pages) publish_pages "$2" "$3" ;;
  *) sed -n '2,22p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2 ;;
esac
