#!/usr/bin/env bash
# Smoke checks for a deployed Coaxial site, run by release.sh after each
# stage. Fails (exit 1) on the first broken promise and says which.
#   smoke.sh https://staging.openagents.com
#   smoke.sh https://openagents.com --via 136.69.24.60   # before DNS points there
#   smoke.sh https://openagents.com --coder              # also: coder's routes still answer
# Checks: the page and every file it names (status, type, caching, wasm
# magic, no comments shipped), the hidden text agents read, HTTP to HTTPS, an unknown path, an unknown
# private project (the locked page; its API says only not found), and
# with --coder, the routes the old service keeps until it is retired
# (docs/2026-10-28-coder-service-deprecation.md).
set -euo pipefail
base="${1%/}"; shift
via="" coder=false
while (($#)); do
  case "$1" in
    --via) via="$2"; shift 2 ;;
    --coder) coder=true; shift ;;
    *) echo "smoke: unknown option $1" >&2; exit 2 ;;
  esac
done
host="${base#https://}"
exec python3 -I - "$base" "$host" "$via" "$coder" <<'PY'
import re, ssl, sys, socket, threading, http.client, uuid
from concurrent.futures import ThreadPoolExecutor
base, host, via, coder = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4] == "true"
failed = []
ctx = ssl.create_default_context()
local = threading.local()   # one kept-alive connection per thread and scheme
def connect(scheme):
    if scheme == "http":
        return http.client.HTTPConnection(via or host, 80, timeout=15)
    c = http.client.HTTPSConnection(host, 443, timeout=15, context=ctx)
    if via:   # connect to the IP, but check the certificate for the host
        c.sock = ctx.wrap_socket(socket.create_connection((via, 443), timeout=15), server_hostname=host)
    return c
def get(path, method="GET", scheme="https"):
    for attempt in (0, 1):
        c = getattr(local, scheme, None) or connect(scheme)
        setattr(local, scheme, c)
        try:
            c.request(method, path, headers={"Host": host, "User-Agent": "coaxial-smoke"})
            r = c.getresponse(); body = r.read()
            if r.getheader("connection", "").lower() == "close": c.close(); setattr(local, scheme, None)
            return r.status, {k.lower(): v for k, v in r.getheaders()}, body
        except (http.client.HTTPException, OSError):
            c.close(); setattr(local, scheme, None)
            if attempt: raise
pool = ThreadPoolExecutor(16)
def fetch(reqs):   # every request at once; answers in the order asked
    return list(pool.map(lambda q: get(*q) if isinstance(q, tuple) else get(q), reqs))
def check(ok, what):
    print(("  ok    " if ok else "  FAIL  ") + what)
    if not ok: failed.append(what)

pages = ["/projects", "/projects/index.md", "/projects/coax", "/projects/coax/index.md", "/projects/coax/demo", "/projects/verse", "/projects/verse/index.md", "/projects/planetary-villa", "/projects/planetary-villa/index.md"]
moved = ["/project/coax", "/project/coax/index.md", "/project/verse/index.md"]   # the old addresses: 301 to /projects/*
extra = [("/llms.txt", {200}), ("/index.md", {200}), ("/v1/projects", {401, 403}), ("/docs", {200, 301, 302, 308})] if coder else [("/" + uuid.uuid4().hex, {404})]
verse = ["/verse", "/verse/index.md", "/verse/galaxy", "/verse/galaxy/index.md"]   # the playtest's landing and galaxy
first = fetch(["/", ("/", "GET", "http"), "/realm/haunted-house", "/realm/haunted-house/index.md"] + pages + moved + [p for p, _ in extra] + verse)
(s, h, page), redirect, house, house_md = first[:4]
s_pages = first[4:4 + len(pages)]
s_moved = first[4 + len(pages):4 + len(pages) + len(moved)]
s_extra = first[4 + len(pages) + len(moved):4 + len(pages) + len(moved) + len(extra)]
s_verse = first[4 + len(pages) + len(moved) + len(extra):]

check(s == 200 and h.get("content-type", "").startswith("text/html"), f"/ is a page ({s})")
check("no-cache" in h.get("cache-control", ""), "/ is checked on every visit (no-cache)")
html = page.decode("utf-8", "replace")
check('data-app="splash"' in html, "/ runs the splash app")
check("<!--" not in html and "/*" not in html, "/ ships no comments")
check(re.search(r"<main[^>]*>\s*\S[\s\S]*?</main>", html) is not None and "<title>" in html, "/ keeps its title and the hidden text for agents")
files = sorted(set(re.findall(r'(?:src|data-wasm|data-wgsl)="(/[^"]+)"', html)))
check(len(files) >= 3, f"/ names its host, wasm and shader ({len(files)} files)")
body = house[2]
named = re.search(r'data-assets="([^"]*)"', body.decode("utf-8", "replace"))
urls = re.findall(r'(/build/[^\s|"]+)', named.group(1)) if named else []
got = fetch(files + [(u, "HEAD") for u in urls])
kinds = {".wasm": "application/wasm", ".js": "javascript", ".wgsl": ""}
for f, (s, h, body) in zip(files, got):
    ext = "." + f.rsplit(".", 1)[-1]
    typ = h.get("content-type", "")
    check(s == 200 and kinds.get(ext, "") in typ, f"{f} ({s}, {typ or 'no type'})")
    if re.search(r"\.[0-9a-f]{12}\.", f):
        check("immutable" in h.get("cache-control", ""), f"{f} is cached for good")
    if ext == ".js":
        check(b"/*" not in body and not re.search(rb"(^|[;{}\n])\s*//", body), f"{f} ships no comments")
    if ext == ".wgsl":
        check(b"//" not in body and b"/*" not in body, f"{f} ships no comments")
    if ext == ".wasm":
        check(body[:4] == b"\0asm", f"{f} is a wasm module ({len(body)} bytes)")
