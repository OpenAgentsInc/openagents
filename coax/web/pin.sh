#!/usr/bin/env bash
# Pinned demos: one exact build of openagents.com at /demo/NAME/... (the
# playtest at /demo/NAME/verse) that stays put until the owner moves it.
# Releasing main (release.sh) never touches it.
#
#   pin.sh                    # the demos, and the build each one serves
#   pin.sh NAME               # NAME's pins, oldest first; the live one marked
#   pin.sh NAME REF           # move NAME's pin to REF (a commit, tag or branch): owner step
#   pin.sh NAME --back        # put back the pin before the live one
#   pin.sh NAME --to STAMP    # put back any kept pin
#
# A move tags REF as demo/NAME/<time> (a git tag that never moves), builds that
# tag in a clean worktree, keeps its pages in the production bucket under
# demo-releases/NAME/<time>/ and makes them the live demo/NAME/. The files the
# pages name are content-named at the bucket's root, next to main's, and are
# never deleted, so every kept pin keeps working. A demo's pages carry
# data-root="/demo/NAME" (host.js keeps the app's links inside the demo) and
# noindex. The API (/api/*) is shared with main: a pin freezes the pages, not
# the server. A move whose smoke fails puts the pin before it back.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../.." && pwd)"
bucket=openagentsgemini-openagents-web site=https://openagents.com
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
kept="gs://$bucket/demo-releases"

if [[ -z "${1:-}" ]]; then
  for d in $(gcloud storage ls "$kept/" 2>/dev/null | sed -E 's#.*/demo-releases/([^/]+)/$#\1#'); do
    echo "$d  $(gcloud storage cat "$kept/$d/LIVE" 2>/dev/null || echo '(none live)')  $site/demo/$d/verse"
  done
  exit 0
fi
name="$1"; shift
[[ "$name" =~ ^[a-z0-9][a-z0-9-]{0,39}$ ]] || { echo "pin: a demo's name is lowercase letters, digits and dashes" >&2; exit 2; }
history() { gcloud storage cat "$kept/$name/HISTORY" 2>/dev/null || true; }   # STAMP TAG COMMIT, one pin a line
live() { gcloud storage cat "$kept/$name/LIVE" 2>/dev/null | cut -d' ' -f1 || true; }

if [[ -z "${1:-}" ]]; then
  h="$(history)"; [[ -n "$h" ]] || { echo "pin: $name has no pins"; exit 0; }
  l="$(live)"
  while read -r stamp tag commit; do
    echo "$([[ "$stamp" == "$l" ]] && echo '*' || echo ' ') $stamp $tag $commit"
  done <<< "$h"
  exit 0
fi

# One writer at a time on this machine, shared with release.sh: both write
# the production bucket.
lock="${TMPDIR:-/tmp}/coaxial-release.lock"
until mkdir "$lock" 2>/dev/null; do
  holder="$(cat "$lock/pid" 2>/dev/null || true)"
  if [[ -n "$holder" ]] && ! kill -0 "$holder" 2>/dev/null; then rm -rf "$lock"; continue; fi
  echo "pin: waiting for release ${holder:-?} to finish"; sleep 5
done
echo $$ > "$lock/pid"
tree="" stage=""
cleanup() { [[ -z "$tree" ]] || git -C "$repo" worktree remove --force "$tree" 2>/dev/null || true; [[ -z "$stage" ]] || rm -rf "$stage"; rm -rf "$lock"; }
trap cleanup EXIT

