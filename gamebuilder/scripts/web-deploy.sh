#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around gcloud; holds no logic).
# gamebuilder.org on Google Cloud: the site's files in a storage bucket behind
# a global HTTPS load balancer, as openagents.com's site is.
#
#   scripts/web-deploy.sh            checks, builds and uploads the site; then measure the live page:
#                                    scripts/web-load.sh https://gamebuilder.org/
#   scripts/web-deploy.sh domain     one time: the bucket, the address, the
#                                    certificate and the load balancer. Safe to
#                                    run again: it makes only what is missing.
#   scripts/web-deploy.sh status     the address, and whether the certificate is active
#
# www.gamebuilder.org and plain HTTP both move for good to https://gamebuilder.org.
# DNS, at the registrar: A records for gamebuilder.org and www.gamebuilder.org
# to the address `status` prints.
set -euo pipefail
gb="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo="$(cd "$gb/.." && pwd)"
export GB_ROOT="$gb" GB_VENDOR="$repo" COAX="${COAX:-$repo/coax/bin/coax}"
project=openagentsgemini
name=gamebuilder-web
# The certificate in use. The first one (gamebuilder-web) was asked for before DNS pointed here and
# never left "not visible"; this one was asked for after.
cert=gamebuilder-web-b
bucket="gs://openagentsgemini-gamebuilder-web"
domain=gamebuilder.org
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
g() { gcloud "$@" --project "$project" --quiet; }
has() { g "$@" > /dev/null 2>&1; }

if [[ "${1:-}" == status ]]; then
  echo "address: $(g compute addresses describe "$name" --global --format 'value(address)')"
  g compute ssl-certificates describe "$cert" --global --format 'yaml(managed.status,managed.domainStatus)'
  exit
fi

if [[ "${1:-}" == domain ]]; then
  has storage buckets describe "$bucket" || g storage buckets create "$bucket" --location US --uniform-bucket-level-access
  g storage buckets update "$bucket" --web-main-page-suffix index.html --web-error-page 404.html > /dev/null
  g storage buckets add-iam-policy-binding "$bucket" --member allUsers --role roles/storage.objectViewer > /dev/null
  has compute addresses describe "$name" --global || g compute addresses create "$name" --global --ip-version IPV4
  has compute backend-buckets describe "$name" || g compute backend-buckets create "$name" --gcs-bucket-name "${bucket#gs://}" --enable-cdn --cache-mode USE_ORIGIN_HEADERS
  has compute ssl-certificates describe "$cert" --global || g compute ssl-certificates create "$cert" --global --domains "$domain,www.$domain"
  # The map: gamebuilder.org is the site; www moves to it; anything else moves to it too.
  map="$(mktemp)"; trap 'rm -f "$map"' EXIT
  base="https://www.googleapis.com/compute/v1/projects/$project/global"
  # The hosted levels API (deploy/api/deploy.sh), once its backend exists: exactly its routes
  # (coax/api.coax) go to it. Until then they fall to the bucket and are 404.
  # /g/CODE is a published game's link and /api/play/CODE what it plays (docs/publishing.md, "What is
  # built"): the link's page is the site's own /play page with the game's name and picture in its head.
  # /api/pictures/ADDRESS.jpg is an agent's newest picture, on its game's group at /agents
  # (docs/agents.md, issue 9).
  api=""
  if has compute backend-services describe gamebuilder-api --global; then
    api="  - paths: ['/api/rows/*', '/api/levels/*', '/api/play/*', '/api/pictures/*', '/g/*', '/api/capabilities', '/api/openapi.json', '/api/count', '/mcp']
    service: $base/backendServices/gamebuilder-api"
  fi
  # The player's release files, a bucket made outside this script and already on the live map:
  # kept, so running this again doesn't drop the route.
  snapshot_paths=""
  if has compute backend-buckets describe gamebuilder-player-releases; then
    snapshot_paths="  - paths: ['/player-releases/*']
    service: $base/backendBuckets/gamebuilder-player-releases"
  fi
  cat > "$map" <<MAP
name: $name
defaultUrlRedirect:
  hostRedirect: $domain
  httpsRedirect: true
  redirectResponseCode: MOVED_PERMANENTLY_DEFAULT
  stripQuery: false
