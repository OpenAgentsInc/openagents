#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around gcloud; holds no logic).
# gamebuilder.org on Google Cloud: static landing page + standalone WebGPU worlds (/fated-dragon, /splicebots, /whispering-spring, /ice-cream-man, /scrappers-arena)
# served from gs://openagentsgemini-gamebuilder-web behind Cloud CDN.
# Accounts use OpenAgents login (https://openagents.com); gamebuilder.org hosts no separate login.
#
#   scripts/web-deploy.sh            checks, builds and uploads the site + worlds
#   scripts/web-deploy.sh domain     creates/updates bucket, IP, cert, and URL map redirects
#   scripts/web-deploy.sh status     prints external IP and certificate status
set -euo pipefail
gb="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo="$(cd "$gb/.." && pwd)"
export GB_ROOT="$gb" GB_VENDOR="$repo" COAX="${COAX:-$repo/coax/bin/coax}"
project=openagentsgemini
name=gamebuilder-web
cert=gamebuilder-web-b
bucket="gs://openagentsgemini-gamebuilder-web"
domain=gamebuilder.org
if [[ -z "${CLOUDSDK_CONFIG:-}" && -d "$HOME/work/.secrets/gcloud-sa-config" ]]; then
  export CLOUDSDK_CONFIG="$HOME/work/.secrets/gcloud-sa-config"
fi
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
  map="$(mktemp)"; trap 'rm -f "$map"' EXIT
  base="https://www.googleapis.com/compute/v1/projects/$project/global"
  api=""
  if has compute backend-services describe gamebuilder-api --global; then
    api="  - paths: ['/api/rows/*', '/api/levels/*', '/api/play/*', '/api/pictures/*', '/g/*', '/api/capabilities', '/api/openapi.json', '/api/count', '/mcp']
    service: $base/backendServices/gamebuilder-api"
  fi
  # The Whispering Spring's shared grove: guest keys, its chat and its ribbon tree are the OpenAgents
  # API's moderated rooms (coax/web/api/room.c), the same Cloud Run service openagents.com routes to.
  rooms=""
  if has compute backend-services describe oa-api --global; then
    rooms="  - paths: ['/api/playtest/player', '/api/playtest/player/name', '/api/playtest/chat/whispering-spring', '/api/playtest/chat/whispering-spring-ribbons', '/api/playtest/presence/whispering-spring']
    service: $base/backendServices/oa-api"
  fi
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
$snapshot_paths
$api
$rooms
  - paths: ['/fated-dragon/', '/fated-dragon/index.html']
    urlRedirect:
      pathRedirect: /fated-dragon
      redirectResponseCode: MOVED_PERMANENTLY_DEFAULT
  - paths: ['/splicebots/', '/splicebots/index.html']
    urlRedirect:
      pathRedirect: /splicebots
      redirectResponseCode: MOVED_PERMANENTLY_DEFAULT
  - paths: ['/spicebots', '/spicebots/', '/spicebots/index.html']
    urlRedirect:
      pathRedirect: /splicebots
      redirectResponseCode: MOVED_PERMANENTLY_DEFAULT
  - paths: ['/whispering-spring/', '/whispering-spring/index.html']
    urlRedirect:
      pathRedirect: /whispering-spring
      redirectResponseCode: MOVED_PERMANENTLY_DEFAULT
  - paths: ['/ice-cream-man/', '/ice-cream-man/index.html', '/icecream', '/icecream/', '/icecream/index.html']
    urlRedirect:
      pathRedirect: /ice-cream-man
      redirectResponseCode: MOVED_PERMANENTLY_DEFAULT
  - paths: ['/scrappers-arena/', '/scrappers-arena/index.html', '/scrapper-arena', '/scrapper-arena/', '/scrapper-arena/index.html', '/scrappers', '/scrappers/', '/scrappers/index.html']
    urlRedirect:
      pathRedirect: /scrappers-arena
      redirectResponseCode: MOVED_PERMANENTLY_DEFAULT
  - paths: ['/credits/', '/credits/index.html']
    urlRedirect:
      pathRedirect: /credits
      redirectResponseCode: MOVED_PERMANENTLY_DEFAULT
  - paths: ['/index.html']
    urlRedirect:
      pathRedirect: /
      redirectResponseCode: MOVED_PERMANENTLY_DEFAULT
MAP
  g compute url-maps import "$name" --global --source "$map"
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

"$GB_ROOT/scripts/web.sh" check > /dev/null
COAXIAL_MODE=release "$GB_ROOT/web/build.sh" > /dev/null
stage="$(mktemp -d)"; pages="$(mktemp -d)"; trap 'rm -rf "$stage" "$pages"' EXIT
"$GB_ROOT/scripts/web-stage.sh" "$stage"
release="$("$GB_ROOT/scripts/web-stage.sh" "$stage" release)"
"$GB_ROOT/scripts/web-stage.sh" "$stage" stored

