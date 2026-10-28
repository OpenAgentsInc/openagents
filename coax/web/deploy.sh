#!/usr/bin/env bash
# coax deploy, for now. An app's files live in a storage bucket that its
# Cloud Run service mounts, so a deploy is a release build and an upload:
# no image build and no new revision.
#
#   deploy.sh APP_DIR SERVICE           # build in release, upload (seconds)
#   deploy.sh --server APP_DIR SERVICE  # also (re)build the server image and
#                                       # (re)create the service and its bucket
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
now() { python3 -c 'import time; print(time.time())'; }
lap() { printf '  %-8s %4.1f s\n' "$1" "$(python3 -c "print($(now) - $2)")"; }

t=$(now); COAXIAL_MODE=release "$app/build.sh"; lap build "$t"

stage="$(mktemp -d)"; trap 'rm -rf "$stage"' EXIT
# The page, index.html at the bucket's root as the CDN serves it. The wasm,
# shaders and host script are named by their content (cached for good); the
# page names them (data-wasm, data-wgsl) and is checked on every visit.
python3 - "$app" "$here/host.js" "$stage" <<'PY'
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
for f in sorted(os.listdir(app)):
    path = os.path.join(app, f)
    if not os.path.isfile(path) or f == "README.md" or not f.endswith((".html", ".md", ".svg")): continue
    text = open(path, encoding="utf-8").read() if f.endswith(".html") else None
    if text is None: shutil.copy(path, os.path.join(site, f)); continue
    def tag(m):
        name = re.search(r'data-app="([^"]+)"', m.group(0)).group(1)
        wasm = put(os.path.join(app, "build", name + ".wasm"), f"build/{name}.wasm")
        wgsl = put(os.path.join(app, name + ".wgsl"), f"{name}.wgsl")
        return f'<script type="module" src="{host_url}" data-app="{name}" data-wasm="{wasm}" data-wgsl="{wgsl}">'
    text = re.sub(r'<script type="module" src="/coaxial/host.js" data-app="[^"]+">', tag, text)
    open(os.path.join(site, f), "w", encoding="utf-8").write(text)
PY

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
  cp "$here/serve.c" "$here/../../metareal/voice-host/ws.c" "$here/../../metareal/voice-host/ws.h" "$stage/image/server/"
  sed -i '' 's#"../../metareal/voice-host/ws.h"#"ws.h"#' "$stage/image/server/serve.c"
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
