#!/usr/bin/env bash
# Smoke checks for a deployed Coaxial site, run by release.sh after each
# stage. Fails (exit 1) on the first broken promise and says which.
#   smoke.sh https://staging.openagents.com
#   smoke.sh https://openagents.com --via 136.69.24.60   # before DNS points there
#   smoke.sh https://openagents.com --coder              # also: coder's routes still answer
# Checks: the page and every file it names (status, type, caching, wasm
# magic, no comments shipped), the hidden text agents read, HTTP to HTTPS, an unknown path, and
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

pages = ["/projects", "/projects/index.md", "/projects/coax", "/projects/coax/index.md", "/projects/coax/demo", "/projects/verse/index.md"]
moved = ["/project/coax", "/project/coax/index.md", "/project/verse/index.md"]   # the old addresses: 301 to /projects/*
extra = [("/llms.txt", {200}), ("/index.md", {200}), ("/v1/projects", {401, 403}), ("/docs", {200, 301, 302, 308})] if coder else [("/" + uuid.uuid4().hex, {404})]
first = fetch(["/", ("/", "GET", "http"), "/realm/haunted-house", "/realm/haunted-house/index.md"] + pages + moved + [p for p, _ in extra])
(s, h, page), redirect, house, house_md = first[:4]
s_pages = first[4:4 + len(pages)]
s_moved = first[4 + len(pages):4 + len(pages) + len(moved)]
s_extra = first[4 + len(pages) + len(moved):]

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
print(f"smoke {base}{' via ' + via if via else ''}: " + ("passed" if not failed else f"{len(failed)} failed"))
sys.exit(1 if failed else 0)
PY
