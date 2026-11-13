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

# INVARIANTS.md: everything visible is Coax. release.sh runs it as a report
# (COAX_VISUALS=report): only what breaks staging against production stops a release.
t=$(now)
if [[ "${COAX_VISUALS:-}" == report ]]; then "$here/check-visuals.sh" "$app" || echo "deploy: check-visuals fails (a report here)" >&2
else "$here/check-visuals.sh" "$app"; fi
lap visuals "$t"
# COAXIAL_PAGES=all: a release ships every page, the old set too (apps/openagents-web/build.sh, #13269).
t=$(now); COAXIAL_MODE=release COAXIAL_PAGES=all "$app/build.sh"; lap build "$t"

stage="$(mktemp -d)"; trap 'rm -rf "$stage"' EXIT
# Bundle host modules before content naming; deployed pages need one immutable
# script, while coax serve can load the source modules from /coaxial/ directly.
t=$(now)
bun build "$here/host.js" --target browser --format esm --outfile "$stage/host.js" > /dev/null
lap host "$t"
t=$(now)
# The page, index.html at the bucket's root as the CDN serves it. The wasm,
# shaders and host script are named by their content (cached for good); the
# page names them (data-wasm, data-wgsl) and is checked on every visit.
python3 - "$app" "$stage/host.js" "$stage" "$here" <<'PY'
import functools, hashlib, json, os, re, shutil, subprocess, sys
app, host, stage, tools = sys.argv[1:]
from pathlib import Path
sys.path.insert(0, tools)
# Many pages name the same module, shader and render package: each is hashed, staged and
# asked of its Coax tool once, not once a page (#13154: staging took 18 s of a release).
@functools.lru_cache(maxsize=None)
def stage_package(app, url, stage):
    return subprocess.check_output([os.path.join(tools, "tool.sh"), "render-stage", str(app), url, str(stage)], text=True).strip()
assets, site = os.path.join(stage, "assets"), os.path.join(stage, "site")
@functools.lru_cache(maxsize=None)
def shader_path(name, declared):
    return subprocess.check_output([os.path.join(tools, "tool.sh"), "shader-path", name, declared], text=True).strip().lstrip("/")
@functools.lru_cache(maxsize=None)
def put(src, rel):
    h = hashlib.sha256(open(src, "rb").read()).hexdigest()[:12]
    stem, ext = os.path.splitext(rel)
    # a gzipped pack keeps .pack.gz at its end: host.js unzips by that ending
    if rel.endswith(".pack.gz"): stem, ext = rel[:-len(".pack.gz")], ".pack.gz"
    named = f"{stem}.{h}{ext}"
    for root in (assets, site):
        os.makedirs(os.path.dirname(os.path.join(root, named)), exist_ok=True)
        shutil.copy(src, os.path.join(root, named))
        # a module's function names ship beside it under its content's name (wasm.sh, #13298): host.js
        # asks for the module's URL + ".syms" only after a trap (docs/deployment/crash-reports.md)
        if rel.endswith(".wasm") and os.path.exists(src + ".syms"): shutil.copy(src + ".syms", os.path.join(root, named + ".syms"))
    return "/" + named