hostRules:
- hosts: ['$domain']
  pathMatcher: site
pathMatchers:
- name: site
  defaultService: $base/backendBuckets/$name
  pathRules:
  # The login, the chat and its build agents are OpenAgents' own service (web/login.coax, web/chat.coax): exactly
  # these paths go to it, and nothing else of its API is reachable from this site. The service
  # lists this site as its own for them (OA_API_SITE_ORIGINS in openagents' coax/web/api/deploy.sh).
  # /api/auth/refresh is the gamebuilder CLI's (cli/account.coax): an app's next pair of tokens,
  # asked with no Origin, so the service treats it as an app's and never as this site's page.
$snapshot_paths
  # /api/auth/cli/code is /login/cli's (a logged-in browser's one-time code for a command line)
  # and /api/auth/cli/redeem the CLI's, with no Origin (docs/cli.md, "Log in from a browser").
  - paths: ['/api/auth/account', '/api/auth/email/start', '/api/auth/email/verify', '/api/auth/logout', '/api/auth/refresh', '/api/auth/cli/code', '/api/auth/cli/redeem', '/api/chat/complete/v1', '/api/gamebuilder/agents/v1', '/api/gamebuilder/agent/v1', '/api/gamebuilder/agent/events/v1']
    service: $base/backendServices/oa-api
$api
  - paths: ['/login/', '/login/index.html']
    urlRedirect:
      pathRedirect: /login
      redirectResponseCode: MOVED_PERMANENTLY_DEFAULT
  - paths: ['/login/cli/', '/login/cli/index.html']
    urlRedirect:
      pathRedirect: /login/cli
      redirectResponseCode: MOVED_PERMANENTLY_DEFAULT
  - paths: ['/play/', '/play/index.html']
    urlRedirect:
      pathRedirect: /play
      redirectResponseCode: MOVED_PERMANENTLY_DEFAULT
  - paths: ['/credits/', '/credits/index.html']
    urlRedirect:
      pathRedirect: /credits
      redirectResponseCode: MOVED_PERMANENTLY_DEFAULT
  - paths: ['/agents/', '/agents/index.html']
    urlRedirect:
      pathRedirect: /agents
      redirectResponseCode: MOVED_PERMANENTLY_DEFAULT
  - paths: ['/splicebots/', '/splicebots/index.html']
    urlRedirect:
      pathRedirect: /splicebots
      redirectResponseCode: MOVED_PERMANENTLY_DEFAULT
  - paths: ['/spicebots', '/spicebots/', '/spicebots/index.html']
    urlRedirect:
      pathRedirect: /splicebots
      redirectResponseCode: MOVED_PERMANENTLY_DEFAULT
  - paths: ['/index.html']
    urlRedirect:
      pathRedirect: /
      redirectResponseCode: MOVED_PERMANENTLY_DEFAULT
MAP
  g compute url-maps import "$name" --global --source "$map"
  # Plain HTTP on any host moves to https://gamebuilder.org.
  cat > "$map" <<MAP
name: $name-http
defaultUrlRedirect:
  hostRedirect: $domain
  httpsRedirect: true
  redirectResponseCode: MOVED_PERMANENTLY_DEFAULT
  stripQuery: false
MAP
  g compute url-maps import "$name-http" --global --source "$map"
  has compute target-https-proxies describe "$name" --global || g compute target-https-proxies create "$name" --global --url-map "$name" --ssl-certificates "$cert"
  has compute target-http-proxies describe "$name-http" --global || g compute target-http-proxies create "$name-http" --global --url-map "$name-http"
  has compute forwarding-rules describe "$name" --global || g compute forwarding-rules create "$name" --global --address "$name" --target-https-proxy "$name" --ports 443 --load-balancing-scheme EXTERNAL_MANAGED
  has compute forwarding-rules describe "$name-http" --global || g compute forwarding-rules create "$name-http" --global --address "$name" --target-http-proxy "$name-http" --ports 80 --load-balancing-scheme EXTERNAL_MANAGED
  exec "$0" status
fi