kept="public, max-age=31536000, immutable"
before="$(g storage cat "$bucket/fated-dragon" 2> /dev/null | sed -n 's|.*"/b/\([0-9a-f]*\)/coaxial/.*|\1|p' | head -1 || true)"
if [[ -z "$before" ]]; then
  before="$(g storage cat "$bucket/index.html" 2> /dev/null | sed -n 's|.*"/b/\([0-9a-f]*\)/fonts/.*|\1|p' | head -1 || true)"
fi

# Upload immutable release assets before HTML pages that reference them.
g storage cp --recursive "$stage/b/$release/fonts" "$bucket/b/$release/" --cache-control "$kept" > /dev/null
g storage cp --recursive "$stage/b/$release/splicebots/fonts" "$bucket/b/$release/splicebots/" --cache-control "$kept" > /dev/null
g storage cp --recursive "$stage/b/$release/coaxial" "$stage/b/$release/build" "$stage/b/$release/fated-dragon" "$stage/b/$release/whispering-spring" "$stage/b/$release/ice-cream-man" "$stage/b/$release/scrappers-arena" "$bucket/b/$release/" --cache-control "$kept" --content-encoding gzip > /dev/null
g storage cp --recursive "$stage/b/$release/splicebots/build" "$stage/b/$release/splicebots/splice-jam.wgsl" "$bucket/b/$release/splicebots/" --cache-control "$kept" --content-encoding gzip > /dev/null

# Extensionless HTML routes served directly (trailing slashes and /index.html 301-redirect here).
# Pull directory index.html files out of $stage before rsync so GCS --web-main-page-suffix
# never sees <world>/index.html, and use ^,^ so gcloud's ArgList parser does not treat the
# regex ^ anchors as a custom list delimiter.
cache="no-cache"
cp "$stage/credits.html" "$pages/credits"
mv "$stage/fated-dragon/index.html" "$pages/fated-dragon"
mv "$stage/splicebots/index.html" "$pages/splicebots"
mv "$stage/whispering-spring/index.html" "$pages/whispering-spring"
mv "$stage/ice-cream-man/index.html" "$pages/ice-cream-man"
mv "$stage/scrappers-arena/index.html" "$pages/scrappers-arena"
rmdir "$stage/fated-dragon" "$stage/splicebots" "$stage/whispering-spring" "$stage/ice-cream-man" "$stage/scrappers-arena" 2>/dev/null || true

# Sync top-level pages, images, unversioned manifests, and content-verified packs; remove stale unmanaged legacy objects.
g storage rsync "$stage" "$bucket" --recursive --delete-unmatched-destination-objects --exclude '^,^^b/,^credits$,^fated-dragon$,^splicebots$,^whispering-spring$,^ice-cream-man$,^scrappers-arena$' --cache-control "$cache" > /dev/null
g storage objects update "$bucket/api/auth/viewer" --cache-control "public, max-age=86400" > /dev/null

# Upload extensionless HTML pages after rsync; packs live under /packs/ and manifests under /manifests/ so no prefix collides.
g storage cp "$pages/credits" "$bucket/credits" --content-type "text/html" --cache-control "$cache" > /dev/null
g storage cp "$pages/fated-dragon" "$bucket/fated-dragon" --content-type "text/html" --cache-control "$cache" > /dev/null
g storage cp "$pages/splicebots" "$bucket/splicebots" --content-type "text/html" --cache-control "$cache" > /dev/null
g storage cp "$pages/whispering-spring" "$bucket/whispering-spring" --content-type "text/html" --cache-control "$cache" > /dev/null
g storage cp "$pages/ice-cream-man" "$bucket/ice-cream-man" --content-type "text/html" --cache-control "$cache" > /dev/null
g storage cp "$pages/scrappers-arena" "$bucket/scrappers-arena" --content-type "text/html" --cache-control "$cache" > /dev/null

# Prune old immutable releases, keeping the previous release during rollout.
for old in $(g storage ls "$bucket/b/" 2> /dev/null); do
  [[ "$old" == */"$release"/ || "$old" == */"${before:-none}"/ ]] || g storage rm --recursive "$old" > /dev/null
done
echo "uploaded gamebuilder.org to $bucket (release $release)"
g compute url-maps invalidate-cdn-cache gamebuilder-web --path "/*" --async > /dev/null 2>&1 || echo "note: the edge cache was not cleared; old files may show for a few minutes"