host_url = put(host, "coaxial/host.js")
os.makedirs(site, exist_ok=True)
# Pages at any depth (projects/coax/index.html is /projects/coax): pages, their
# Markdown twins, icons and link-preview cards (.jpg). Work files, builds and sources stay behind.
pages = []
for root, dirs, files in os.walk(app):
    # A directory holding a DEV-ONLY file is a page for development only: never released.
    dirs[:] = sorted(d for d in dirs if d not in ("build", "work", "node_modules") and not d.startswith(".")
                     and not os.path.exists(os.path.join(root, d, "DEV-ONLY")))
    for f in sorted(files):
        rel = os.path.relpath(os.path.join(root, f), app)
        # a realm's reference for agents, generated with the build (coax/web/agent-reference.coax, #13430)
        agent = (rel.startswith("realms" + os.sep) and f == "llms.txt") or rel in ("llms.txt", "robots.txt", "pylon" + os.sep + "join.sh") or (rel.startswith("cli" + os.sep) and f.endswith((".sh", ".ps1"))) or rel == "docs/connect-a-pylon/confidential-round.json"
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
        # Coax selects the repository page's declared shader; this wrapper copies its bytes.
        declared_shader = re.search(r'\bdata-wgsl="([^"]+)"', m.group(0))
        shader_source = shader_path(name, declared_shader.group(1) if declared_shader else "")
        wgsl = put(os.path.join(app, shader_source), "build/" + shader_source)
        rest = re.sub(r'\sdata-wgsl="[^"]+"', "", m.group(1))
        # The page's UI font (data-ui-font-src, realms.json ui_font_src): named by its content too, or it isn't on the site.
        rest = re.sub(r'data-ui-font-src="(/[^"]+)"', lambda r: 'data-ui-font-src="' + put(os.path.join(app, r.group(1)[1:]), "build/" + r.group(1)[1:]) + '"', rest)
        # A HUD style's title face (data-ui-title-font-src, realms.json ui_title_font_src) ships the same way.
        rest = re.sub(r'data-ui-title-font-src="(/[^"]+)"', lambda r: 'data-ui-title-font-src="' + put(os.path.join(app, r.group(1)[1:]), "build/" + r.group(1)[1:]) + '"', rest)
        rest = re.sub(r'data-render="([^"]+)"', lambda r: 'data-render="' + stage_package(Path(app), r.group(1), Path(stage)) + '"', rest)
        # A scene's assets (data-assets): each URL named by its content too,
        # under /build/ (the page's directory's assets/ in development). A local: asset stays as it
        # is: only a local address loads it, from a cache outside the repo (host.js), so it never ships.
        def asset(u):
            if u.group(1): return u.group(0)
            # An asset under /private/ never ships (private-guard below): the page keeps its address,
            # which only a local address with the owner's private build serves (Valley World's detail pages).
            if u.group(3).startswith("/private/"): return u.group(0)
            # An asset the API answers (/api/worlds/ID/assets/pack, a world's imports, #13420) is no file
            # of the app: the page asks the API for it as it runs, so its address stays as it is.
            if u.group(3).startswith("/api/"): return u.group(0)
            stem = u.group(3)[1:]
            # A sounds:/DIR/ asset: the page names its files at run time (DIR/NAME.ogg), so they
            # keep their names, under the site at the same path (metaverse-engine/audio/SOURCES.md).
            if stem.endswith("/"):
                for f in sorted(os.listdir(os.path.join(app, stem))):
                    if f.endswith(".ogg"):
                        os.makedirs(os.path.join(site, stem), exist_ok=True)
                        shutil.copy(os.path.join(app, stem, f), os.path.join(site, stem, f))
                return u.group(0)
            return u.group(2) + put(os.path.join(app, stem), "build/" + stem)
        rest = re.sub(r'data-assets="[^"]*"', lambda a: re.sub(r'(\blocal)?([:|=])(/[^\s|"]+)', asset, a.group(0)), rest)
        # A page that talks (data-voice): the voice engine (coax/web/voice/build.sh), named by its content too.
        if "data-voice=" in rest: rest += f' data-voice-wasm="{put(os.path.join(app, "build", "voice.wasm"), "build/voice.wasm")}"'
        # Everglade's offline room (#12784): the room server's simulation for the local host (coax/web/room/worker.coax).
        if name == "everglade5e": rest += f' data-room-wasm="{put(os.path.join(app, "build", "everglade-room.wasm"), "build/everglade-room.wasm")}"'
        # The engine is the page's biggest file and host.js asks for it only once it has run:
        # preloaded, it downloads beside the script (crossorigin: the same request host.js's fetch makes).
        return f'<link rel="preload" href="{wasm}" as="fetch" crossorigin><script type="module" src="{host_url}" data-app="{name}"{rest} data-wasm="{wasm}" data-wgsl="{wgsl}">'
    text = re.sub(r'<script type="module" src="/coaxial/host.js" data-app="[^"]+"([^>]*)>', tag, text)
    # A page's own script (/login/key.js: passkeys and NIP-07 beside the engine) ships named by its
    # content too. Without this the page asked for a file the site never had, and its passkey and
    # key buttons did nothing. A script the page names and the app lacks stops the build.
    def page_script(m):
        src = os.path.join(app, m.group(2)[1:])
        if not os.path.isfile(src): sys.exit(f"deploy: {f} names the script {m.group(2)}, which is not in the app")
        return m.group(1) + put(src, "build/" + m.group(2)[1:]) + m.group(3)
    text = re.sub(r'(<script type="module" src=")(/(?!coaxial/|build/)[^"]+\.js)(")', page_script, text)
    open(os.path.join(site, f), "w", encoding="utf-8").write(text)
    # A directory's page also answers at the bare path (/projects/coax), which
    # the bucket can't map to index.html on its own; upload.py serves it as HTML.
    if os.path.basename(f) == "index.html" and os.path.dirname(f):
        shutil.copy(os.path.join(site, f), os.path.join(site, os.path.dirname(f) + ".page"))
