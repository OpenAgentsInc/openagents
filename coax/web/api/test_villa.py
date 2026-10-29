# Planetary Villa's pets (villa.c, 0028_villa_pets.sql): a pet made, its photo
# uploaded to the fake bucket (test_project_drops.py serves it) and scanned,
# the avatar agent, PLAY placing its tombstone, the memorial and its notes, the
# share link, and privacy. test.py runs this file with its helpers.

villa_asked = []
villa_answer = {"species": "dog", "size": 0.5, "legs": 0.8, "body": 1.5, "girth": 1, "head": 1, "snout": 1.2, "ears": 1,
                "ear_size": 1.4, "tail": 1, "fluff": 1, "pattern": "socks", "coat": [0.3, 0.15, 0.05], "marks": [0.9, 0.9, 0.9],
                "choices": [{"part": "body", "why": "Her long back in the photo."}], "extra": 9}
villa_emote = {"name": "Spin Twice!", "label": "spin", "text": "motion spin_twice; rig animal; seconds 1.6; once; still; at 0.8; pelvis 0 -180 0; at 1.6; pelvis 0 180 0"}
_villa_post = FakeGcs.do_POST


def _villa_do_post(self):
    if self.path == "/villa-model":
        asked = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        villa_asked.append(asked)
        emote = "animation" in asked["systemInstruction"]["parts"][0]["text"]
        said = json.dumps(villa_emote if emote else villa_answer)
        return self.answer(200, json.dumps({"candidates": [{"content": {"parts": [{"text": said}]}}]}).encode())
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

# Emotes the agent writes from a sentence (0032_villa_emotes.sql).
check("villa: an emote needs words", v("POST", "pets/%s/emote" % pid, kiki, {"said": ""})[0] == 400)
check("villa: someone else can't ask for one", v("POST", "pets/%s/emote" % pid, guest, {"said": "spins"})[0] == 404)
s, em = v("POST", "pets/%s/emote" % pid, kiki, {"said": "She always spun twice before lying down."})
asked = villa_asked[-1]["contents"][0]["parts"][0]["text"]
check("villa: the agent writes a motion from what was said", s == 200 and em["motion"]["name"] == "spin_twice_" and em["motion"]["label"] == "SPIN"
      and em["motion"]["text"].startswith("motion spin_twice;") and "spun twice" in asked, em)
check("villa: one ask at a time", v("POST", "pets/%s/emote" % pid, kiki, {"said": "again"})[0] == 429)
__import__("time").sleep(4.1)
v("POST", "pets/%s/emote" % pid, kiki, {"said": "spins", "error": "line 5: pelvis yaw 400 is out of range", "last": "motion x"})
check("villa: a refused motion is asked again with the reason", "out of range" in villa_asked[-1]["contents"][0]["parts"][0]["text"])
s, pet = v("POST", "pets/%s/emote/keep" % pid, kiki, {"name": "spin_twice", "label": "spin", "text": em["motion"]["text"], "said": "She spun."})
check("villa: a motion that played is kept on the pet", s == 200 and [e["name"] for e in pet["emotes"]] == ["spin_twice"] and pet["emotes"][0]["label"] == "SPIN", pet)
for i in range(9):
    s, pet = v("POST", "pets/%s/emote/keep" % pid, kiki, {"name": "m%d" % i, "label": "M", "text": "motion m"})
