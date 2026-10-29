"""Uploads a staged site to a bucket fast: one listing, then only the files
whose bytes changed, in parallel. Content-named assets go first (cached for
good), then the pages (checked on every visit), then stale objects go.

    upload.py STAGE BUCKET TOKEN    (STAGE holds assets/ and site/)
    upload.py STAGE BUCKET TOKEN PREFIX
        the pages go under PREFIX/ and only PREFIX/'s stale objects go; the
        content-named assets go to the bucket's root, as always (pin.sh)
"""
import base64, concurrent.futures as cf, hashlib, json, mimetypes, os, sys, urllib.parse, urllib.request

stage, bucket, token = sys.argv[1:4]
prefix = sys.argv[4].strip("/") + "/" if len(sys.argv) > 4 else ""
api = "https://storage.googleapis.com/storage/v1/b/" + bucket
up = "https://storage.googleapis.com/upload/storage/v1/b/" + bucket
auth = {"Authorization": "Bearer " + token}
types = {".wasm": "application/wasm", ".wgsl": "text/plain; charset=utf-8", ".md": "text/markdown; charset=utf-8",
         ".html": "text/html; charset=utf-8", ".js": "text/javascript; charset=utf-8", ".svg": "image/svg+xml", ".jpg": "image/jpeg",
         ".json": "application/json", ".txt": "text/plain; charset=utf-8", ".xml": "application/xml; charset=utf-8",
         ".sh": "text/plain; charset=utf-8", ".ps1": "text/plain; charset=utf-8"}

def call(url, method="GET", body=None, headers={}):
    req = urllib.request.Request(url, data=body, method=method, headers={**auth, **headers})
    with urllib.request.urlopen(req, timeout=30) as r:
        return json.loads(r.read() or b"{}")

def listing():
    have, page = {}, None
    while True:
        q = {"fields": "items(name,md5Hash),nextPageToken"} | ({"pageToken": page} if page else {})
        r = call(api + "/o?" + urllib.parse.urlencode(q))
        have |= {o["name"]: o["md5Hash"] for o in r.get("items", [])}
        page = r.get("nextPageToken")
        if not page: return have

def files(root):
    for d, _, fs in os.walk(root):
        for f in fs:
            p = os.path.join(d, f)
            yield os.path.relpath(p, root), p

def object_name(name, site=False):
    # x.page is the page for the bare path x (deploy.sh): /projects/coax; x.wellknown is JSON at x.
    if name.endswith(".wellknown"): return (prefix if site else "") + name[:-10]
    return (prefix if site else "") + (name[:-5] if name.endswith(".page") else name)

def put(name, path, cache, site=False):
    data = open(path, "rb").read()
    kind = ".html" if name.endswith(".page") else ".json" if name.endswith(".wellknown") else os.path.splitext(name)[1]
    name = object_name(name, site)
    meta = {"name": name, "cacheControl": cache,
            "contentType": types.get(kind) or mimetypes.guess_type(name)[0] or "application/octet-stream"}
    b = "coaxial-upload"
    body = (f"--{b}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{json.dumps(meta)}\r\n"
            f"--{b}\r\nContent-Type: {meta['contentType']}\r\n\r\n").encode() + data + f"\r\n--{b}--".encode()
    call(up + "/o?uploadType=multipart", "POST", body, {"Content-Type": f"multipart/related; boundary={b}"})
    return name

def md5(path): return base64.b64encode(hashlib.md5(open(path, "rb").read()).digest()).decode()

have = listing()
pool = cf.ThreadPoolExecutor(16)
sent = []
for root, cache in (("assets", "public, max-age=31536000, immutable"), ("site", "no-cache")):
    site = root == "site"
    todo = [(n, p) for n, p in files(os.path.join(stage, root)) if have.get(object_name(n, site)) != md5(p) and not (site and os.path.exists(os.path.join(stage, "assets", n)))]
    sent += list(pool.map(lambda np: put(np[0], np[1], cache, site), todo))
keep = {object_name(n, True) for n, _ in files(os.path.join(stage, "site"))}
# Projects published from a drop (coax/web/api/drops.c, rc-publish) are written into the bucket by
# the API, never by a build: their pages, files and the lists stay.
published = ["projects/published.txt", "projects/published-slugs.txt"]
if "projects/published-slugs.txt" in have:
    req = urllib.request.Request(api + "/o/" + urllib.parse.quote("projects/published-slugs.txt", safe="") + "?alt=media", headers=auth)
    with urllib.request.urlopen(req, timeout=30) as r:
        published += [f"projects/{s}/" for s in r.read().decode().split() if s]
stale = [n for n in have if n not in keep and n.startswith(prefix) and not any(n == p.rstrip("/") or (p.endswith("/") and n.startswith(p)) for p in published)]
list(pool.map(lambda n: call(api + "/o/" + urllib.parse.quote(n, safe=""), "DELETE"), stale))
print(f"  sent {len(sent)}, removed {len(stale)}, unchanged {len(keep) - len(sent)}")