# The homepage's seed (`openagents project landing`): this build's captured, admitted events, which the
# page reads itself (build/home-seed/<capture>-<part>.txt, named in its Wasm), so a first visit asks
# the relay only for what is newer.
seed = os.path.join(app, "build", "home-seed")
for f in sorted(os.listdir(seed)) if os.path.isdir(seed) else []:
    os.makedirs(os.path.join(site, "build", "home-seed"), exist_ok=True)
    shutil.copy(os.path.join(seed, f), os.path.join(site, "build", "home-seed", f))
# What the bundled host fetches beside itself (new URL(PATH, import.meta.url)): its carriers
# ../build/NAME.wasm (host-services, host-audio, character-host...), ../character.wgsl and
# ./audio-worklet.js. From /coaxial/host.*.js those are /build/NAME.wasm, /character.wgsl and
# /coaxial/audio-worklet.js, under their fixed names (checked on every visit, like the pages).
for rel in sorted(set(re.findall(r"new URL\(\"(\.\.?/[A-Za-z0-9/_.-]+)\", import\.meta\.url\)", open(host, encoding="utf-8").read()))):
    src, dst = (os.path.join(app, rel[3:]), os.path.join(site, rel[3:])) if rel.startswith("../") else (os.path.join(tools, rel[2:]), os.path.join(site, "coaxial", rel[2:]))
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    shutil.copy(src, dst)
    # a worklet's own imports (./audio-bridge.js) come from beside it, unbundled
    todo = [rel[2:]] if rel.startswith("./") and rel.endswith(".js") else []
    while todo:
        for dep in re.findall(r"from\s+['\"]\./([A-Za-z0-9_.-]+\.js)['\"]", open(os.path.join(tools, todo.pop()), encoding="utf-8").read()):
            if not os.path.exists(os.path.join(site, "coaxial", dep)): shutil.copy(os.path.join(tools, dep), os.path.join(site, "coaxial", dep)); todo.append(dep)
# well-known/apple-app-site-association.json is served without its extension, as JSON (upload.py's
# .wellknown): the phone app's universal links (docs/phone-sign-in.md). Others keep their name (nostr.json, NIP-05).
for f in sorted(os.listdir(os.path.join(app, "well-known"))) if os.path.isdir(os.path.join(app, "well-known")) else []:
    os.makedirs(os.path.join(site, ".well-known"), exist_ok=True)
    shutil.copy(os.path.join(app, "well-known", f), os.path.join(site, ".well-known", f[:-5] + ".wellknown" if f == "apple-app-site-association.json" else f))
# For agents: llms-full.txt is every public page's Markdown twin in one file, and sitemap.xml lists those pages.
# Coax projects the registry's hidden directories. Direct realm pages ship;
# neither those pages nor their child pages enter agent/search discovery.
registry = os.path.join(app, "realms.json")
hidden_realms = subprocess.check_output([os.path.join(tools, "tool.sh"), "discovery-realms", registry], text=True).splitlines() if os.path.isfile(registry) else []
def unlisted(f):
    if any(f.replace(os.sep, "/").startswith(prefix) for prefix in hidden_realms): return True
    if os.path.exists(os.path.join(app, os.path.dirname(f), "UNLISTED")): return True   # an unlisted page: shipped, never listed
    j = os.path.join(app, os.path.dirname(f), "project.json")
    return f.startswith("projects" + os.sep) and os.path.exists(j) and json.load(open(j, encoding="utf-8")).get("visibility") in ("unlisted", "private")
