# Firm reason: 2 (black-box HTTP checks of the C API from another language, run by test.py with the other API tests).
# Attachments on a project and its worlds (#13468; worlds.c, world-files.coax,
# migrations/0184_world_attachments.sql, docs/protocol/world-files-v1.md): who may list,
# read and change them; a file by direct upload into a fake private bucket, read back and
# scanned; links, edits and a detach that removes the stored file; the limits in words; and
# the invariant that an attachment never reaches a player or a pack. test.py runs this file
# with its helpers (call, check, sql). It serves its own fake bucket on GCS_PORT.
import hashlib, http.server, json, os, subprocess, threading, urllib.parse, urllib.request

WF_GCS = "http://127.0.0.1:" + os.environ["GCS_PORT"]
wf_bucket = {"objects": {}, "sessions": {}, "parts": {}}


class WfBucket(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def answer(self, status, body=b"", headers=None):
        self.send_response(status)
        for k, v in (headers or {}).items():
            self.send_header(k, v)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def name(self):
        u = urllib.parse.urlparse(self.path)
        return urllib.parse.unquote(u.path.split("/o/", 1)[1]), urllib.parse.parse_qs(u.query)

    def do_POST(self):
        u = urllib.parse.urlparse(self.path)
        q = urllib.parse.parse_qs(u.query)
        if self.headers.get("Authorization") != "Bearer test-gcs" or u.path != "/upload/storage/v1/b/drops-test/o":
            return self.answer(403)
        n = str(len(wf_bucket["sessions"]))
        wf_bucket["sessions"][n] = q["name"][0]
        self.answer(200, headers={"Location": WF_GCS + "/session/" + n})

    def do_PUT(self):
        name = wf_bucket["sessions"].get(self.path.rsplit("/", 1)[-1])
        data = self.rfile.read(int(self.headers.get("Content-Length", "0")))
        if not name:
            return self.answer(404)
        # a resumable upload in parts: "Content-Range: bytes A-B/TOTAL", answered 308 until the last
        r = self.headers.get("Content-Range", "")
        if r.startswith("bytes ") and "-" in r:
            first, total = int(r[6:].split("-")[0]), int(r.rsplit("/", 1)[1])
            held = wf_bucket["parts"].get(name, b"")
            if first != len(held):
                return self.answer(400)
            held += data
            if len(held) < total:
                wf_bucket["parts"][name] = held
                return self.answer(308, headers={"Range": f"bytes=0-{len(held) - 1}"})
            wf_bucket["parts"].pop(name, None)
            data = held
        wf_bucket["objects"][name] = data
        self.answer(200, b"{}")

    def do_GET(self):
        if self.headers.get("Authorization") != "Bearer test-gcs":
            return self.answer(403)
        name, q = self.name()
        data = wf_bucket["objects"].get(name)
        if data is None:
            return self.answer(404)
        if q.get("alt") == ["media"]:
            return self.answer(206, data[:4096]) if self.headers.get("Range") else self.answer(200, data)
        self.answer(200, json.dumps({"size": str(len(data)), "md5Hash": "bWQ1"}).encode())

    def do_DELETE(self):
        name, _ = self.name()
        wf_bucket["objects"].pop(name, None)
        self.answer(204)


try:
    gcs_server.server_close()   # an earlier file's fake bucket, stopped already
except NameError:
    pass
wf_server = http.server.ThreadingHTTPServer(("127.0.0.1", int(os.environ["GCS_PORT"])), WfBucket)
threading.Thread(target=wf_server.serve_forever, daemon=True).start()


def wf_key(name):
    path = os.environ["ADMIN_KEY"] + "." + name
    if not os.path.exists(path):
        return path, subprocess.run([os.environ["SIGN"], "--new", path], capture_output=True, text=True, check=True).stdout.strip()
    return path, subprocess.run([os.environ["SIGN"], "--key", path, "--pubkey"], capture_output=True, text=True, check=True).stdout.strip()


def wf_call(method, path, body=None, keyfile=None, origin=None):
    h = {}
    if keyfile:
        args = [os.environ["SIGN"], "--key", keyfile, method, API + path]
        if body is not None:
            f = tempfile.NamedTemporaryFile(delete=False)
            f.write(body)
            f.close()
            args.append(f.name)
        h["Authorization"] = "Nostr " + subprocess.run(args, capture_output=True, text=True, check=True).stdout.strip()
    if origin:
        h["Origin"] = origin
    return call(method, path, body, h)


import tempfile
admin = os.environ["ADMIN_KEY"]
member, member_pk = wf_key("files-member")
stranger, _ = wf_key("files-stranger")
sql("INSERT INTO projects.docs VALUES ('verse', 1, '{\"members\": []}'::jsonb, 1, 'test') ON CONFLICT (slug) DO NOTHING")
sql(f"UPDATE projects.docs SET doc = jsonb_set(doc, '{{members}}', coalesce(doc->'members', '[]'::jsonb) || '[{{\"pubkey\": \"{member_pk}\", \"name\": \"Member\"}}]'::jsonb) WHERE slug = 'verse'")
packs_before = sql("SELECT count(*) || ':' || coalesce(string_agg(hash, ',' ORDER BY hash), '') FROM verse.world_packs")
docs_before = sql("SELECT coalesce(string_agg(id || dev_rev || dev_pack || live_pack, ',' ORDER BY id), '') FROM verse.world_docs")
L = "/api/worlds/splice/files"

# Who may see them: the project's members and the site's admins, nobody else.
check("files: nobody logged in is told to log in", wf_call("GET", L)[0] == 401)
check("files: a stranger's signed list is refused", wf_call("GET", L, keyfile=stranger)[0] == 403)
check("files: and so is the project's page list", wf_call("GET", "/api/projects/verse/files", keyfile=stranger)[0] == 403
      and wf_call("GET", "/api/projects/verse/files")[0] == 401)
status, body, headers = wf_call("GET", L, keyfile=member)
check("files: a member reads the world's list, with the limits", status == 200
      and body.decode().split("\n")[0].startswith("world.files.v1 splice verse 0 0 26214400 200 1073741824")
      and "no-store" in headers.get("Cache-Control", ""), (status, body))
check("files: an admin reads it too", wf_call("GET", L, keyfile=admin)[0] == 200)
check("files: a page of another site may not", wf_call("GET", L, keyfile=member, origin="https://evil.example")[0] == 403)

# A file by direct upload: named by its SHA-256, sent, read back, scanned.
png = b"\x89PNG\r\n\x1a\n" + bytes(range(256)) * 40
h = hashlib.sha256(png).hexdigest()
ask = f"world.file.upload.v1 project {len(png)} Raider garage.png\nn The garage at night\nt garage,raiders\n{h}\n".encode()
status, body, _ = wf_call("POST", L, ask, keyfile=member)
line = body.decode().strip().split(" ")
check("files: an upload is opened for exactly the file", status == 200 and line[:4] == ["world.file.upload.v1", "put", h, str(len(png))] and line[4].startswith(WF_GCS + "/session/"), body)
check("files: nothing is stored before the bytes arrive", wf_call("POST", L + "/done", ask, keyfile=member)[0] == 422)
status, body, _ = wf_call("POST", L, ask, keyfile=member)
urllib.request.urlopen(urllib.request.Request(body.decode().split(" ")[4].strip(), data=png, method="PUT")).read()
status, body, _ = wf_call("POST", L + "/done", ask, keyfile=member)
text = body.decode()
fid = text.split("\nf ", 1)[1].split(" ", 1)[0] if "\nf " in text else ""
check("files: once sent, it is attached to the project", status == 200 and f" project image {len(png)} {h} " in text
      and "\nn The garage at night\n" in text and "\nt garage,raiders\n" in text and text.endswith(f"did attached {fid}\n"), body)
check("files: its bytes are one object of the private bucket, under its hash", wf_bucket["objects"].get(f"test/attachments/{h}") == png)
status, raw, headers = wf_call("GET", f"{L}/{fid}", keyfile=member)
check("files: a member reads its bytes as the type its name says, never stored", status == 200 and raw == png
      and headers.get("Content-Type") == "image/png" and "no-store" in headers.get("Cache-Control", "")
      and headers.get("X-Content-Type-Options") == "nosniff", (status, headers))
status, part, _ = wf_call("GET", f"{L}/{fid}?at=8", keyfile=member)
check("files: ?at=N answers from that byte", status == 200 and part == png[8:])
check("files: its bytes are refused to anyone else", wf_call("GET", f"{L}/{fid}", keyfile=stranger)[0] == 403 and wf_call("GET", f"{L}/{fid}")[0] == 401)
status, body, _ = wf_call("GET", "/api/projects/verse/files", keyfile=member)
check("files: the project's page lists it", status == 200 and body.decode().startswith("world.files.v1 - verse 1 ") and fid in body.decode(), body)

# The scan and the limits.
exe = b"MZ\x90\x00" + bytes(2000)
he = hashlib.sha256(exe).hexdigest()
ask2 = f"world.file.upload.v1 world {len(exe)} Concept.png\n{he}\n".encode()
status, body, _ = wf_call("POST", L, ask2, keyfile=member)
urllib.request.urlopen(urllib.request.Request(body.decode().split(" ")[4].strip(), data=exe, method="PUT")).read()
status, body, _ = wf_call("POST", L + "/done", ask2, keyfile=member)
check("files: a program called an image is refused, and its object deleted", status == 422 and "program" in body.decode()
      and f"test/attachments/{he}" not in wf_bucket["objects"], body)
other = b"\x89PNG\r\n\x1a\nnot the file"
ask3 = f"world.file.upload.v1 world {len(other)} City.png\n{hashlib.sha256(b'something else!!!!!!').hexdigest()}\n".encode()
status, body, _ = wf_call("POST", L, ask3, keyfile=member)
urllib.request.urlopen(urllib.request.Request(body.decode().split(" ")[4].strip(), data=other, method="PUT")).read()
check("files: bytes that are not the ones named are refused", wf_call("POST", L + "/done", ask3, keyfile=member)[0] == 422)
status, body, _ = wf_call("POST", L, f"world.file.upload.v1 world 26214401 Huge.png\n{h}\n".encode(), keyfile=member)
check("files: a file over 25 MB is refused in words that name the limit", status == 422 and "A file can be 25 MB at most." in body.decode(), body)
status, body, _ = wf_call("POST", L, f"world.file.upload.v1 world 10 run.exe\n{h}\n".encode(), keyfile=member)
check("files: a kind we don't take is refused", status == 422 and "not a file we attach" in body.decode(), body)
status, body, _ = wf_call("POST", L, b"world.file.attach.v1 world 5 0 a.md\n-\nhello", keyfile=member)
check("files: a file in the request is told to go by direct upload", status == 415 and "direct upload" in body.decode(), body)

# A link, an edit and a detach.
status, body, _ = wf_call("POST", L, b"world.file.link.v1 world https://docs.google.com/document/d/x\nname The design doc\nt design\n", keyfile=member)
lid = body.decode().rsplit("did linked ", 1)[-1].strip()
check("files: a link is attached to the world", status == 200 and " world link 0 - " in body.decode() and "\nu https://docs.google.com/document/d/x\n" in body.decode(), body)
status, body, _ = wf_call("POST", L, f"world.file.edit.v1 {fid} world\nname Garage.png\nn Night\n".encode(), keyfile=member)
check("files: an edit renames it, moves it to the world and replaces its note and tags", status == 200
      and f" world image {len(png)} {h} " in body.decode() and "Garage.png\nn Night\n" in body.decode() and "garage,raiders" not in body.decode(), body)
check("files: a rename keeps the extension", wf_call("POST", L, f"world.file.edit.v1 {fid} world\nname Garage.md\n".encode(), keyfile=member)[0] == 422)
check("files: a stranger may not change one", wf_call("POST", L, f"world.file.detach.v1 {fid}\n".encode(), keyfile=stranger)[0] == 403)
status, body, _ = wf_call("POST", L, f"world.file.detach.v1 {fid}\n".encode(), keyfile=member)
check("files: a detach removes it and its stored file", status == 200 and body.decode().endswith(f"did detached {fid}\n") and fid not in body.decode().split("did")[0]
      and f"test/attachments/{h}" not in wf_bucket["objects"], body)
check("files: the link stays", lid in wf_call("GET", L, keyfile=admin)[1].decode())

# The realm command, on the API: the first part is told to go by direct upload, so it names the file,
# PUTs it in parts of 256 KiB and says it is done. Run when the CLI is built (apps/openagents-cli/build.sh --cli).
cli = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "apps", "openagents-cli", "build", "openagents")
if os.path.exists(cli):
    doc = tempfile.NamedTemporaryFile(delete=False, suffix=".md")
    doc.write(b"# Garage notes\n" + b"The raiders keep their rovers under one roof.\n" * 14000)
    doc.close()
    env = dict(os.environ, OPENAGENTS_NSEC=open(member).read().strip(), BROWSER="/usr/bin/true")
    run = subprocess.run([cli, "realm", "attach", "splice", doc.name, "--scope", "project", "--name", "garage.md", "--origin", API],
                         capture_output=True, text=True, env=env, timeout=120)
    listed = wf_call("GET", L, keyfile=member)[1].decode()
    big = hashlib.sha256(open(doc.name, "rb").read()).hexdigest()
    check("files: the attach command sends a file by direct upload in parts", run.returncode == 0 and " project document " in listed
          and "garage.md" in listed and wf_bucket["objects"].get(f"test/attachments/{big}") == open(doc.name, "rb").read(), (run.stdout, run.stderr))
    os.unlink(doc.name)

