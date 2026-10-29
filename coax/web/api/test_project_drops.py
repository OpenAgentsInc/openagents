# Drops for a new project (rc-drop-zone, drops.c, 0016_project_drops.sql):
# sign-in, limits, uploads into a fake bucket, the scan, and privacy. test.py
# runs this file with its helpers (call, check, sql).
import http.server, threading, urllib.parse

GCS = "http://127.0.0.1:" + os.environ["GCS_PORT"]
bucket = {"objects": {}, "sessions": {}, "opened": [], "asked": [], "draft": {
    "card": {"name": "Planetary Villa", "slug": "Planetary Villa!", "line": "A world on the Verse.", "owner": "Kiki", "cover": "", "art_direction": "Moonlit, low-poly."},
    "mechanics": ["glowing flowers"], "design": {"markdown": "# Planetary Villa", "open_questions": ["How many flowers?"]},
    "roadmap": {"initiative": {"slug": "pv", "title": "PV", "body": "b"}, "milestones": [{"slug": "m", "title": "Flowers", "body": "b", "tasks": [
        {"slug": "t", "title": "Pick flowers", "body": "b", "size": "S", "acceptance": ["Pick a flower: it is in your hand"], "mechanics": ["glowing flowers"], "needs": []}]}]},
    "assets": [], "to_make": [], "engine_needs": [{"slug": "verse-glow", "title": "Glow", "why": "flowers glow", "reuses": True}]}}


