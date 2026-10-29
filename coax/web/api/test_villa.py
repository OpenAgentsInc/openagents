# Planetary Villa's pets (villa.c, 0028_villa_pets.sql): a pet made, its photo
# uploaded to the fake bucket (test_project_drops.py serves it) and scanned,
# the avatar agent, PLAY placing its tombstone, the memorial and its notes, the
# share link, and privacy. test.py runs this file with its helpers.

villa_asked = []
villa_answer = {"species": "dog", "size": 0.5, "legs": 0.8, "body": 1.5, "girth": 1, "head": 1, "snout": 1.2, "ears": 1,
                "ear_size": 1.4, "tail": 1, "fluff": 1, "pattern": "socks", "coat": [0.3, 0.15, 0.05], "marks": [0.9, 0.9, 0.9],
                "choices": [{"part": "body", "why": "Her long back in the photo."}], "extra": 9}
_villa_post = FakeGcs.do_POST


def _villa_do_post(self):
    if self.path == "/villa-model":
        villa_asked.append(json.loads(self.rfile.read(int(self.headers["Content-Length"]))))
        return self.answer(200, json.dumps({"candidates": [{"content": {"parts": [{"text": json.dumps(villa_answer)}]}}]}).encode())
    return _villa_post(self)


FakeGcs.do_POST = _villa_do_post
gcs_server.server_close()   # test_project_drops.py stopped it; this file serves the same fake bucket again
gcs_server = http.server.ThreadingHTTPServer(("127.0.0.1", int(os.environ["GCS_PORT"])), FakeGcs)
threading.Thread(target=gcs_server.serve_forever, daemon=True).start()


def villa_player():
    status, raw, _ = call("POST", "/api/playtest/player", b"{}", {"Content-Type": "application/json"})
    return json.loads(raw)["key"]


def v(method, path, key=None, body=None, origin=None):
    h = {"Content-Type": "application/json"}
    if key:
        h["X-Verse-Player"] = key
    if origin:
        h["Origin"] = origin
    status, raw, _ = call(method, "/api/villa/" + path, json.dumps(body).encode() if body is not None else (b"{}" if method == "POST" else None), h)
    try:
        return status, json.loads(raw)
    except ValueError:
        return status, raw.decode()


kiki, guest = villa_player(), villa_player()
check("villa: no key, no pets", v("GET", "pets")[0] == 403)
s, pet = v("POST", "pets", kiki, {"name": "Hecate\u0007", "species": "dog"})
check("villa: a pet is made, its name cleaned", s == 200 and pet["name"] == "Hecate" and pet["status"] == "draft" and len(pet["slug"]) == 8, pet)
pid = pet["id"]
check("villa: someone else can't see it", v("GET", "pets/" + pid, guest)[0] == 404)
check("villa: a write from another site is refused", v("POST", "pets/" + pid, kiki, {"line": "x"}, origin="https://evil.example")[0] == 403)
s, pet = v("POST", "pets/" + pid, kiki, {"born": "2012", "died": "2025", "line": "Queen of the garden", "story": "She dug holes.", "avatar": {"size": 9}})
check("villa: edits kept, avatar numbers held to range", s == 200 and pet["line"] == "Queen of the garden" and pet["avatar"]["size"] == 1.8, pet)

check("villa: a program is refused by name", v("POST", "pets/%s/files" % pid, kiki, {"name": "x.exe", "bytes": 10})[0] == 415)
check("villa: a big photo is refused", v("POST", "pets/%s/files" % pid, kiki, {"name": "a.jpg", "bytes": 17 << 20})[0] == 413)
s, up = v("POST", "pets/%s/files" % pid, kiki, {"name": "hecate.jpg", "bytes": 6})
check("villa: an upload is opened in the bucket", s == 200 and up["upload"].startswith(GCS + "/session/"), up)
urllib.request.urlopen(urllib.request.Request(up["upload"], method="PUT", data=b"\xff\xd8\xff\xe0ok"))
s, pet = v("POST", "pets/%s/files/%s/done" % (pid, up["file"]), kiki)
check("villa: a real JPEG is kept", s == 200 and pet["files"][0]["status"] == "kept", pet)
obj = [k for k in bucket["objects"] if "/villa/" + pid in k]
check("villa: it is under the environment's villa folder", obj and obj[0].startswith("test/villa/"), obj)
s, up2 = v("POST", "pets/%s/files" % pid, kiki, {"name": "fake.png", "bytes": 4})
urllib.request.urlopen(urllib.request.Request(up2["upload"], method="PUT", data=b"MZ\x90\x00"))
s, pet = v("POST", "pets/%s/files/%s/done" % (pid, up2["file"]), kiki)
check("villa: a program called a PNG is refused and leaves the bucket",
      [f["status"] for f in pet["files"]] == ["kept", "refused"] and not any(up2["file"] in k for k in bucket["objects"]), pet)

