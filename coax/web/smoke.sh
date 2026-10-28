#!/usr/bin/env bash
# Smoke checks for a deployed Coaxial site, run by release.sh after each
# stage. Fails (exit 1) on the first broken promise and says which.
#   smoke.sh https://staging.openagents.com
#   smoke.sh https://openagents.com --via 136.69.24.60   # before DNS points there
#   smoke.sh https://openagents.com --coder              # also: coder's routes still answer
# Checks: the page and every file it names (status, type, caching, wasm
# magic), the hidden text agents read, HTTP to HTTPS, an unknown path, and
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
import re, ssl, sys, socket, http.client, uuid
base, host, via, coder = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4] == "true"
failed = []
def get(path, method="GET", scheme="https"):
    ip = via or host
    if scheme == "https":
        ctx = ssl.create_default_context()
        c = http.client.HTTPSConnection(ip, 443, timeout=15, context=ctx)
        if via:   # connect to the IP, but check the certificate for the host
            c = http.client.HTTPSConnection(host, 443, timeout=15, context=ctx)
            c.sock = ctx.wrap_socket(socket.create_connection((via, 443), timeout=15), server_hostname=host)
    else:
        c = http.client.HTTPConnection(ip, 80, timeout=15)
    c.request(method, path, headers={"Host": host, "User-Agent": "coaxial-smoke"})
    r = c.getresponse(); body = r.read()
    return r.status, {k.lower(): v for k, v in r.getheaders()}, body
def check(ok, what):
    print(("  ok    " if ok else "  FAIL  ") + what)
    if not ok: failed.append(what)

s, h, page = get("/")
check(s == 200 and h.get("content-type", "").startswith("text/html"), f"/ is a page ({s})")
check("no-cache" in h.get("cache-control", ""), "/ is checked on every visit (no-cache)")
html = page.decode("utf-8", "replace")
check('data-app="splash"' in html, "/ runs the splash app")
check(re.search(r"<main[^>]*>\s*\S[\s\S]*?</main>", html) is not None and "<title>" in html, "/ keeps its title and the hidden text for agents")
files = set(re.findall(r'(?:src|data-wasm|data-wgsl)="(/[^"]+)"', html))
check(len(files) >= 3, f"/ names its host, wasm and shader ({len(files)} files)")
kinds = {".wasm": "application/wasm", ".js": "javascript", ".wgsl": ""}
for f in sorted(files):
    s, h, body = get(f)
    ext = "." + f.rsplit(".", 1)[-1]
    typ = h.get("content-type", "")
    check(s == 200 and kinds.get(ext, "") in typ, f"{f} ({s}, {typ or 'no type'})")
    if re.search(r"\.[0-9a-f]{12}\.", f):
        check("immutable" in h.get("cache-control", ""), f"{f} is cached for good")
    if ext == ".wasm":
        check(body[:4] == b"\0asm", f"{f} is a wasm module ({len(body)} bytes)")
s, h, _ = get("/", scheme="http")
check(s in (301, 308) and h.get("location", "").startswith("https://"), f"http:// redirects to https:// ({s})")
for path in ("/project/coax", "/project/coax/index.md", "/project/coax/demo"):
    s, h, body = get(path)
    ok = s == 200 and (path.endswith(".md") or b'data-app="project"' in body)
    check(ok, f"{path} ({s})")
# The Haunted House: its page, its Markdown, and every asset it names (the
# house's document, geometry and sheets; sounds and their loop).
s, h, body = get("/realm/haunted-house")
check(s == 200 and b'data-app="realm"' in body, f"/realm/haunted-house ({s})")
s, _, _ = get("/realm/haunted-house/index.md")
check(s == 200, f"/realm/haunted-house/index.md ({s})")
named = re.search(r'data-assets="([^"]*)"', body.decode("utf-8", "replace"))
urls = re.findall(r'(/build/[^\s|"]+)', named.group(1)) if named else []
check(len(urls) >= 16, f"/realm/haunted-house names its assets ({len(urls)})")
for u in urls:
    s, h, _ = get(u, method="HEAD")
    check(s == 200 and "immutable" in h.get("cache-control", ""), f"{u} ({s})")
if coder:
    for path, want in [("/llms.txt", {200}), ("/index.md", {200}), ("/v1/projects", {401, 403}), ("/docs", {200, 301, 302, 308})]:
        s, _, _ = get(path)
        check(s in want, f"coder still answers {path} ({s})")
else:
    s, _, body = get("/" + uuid.uuid4().hex)
    check(s == 404 and b'data-page="1"' in body, f"an unknown path gets the Coax 404 ({s})")
print(f"smoke {base}{' via ' + via if via else ''}: " + ("passed" if not failed else f"{len(failed)} failed"))
sys.exit(1 if failed else 0)
PY
