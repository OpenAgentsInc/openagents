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
            return self.answer(206, data[:4096])
        self.answer(200, json.dumps({"size": str(len(data)), "md5Hash": "bWQ1"}).encode())

    def do_DELETE(self):
        name, _ = self.object_name()
        bucket["objects"].pop(name, None)
        self.answer(204)


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
check("review: PUBLISH freezes the reviewed draft for the publisher",
      status == 200 and pub["approved"] is True and sql(f"SELECT status FROM drops.drops WHERE id = '{d}'") == "approved", pub)
check("review: after PUBLISH nothing changes and nothing is read again",
      dcall("POST", E, {"path": ["card", "name"], "value": "x"}, cookie=jar)[0] == 409 and dcall("POST", f"/api/drops/{d}/draft", {}, cookie=jar)[0] == 409)
kept = [k for k in bucket["objects"] if f"/{d}/" in k]
status, gone, _ = dcall("POST", f"/api/drops/{d}/discard", {}, cookie=mjar)
check("review: someone else can't discard it", status == 404 and all(k in bucket["objects"] for k in kept))
status, gone, _ = dcall("POST", f"/api/drops/{d}/discard", {}, cookie=jar)
check("review: DISCARD takes the files out of the bucket and forgets the draft",
      status == 200 and kept and not [k for k in bucket["objects"] if f"/{d}/" in k]
      and sql(f"SELECT count(*) FROM drops.files WHERE drop_id = '{d}'") == "0"
      and dcall("GET", f"/api/drops/{d}/draft", cookie=jar)[1] == {"status": "none"} and "objects" not in gone, (status, gone, kept))
check("review: a discarded drop is closed", dcall("POST", f"/api/drops/{d}/discard", {}, cookie=jar)[0] == 409)
status, _, oh = dcall("POST", "/api/drops/signout", {}, cookie=jar)
check("drops: signing out ends the session", status == 200 and "Max-Age=0" in oh.get("Set-Cookie", "") and dcall("GET", "/api/drops/me", cookie=jar)[0] == 401)
gcs_server.shutdown()
