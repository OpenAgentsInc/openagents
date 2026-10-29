#!/usr/bin/env bash
# coax deploy, for now. An app's files live in a storage bucket that its
# Cloud Run service mounts, so a deploy is a release build and an upload:
# no image build and no new revision.
#
#   deploy.sh APP_DIR SERVICE           # build in release, upload (seconds)
#   deploy.sh --server APP_DIR SERVICE  # also (re)build the server image and
#                                       # (re)create the service and its bucket
#
# COAXIAL_UNLOCKS adds hidden presentations to index.html (unlock/README.md).
# COAXIAL_STAGE_TO=DIR builds and stages into DIR (assets/, site/) and stops
# there, uploading nothing: pin.sh ships a pinned demo from it.
#
# The first deploy of a service needs --server, then a domain mapping:
#   gcloud beta run domain-mappings create --service SERVICE --domain HOST --region us-central1
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
server=false
if [[ "${1:-}" == --server ]]; then server=true; shift; fi
app="$(cd "$1" && pwd)"; service="$2"
project=openagentsgemini region=us-central1
bucket="$project-$service"
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
now() { [[ -n "${EPOCHREALTIME:-}" ]] && echo "${EPOCHREALTIME//[.,]/}" || echo $(( $(date +%s) * 1000000 )); }   # microseconds
lap() { local d=$(( $(now) - $2 )); printf '  %-8s %4s s\n' "$1" "$(( d / 1000000 )).$(( d % 1000000 / 100000 ))"; }

"$here/check-visuals.sh" "$app"   # INVARIANTS.md: everything visible is Coax
t=$(now); COAXIAL_MODE=release "$app/build.sh"; lap build "$t"

stage="$(mktemp -d)"; trap 'rm -rf "$stage"' EXIT
# Bundle host modules before content naming; deployed pages need one immutable
# script, while coax serve can load the source modules from /coaxial/ directly.
bun build "$here/host.js" --target browser --format esm --outfile "$stage/host.js" > /dev/null
# The page, index.html at the bucket's root as the CDN serves it. The wasm,
# shaders and host script are named by their content (cached for good); the
# page names them (data-wasm, data-wgsl) and is checked on every visit.
python3 - "$app" "$stage/host.js" "$stage" <<'PY'
import hashlib, os, re, shutil, sys
app, host, stage = sys.argv[1:]
assets, site = os.path.join(stage, "assets"), os.path.join(stage, "site")
def put(src, rel):
    h = hashlib.sha256(open(src, "rb").read()).hexdigest()[:12]
    stem, ext = os.path.splitext(rel)
    named = f"{stem}.{h}{ext}"
    for root in (assets, site):
        os.makedirs(os.path.dirname(os.path.join(root, named)), exist_ok=True)
        shutil.copy(src, os.path.join(root, named))
    return "/" + named
host_url = put(host, "coaxial/host.js")
os.makedirs(site, exist_ok=True)
# Pages at any depth (projects/coax/index.html is /projects/coax): pages, their
# Markdown twins, icons and link-preview cards (.jpg). Work files, builds and sources stay behind.
pages = []
for root, dirs, files in os.walk(app):
    dirs[:] = sorted(d for d in dirs if d not in ("build", "work", "node_modules") and not d.startswith("."))
    for f in sorted(files):
        rel = os.path.relpath(os.path.join(root, f), app)
        agent = rel in ("llms.txt", "robots.txt") or (rel.startswith("cli" + os.sep) and f.endswith((".sh", ".ps1")))
        if f == "README.md" or not (agent or f.endswith((".html", ".md", ".svg", ".jpg"))): continue
        pages.append(rel)
for f in pages:
    path = os.path.join(app, f)
    os.makedirs(os.path.dirname(os.path.join(site, f)) or site, exist_ok=True)
    text = open(path, encoding="utf-8").read() if f.endswith(".html") else None
    if text is None: shutil.copy(path, os.path.join(site, f)); continue
    def tag(m):
        name = re.search(r'data-app="([^"]+)"', m.group(0)).group(1)
        wasm = put(os.path.join(app, "build", name + ".wasm"), f"build/{name}.wasm")
        wgsl = put(os.path.join(app, name + ".wgsl"), f"build/{name}.wgsl")
        rest = m.group(1)
        # A scene's assets (data-assets): each URL named by its content too,
        # under /build/ (the page's directory's assets/ in development).
        def asset(u):
            stem = u.group(2)[1:]
            return u.group(1) + put(os.path.join(app, stem), "build/" + stem)
        rest = re.sub(r'data-assets="[^"]*"', lambda a: re.sub(r'([:|=])(/[^\s|"]+)', asset, a.group(0)), rest)
        return f'<script type="module" src="{host_url}" data-app="{name}"{rest} data-wasm="{wasm}" data-wgsl="{wgsl}">'
    text = re.sub(r'<script type="module" src="/coaxial/host.js" data-app="[^"]+"([^>]*)>', tag, text)
    open(os.path.join(site, f), "w", encoding="utf-8").write(text)
    # A directory's page also answers at the bare path (/projects/coax), which
    # the bucket can't map to index.html on its own; upload.py serves it as HTML.
    if os.path.basename(f) == "index.html" and os.path.dirname(f):
        shutil.copy(os.path.join(site, f), os.path.join(site, os.path.dirname(f) + ".page"))
