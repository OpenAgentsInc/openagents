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
if [[ -n "${COAXIAL_ONLY_PAGE:-}${COAXIAL_ONLY_MODULE:-}${COAXIAL_ASSET_PREFIX:-}" ]]; then
  [[ -n "${COAXIAL_STAGE_TO:-}" && -n "${COAXIAL_ONLY_PAGE:-}" && -n "${COAXIAL_ONLY_MODULE:-}" && -n "${COAXIAL_ASSET_PREFIX:-}" ]] ||
    { echo "deploy: isolated pages require a stage destination, page, module and asset prefix" >&2; exit 2; }
  [[ "$COAXIAL_ONLY_PAGE" == demos/*/index.html ]] ||
    { echo "deploy: isolated staging supports demo pages only" >&2; exit 2; }
fi
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
t=$(now)
# Keep browser checks on the Mac's GPU while page compilation uses the shared
# build hosts. remote.sh maps this checkout's absolute app path on the host.
if [[ -n "${COAXIAL_ONLY_MODULE:-}" ]]; then
  (cd "$here/../.." && "$here/../remote/remote.sh" run -- "$here/../remote/check.sh" module "$COAXIAL_ONLY_MODULE")
else
  (cd "$here/../.." && "$here/../remote/remote.sh" run -- env COAXIAL_MODE=release COAXIAL_PAGES=all "$app/build.sh")
fi
lap build "$t"
# Generated gitignored NIP/Pylon shaders are not synced back when build.sh offloads to a remote host.
if [[ -f "$app/gpu/nip.coax" && ( ! -f "$app/nip01.wgsl" || ! -f "$app/nip-guide.wgsl" || ! -f "$app/pylon.wgsl" ) ]]; then
  (cd "$here/../.." && "$here/tool.sh" "$here/wgsl-module.coax" "$app/gpu/nip.coax" "$app/nip01.wgsl" "$app/gpu/nip.module")
  cmp -s "$app/nip01.wgsl" "$app/nip-guide.wgsl" || cp "$app/nip01.wgsl" "$app/nip-guide.wgsl"
  cmp -s "$app/nip01.wgsl" "$app/pylon.wgsl" || cp "$app/nip01.wgsl" "$app/pylon.wgsl"
fi

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
asset_prefix = os.environ.get("COAXIAL_ASSET_PREFIX", "").strip("/")
assets, site = os.path.join(stage, "assets", asset_prefix), os.path.join(stage, "site", asset_prefix)
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
    return "/" + (asset_prefix + "/" if asset_prefix else "") + named
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
only_page = os.environ.get("COAXIAL_ONLY_PAGE", "")
if only_page:
    if only_page not in pages: raise SystemExit("The selected page is not a public source page")
    pages = [only_page]
    # The shared preview fallback is a dependency even for one noindex page.
    os.makedirs(os.path.join(site, "og"), exist_ok=True)
    shutil.copy(os.path.join(app, "og", "default.jpg"), os.path.join(site, "og", "default.jpg"))
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
            # An absolute HTTPS URL is fetched by the browser, not a source
            # file of this checkout (the regex sees its :// separator).
            if u.group(3).startswith("//"): return u.group(0)
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
        if asset_prefix and m.group(2).startswith("/" + asset_prefix + "/"): return m.group(0)
        src = os.path.join(app, m.group(2)[1:])
        if not os.path.isfile(src): sys.exit(f"deploy: {f} names the script {m.group(2)}, which is not in the app")
        return m.group(1) + put(src, "build/" + m.group(2)[1:]) + m.group(3)
    text = re.sub(r'(<script type="module" src=")(/(?!coaxial/|build/)[^"]+\.js)(")', page_script, text)
    # DOM pages use the same isolated browser SDK as GPU pages. Their host
    # resolves its Wasm and nested shell relative to this script's location.
    if asset_prefix:
        text = text.replace('src="/coaxial/dom-host.js"', f'src="/{asset_prefix}/coaxial/dom-host.js"')
    open(os.path.join(site, f), "w", encoding="utf-8").write(text)
    # A directory's page also answers at the bare path (/projects/coax), which
    # the bucket can't map to index.html on its own; upload.py serves it as HTML.
    if os.path.basename(f) == "index.html" and os.path.dirname(f):
        shutil.copy(os.path.join(site, f), os.path.join(site, os.path.dirname(f) + ".page"))
# The pages the browser draws (coax/web/dom-host.js: /chat, /components, /settings, /worlds, and the panes
# host.js opens over a world) load /coaxial/dom-host.js and /build/NAME.wasm under those fixed names,
# checked on every visit like the pages. Every module whose face answers dom-host (it exports dom_frame)
# ships so, with its function names; a page that names a module the app lacks stops the build. Before
# this none of them shipped, and /chat drew nothing in production.
dom_host = os.path.join(site, "coaxial", "dom-host.js")
os.makedirs(os.path.dirname(dom_host), exist_ok=True)
shutil.copy(os.path.join(tools, "dom-host.js"), dom_host)
# ... and what dom-host.js imports beside it, unbundled: the app shell's glue (./shell.js; host.js's bundle carries
# its own copy), ./tree-commands.js, and ./dom-wire.js with what it imports (sockets and byte storage, loaded only
# for a module that uses them).
todo, held = ["dom-host.js"], {"dom-host.js"}
while todo:
    for dep in re.findall(r"(?:from\s+|import\s*\(?\s*)['\"]\./([A-Za-z0-9_.-]+\.js)['\"]", open(os.path.join(tools, todo.pop()), encoding="utf-8").read()):
        if dep not in held: held.add(dep); shutil.copy(os.path.join(tools, dep), os.path.join(site, "coaxial", dep)); todo.append(dep)
dom_mods = set()
for w in sorted(os.listdir(os.path.join(app, "build"))):
    src = os.path.join(app, "build", w)
    if not w.endswith(".wasm") or not os.path.isfile(src) or b"dom_frame" not in open(src, "rb").read(): continue
    os.makedirs(os.path.join(site, "build"), exist_ok=True)
    shutil.copy(src, os.path.join(site, "build", w))
    if os.path.exists(src + ".syms"): shutil.copy(src + ".syms", os.path.join(site, "build", w + ".syms"))
    dom_mods.add(w[:-len(".wasm")])
for f in pages:
    if not f.endswith(".html"): continue
    text = open(os.path.join(app, f), encoding="utf-8").read()
    named = re.findall(r'src="/coaxial/dom-host\.js" data-app="([^"]+)"', text) + re.findall(r'/coaxial/dom-host\.js\?app=([A-Za-z0-9_-]+)', text)
    for name in named:
        if name not in dom_mods: sys.exit(f"deploy: {f} draws with the module {name}, which the build did not make for dom-host.js")
# Settings, About (apps/openagents-web/settings-web.coax) reads /build/about.txt: the commit this build is
# of and the day it was built. A tree that isn't a git checkout writes none, and the page says so.
import time
rev = subprocess.run(["git", "-C", app, "log", "-1", "--format=%h", "--abbrev=10"], capture_output=True, text=True).stdout.strip()
if re.fullmatch(r"[0-9a-f]{10}", rev):
    os.makedirs(os.path.join(site, "build"), exist_ok=True)
    open(os.path.join(site, "build", "about.txt"), "w").write(f"commit {rev}\ndate {time.strftime('%Y-%m-%d', time.gmtime())}\n")
# The homepage's seed (`openagents project landing`): this build's captured, admitted events, which the
# page reads itself (build/home-seed/<capture>-<part>.txt, named in its Wasm), so a first visit asks
# the relay only for what is newer.
seed = os.path.join(app, "build", "home-seed")
for f in sorted(os.listdir(seed)) if os.path.isdir(seed) else []:
    os.makedirs(os.path.join(site, "build", "home-seed"), exist_ok=True)
    shutil.copy(os.path.join(seed, f), os.path.join(site, "build", "home-seed", f))
# Apps SDK SVG icons (/build/icons/*.svg, components/core/icons.coax) and Geist web fonts + legacy
# icon bundle (/build/zeron/Geist*.ttf, components/skin/mono.coax). DOM pages (/chat, /components,
# /settings, /worlds) fetch these at fixed paths; populate from repo sources if a remote build did
# not sync unchanged files back into app/build/.
repo_root = os.path.abspath(os.path.join(tools, "..", ".."))
icons_src = os.path.join(app, "build", "icons")
if not os.path.isdir(icons_src) or not any(f.endswith(".svg") for f in os.listdir(icons_src)):
    cand = os.path.join(repo_root, "components", "icons", "apps-sdk")
    if os.path.isdir(cand):
        os.makedirs(icons_src, exist_ok=True)
        for f in sorted(os.listdir(cand)):
            if f.endswith(".svg"):
                shutil.copy(os.path.join(cand, f), os.path.join(icons_src, f))
        notice = os.path.join(repo_root, "components", "icons", "NOTICE")
        if os.path.isfile(notice):
            shutil.copy(notice, os.path.join(icons_src, "NOTICE"))
if os.path.isdir(icons_src):
    icons_dst = os.path.join(site, "build", "icons")
    os.makedirs(icons_dst, exist_ok=True)
    for f in sorted(os.listdir(icons_src)):
        src = os.path.join(icons_src, f)
        if os.path.isfile(src):
            shutil.copy(src, os.path.join(icons_dst, f))
zeron_src = os.path.join(app, "build", "zeron")
if not os.path.isdir(zeron_src) or not any(f.endswith(".ttf") for f in os.listdir(zeron_src)):
    fcand = os.path.join(repo_root, "components", "fonts")
    if os.path.isdir(fcand):
        os.makedirs(zeron_src, exist_ok=True)
        for f in sorted(os.listdir(fcand)):
            if f.startswith("Geist") and (f.endswith(".ttf") or f.endswith(".txt")):
                shutil.copy(os.path.join(fcand, f), os.path.join(zeron_src, f))
if os.path.isdir(zeron_src):
    zeron_dst = os.path.join(site, "build", "zeron")
    os.makedirs(zeron_dst, exist_ok=True)
    for f in sorted(os.listdir(zeron_src)):
        src = os.path.join(zeron_src, f)
        if os.path.isfile(src):
            shutil.copy(src, os.path.join(zeron_dst, f))
# What the bundled host fetches beside itself (new URL(PATH, import.meta.url) and lazyMod("./NAME.js")):
# its carriers ../build/NAME.wasm (host-services, host-audio, host-packs, character-host...),
# ../character.wgsl, ./audio-worklet.js, ./pack-worker.js, and the lazy subsystem modules.
# From /coaxial/host.*.js those are /build/NAME.wasm, /character.wgsl and /coaxial/*.js under
# their fixed names (checked on every visit, like the pages).
host_seen, host_todo = set(), [host]
while host_todo:
    cur = host_todo.pop()
    src_text = open(cur, encoding="utf-8").read()
    rels = set(re.findall(r"new URL\(['\"](\.\.?/[A-Za-z0-9/_.-]+)['\"],\s*import\.meta\.url\)", src_text))
    for dep in re.findall(r"(?:lazyMod\(|from\s+|import\s*\(\s*)['\"]\./([A-Za-z0-9_.-]+\.js)['\"]", src_text):
        rels.add("./" + dep)
    for rel in sorted(rels):
        if rel in host_seen: continue
        host_seen.add(rel)
        src, dst = (os.path.join(app, rel[3:]), os.path.join(site, rel[3:])) if rel.startswith("../") else (os.path.join(tools, rel[2:]), os.path.join(site, "coaxial", rel[2:]))
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        shutil.copy(src, dst)
        if rel.startswith("./") and rel.endswith(".js"): host_todo.append(src)
# well-known/apple-app-site-association.json is served without its extension, as JSON (upload.py's
# .wellknown): the phone app's universal links (docs/phone-sign-in.md). Others keep their name (nostr.json, NIP-05).
for f in sorted(os.listdir(os.path.join(app, "well-known"))) if os.path.isdir(os.path.join(app, "well-known")) else []:
    os.makedirs(os.path.join(site, ".well-known"), exist_ok=True)
    shutil.copy(os.path.join(app, "well-known", f), os.path.join(site, ".well-known", f[:-5] + ".wellknown" if f == "apple-app-site-association.json" else f))
# Static SSR books (apps/openagents-web/books-build.coax -> build/books/): stage /books, /books/mono.css,
# and the 9 book roots before og.ts so og.ts validates them while /og stays within its row/byte cap;
# chapter subdirectories stage right after og.ts below so they still pass strip.ts and private-guard.
books_src = os.path.join(app, "build", "books")
if not only_page and os.path.isdir(books_src):
    books_dst = os.path.join(site, "books")
    os.makedirs(books_dst, exist_ok=True)
    for name in ("mono.css", "search.json", "search.js", "index.html", "index.md"):
        src = os.path.join(books_src, name)
        if os.path.isfile(src):
            shutil.copy(src, os.path.join(books_dst, name))
            if name == "index.html":
                shutil.copy(src, os.path.join(site, "books.page"))
    for slug in sorted(os.listdir(books_src)):
        sdir = os.path.join(books_src, slug)
        if not os.path.isdir(sdir): continue
        ddir = os.path.join(books_dst, slug)
        os.makedirs(ddir, exist_ok=True)
        for name in ("index.html", "index.md"):
            src = os.path.join(sdir, name)
            if os.path.isfile(src):
                shutil.copy(src, os.path.join(ddir, name))
                if name == "index.html":
                    shutil.copy(src, os.path.join(books_dst, slug + ".page"))
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
books_dir = "" if os.environ.get("COAXIAL_ONLY_PAGE") else os.path.join(app, "build", "books")
book_urls = []
if books_dir and os.path.isdir(books_dir):
    for b_root, b_dirs, b_files in os.walk(books_dir):
        b_dirs[:] = sorted(b_dirs)
        if "index.html" in b_files:
            rel_b = os.path.relpath(b_root, books_dir).replace(os.sep, "/")
            book_urls.append("https://openagents.com/books" if rel_b == "." else "https://openagents.com/books/" + rel_b)
    book_urls.sort()
with open(os.path.join(site, "llms-full.txt"), "w", encoding="utf-8") as out:
    out.write(open(os.path.join(app, "llms.txt"), encoding="utf-8").read())
    for f in public: out.write(f"\n\n---\n\nSource: {url(f)}\n\n" + open(os.path.join(app, f), encoding="utf-8").read())
    books_md = os.path.join(books_dir, "index.md") if books_dir else ""
    if books_md and os.path.isfile(books_md):
        out.write("\n\n---\n\nSource: https://openagents.com/books\n\n" + open(books_md, encoding="utf-8").read())
with open(os.path.join(site, "sitemap.xml"), "w", encoding="utf-8") as out:
    out.write('<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n')
    out.write("".join(f"<url><loc>{url(f)}</loc></url>\n" for f in public))
    out.write("".join(f"<url><loc>{u}</loc></url>\n" for u in book_urls) + "</urlset>\n")
PY

# Link previews (docs/web/og-tags.md): every page gets the Open Graph and Twitter tags it lacks, and
# og/pages.txt lists them for /og. A page without a title or description, a broken or wrong-size
# image, or a realm or project without its own card refuses the release (COAXIAL_OG_DRAFT=1 stages anyway).
lap stage "$t"
t=$(now); bun "$here/og.ts" "$stage/site${COAXIAL_ASSET_PREFIX:+/$COAXIAL_ASSET_PREFIX}" "$app"; lap og "$t"

# Stage static SSR book chapter pages (complete canonical + OG/Twitter tags emitted by books.coax)
# after og.ts so og/pages.txt stays bounded while every chapter still runs through strip.ts and private-guard.
if [[ -z "${COAXIAL_ONLY_PAGE:-}" && -d "$app/build/books" ]]; then
  t=$(now)
  python3 - "$app/build/books" "$stage/site${COAXIAL_ASSET_PREFIX:+/$COAXIAL_ASSET_PREFIX}/books" <<'PY_BOOKS'
import os, shutil, sys
src_root, dst_root = sys.argv[1:]
os.makedirs(dst_root, exist_ok=True)
for slug in sorted(os.listdir(src_root)):
    sdir = os.path.join(src_root, slug)
    if not os.path.isdir(sdir): continue
    for ch in sorted(os.listdir(sdir)):
        cdir = os.path.join(sdir, ch)
        if not os.path.isdir(cdir): continue
        ddir = os.path.join(dst_root, slug, ch)
        os.makedirs(ddir, exist_ok=True)
        for name in ("index.html", "index.md"):
            src = os.path.join(cdir, name)
            if os.path.isfile(src):
                shutil.copy(src, os.path.join(ddir, name))
                if name == "index.html":
                    shutil.copy(src, os.path.join(dst_root, slug, ch + ".page"))
PY_BOOKS
  lap books "$t"
fi

# Hidden presentations (unlock/), when COAXIAL_UNLOCKS names any: from the
# environment, or this machine's secrets file. The phrases stay here.
secrets="${COAXIAL_UNLOCKS_FILE:-$HOME/work/.secrets/coaxial-unlocks.env}"
if [[ -z "${COAXIAL_ONLY_PAGE:-}" && -z "${COAXIAL_UNLOCKS:-}" && -f "$secrets" ]]; then set -a; source "$secrets"; set +a; fi
if [[ -z "${COAXIAL_ONLY_PAGE:-}" && -n "${COAXIAL_UNLOCKS:-}" ]]; then
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
if [[ -z "${COAXIAL_ONLY_PAGE:-}" ]]; then
  t=$(now); "$here/tool.sh" engine-served write "$stage/site"; lap served "$t"
fi
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