public = sorted(f for f in pages if f.endswith("index.md") and not f.startswith(("admin", "chat", "login", "settings", "og" + os.sep, "projects" + os.sep + "private")) and not unlisted(f))
def url(f): return "https://openagents.com/" + os.path.dirname(f).replace(os.sep, "/")
with open(os.path.join(site, "llms-full.txt"), "w", encoding="utf-8") as out:
    out.write(open(os.path.join(app, "llms.txt"), encoding="utf-8").read())
    for f in public: out.write(f"\n\n---\n\nSource: {url(f)}\n\n" + open(os.path.join(app, f), encoding="utf-8").read())
with open(os.path.join(site, "sitemap.xml"), "w", encoding="utf-8") as out:
    out.write('<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n')
    out.write("".join(f"<url><loc>{url(f)}</loc></url>\n" for f in public) + "</urlset>\n")
PY

# Link previews (docs/web/og-tags.md): every page gets the Open Graph and Twitter tags it lacks, and
# og/pages.txt lists them for /og. A page without a title or description, a broken or wrong-size
# image, or a realm or project without its own card refuses the release (COAXIAL_OG_DRAFT=1 stages anyway).
lap stage "$t"
t=$(now); bun "$here/og.ts" "$stage/site" "$app"; lap og "$t"

# Hidden presentations (unlock/), when COAXIAL_UNLOCKS names any: from the
# environment, or this machine's secrets file. The phrases stay here.
secrets="${COAXIAL_UNLOCKS_FILE:-$HOME/work/.secrets/coaxial-unlocks.env}"
if [[ -z "${COAXIAL_UNLOCKS:-}" && -f "$secrets" ]]; then set -a; source "$secrets"; set +a; fi
if [[ -n "${COAXIAL_UNLOCKS:-}" ]]; then
  [[ -d "$here/unlock/node_modules" ]] || (cd "$here/unlock" && bun install --silent)
  # COAXIAL_UNLOCK_SIGNER=coax: the opener signs, verifies and opens NIP-44 with unlock-nostr.coax (#12630);
  # unset, nostr-effect as before (the rollback).
  if [[ "${COAXIAL_UNLOCK_SIGNER:-}" == coax ]]; then
    export COAXIAL_UNLOCK_NOSTR_WASM="$(mktemp -d)/unlock-nostr.wasm"
    "$here/wasm.sh" "$here/unlock/unlock-nostr.coax" "$here/unlock/unlock-nostr-face.c" "$COAXIAL_UNLOCK_NOSTR_WASM"
  fi
  bun "$here/unlock/inject.ts" "$stage"
fi

# No comments reach a browser: the staged pages, scripts and shaders lose them
# here, after the hidden presentations are in.
t=$(now); bun "$here/strip.ts" "$stage"; lap strip "$t"
# No private pack ships in a release (a MetaHuman reaches a visitor only through the broker,
# coax/web/api/verse_packs.c): a file under private/ or a pack naming the MetaHuman licence refuses it.
t=$(now); "$here/tool.sh" private-guard "$stage"; lap guard "$t"
# The made-realm engine's script gains its served files' own hashes, after the strip changed their
# bytes (engine-served.coax, #12629): a publish pins them with the script (migration 0107).
t=$(now); "$here/tool.sh" engine-served write "$stage/site"; lap served "$t"
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
# Each release's function names are kept apart from the site (the site keeps only this release's
# files), so a crash from an older page still resolves (symbols.sh, #13298). Never stops a release.
t=$(now); "$here/symbols.sh" pages "$stage/site" || echo "deploy: this release's symbols were not kept (a report here)" >&2; lap symbols "$t"

if $server; then
  t=$(now)
  # serve.c, ws.c and every Coax part serve.c includes, and the Dockerfile: one list (#13447)
  COAX="$here/../bin/coax" "$here/serve-parts.sh" --image "$stage/image"
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