s, pet = v("POST", "pets/%s/agent" % pid, kiki)
parts = villa_asked[-1]["contents"][0]["parts"] if villa_asked else []
check("villa: the agent reads the story and the kept photo only",
      s == 200 and "She dug holes." in parts[0]["text"] and [p["fileData"]["fileUri"] for p in parts[1:]] == ["gs://drops-test/" + obj[0]], parts)
check("villa: its avatar and choices are kept", pet["agent"] == "ready" and pet["avatar"]["body"] == 1.5 and pet["avatar"]["pattern"] == "socks"
      and "extra" not in pet["avatar"] and pet["choices"][0]["part"] == "body", pet)

check("villa: the garden has no draft", all(p["slug"] != pet["slug"] for p in v("GET", "garden")[1]["pets"]))
check("villa: a memorial is not open before PLAY", v("GET", "memorial/" + pet["slug"])[0] == 404)
s, pet = v("POST", "pets/%s/place" % pid, kiki)
check("villa: PLAY gives it a plot, never an old stone's", s == 200 and pet["status"] == "placed" and pet["plot"] not in (0, 7, 59, 60), pet)
vg = v("GET", "garden", guest)[1]["pets"]
check("villa: everyone sees its tombstone", any(p["slug"] == pet["slug"] and p["plot"] == pet["plot"] and not p["mine"] for p in vg), vg)
s, m = v("GET", "memorial/" + pet["slug"])
check("villa: the memorial opens for anyone, without files", s == 200 and m["story"] == "She dug holes." and "files" not in m and "id" not in m, m)
check("villa: a note needs a key", v("POST", "memorial/%s/notes" % pet["slug"], None, {"text": "hi"})[0] == 403)
check("villa: a link in a note is refused", v("POST", "memorial/%s/notes" % pet["slug"], guest, {"text": "see www.spam.com"})[0] == 400)
s, n = v("POST", "memorial/%s/notes" % pet["slug"], guest, {"text": "She was the best girl."})
check("villa: a note is left", s == 200 and n["notes"][-1]["text"] == "She was the best girl.", n)
v("POST", "pets/%s/notes/%d/hide" % (pid, n["notes"][-1]["n"]), kiki)
check("villa: the owner hides it", v("GET", "memorial/" + pet["slug"])[1]["notes"] == [])
s, vpage = v("GET", "m/" + pet["slug"])
check("villa: the share link names her and goes on to the realm", s == 200 and "Hecate, in Hecate&#39;s Cemetery" not in vpage
      and "Hecate, in Hecate's Cemetery" in vpage and "#m/" + pet["slug"] in vpage and "og:image" in vpage, vpage[:200])
s, vgone = v("POST", "pets/%s/remove" % pid, kiki)
check("villa: removing takes the pet, its tombstone and its files", s == 200 and v("GET", "memorial/" + pet["slug"])[0] == 404
      and not any("/villa/" + pid in k for k in bucket["objects"]), bucket["objects"].keys())
gcs_server.shutdown()

# Skills and vases (0029_villa_play.sql).
s, pet2 = v("POST", "pets", kiki, {"name": "Bo"})
check("villa: a new pet has every skill", sorted(pet2["avatar"]["skills"]) == ["digging", "fetch", "obedience"], pet2)
s, pet2 = v("POST", "pets/" + pet2["id"], kiki, {"avatar": {"skills": ["fetch", "flying"]}})
check("villa: skills are only the known ones", pet2["avatar"]["skills"] == ["fetch"], pet2)
check("villa: an empty vase is refused", v("POST", "vases", kiki, {"x": 1, "z": 2, "flowers": []})[0] == 400)
s, vase = v("POST", "vases", kiki, {"x": 1, "z": 2, "yaw": 0.5, "flowers": [{"kind": 0, "slot": 0}, {"kind": 9, "slot": 1}, {"kind": 2, "slot": 3}], "plot": 12})
check("villa: a vase is left, its bad flower dropped", s == 200 and [f["kind"] for f in vase["flowers"]] == [0, 2] and vase["plot"] == 12 and vase["mine"], vase)
check("villa: one vase a minute", v("POST", "vases", kiki, {"x": 1, "z": 2, "flowers": [{"kind": 1, "slot": 0}]})[0] == 429)
vs = v("GET", "vases", guest)[1]["vases"]
check("villa: everyone sees it", any(x["n"] == vase["n"] and not x["mine"] for x in vs), vs)
v("POST", "vases/%d/clean" % vase["n"], guest)
check("villa: someone else can't clean it up", any(x["n"] == vase["n"] for x in v("GET", "vases")[1]["vases"]))
v("POST", "vases/%d/clean" % vase["n"], kiki)
check("villa: its maker cleans it up", not any(x["n"] == vase["n"] for x in v("GET", "vases")[1]["vases"]))