# Serve kept pin STAMP at demo/NAME/, then smoke it.
serve() {
  gcloud storage rsync -r "$kept/$name/$1/site" "gs://$bucket/demo/$name" --delete-unmatched-destination-objects --quiet > /dev/null 2>&1 ||
    { echo "pin: couldn't copy $1 to demo/$name" >&2; exit 1; }
  echo "$1 $(date -u +%Y%m%dT%H%M%SZ)" | gcloud storage cp - "$kept/$name/LIVE" --quiet > /dev/null 2>&1
}
smoke() {
  local names; names="$(gcloud storage ls -r "$kept/$name/$1/site/**" | sed -E "s#^$kept/$name/$1/site/##")"
  python3 -I - "$site" "/demo/$name" "$names" <<'PY'
import re, sys, urllib.request
site, root = sys.argv[1], sys.argv[2]
names = sys.argv[3].split()
pages = [n for n in names if n.endswith(".html") or "." not in n.rsplit("/", 1)[-1]]   # x.html, and x (a bare path's page)
failed = 0
def get(path):
    try:
        with urllib.request.urlopen(urllib.request.Request(site + path, headers={"User-Agent": "coaxial-pin-smoke"}), timeout=20) as r:
            return r.status, r.read()
    except urllib.error.HTTPError as e: return e.code, b""
for p in pages:
    s, body = get(f"{root}/{p}")
    ok = s == 200 and (b"data-app=" not in body or f'data-root="{root}"'.encode() in body)
    for f in re.findall(rb'(?:src|data-wasm|data-wgsl)="(/(?:build|coaxial)/[^"]+)"', body):
        fs, fb = get(f.decode()); ok = ok and fs == 200
    print(("  ok    " if ok else "  FAIL  ") + f"{root}/{p} ({s})"); failed += not ok
sys.exit(1 if failed else 0)
PY
}

case "$1" in
  --back|--to)
    h="$(history)"; l="$(live)"
    if [[ "$1" == --to ]]; then to="${2:-}"
    else to="$(awk -v l="$l" '$1 == l { print prev; exit } { prev = $1 }' <<< "$h")"; fi
    [[ -n "$to" ]] && grep -q "^$to " <<< "$h" || { echo "pin: no such earlier pin of $name (pin.sh $name lists them)" >&2; exit 1; }
    serve "$to"; smoke "$to"
    echo "pin $name: $(grep "^$to " <<< "$h" | cut -d' ' -f2-) is live again at $site/demo/$name/verse" ;;
  -*) echo "pin: unknown option $1" >&2; exit 2 ;;
  *)
    git -C "$repo" fetch -q origin main --tags
    commit="$(git -C "$repo" rev-parse --verify --short=10 "$1^{commit}")"
    stamp="$(date -u +%Y%m%dT%H%M%SZ)"; tag="demo/$name/$stamp"
    tree="$(mktemp -d)/pin"
    git -C "$repo" worktree add -q --detach "$tree" "$commit"
    grep -q COAXIAL_STAGE_TO "$tree/coax/web/deploy.sh" || { echo "pin: $commit is older than pinned demos (its deploy.sh can't stage); pin a later commit" >&2; exit 1; }
    git -C "$repo" tag -a "$tag" "$commit" -m "Demo $name pinned to $commit"
    git -C "$repo" push -q origin "refs/tags/$tag"
    echo "pin $name: $tag ($(git -C "$tree" log -1 --format=%s | cut -c1-70))"
    [[ -x "$tree/coax/bin/coax" ]] || { mkdir -p "$tree/coax/bin"; cp "$repo/coax/bin/coax" "$tree/coax/bin/"; }
    stage="$(mktemp -d)"
    COAXIAL_STAGE_TO="$stage" "$tree/coax/web/deploy.sh" "$tree/apps/openagents-web" openagents-web > /dev/null
    # The demo's pages: links and the app's navigation stay under /demo/NAME;
    # crawlers leave the copy alone.
    python3 -I - "$stage/site" "/demo/$name" <<'PY'
import os, re, sys
site, root = sys.argv[1:]
for d, _, fs in os.walk(site):
    for f in fs:
        if not (f.endswith(".html") or f.endswith(".page")): continue
        p = os.path.join(d, f); t = open(p, encoding="utf-8").read()
        t = re.sub(r'(<script type="module" src="/coaxial/host\.[^"]+")', rf'\1 data-root="{root}"', t)
        t = re.sub(r'href="/(?!build/|coaxial/|favicon)([^"]*)"', rf'href="{root}/\1"', t)
        t = t.replace("<head>", '<head>\n<meta name="robots" content="noindex">', 1)
        open(p, "w", encoding="utf-8").write(t)
PY
    python3 -I "$here/upload.py" "$stage" "$bucket" "$(gcloud auth print-access-token)" "demo-releases/$name/$stamp/site" > /dev/null
    { history; echo "$stamp $tag $commit"; } | gcloud storage cp - "$kept/$name/HISTORY" --quiet > /dev/null 2>&1
    before="$(live)"
    serve "$stamp"
    if ! smoke "$stamp"; then
      if [[ -n "$before" ]]; then echo "pin $name: smoke failed; putting $before back" >&2; serve "$before"
      else echo "pin $name: smoke failed (no earlier pin to put back)" >&2; fi
      exit 1
    fi
    echo "pin $name: $commit is live at $site/demo/$name/verse (back: coax/web/pin.sh $name --back)" ;;
esac