# A hosted world turn opens attachments (chat.c /api/chat/complete with "attachments"; the editor's Gemini turn): each
# joins the request's last message, the text as text and an image as the file itself, before the model is asked; a
# link is its address; a turn reads 8 MB at most. Only those who may see the attachments may open them.
if "infer_seen" in globals():   # test_inference.py's fake model, when it ran
    def wf_put(scope, name, data):
        a = f"world.file.upload.v1 {scope} {len(data)} {name}\n{hashlib.sha256(data).hexdigest()}\n".encode()
        s1, b1, _ = wf_call("POST", L, a, keyfile=member)
        if s1 == 200 and b" put " in b1:
            urllib.request.urlopen(urllib.request.Request(b1.decode().split(" ")[4].strip(), data=data, method="PUT")).read()
        t = wf_call("POST", L + "/done", a, keyfile=member)[1].decode()
        return t.split("did attached ", 1)[1].strip() if "did attached " in t else ""
    note = b"# The raiders' garage\nA low hangar of rusted panels; three rovers under one roof.\n"
    tid, iid = wf_put("world", "garage-note.md", note), wf_put("world", "garage-art.png", png)
    infer_mode["status"] = 200
    sql(f"delete from inference.account_day where pubkey = '{infer_admin}'")
    def wf_turn(ids, keyfile=admin):
        ask = {"model": "google/gemini-3.8-flash", "stream": False, "messages": [{"role": "system", "content": "the world"}, {"role": "user", "content": "what does the garage look like?"}],
               "attachments": {"world": "splice", "ids": ids}}
        before = len(infer_seen)
        s1, b1, _ = wf_call("POST", "/api/chat/complete", json.dumps(ask).encode(), keyfile=keyfile)
        return s1, b1.decode(), (infer_seen[-1]["body"] if len(infer_seen) > before else None)
    s1, b1, sent = wf_turn([tid, iid, lid])
    parts = sent["messages"][-1]["content"] if sent else []
    check("files: a world turn's opened attachments join its last message: the note as text, the image as itself, the link as its address",
          s1 == 200 and "attachments" not in sent and parts[0] == {"type": "text", "text": "what does the garage look like?"}
          and parts[1]["text"] == "Attachment garage-note.md:" and parts[2]["text"] == note.decode()
          and parts[3]["text"] == "Attachment garage-art.png:" and parts[4]["image_url"]["url"] == "data:image/png;base64," + __import__("base64").b64encode(png).decode()
          and parts[5]["text"].startswith("Attachment ") and "Its page was not opened." in parts[5]["text"] and len(parts) == 6, (s1, b1[:200], parts and [p.get("text", "")[:60] for p in parts]))
    check("files: the system message is left as it was", sent and sent["messages"][0] == {"role": "system", "content": "the world"})
    s1, b1, sent = wf_turn([iid], keyfile=stranger)
    check("files: someone who may not see them opens none, and the model is not asked", s1 in (401, 403) and sent is None, (s1, b1[:200]))
    s1, b1, sent = wf_turn(["0011223344556677"])
    check("files: an attachment no longer there is said in words", s1 == 200 and sent["messages"][-1]["content"][1]["text"] == "That attachment is no longer on this world.", (s1, b1[:200]))
    s1, b1, sent = wf_turn([tid, tid, tid, tid])
    check("files: a turn opens three at most", s1 == 400 and sent is None, (s1, b1[:200]))
    big = b"\x89PNG\r\n\x1a\n" + bytes(9 * 1024 * 1024)
    bid = wf_put("world", "huge.png", big)
    s1, b1, sent = wf_turn([bid])
    check("files: a file over what a turn reads is said to be too large, and not read", s1 == 200 and sent["messages"][-1]["content"][1]["text"]
          == "Attachment huge.png is 10 MB. A turn reads 8 MB of attachments at most, so it was not read." and len(sent["messages"][-1]["content"]) == 2, (s1, b1[:200]))
    for _id in (tid, iid, bid):
        wf_call("POST", L, f"world.file.detach.v1 {_id}\n".encode(), keyfile=member)

# Attachments are private and never reach a player or a pack (INVARIANTS.md).
check("files: attaching changed no pack and no world document",
      sql("SELECT count(*) || ':' || coalesce(string_agg(hash, ',' ORDER BY hash), '') FROM verse.world_packs") == packs_before
      and sql("SELECT coalesce(string_agg(id || dev_rev || dev_pack || live_pack, ',' ORDER BY id), '') FROM verse.world_docs") == docs_before)
status, body, _ = wf_call("GET", "/api/worlds/splice/assets/pack")
check("files: a player's pack has no attachment in it", h.encode() not in body and png[8:64] not in body)
check("files: nothing about a file is in the service's log", os.path.exists(os.environ.get("API_LOG", "/nonexistent")) is False
      or "Raider garage" not in open(os.environ["API_LOG"], errors="replace").read())
wf_server.shutdown()
wf_server.server_close()