# The site: the page's checks, its module, and the files a browser asks for, as the dev server serves them.
"$COAX" test "$GB_ROOT/web/site.coax" --library > /dev/null
"$COAX" test "$GB_ROOT/web/site-agents.coax" --library > /dev/null
"$COAX" test "$GB_ROOT/web/hud.coax" --library > /dev/null
(cd "$GB_ROOT" && "$GB_VENDOR/coax/web/tool.sh" tools/village-pack-check.coax web/assets/village.pack)
"$COAX" test "$GB_ROOT/web/login.coax" --library > /dev/null
"$COAX" test "$GB_ROOT/web/chat.coax" --library > /dev/null
COAXIAL_MODE=release "$GB_ROOT/web/build.sh" > /dev/null
stage="$(mktemp -d)"; trap 'rm -rf "$stage"' EXIT
"$GB_ROOT/scripts/web-stage.sh" "$stage"
# A qualification copy uses a separate public asset bucket, protected from main's
# rsync deletion. The Coax tool pins URL carriage and preserves current API routes.
if [[ "${1:-}" == --snapshot ]]; then
  [[ -n "${2:-}" ]] || { echo "usage: scripts/web-deploy.sh --snapshot NAME" >&2; exit 2; }
  "$GB_VENDOR/coax/web/tool.sh" "$GB_ROOT/tools/player-release.coax" "$stage" "$2" --out "$GB_ROOT/build/player-release/$2"
  exit
fi
# What a first visit loads is under /b/NAME/ (scripts/web-stage.sh: the one script, the modules, the
# shaders, the fonts; NAME is a digest of them), kept by browsers and the edge for good, and the
# script, modules and shaders are stored gzipped. They go up before the page that names them.
# Everything else (the pages, the pictures, the pack) is checked against the bucket before a
# browser or the edge uses its copy ("no-cache": keep it, but ask first; an unchanged file costs
# one small answer), so a deploy shows at once and a second visit re-fetches the page alone.
release="$("$GB_ROOT/scripts/web-stage.sh" "$stage" release)"
"$GB_ROOT/scripts/web-stage.sh" "$stage" stored
kept="public, max-age=31536000, immutable"
# the release the live page names now: it is left in the bucket, for a page loaded just before this deploy
before="$(g storage cat "$bucket/index.html" 2> /dev/null | sed -n 's|.*"/b/\([0-9a-f]*\)/host/.*|\1|p' | head -1 || true)"
g storage cp --recursive "$stage/b/$release/fonts" "$stage/b/$release/assets" "$stage/b/$release/img" "$bucket/b/$release/" --cache-control "$kept" > /dev/null
g storage cp --recursive "$stage/b/$release/host" "$stage/b/$release/build" "$bucket/b/$release/" --cache-control "$kept" --content-encoding gzip > /dev/null
cache="no-cache"
g storage rsync "$stage" "$bucket" --recursive --delete-unmatched-destination-objects --exclude '^(login|login/cli|play|credits|agents|splicebots)$|^(b|splicebots)/' --cache-control "$cache" > /dev/null
# nobody is logged in to the bucket: its answer to the host's question is the same all day
g storage objects update "$bucket/api/auth/viewer" --cache-control "public, max-age=86400" > /dev/null
# A page's address has no file name in it: /login, /login/cli, /play, /credits and /agents are the objects of those names
# themselves (the same page: one module draws every address),
# and the load balancer's map sends /login/, /login/index.html and the like back to them.
for page in login login/cli play credits agents; do g storage cp "$stage/index.html" "$bucket/$page" --content-type "text/html" --cache-control "$cache" > /dev/null; done
# releases older than the one before this are gone
for old in $(g storage ls "$bucket/b/" 2> /dev/null); do [[ "$old" == */"$release"/ || "$old" == */"${before:-none}"/ ]] || g storage rm --recursive "$old" > /dev/null; done
echo "uploaded web/ to $bucket (release $release)"
# what the edge still holds from before goes now, not when it would have expired
g compute url-maps invalidate-cdn-cache gamebuilder-web --path "/*" --async > /dev/null 2>&1 || echo "note: the edge cache was not cleared; old files may show for a few minutes"