# well-known/X.json is served at /.well-known/X as JSON (upload.py's .wellknown): the phone
# app's universal links (docs/phone-sign-in.md) and nostr.json (NIP-05); other files keep their name.
for f in sorted(os.listdir(os.path.join(app, "well-known"))) if os.path.isdir(os.path.join(app, "well-known")) else []:
    os.makedirs(os.path.join(site, ".well-known"), exist_ok=True)
    shutil.copy(os.path.join(app, "well-known", f), os.path.join(site, ".well-known", f[:-5] + ".wellknown" if f.endswith(".json") else f))
# For agents: llms-full.txt is every public page's Markdown twin in one file, and sitemap.xml lists those pages.
public = sorted(f for f in pages if f.endswith("index.md") and not f.startswith(("admin", "chat", "login", "projects" + os.sep + "private")))
def url(f): return "https://openagents.com/" + os.path.dirname(f).replace(os.sep, "/")
with open(os.path.join(site, "llms-full.txt"), "w", encoding="utf-8") as out:
    out.write(open(os.path.join(app, "llms.txt"), encoding="utf-8").read())
    for f in public: out.write(f"\n\n---\n\nSource: {url(f)}\n\n" + open(os.path.join(app, f), encoding="utf-8").read())
with open(os.path.join(site, "sitemap.xml"), "w", encoding="utf-8") as out:
    out.write('<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n')
    out.write("".join(f"<url><loc>{url(f)}</loc></url>\n" for f in public) + "</urlset>\n")
PY

# Hidden presentations (unlock/), when COAXIAL_UNLOCKS names any: from the
# environment, or this machine's secrets file. The phrases stay here.
secrets="${COAXIAL_UNLOCKS_FILE:-$HOME/work/.secrets/coaxial-unlocks.env}"
if [[ -z "${COAXIAL_UNLOCKS:-}" && -f "$secrets" ]]; then set -a; source "$secrets"; set +a; fi
if [[ -n "${COAXIAL_UNLOCKS:-}" ]]; then
  [[ -d "$here/unlock/node_modules" ]] || (cd "$here/unlock" && bun install --silent)
  bun "$here/unlock/inject.ts" "$stage"
fi

# No comments reach a browser: the staged pages, scripts and shaders lose them
# here, after the hidden presentations are in.
bun "$here/strip.ts" "$stage"
if [[ -n "${COAXIAL_STAGE_TO:-}" ]]; then mkdir -p "$COAXIAL_STAGE_TO" && cp -R "$stage/assets" "$stage/site" "$COAXIAL_STAGE_TO/"; exit 0; fi

if $server; then
  t=$(now)
  gcloud storage buckets describe "gs://$bucket" --project "$project" > /dev/null 2>&1 ||
    gcloud storage buckets create "gs://$bucket" --project "$project" --location "$region" --uniform-bucket-level-access --quiet
  runtime="$(gcloud projects describe "$project" --format='value(projectNumber)')-compute@developer.gserviceaccount.com"
  gcloud storage buckets add-iam-policy-binding "gs://$bucket" --member "serviceAccount:$runtime" --role roles/storage.objectViewer --quiet > /dev/null
  lap bucket "$t"
fi

t=$(now)
python3 -I "$here/upload.py" "$stage" "$bucket" "$(gcloud auth print-access-token)"
lap upload "$t"

if $server; then
  t=$(now)
  mkdir -p "$stage/image/server"
  cp "$here/serve.c" "$here/../../verse/voice-host/ws.c" "$here/../../verse/voice-host/ws.h" "$stage/image/server/"
  sed -i '' 's#"../../verse/voice-host/ws.h"#"ws.h"#' "$stage/image/server/serve.c"
  cp "$here/Dockerfile" "$stage/image/"
  image="$region-docker.pkg.dev/$project/cloud-run-source-deploy/coaxial-serve:$(date +%Y%m%d-%H%M%S)"
  gcloud builds submit "$stage/image" --project "$project" --region "$region" --tag "$image" --quiet \
    --service-account "projects/$project/serviceAccounts/oa-mvp-automation@$project.iam.gserviceaccount.com" \
    --default-buckets-behavior REGIONAL_USER_OWNED_BUCKET > /dev/null
  lap image "$t"
  t=$(now)
  gcloud run deploy "$service" --image "$image" --project "$project" --region "$region" \
    --allow-unauthenticated --port 8080 --cpu 1 --memory 512Mi --min-instances 0 --max-instances 3 \
    --concurrency 200 --execution-environment gen2 \
    --clear-volumes --clear-volume-mounts \
    --add-volume "name=site,type=cloud-storage,bucket=$bucket,readonly=true,mount-options=implicit-dirs;metadata-cache-ttl-secs=0" \
    --add-volume-mount "volume=site,mount-path=/site" --quiet 2>&1 | grep -E "Service URL|ERROR" || true
  lap service "$t"
fi