s, h, _ = redirect
check(s in (301, 308) and h.get("location", "").startswith("https://"), f"http:// redirects to https:// ({s})")
for path, (s, h, body) in zip(pages, s_pages):
    app = b'data-app="projects"' if path == "/projects" else b'data-app="project"'
    ok = s == 200 and (path.endswith(".md") or (app in body and b"<!--" not in body))
    check(ok, f"{path} ({s})")
    # a real project page names its project (key and d) for the page's own line to the relay
    if path.count("/") == 2 and not path.endswith(".md"):
        slug = path.rsplit("/", 1)[1]
        check(re.search(rb'data-text="[0-9a-f]{64} ' + slug.encode() + rb'"', body) is not None, f"{path} names its project")
# The old /project/* addresses move for good to /projects/*, path kept.
for path, (s, h, _) in zip(moved, s_moved):
    want = "/projects/" + path[len("/project/"):]
    where = h.get("location", "")
    check(s == 301 and where in (want, f"https://{host}{want}"), f"{path} moves to {want} ({s} {where})")
# The Haunted House: its page, its Markdown, and every asset it names (the
# house's document, geometry and sheets; sounds and their loop).
check(house[0] == 200 and b'data-app="realm"' in house[2], f"/realm/haunted-house ({house[0]})")
check(house_md[0] == 200, f"/realm/haunted-house/index.md ({house_md[0]})")
check(len(urls) >= 16, f"/realm/haunted-house names its assets ({len(urls)})")
for u, (s, h, _) in zip(urls, got[len(files):]):
    check(s == 200 and "immutable" in h.get("cache-control", ""), f"{u} ({s})")
for (path, want), (s, _, body) in zip(extra, s_extra):
    if coder:
        check(s in want, f"coder still answers {path} ({s})")
    else:
        check(s in want and b'data-page="1"' in body, f"an unknown path gets the Coax 404 ({s})")
# The Verse's way in: the landing (page 0) and the galaxy (page 1), one app, and their Markdown.
for path, (s, h, body) in zip(verse, s_verse):
    if path.endswith(".md"):
        check(s == 200 and body.startswith(b"# "), f"{path} ({s})")
    else:
        want = b'data-page="1"' if path.endswith("galaxy") else b'data-page="0"'
        ok = s == 200 and b'data-app="verse"' in body and want in body and b"<!--" not in body
        check(ok, f"{path} runs the verse app ({s})")
        if ok:
            for f in sorted(set(re.findall(rb'(?:data-wasm|data-wgsl)="(/[^"]+)"', body))):
                fs, fh, fb = get(f.decode())
                check(fs == 200 and (not f.endswith(b".wasm") or fb[:4] == b"\0asm"), f"{path}: {f.decode()} ({fs})")
        cards = re.findall(rb'<meta (?:property="og:image"|name="twitter:image") content="https://openagents\.com(/[^"]+)">', body)
        check(len(cards) == 2 and b'name="twitter:card" content="summary_large_image"' in body, f"{path} names its link-preview card")
# The link-preview card those tags name: a JPEG, served without a script.
cs, ch, cb = get("/verse/og-verse.jpg")
check(cs == 200 and ch.get("content-type", "").startswith("image/jpeg") and cb[:3] == b"\xff\xd8\xff", f"/verse/og-verse.jpg is the card ({cs}, {ch.get('content-type')})")
# Private projects (docs/private-projects.md): the generic locked page, and
# a slug nobody made answers like one nobody may see: 404, no name, no data.
if not coder:
    nobody = uuid.uuid4().hex[:20]
    (s1, h1, b1), (s2, h2, b2), (s3, h3, b3) = fetch(["/projects/private", f"/projects/{nobody}", f"/api/private/{nobody}/state"])
    check(s1 == 200 and b'data-app="private"' in b1 and b'data-page="1"' in b1, f"/projects/private is the locked page ({s1})")
    # 404 naming nothing; the locked page itself once the load balancer's error policy serves it (lb.sh private-pages)
    check(s2 == 404 and b'data-page="1"' in b2 and nobody.encode() not in b2.split(b"<script", 1)[0], f"an unknown private slug is a 404 that names nothing ({s2})")
    check(s3 == 404 and b3 == b'{"error": "not found"}' and "no-store" in h3.get("cache-control", ""), f"an unknown private slug's API says only not found ({s3})")
print(f"smoke {base}{' via ' + via if via else ''}: " + ("passed" if not failed else f"{len(failed)} failed"))
sys.exit(1 if failed else 0)
PY