class FakeGcs(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def answer(self, status, body=b"", headers=None):
        self.send_response(status)
        for k, v in (headers or {}).items():
            self.send_header(k, v)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def object_name(self):
        u = urllib.parse.urlparse(self.path)
        return urllib.parse.unquote(u.path.split("/o/", 1)[1]), urllib.parse.parse_qs(u.query)

    def do_POST(self):
        u = urllib.parse.urlparse(self.path)
        if u.path == "/model":
            req = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
            bucket["asked"].append(req)
            return self.answer(200, json.dumps({"candidates": [{"content": {"parts": [
                {"text": "thinking", "thought": True}, {"text": json.dumps(bucket["draft"])}]}}]}).encode())
        q = urllib.parse.parse_qs(u.query)
        if self.headers.get("Authorization") == "Bearer test-gcs" and u.path == "/upload/storage/v1/b/site-test/o" and q.get("uploadType") == ["multipart"]:
            raw = self.rfile.read(int(self.headers["Content-Length"]))
            parts = raw.split(b"--oa-site")
            meta = json.loads(parts[1].split(b"\r\n\r\n", 1)[1].rsplit(b"\r\n", 1)[0])
            bucket["site"][meta["name"]] = {"meta": meta, "data": parts[2].split(b"\r\n\r\n", 1)[1][:-2]}
            return self.answer(200, b"{}")
        if self.headers.get("Authorization") == "Bearer test-gcs" and "/rewriteTo/b/site-test/o/" in u.path:
            src, dest = u.path.split("/storage/v1/b/drops-test/o/", 1)[1].split("/rewriteTo/b/site-test/o/")
            data = bucket["objects"].get(urllib.parse.unquote(src))
            if data is None:
                return self.answer(404)
            bucket["site"][urllib.parse.unquote(dest)] = {"meta": {}, "data": data}
            return self.answer(200, b'{"kind": "storage#rewriteResponse", "done": true}')
        if self.headers.get("Authorization") != "Bearer test-gcs" or u.path != "/upload/storage/v1/b/drops-test/o":
            return self.answer(403)
        n = str(len(bucket["sessions"]))
        bucket["sessions"][n] = {"name": q["name"][0], "bytes": int(self.headers["X-Upload-Content-Length"])}
        bucket["opened"].append({"name": q["name"][0], "type": self.headers["X-Upload-Content-Type"], "origin": self.headers.get("Origin"),
                                 "bytes": self.headers["X-Upload-Content-Length"], "if": q.get("ifGenerationMatch", [""])[0]})
        self.answer(200, headers={"Location": GCS + "/session/" + n})

    def do_PUT(self):
        s = bucket["sessions"].get(self.path.rsplit("/", 1)[-1])
        data = self.rfile.read(int(self.headers.get("Content-Length", "0")))
        if not s:
            return self.answer(404)
        bucket["objects"][s["name"]] = data
        self.answer(200, b"{}")

    def do_GET(self):
        if self.headers.get("Authorization") != "Bearer test-gcs":
            return self.answer(403)
        name, q = self.object_name()
        data = bucket["objects"].get(name)
        if data is None:
            return self.answer(404)
        if q.get("alt") == ["media"]:
            return self.answer(206, data[:4096]) if self.headers.get("Range") else self.answer(200, data)
        self.answer(200, json.dumps({"size": str(len(data)), "md5Hash": "bWQ1"}).encode())

    def do_DELETE(self):
        name, _ = self.object_name()
        bucket["objects"].pop(name, None)
        self.answer(204)


# rc-publish: a relay that asks for AUTH, checks each event's id against its NIP-01 serialization and keeps it.
import base64 as b64mod, hashlib, socketserver, struct
pub_relay = {"events": [], "auth": []}


class FakeRelay(socketserver.BaseRequestHandler):
    def frame(self, text):
        data = text.encode()
        head = bytes([0x81]) + (bytes([len(data)]) if len(data) < 126 else bytes([126]) + struct.pack(">H", len(data)) if len(data) < 65536
                                else bytes([127]) + struct.pack(">Q", len(data)))
        self.request.sendall(head + data)

    def read(self, n):
        out = b""
        while len(out) < n:
            got = self.request.recv(n - len(out))
            if not got:
                raise EOFError
            out += got
        return out

    def handle(self):
        req = b""
        while b"\r\n\r\n" not in req:
            req += self.request.recv(4096)
        key = [l.split(":", 1)[1].strip() for l in req.decode().split("\r\n") if l.lower().startswith("sec-websocket-key:")][0]
        accept = b64mod.b64encode(hashlib.sha1((key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()).decode()
        self.request.sendall(f"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n".encode())
        self.frame('["AUTH","challenge-1"]')
        try:
            while True:
                h = self.read(2)
                n = h[1] & 0x7F
                if n == 126:
                    n = struct.unpack(">H", self.read(2))[0]
                elif n == 127:
                    n = struct.unpack(">Q", self.read(8))[0]
                mask = self.read(4) if h[1] & 0x80 else b"\0\0\0\0"
                data = bytes(c ^ mask[i % 4] for i, c in enumerate(self.read(n)))
                if h[0] & 0x0F == 8:
                    return
                msg = json.loads(data)
                ev = msg[1]
                serial = json.dumps([0, ev["pubkey"], ev["created_at"], ev["kind"], ev["tags"], ev["content"]], separators=(",", ":"), ensure_ascii=False)
                ok = hashlib.sha256(serial.encode()).hexdigest() == ev["id"] and len(ev["sig"]) == 128 and h[1] & 0x80
                if msg[0] == "AUTH":
                    pub_relay["auth"].append(ev)
                    ok = ok and ev["kind"] == 22242 and ["challenge", "challenge-1"] in ev["tags"]
                elif not pub_relay["auth"]:
                    self.frame(json.dumps(["OK", ev["id"], False, "auth-required: first"]))
                    continue
                else:
                    pub_relay["events"].append(ev)
                self.frame(json.dumps(["OK", ev["id"], bool(ok), "" if ok else "invalid: id"]))
        except EOFError:
            return


pub_relay_server = socketserver.ThreadingTCPServer(("127.0.0.1", int(os.environ["RELAY_PORT"])), FakeRelay)
threading.Thread(target=pub_relay_server.serve_forever, daemon=True).start()
bucket["site"] = {}

gcs_server = http.server.ThreadingHTTPServer(("127.0.0.1", int(os.environ["GCS_PORT"])), FakeGcs)
threading.Thread(target=gcs_server.serve_forever, daemon=True).start()


def drop_sign(keyfile, method, path, body=None):
    args = [os.environ["SIGN"], "--key", keyfile, method, API + path]
    if body is not None:
        f = tempfile.NamedTemporaryFile(delete=False)
        f.write(body)
        f.close()
        args.append(f.name)
    return "Nostr " + subprocess.run(args, capture_output=True, text=True, check=True).stdout.strip()


def dcall(method, path, body=None, cookie=None, origin=True, keyfile=None):
    raw = json.dumps(body).encode() if body is not None else None
    h = {"Content-Type": "application/json"} if raw is not None else {}
    if cookie:
        h["Cookie"] = cookie
    if origin:
        h["Origin"] = API
    if keyfile:
        h["Authorization"] = drop_sign(keyfile, method, path, raw)
    status, out, headers = call(method, path, raw, h)
    try:
        return status, json.loads(out), headers
    except ValueError:
        return status, out, headers


def put(url, data):
    req = urllib.request.Request(url, method="PUT", data=data)
    with urllib.request.urlopen(req) as r:
        return r.status


import tempfile
admin_key, other_key = os.environ["ADMIN_KEY"], os.environ["OTHER_KEY"]
member_key = os.environ["ADMIN_KEY"] + ".dropper"
member = subprocess.run([os.environ["SIGN"], "--new", member_key], capture_output=True, text=True, check=True).stdout.strip()
stranger_key = os.environ["ADMIN_KEY"] + ".stranger"
subprocess.run([os.environ["SIGN"], "--new", stranger_key], capture_output=True, text=True, check=True)

check("drops: nobody signed in is told to sign in", dcall("GET", "/api/drops/me")[0] == 401)
check("drops: a key with no account, membership or admin can't sign in",
      dcall("POST", "/api/drops/signin", {}, keyfile=stranger_key, origin=False)[0] == 403)
status, me, headers = dcall("POST", "/api/drops/signin", {}, keyfile=admin_key, origin=False)
cookie = headers.get("Set-Cookie", "")
check("drops: an admin's signed sign-in sets a session cookie for /api/drops only",
      status == 200 and cookie.startswith("oa_drops=") and "Path=/api/drops;" in cookie and "HttpOnly" in cookie
      and "SameSite=Strict" in cookie and "Secure" in cookie, (status, headers))
jar = cookie.split(";")[0]
check("drops: only the session token's hash is kept", sql(f"select count(*) from drops.sessions where token_sha256 = '{jar.split('=')[1]}'") == "0"
      and sql("select count(*) from drops.sessions") == "1")
status, me, _ = dcall("GET", "/api/drops/me", cookie=jar)
check("drops: me says who and the limits", status == 200 and me["may"] and me["used"] == 0 and me["limits"]["file"] == 64 << 20
      and me["limits"]["files"] == 40, me)
check("drops: a write from another site is refused", call("POST", "/api/drops", b"{}", {"Cookie": jar, "Origin": "https://evil.example"})[0] == 403
      and call("POST", "/api/drops", b"{}", {"Cookie": jar})[0] == 403)
status, made, _ = dcall("POST", "/api/drops", {}, cookie=jar)
d = made.get("id", "")
check("drops: a new drop", status == 200 and d.startswith("d-") and len(d) == 18, made)

notes = b"# Planetary Villa\n\nA villa on a small planet. Kiki's notes.\n"
status, added, _ = dcall("POST", f"/api/drops/{d}/files", {"name": "Villa notes.md", "bytes": len(notes)}, cookie=jar)
opened = bucket["opened"][-1] if bucket["opened"] else {}
check("drops: an admitted file gets an upload for exactly its bytes and our type, for this site",
      status == 200 and added["upload"].startswith(GCS + "/session/") and added["file"]["status"] == "uploading"
      and opened.get("type") == "text/markdown; charset=utf-8" and opened.get("bytes") == str(len(notes)) and opened.get("origin") == API
      and opened.get("if") == "0" and opened.get("name", "").startswith("test/") and d in opened.get("name", ""), (added, opened))
f1 = added["file"]["id"]
check("drops: the session isn't readable from a listing", "session" not in json.dumps(dcall("GET", f"/api/drops/{d}", cookie=jar)[1])
      and "/session/" not in json.dumps(dcall("GET", f"/api/drops/{d}", cookie=jar)[1]))
check("drops: done before the bytes arrive waits", dcall("POST", f"/api/drops/{d}/files/{f1}/done", {}, cookie=jar)[0] == 409)
put(added["upload"], notes)
status, done, _ = dcall("POST", f"/api/drops/{d}/files/{f1}/done", {}, cookie=jar)
check("drops: an uploaded document is scanned", status == 200 and done["file"]["status"] == "scanned" and done["file"]["kind"] == "document", done)
check("drops: done again is the same answer", dcall("POST", f"/api/drops/{d}/files/{f1}/done", {}, cookie=jar)[1]["file"]["status"] == "scanned")

for name, size, want in [("run.exe", 10, 415), ("noext", 10, 415), ("big.glb", (64 << 20) + 1, 413), ("empty.png", 0, 400),
                         ("a/b.png", 10, 400), ("x.png", "ten", 400)]:
    status, body, _ = dcall("POST", f"/api/drops/{d}/files", {"name": name, "bytes": size}, cookie=jar)
    check(f"drops: {name} ({size}) is refused with {want}", status == want, (status, body))

fake = b"MZ\x90\x00" + b"\x00" * 60
status, added, _ = dcall("POST", f"/api/drops/{d}/files", {"name": "cover.png", "bytes": len(fake)}, cookie=jar)
put(added["upload"], fake)
obj = bucket["opened"][-1]["name"]
status, done, _ = dcall("POST", f"/api/drops/{d}/files/{added['file']['id']}/done", {}, cookie=jar)
check("drops: a program called .png is refused and its object deleted",
      done["file"]["status"] == "refused" and done["file"]["note"] == "it is a program" and obj not in bucket["objects"], done)
short = b"\x89PNG\r\n\x1a\n" + b"\x00" * 8
status, added, _ = dcall("POST", f"/api/drops/{d}/files", {"name": "art.png", "bytes": 100}, cookie=jar)
put(added["upload"], short)
status, done, _ = dcall("POST", f"/api/drops/{d}/files/{added['file']['id']}/done", {}, cookie=jar)
check("drops: a file that arrives at another size is refused", done["file"]["status"] == "refused" and "size" in done["file"]["note"], done)

sniffs = [("glb", "676c54460200000000", True), ("wav", "52494646000000005741564566", True), ("jpg", "ffd8ffe0", True),
          ("mp3", "4944330300", True), ("m4a", "0000002066747970", True), ("pdf", "255044462d312e37", True),
          ("docx", "504b030414", True), ("svg", "3c3f786d6c3f3e3c73766720", True), ("md", "68690a", True),
          ("md", "6869000a", False), ("png", "ffd8ff", False), ("glb", "7f454c46", False), ("ogg", "4f67675300", True),
          ("fbx", "4b617964617261204642582042696e61727900", True), ("blend", "424c454e444552", True), ("stl", "736f6c6964", True)]
for ext, head, ok in sniffs:
    got = sql(f"select coalesce(drops.sniff('{ext}', decode('{head}', 'hex')), 'ok')")
    check(f"drops: the scan {'takes' if ok else 'refuses'} {ext} {head[:12]}", (got == "ok") == ok, got)
check("drops: a cut UTF-8 character at the end of the head is still text",
      sql("select coalesce(drops.sniff('md', decode('68c3', 'hex')), 'ok')") == "ok")

status, _, _ = dcall("POST", "/api/drops/signin", {}, keyfile=member_key, origin=False)
check("drops: a key with no Verse account can't drop", status == 403)
sql(f"INSERT INTO verse.accounts (id, npub, created_ms) VALUES ('n-{member}', '{member}', 0)")
status, _, mh = dcall("POST", "/api/drops/signin", {"again": 1}, keyfile=member_key, origin=False)
mjar = mh.get("Set-Cookie", "").split(";")[0]
check("drops: a Verse account's key may", status == 200 and mjar.startswith("oa_drops="))
check("drops: someone else's drop is not found", dcall("GET", f"/api/drops/{d}", cookie=mjar)[0] == 404
      and dcall("POST", f"/api/drops/{d}/files", {"name": "x.png", "bytes": 10}, cookie=mjar)[0] == 404
      and dcall("POST", f"/api/drops/{d}/files/{f1}/done", {}, cookie=mjar)[0] == 404)
check("drops: and their list is their own", dcall("GET", "/api/drops", cookie=mjar)[1]["drops"] == [])
mine = dcall("GET", "/api/drops", cookie=jar)[1]["drops"]
check("drops: the admin's list counts the files kept", len(mine) == 1 and mine[0]["files"] == 1 and mine[0]["bytes"] == len(notes), mine)
check("drops: a signed request works without a cookie",
      dcall("GET", f"/api/drops/{d}", keyfile=admin_key, origin=False)[1]["files"][0]["name"] == "Villa notes.md")

auth = urllib.parse.quote(drop_sign(admin_key, "GET", "/api/admin/login")[6:], safe="")
req = urllib.request.Request(API + "/api/admin/login?auth=" + auth)


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *a):
        return None


try:
    r = urllib.request.build_opener(NoRedirect).open(req)
    cookies = r.headers.get_all("Set-Cookie")
except urllib.error.HTTPError as e:
    cookies = e.headers.get_all("Set-Cookie") or []
admin_drop = [c for c in cookies if "Path=/api/drops;" in c]
check("drops: the admin sign-in link opens /api/drops too", len(admin_drop) == 1 and any("Path=/api/admin;" in c for c in cookies), cookies)
if admin_drop:
    check("drops: with that cookie the admin sees their drops",
          dcall("GET", "/api/drops", cookie=admin_drop[0].split(";")[0])[1]["drops"][0]["id"] == d)
check("drops: a drop with nothing kept has nothing to read",
      dcall("POST", f"/api/drops/{dcall('POST', '/api/drops', {}, cookie=jar)[1]['id']}/draft", {}, cookie=jar)[0] == 409)
check("drops: no draft yet", dcall("GET", f"/api/drops/{d}/draft", cookie=jar)[1] == {"status": "none"})
status, drafted, _ = dcall("POST", f"/api/drops/{d}/draft", {}, cookie=jar)
asked = bucket["asked"][-1] if bucket["asked"] else {}
parts = asked.get("contents", [{}])[0].get("parts", [])
check("drops: the model reads the kept files from the bucket, with the schema and the engine's tasks",
      any(p.get("fileData", {}).get("fileUri", "").startswith("gs://drops-test/test/") and p["fileData"]["mimeType"] == "text/plain" for p in parts)
      and not any("cover" in json.dumps(p) and "fileData" in p for p in parts)
      and asked["generationConfig"]["responseMimeType"] == "application/json" and "verse-quadruped" in asked["systemInstruction"]["parts"][0]["text"], asked)
check("drops: the draft is kept, its slug made a slug", status == 200 and drafted["status"] == "ready"
      and drafted["draft"]["card"]["slug"] == "planetary-villa" and drafted["model"] == "gemini-3.8-flash", drafted)
check("drops: and read back", dcall("GET", f"/api/drops/{d}/draft", cookie=jar)[1]["draft"]["roadmap"]["milestones"][0]["title"] == "Flowers")
check("drops: someone else can't read or start it", dcall("GET", f"/api/drops/{d}/draft", cookie=mjar)[0] == 404
      and dcall("POST", f"/api/drops/{d}/draft", {}, cookie=mjar)[0] == 404)
bucket["draft"]["roadmap"]["milestones"][0]["tasks"][0]["acceptance"] = []
status, refused_draft, _ = dcall("POST", f"/api/drops/{d}/draft", {}, cookie=jar)
check("drops: a draft with a task no one could check is refused, and the good one kept",
      status == 502 and refused_draft["status"] == "failed" and "acceptance" in refused_draft["note"] and refused_draft["draft"]["card"]["slug"] == "planetary-villa", refused_draft)

# rc-review: every field is the owner's to change; PUBLISH freezes it; DISCARD forgets it.
bucket["draft"]["roadmap"]["milestones"][0]["tasks"][0]["acceptance"] = ["Pick a flower: it is in your hand"]
dcall("POST", f"/api/drops/{d}/draft", {}, cookie=jar)
E = f"/api/drops/{d}/draft/edit"
T = ["roadmap", "milestones", "0", "tasks", "0"]
status, ed, _ = dcall("POST", E, {"path": ["card", "name"], "value": "  Moon Villa "}, cookie=jar)
check("review: the owner renames the project", status == 200 and ed["draft"]["card"]["name"] == "Moon Villa" and ed["edits"] == 1, ed)
check("review: a slug stays a slug", dcall("POST", E, {"path": ["card", "slug"], "value": "Moon Villa!"}, cookie=jar)[1]["draft"]["card"]["slug"] == "moon-villa")
check("review: a check is added at a list's end",
      dcall("POST", E, {"path": T + ["acceptance", "1"], "value": "Two moons rise"}, cookie=jar)[1]["draft"]["roadmap"]["milestones"][0]["tasks"][0]["acceptance"]
      == ["Pick a flower: it is in your hand", "Two moons rise"])
check("review: an emptied item leaves its list",
      dcall("POST", E, {"path": T + ["acceptance", "0"], "value": ""}, cookie=jar)[1]["draft"]["roadmap"]["milestones"][0]["tasks"][0]["acceptance"] == ["Two moons rise"])
check("review: the last check can't go", dcall("POST", E, {"path": T + ["acceptance", "0"], "value": ""}, cookie=jar)[0] == 400)
check("review: a yes or no", dcall("POST", E, {"path": ["engine_needs", "0", "reuses"], "value": "no"}, cookie=jar)[1]["draft"]["engine_needs"][0]["reuses"] is False)
check("review: a size is S, M or L", dcall("POST", E, {"path": T + ["size"], "value": "XL"}, cookie=jar)[0] == 400
      and dcall("POST", E, {"path": T + ["size"], "value": "L"}, cookie=jar)[1]["draft"]["roadmap"]["milestones"][0]["tasks"][0]["size"] == "L")
check("review: no field outside the draft, no title emptied, nothing too long",
      dcall("POST", E, {"path": ["status"], "value": "x"}, cookie=jar)[0] == 400
      and dcall("POST", E, {"path": ["card", "nope"], "value": "x"}, cookie=jar)[0] == 400
      and dcall("POST", E, {"path": T + ["title"], "value": " "}, cookie=jar)[0] == 400
      and dcall("POST", E, {"path": ["card", "line"], "value": "x" * 20001}, cookie=jar)[0] == 400)
check("review: someone else can't change it", dcall("POST", E, {"path": ["card", "name"], "value": "Mine"}, cookie=mjar)[0] == 404)
check("review: an edit from another site is refused", dcall("POST", E, {"path": ["card", "name"], "value": "x"}, cookie=jar, origin=False)[0] == 403)
got = dcall("GET", f"/api/drops/{d}/draft", cookie=jar)[1]
check("review: the edits are kept, the model's answer too",
      got["draft"]["card"]["name"] == "Moon Villa" and got["edits"] == 6 and got["approved"] is False
      and sql(f"SELECT original->'card'->>'name' FROM drops.drafts WHERE drop_id = '{d}'") == "Planetary Villa", got)
status, pub, _ = dcall("POST", f"/api/drops/{d}/publish", {}, cookie=jar)
check("review: PUBLISH freezes the reviewed draft and publishes it",
      status == 200 and pub["approved"] is True and pub.get("published") == {"slug": "moon-villa", "page": "/projects/moon-villa"}
      and sql(f"SELECT status FROM drops.drops WHERE id = '{d}'") == "published", pub)
# rc-publish: the key, the events, the files, the page and the list.
pk = sql("SELECT pubkey FROM drops.projects WHERE slug = 'moon-villa'")
admin_pk = sql(f"SELECT owner FROM drops.drops WHERE id = '{d}'")
sealed = sql("SELECT key_sealed FROM drops.projects WHERE slug = 'moon-villa'")
check("publish: the project's key is kept only sealed (nonce, 32 bytes, tag)", len(pk) == 64 and len(sealed) == 120
      and sql("SELECT count(*) FROM information_schema.columns WHERE table_schema = 'drops' AND table_name = 'projects' AND column_name ~ 'secret|^key$'") == "0")
evs = pub_relay["events"]
kinds = [e["kind"] for e in evs]
check("publish: the relay asked for AUTH and the project's key answered", pub_relay["auth"] and pub_relay["auth"][0]["pubkey"] == pk)
check("publish: repository, project, initiative, milestone, task and order, signed by the project's key, each id its NIP-01 hash",
      kinds == [30617, 30621, 1621, 1621, 1621, 30623] and all(e["pubkey"] == pk for e in evs), kinds)
ptag = lambda e, n: [t for t in e["tags"] if t[0] == n]
proj, ini, mil, task, order = evs[1], evs[2], evs[3], evs[4], evs[5]
check("publish: the project is the draft as reviewed, its dropper the maintainer",
      ptag(proj, "d") == [["d", "moon-villa"]] and ptag(proj, "name") == [["name", "Moon Villa"]] and ptag(proj, "visibility") == [["visibility", "public"]]
      and ["p", admin_pk, "", "maintainer"] in proj["tags"] and ptag(proj, "a")[0][1] == f"30617:{pk}:openagents", proj["tags"])
check("publish: the tasks hang from their milestone, the milestone from the initiative, the checks in the body",
      ["t", "initiative"] in ini["tags"] and ["t", "milestone"] in mil["tags"] and ptag(mil, "parent") == [["parent", ini["id"]]]
      and ptag(task, "parent") == [["parent", mil["id"]]] and ptag(task, "size") == [["size", "L"]] and "## Acceptance\n\n- Two moons rise" in task["content"]
      and ptag(task, "slug") == [["slug", "t"]] and [f"30621:{pk}:moon-villa", "wss://relay.openagents.com"] == ptag(task, "a")[1][1:], task)
check("publish: the order ranks every issue", [t[1] for t in ptag(order, "item")] == [ini["id"], mil["id"], task["id"]]
      and [t[2] for t in ptag(order, "item")] == ["11", "12", "13"], order["tags"])
ppage = bucket["site"].get("projects/moon-villa/index.html", {})
check("publish: the page is in the site's bucket, uncached, drawn live by the project app",
      ppage.get("meta", {}).get("cacheControl") == "no-cache" and f'data-project="{pk} moon-villa"'.encode() in ppage.get("data", b"")
      and b"<h1>Moon Villa</h1>" in ppage.get("data", b"") and bucket["site"].get("projects/moon-villa", {}).get("data") == ppage.get("data"), ppage.get("meta"))
plisted = bucket["site"].get("projects/published.txt", {}).get("data", b"").decode()
check("publish: /projects lists it from published.txt, with no release",
      plisted.startswith("moon-villa|Moon Villa|") and f"|/projects/moon-villa|{pk}|{admin_pk}\n" in plisted
      and bucket["site"]["projects/published-slugs.txt"]["data"] == b"moon-villa\n", plisted)
pfiles = [k for k in bucket["site"] if k.startswith("projects/moon-villa/files/")]
kept_files = sql(f"SELECT string_agg(object, ' ') FROM drops.files WHERE drop_id = '{d}' AND status = 'scanned'").split()
check("publish: the kept files are copied next to the page", len(pfiles) == len(kept_files) >= 1
      and sorted(bucket["site"][k]["data"] for k in pfiles) == sorted(bucket["objects"][o] for o in kept_files), (pfiles, kept_files))
check("publish: index.md for agents names the design and the relay",
      b"## Design" in bucket["site"]["projects/moon-villa/index.md"]["data"] and pk.encode() in bucket["site"]["projects/moon-villa/index.md"]["data"])
n_events = len(evs)
status, again, _ = dcall("POST", f"/api/drops/{d}/publish", {}, cookie=jar)
check("publish: PUBLISH again is the same answer and signs nothing twice",
      status == 200 and again["published"]["slug"] == "moon-villa" and len(pub_relay["events"]) == n_events, again)
check("review: after PUBLISH nothing changes and nothing is read again",
      dcall("POST", E, {"path": ["card", "name"], "value": "x"}, cookie=jar)[0] == 409 and dcall("POST", f"/api/drops/{d}/draft", {}, cookie=jar)[0] == 409)
check("publish: a published drop can't be discarded", dcall("POST", f"/api/drops/{d}/discard", {}, cookie=jar)[0] == 409)
# DISCARD, on a second drop
d = dcall("POST", "/api/drops", {}, cookie=jar)[1]["id"]
status, added, _ = dcall("POST", f"/api/drops/{d}/files", {"name": "Villa notes.md", "bytes": len(notes)}, cookie=jar)
put(added["upload"], notes)
dcall("POST", f"/api/drops/{d}/files/{added['file']['id']}/done", {}, cookie=jar)
dcall("POST", f"/api/drops/{d}/draft", {}, cookie=jar)
kept = [k for k in bucket["objects"] if f"/{d}/" in k]
status, gone, _ = dcall("POST", f"/api/drops/{d}/discard", {}, cookie=mjar)
check("review: someone else can't discard it", status == 404 and all(k in bucket["objects"] for k in kept))
status, gone, _ = dcall("POST", f"/api/drops/{d}/discard", {}, cookie=jar)
check("review: DISCARD takes the files out of the bucket and forgets the draft",
      status == 200 and kept and not [k for k in bucket["objects"] if f"/{d}/" in k]
      and sql(f"SELECT count(*) FROM drops.files WHERE drop_id = '{d}'") == "0"
      and dcall("GET", f"/api/drops/{d}/draft", cookie=jar)[1] == {"status": "none"} and "objects" not in gone, (status, gone, kept))
check("review: a discarded drop is closed", dcall("POST", f"/api/drops/{d}/discard", {}, cookie=jar)[0] == 409)

# rc-first-realm: a drop's .glb models become a realm: one pack and a page, linked from the project.
def realm_checks():
    import zlib, struct


    def png(w, h, rgb):
        chunk = lambda k, d: struct.pack(">I", len(d)) + k + d + struct.pack(">I", zlib.crc32(k + d) & 0xFFFFFFFF)
        raw = b"".join(b"\0" + bytes(rgb) * w for _ in range(h))
        return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b"")


    def glb(doc, binary):
        binary += b"\0" * (-len(binary) % 4)
        doc["buffers"] = [{"byteLength": len(binary)}]
        js = json.dumps(doc).encode(); js += b" " * (-len(js) % 4)
        return (b"glTF" + struct.pack("<II", 2, 12 + 8 + len(js) + 8 + len(binary)) + struct.pack("<I", len(js)) + b"JSON" + js
                + struct.pack("<I", len(binary)) + b"BIN\0" + binary)


    tex = png(4, 4, (200, 100, 50))
    lantern_bin = struct.pack("<12f", -0.5, 0, 0, 0.5, 0, 0, 0.5, 1, 0, -0.5, 1, 0) + struct.pack("<12f", *([0, 0, 1] * 4)) \
        + struct.pack("<8f", 0, 0, 1, 0, 1, 1, 0, 1) + struct.pack("<6H", 0, 1, 2, 0, 2, 3) + tex
    lantern = glb({"asset": {"version": "2.0"}, "scene": 0, "scenes": [{"nodes": [0]}],
                   "nodes": [{"mesh": 0, "translation": [10, 0, 0], "rotation": [0, 0.70710678, 0, 0.70710678], "scale": [2, 2, 2]}],
                   "meshes": [{"primitives": [{"attributes": {"POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2}, "indices": 3, "material": 0},
                                              {"attributes": {"POSITION": 0}, "mode": 1}]}],
                   "materials": [{"pbrMetallicRoughness": {"baseColorFactor": [1, 0.5, 0.25, 1], "baseColorTexture": {"index": 0}}}],
                   "textures": [{"source": 0}], "images": [{"bufferView": 4, "mimeType": "image/png"}],
                   "accessors": [{"bufferView": 0, "componentType": 5126, "count": 4, "type": "VEC3"},
                                 {"bufferView": 1, "componentType": 5126, "count": 4, "type": "VEC3"},
                                 {"bufferView": 2, "componentType": 5126, "count": 4, "type": "VEC2"},
                                 {"bufferView": 3, "componentType": 5123, "count": 6, "type": "SCALAR"}],
                   "bufferViews": [{"buffer": 0, "byteOffset": 0, "byteLength": 48}, {"buffer": 0, "byteOffset": 48, "byteLength": 48},
                                   {"buffer": 0, "byteOffset": 96, "byteLength": 32}, {"buffer": 0, "byteOffset": 128, "byteLength": 12},
                                   {"buffer": 0, "byteOffset": 140, "byteLength": len(tex)}]}, lantern_bin)
    crate = glb({"asset": {"version": "2.0"}, "nodes": [{"matrix": [100, 0, 0, 0, 0, 100, 0, 0, 0, 0, 100, 0, 0, 0, 0, 1], "children": [1]},
                                                      {"mesh": 0, "translation": [0, 0, 0.01]}],
                 "meshes": [{"primitives": [{"attributes": {"POSITION": 0}}]}],
                 "accessors": [{"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3"},
                               {"bufferView": 0, "componentType": 5126, "count": 900, "type": "VEC3"}],
                 "bufferViews": [{"buffer": 0, "byteLength": 36}]}, struct.pack("<9f", 0, 0, 0, 1, 0, 0, 0, 1, 0))
    broken = b"glTF" + struct.pack("<III", 2, 32, 12) + b"JSON" + b'{"asset":{} }'[:12]
    cover = png(8, 8, (10, 20, 30))
    d = dcall("POST", "/api/drops", {}, cookie=jar)[1]["id"]
    for name, data in [("lantern.glb", lantern), ("Old Crate.glb", crate), ("broken.glb", broken), ("cover.png", cover), ("notes.md", notes)]:
        status, added, _ = dcall("POST", f"/api/drops/{d}/files", {"name": name, "bytes": len(data)}, cookie=jar)
        put(added["upload"], data)
        status, done, _ = dcall("POST", f"/api/drops/{d}/files/{added['file']['id']}/done", {}, cookie=jar)
        check(f"realm: {name} is kept", done["file"]["status"] == "scanned", done)
    bucket["draft"] = json.loads(json.dumps(bucket["draft"]))
    bucket["draft"]["card"].update({"name": "Lantern Garden", "slug": "lantern-garden", "cover": "cover.png"})
    bucket["draft"]["assets"] = [{"file": "lantern.glb", "kind": "model", "licence": "CC0 1.0", "use": "the lanterns"}]
    dcall("POST", f"/api/drops/{d}/draft", {}, cookie=jar)
    before = len(pub_relay["events"])
    status, pub, _ = dcall("POST", f"/api/drops/{d}/publish", {}, cookie=jar)
    check("realm: the drop is published", status == 200 and pub.get("published", {}).get("slug") == "lantern-garden", pub)
    proj = [e for e in pub_relay["events"][before:] if e["kind"] == 30621]
    check("realm: the project's event names its realm", proj and ["realm", "/projects/lantern-garden/realm"] in proj[0]["tags"], proj)
    check("realm: a project with no model names none", not [t for t in evs[1]["tags"] if t[0] == "realm"])
    raw = bucket["site"].get("projects/lantern-garden/realm/realm.pack", {}).get("data", b"")
    check("realm: the pack is in the site's bucket", raw[:4] == b"VPK1" and struct.unpack_from("<I", raw, 8)[0] == len(raw), raw[:16])
    entries = []
    if raw[:4] == b"VPK1":
        for k in range(struct.unpack_from("<I", raw, 4)[0]):
            kind, lic, _, off, n = struct.unpack_from("<BBHII", raw, 32 + 48 * k)
            entries.append((kind, lic, raw[32 + 48 * k + 16:32 + 48 * k + 48].rstrip(b"\0").decode(), raw[off:off + n]))
    check("realm: catalog, vertices, indices, the model's texture, then white", [(e[0], e[2]) for e in entries]
          == [(1, "catalog"), (2, "vertices"), (3, "indices"), (4, "layer0"), (4, "white")] and entries[3][3] == tex, [(e[0], e[2]) for e in entries])
    cat = entries[0][3].decode() if entries else ""
    check("realm: each model a licence and a mesh, stood where its nodes put it (translation, rotation, scale, a parent's matrix)",
          "L 0 lantern.glb|CC0 1.0|\nM lantern 0 6 0 10.000 0.000 -1.000 10.000 2.000 1.000\n" in cat
          and "L 1 Old Crate.glb|licence unknown: ask the owner|\nM Old-Crate 6 3 1 0.000 0.000 1.000 100.000 100.000 1.000\n" in cat
          and "T 0 image0 0\n" in cat and f"bytes {len(raw)}\n" in cat and "broken" not in cat, cat)
    if len(entries) == 5:
        vs, ix = entries[1][3], struct.unpack(f"<{len(entries[2][3]) // 4}I", entries[2][3])
        v = lambda i: (struct.unpack_from("<3f3f2f", vs, i * 48), vs[i * 48 + 32:i * 48 + 36])
        check("realm: the triangles index the model's own vertices, a line strip left out", ix == (0, 1, 2, 0, 2, 3, 4, 5, 6) and len(vs) == 7 * 48, ix)
        (p0, c0), (p4, c4) = v(2), v(5)
        check("realm: positions and normals in the world, the uv kept, the colour and the layer in the colour's bytes",
              abs(p0[0] - 10) < 1e-4 and abs(p0[1] - 2) < 1e-4 and abs(p0[2] + 1) < 1e-4 and abs(p0[3] - 1) < 1e-4 and abs(p0[5]) < 1e-4
              and p0[6:8] == (1.0, 1.0) and c0 == bytes([255, 128, 64, 0]), (p0, c0))
        check("realm: a model without normals gets its faces', and without a texture the white layer",
              abs(p4[0] - 100) < 1e-3 and abs(p4[5] - 1) < 1e-4 and c4 == bytes([255, 255, 255, 1]), (p4, c4))
    page = bucket["site"].get("projects/lantern-garden/realm", {}).get("data", b"")
    check("realm: its page opens the first-realm module on the pack, named for the project",
          b'data-realm-of="lantern-garden"' in page and b'data-name="Lantern Garden"' in page and b'fetch("/realm/drop"' in page
          and b"/realm/realm.pack" in page and bucket["site"].get("projects/lantern-garden/realm/index.html", {}).get("data") == page
          and b'<p><a href="/projects/lantern-garden/realm/index.md">A first realm</a></p>' in page, page[:200])
    rmd = bucket["site"].get("projects/lantern-garden/realm/index.md", {}).get("data", b"").decode()
    check("realm: its index.md lists what stands there and says what couldn't be placed",
          "- lantern.glb (CC0 1.0)" in rmd and "- Old Crate.glb (licence unknown: ask the owner)" in rmd and "broken.glb: it has no binary chunk" in rmd, rmd)
    pmd = bucket["site"].get("projects/lantern-garden/index.md", {}).get("data", b"").decode()
    check("realm: the project's index.md links its realm", "[/projects/lantern-garden/realm](/projects/lantern-garden/realm)" in pmd, pmd[:400])
    cover_at = [k for k in bucket["site"] if k.startswith("projects/lantern-garden/files/cover-")]
    ppage = bucket["site"].get("projects/lantern-garden", {}).get("data", b"").decode()
    check("realm: the project's link preview is the drop's cover", cover_at and f'<meta property="og:image" content="{API}/{cover_at[0]}">' in ppage
          and f'<meta property="og:url" content="{API}/projects/lantern-garden">' in ppage, (cover_at, ppage[:900]))
    check("realm: kept once", sql("SELECT realm_ms IS NOT NULL FROM drops.projects WHERE slug = 'lantern-garden'") == "t"
          and "broken.glb" in sql("SELECT realm_note FROM drops.projects WHERE slug = 'lantern-garden'"))

realm_checks()
status, _, oh = dcall("POST", "/api/drops/signout", {}, cookie=jar)
check("drops: signing out ends the session", status == 200 and "Max-Age=0" in oh.get("Set-Cookie", "") and dcall("GET", "/api/drops/me", cookie=jar)[0] == 401)
gcs_server.shutdown()