check("villa: eight emotes at most, the newest kept", len(pet["emotes"]) == 8 and pet["emotes"][-1]["name"] == "m8" and pet["emotes"][0]["name"] == "m1", [e["name"] for e in pet["emotes"]])
s, pet = v("POST", "pets/%s/emote/m8/delete" % pid, kiki)
check("villa: an emote is taken off", s == 200 and "m8" not in [e["name"] for e in pet["emotes"]], pet)

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
# Reports and the review queue (0031_villa_reports.sql).
s, n2 = v("POST", "memorial/%s/notes" % pet["slug"], guest, {"text": "A note someone dislikes."})
note2 = str(n2["notes"][-1]["n"])
check("villa: a report needs a key", v("POST", "report", None, {"kind": "note", "ref": note2})[0] == 403)
check("villa: a report of nothing is refused", v("POST", "report", kiki, {"kind": "note", "ref": "999999"})[0] == 404)
r1, r2, r3 = villa_player(), villa_player(), villa_player()
check("villa: a note is reported", v("POST", "report", r1, {"kind": "note", "ref": note2, "why": "rude"})[0] == 200)
v("POST", "report", r1, {"kind": "note", "ref": note2})
v("POST", "report", r2, {"kind": "note", "ref": note2})
check("villa: two reports (one twice) don't hold it", any(x["text"] == "A note someone dislikes." for x in v("GET", "memorial/" + pet["slug"])[1]["notes"]))
v("POST", "report", r3, {"kind": "note", "ref": note2})
check("villa: three people's reports hold it", not any(x["text"] == "A note someone dislikes." for x in v("GET", "memorial/" + pet["slug"])[1]["notes"]))
for who in (r1, r2, r3):
    v("POST", "report", who, {"kind": "memorial", "ref": pet["slug"]})
check("villa: reports never hide a memorial", v("GET", "memorial/" + pet["slug"])[0] == 200)
hv = {"Content-Type": "application/json", "X-Verse-Player": guest}
call("POST", "/api/playtest/chat/hecates-cemetery", json.dumps({"text": "a rude line"}).encode(), hv)
line = str(json.loads(call("GET", "/api/playtest/chat/hecates-cemetery?after=0")[1])["last"])
for who in (r1, r2, r3):
    v("POST", "report", who, {"kind": "chat", "ref": line})
said = json.loads(call("GET", "/api/playtest/chat/hecates-cemetery?after=0")[1])["messages"]
check("villa: three reports hold a chat line", [m["text"] for m in said if str(m["n"]) == line] == ["This line is waiting to be looked at"], said)
check("villa: the queue is for admins only", v("GET", "reports", kiki)[0] == 401)
# an admin's session, as a login leaves one (test_private.py has taken the name signed())
acookie = "oa_admin=" + "a" * 64
sql("insert into analytics.admin_sessions (token_sha256, pubkey, created_ms, expires_ms) values (encode(sha256(convert_to('%s', 'UTF8')), 'hex'), '%s', 0, 99999999999999) on conflict do nothing"
    % ("a" * 64, os.environ["OA_ADMIN_PUBKEYS"].split(",")[0]))


def va(method, path):
    status, raw, _ = call(method, "/api/villa/" + path, b"{}" if method == "POST" else None, {"Content-Type": "application/json", "Cookie": acookie})
    return status, json.loads(raw)


s, q = va("GET", "reports")
kinds = sorted((x["kind"], x["reports"]) for x in q.get("open", []) if x["ref"] in (note2, pet["slug"], line))
check("villa: an admin sees each reported thing once, with how many reported it", s == 200 and kinds == [("chat", 3), ("memorial", 3), ("note", 3)], q)
check("villa: what the reviewer reads is what it said", any(x["kind"] == "note" and "dislikes" in x["shown"] and x["why"] == ["rude"] for x in q["open"]), q)
check("villa: a kept note comes back", va("POST", "reports/note/%s/keep" % note2)[0] == 200
      and any(x["text"] == "A note someone dislikes." for x in v("GET", "memorial/" + pet["slug"])[1]["notes"]))
check("villa: a kept chat line comes back as it was", va("POST", "reports/chat/%s/keep" % line)[0] == 200
      and [m["text"] for m in json.loads(call("GET", "/api/playtest/chat/hecates-cemetery?after=0")[1])["messages"] if str(m["n"]) == line] == ["a rude line"])
v("POST", "report", r1, {"kind": "note", "ref": note2})
check("villa: a removed note is gone", va("POST", "reports/note/%s/remove" % note2)[0] == 200
      and not any(x["text"] == "A note someone dislikes." for x in v("GET", "memorial/" + pet["slug"])[1]["notes"]))
check("villa: decided, it leaves the queue", not any(x["ref"] == note2 for x in va("GET", "reports")[1]["open"]))
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
