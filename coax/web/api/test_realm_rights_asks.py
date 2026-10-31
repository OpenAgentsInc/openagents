# What an owner asked for (realm-asks; migrations/0078_realm_asks.sql, realms.c): every realm's edit
# mode lists its features, each APPLIED or NOT APPLIED, with ADD and DELETE. An ADD goes through the
# board's revision (the model under the realm's rows); what it changed is kept, and DELETE puts that
# back and leaves nothing behind. test.py runs this file after test_realm_rights.py (call, check,
# sql, nip98, b_call, b_player, FakeBuilderModel, builder_mode and builder_seen come from before).


import base64, io, zipfile


def realm_asks_checks():
    fake = http.server.ThreadingHTTPServer(("127.0.0.1", int(os.environ["BUILDER_PORT"])), FakeBuilderModel)
    threading.Thread(target=fake.serve_forever, daemon=True).start()
    builder_mode.update(status=200, texts=[], statuses=[])
    owner = b_player()
    pt("POST", "join", {"email": "asks-owner@example.com", "how": "email"}, key=owner)
    acct = pt("GET", "player", key=owner)[1]["account"]
    visitor = b_player()
    sql("insert into verse.realm_rights (slug, owner_account, rows) values ('test-asks', '" + acct + "', "
        "'[{\"id\":\"lights\",\"label\":\"Lights\",\"says\":\"lanterns\"},{\"id\":\"owls\",\"label\":\"Owls\",\"says\":\"owls in the trees\"}]')")
    base = "/api/playtest/realms/test-asks"

    def asks(key=owner):
        return b_call("GET", base + "/asks", key=key)[1]

    def add(words, answer, key=owner):
        time.sleep(2.1)   # one revision a realm every 2 seconds
        builder_mode["text"] = json.dumps(answer)
        st, raw, _ = call("POST", base + "/asks", json.dumps({"words": words}).encode(), {"Content-Type": "application/json", "X-Verse-Player": key})
        return st, raw.decode()

    check("a visitor can't add or read the list", add("no owls", {}, visitor)[0] == 403 and asks(visitor) == "")
    st, text = add("no owls", {"features": [{"id": "lights", "on": True}, {"id": "owls", "on": False}]})
    check("ADD goes to the model under the realm's rows, and an answer that changes the realm is APPLIED",
          st == 200 and text.split("\n")[0].endswith(" applied") and "row owls 0 Owls\n" in text
          and "no owls" in str(builder_seen[-1].get("contents")), (st, text))
    first = text.split()[1]
    st, text = add("a dragon over the gate", {"features": [{"id": "lights", "on": True}, {"id": "owls", "on": False}]})
    check("one the realm's code can't do yet changes nothing and waits, NOT APPLIED", st == 200 and text.split("\n")[0].endswith(" waiting"), text)
    dragon = text.split()[1]
    got = asks()
    check("the list, newest first: each ask, its state and its words",
          got == "ask " + dragon + " waiting ask a dragon over the gate\nask " + first + " applied ask no owls\n", got)

    # DELETE an applied ask: what it changed goes back, and it is gone
    check("a visitor can't delete one", b_call("POST", base + "/asks/" + first + "/delete", key=visitor)[0] == 403)
    st, got = b_call("POST", base + "/asks/" + first + "/delete", key=owner)
    spec = b_call("GET", base, key=owner)[1]
    check("DELETE puts the owls back and the ask leaves the list", st == 200 and got["reverted"] == 1 and "row owls 1 Owls\n" in spec
          and asks() == "ask " + dragon + " waiting ask a dragon over the gate\n" and sql("select switched from verse.realm_edit where slug = 'test-asks'") == "{}", (got, spec))
    check("deleting it again is a 404", b_call("POST", base + "/asks/" + first + "/delete", key=owner)[0] == 404)
    # a later change owns what it changed: DELETE leaves that alone
    st, text = add("lights off", {"features": [{"id": "lights", "on": False}, {"id": "owls", "on": True}]})
    lights = text.split()[1]
    b_call("POST", base + "/switch", {"row": "lights", "on": True}, owner)
    st, got = b_call("POST", base + "/asks/" + lights + "/delete", key=owner)
    check("an ask whose change was undone by hand since puts nothing back", st == 200 and got["reverted"] == 0 and "row lights 1" in b_call("GET", base, key=owner)[1], got)
    check("a waiting ask's DELETE just takes it off", b_call("POST", base + "/asks/" + dragon + "/delete", key=owner)[1].get("deleted") == int(dragon) and asks() == "")

    # DELETE a hand-built realm's row: its code has to come out, so it's off now and a remove ask waits
    st, got = b_call("POST", base + "/rows/owls/delete", key=owner)
    check("a hand-built row's DELETE switches it off and files a remove ask", st == 200 and got["state"] == "waiting"
          and "row owls 0 Owls\n" in b_call("GET", base, key=owner)[1] and asks() == "ask " + str(got["ask"]) + " waiting remove Take out Owls\n", (got, asks()))
    remove = str(got["ask"])
    check("asked twice, it's the same ask", b_call("POST", base + "/rows/owls/delete", key=owner)[1].get("ask") == int(remove))
    b_call("POST", base + "/asks/" + remove + "/delete", key=owner)
    check("DELETE on the remove ask puts the row back", "row owls 1 Owls\n" in b_call("GET", base, key=owner)[1] and asks() == "")
    check("an unknown row is a 404", b_call("POST", base + "/rows/bats/delete", key=owner)[0] == 404)

    # built in code: an agent (a site admin, NIP-98) says so with the commit; DELETE then waits for its code to come out
    st, text = add("a bell that rings at midnight", {"features": [{"id": "lights", "on": True}, {"id": "owls", "on": True}]})
    bell = text.split()[1]
    h = "a" * 40
    done = lambda aid, commit=h: call("POST", base + "/asks/" + aid + "/done", json.dumps({"commit": commit}).encode(),
                                      {"Content-Type": "application/json", "Authorization": nip98(os.environ["ADMIN_KEY"], "POST", "realms/test-asks/asks/" + aid + "/done", json.dumps({"commit": commit}).encode())})
    check("only a site admin marks one built", b_call("POST", base + "/asks/" + bell + "/done", {"commit": h}, owner)[0] == 403)
    check("and only with a whole commit hash", done(bell, "abc")[0] == 400)
    check("built in code, it is APPLIED", done(bell)[0] == 200 and asks() == "ask " + bell + " applied ask a bell that rings at midnight\n", asks())
    st, got = b_call("POST", base + "/asks/" + bell + "/delete", key=owner)
    check("its DELETE waits for its code to come out", st == 200 and got["state"] == "removing" and " removing " in asks(), (got, asks()))
    time.sleep(1.1)   # a NIP-98 signature is good once: the next one is a new second's
    check("and when that commit lands it is gone", done(bell)[0] == 200 and asks() == "", asks())

    # a made realm: a feature is data, and DELETE takes it off the board for good
    time.sleep(2.1)
    maker = b_player()
    builder_mode["text"] = json.dumps({"name": "Ask Glade", "style": "lowpoly", "ground": "meadow", "time": "day", "features": [
        {"feature": "water", "on": True, "size": "small"}, {"feature": "rocks", "on": True, "count": 4}]})
    st, plan = b_call("POST", "/api/playtest/realms/plan", {"prompt": "a glade"}, maker)
    st, made = b_call("POST", "/api/playtest/realms", {"plan": plan.get("plan"), "on": [True, True]}, maker)
    slug = made.get("slug", "")
    mbase = "/api/playtest/realms/" + slug
    time.sleep(2.1)
    builder_mode["text"] = json.dumps({"name": "x", "style": "lowpoly", "ground": "snow", "time": "night", "features": [
        {"feature": "water", "on": True, "size": "small"}, {"feature": "rocks", "on": True, "count": 4}, {"feature": "lanterns", "on": True, "colour": "blue", "count": 6}]})
    st, raw, _ = call("POST", mbase + "/asks", json.dumps({"words": "snowy night with blue lanterns"}).encode(), {"Content-Type": "application/json", "X-Verse-Player": maker})
    text = raw.decode()
    spec = b_call("GET", mbase, key=maker)[1]
    check("a made realm's ADD revises it and is APPLIED", st == 200 and text.split("\n")[0].endswith(" applied") and "feature lanterns blue 6" in spec and "ground snow" in spec, (text, spec))
    lanterns = text.split()[1]
    st, got = b_call("POST", mbase + "/asks/" + lanterns + "/delete", key=maker)
    spec = b_call("GET", mbase, key=maker)[1]
    check("DELETE takes the lanterns off the board entirely and puts the meadow and the day back",
          st == 200 and "lanterns" not in spec and "ground meadow" in spec and "time day" in spec and "feature rocks 4" in spec, (got, spec))
    st, got = b_call("POST", mbase + "/rows/rocks/delete", key=maker)
    spec = b_call("GET", mbase, key=maker)[1]
    check("a made realm's row DELETE removes the feature from it, on or off", st == 200 and "rocks" not in spec and "feature water small" in spec, (got, spec))

    # nothing asked for is dropped (0079): "only pastel colours, bison everywhere and tepees"
    time.sleep(2.1)
    herder = b_player()
    h = {"Content-Type": "application/json", "X-Verse-Player": herder}
    builder_mode["text"] = json.dumps({"asked": [{"thing": "only pastel colors", "feature": "setup"}, {"thing": "bison everywhere", "feature": "herd"}, {"thing": "tepees", "feature": "dwellings"},
                                                 {"thing": "a rainbow river", "feature": "none"}],
        "name": "Pastel Plains", "camera": "third", "style": "flat", "ground": "meadow", "time": "day", "palette": "pastel", "features": [
        {"feature": "herd", "on": True, "variant": "bison", "count": 8}, {"feature": "dwellings", "on": True, "variant": "tepee", "count": 5},
        {"feature": "village", "on": True, "count": 5}]})
    st, raw, _ = call("POST", "/api/playtest/realms/plan?as=lines", json.dumps({"prompt": "I want a world that is only pastel colors. and There's bison everywhere and tepees."}).encode(), h)
    text = raw.decode()
    req = builder_seen[-1]
    check("the planner is asked for a palette, and the catalogue has a herd, dwellings and a village",
          "palette" in req["generationConfig"]["responseSchema"]["required"] and "pastel" in json.dumps(req["generationConfig"]["responseSchema"])
          and "herd" in json.dumps(req["generationConfig"]["responseSchema"]) and "tepee" in req["systemInstruction"]["parts"][0]["text"], req["generationConfig"]["responseSchema"].get("required"))
    check("her prompt's board: pastel, a herd of bison, tepees, its people, and what no feature makes kept as wanted",
          st == 200 and "palette pastel\n" in text and "f herd 1 A herd of 8 bison\n" in text and "f dwellings 1 5 tepees\n" in text
          and "f village 1 A village of 5 people\n" in text and "want a rainbow river\n" in text, text)
    items = req["generationConfig"]["responseSchema"]["properties"]["asked"]["items"]
    check("each asked thing is said as said and then without its mood words; a whole place is its own kind (0088)",
          items["required"][:2] == ["said", "thing"] and "place" in items["properties"]["feature"]["enum"]
          and "building" in items["properties"]["feature"]["enum"] and "friendly owls" in req["systemInstruction"]["parts"][0]["text"], items)
    check("her colours are the setup's palette, not a line still wanted (0081)",
          "setup" in req["generationConfig"]["responseSchema"]["properties"]["asked"]["items"]["properties"]["feature"]["enum"]
          and "want only pastel" not in text, text)
    plan_id = int(text.split()[1])
    time.sleep(2.1)
    builder_mode["text"] = json.dumps({"asked": [{"thing": "bison", "feature": "herd"}], "name": "Pastel Plains", "camera": "third", "style": "flat",
        "ground": "meadow", "time": "day", "palette": "pastel", "features": [{"feature": "herd", "on": True, "variant": "bison", "count": 8},
        {"feature": "dwellings", "on": True, "variant": "tepee", "count": 5}, {"feature": "village", "on": True, "count": 5}]})
    st, raw, _ = call("POST", "/api/playtest/realms/plan?as=lines", json.dumps({"prompt": "bison", "plan": plan_id, "on": [True, True, True]}).encode(), h)
    text = raw.decode()
    check("a revision keeps what was wanted before", st == 200 and "want a rainbow river\n" in text, text)
    plan_id = int(text.split()[1])
    st, raw, _ = call("POST", "/api/playtest/realms?as=lines", json.dumps({"plan": plan_id, "on": [True, True, True]}).encode(), h)
    slug = [l[5:] for l in raw.decode().split("\n") if l.startswith("slug ")][0]
    spec = b_call("GET", "/api/playtest/realms/" + slug, key=herder)[1]
    got = b_call("GET", "/api/playtest/realms/" + slug + "/asks", key=herder)[1]
    check("MAKE IT makes it: pastel, the herd, the tepees, the village", "palette pastel\n" in spec and "feature herd bison 8\n" in spec
          and "feature dwellings tepee 5\n" in spec and "feature village 5\n" in spec, spec)
    check("and what it can't draw yet waits on its feature list, NOT APPLIED", got.endswith(" waiting ask a rainbow river\n"), got)
    # edit mode on it: a change of palette is undone by DELETE; what no feature makes waits as its own ask
    time.sleep(2.1)
    builder_mode["text"] = json.dumps({"asked": [{"thing": "brighter", "feature": "none"}, {"thing": "a waterfall", "feature": "none"}], "name": "x", "camera": "third", "style": "flat",
        "ground": "meadow", "time": "day", "palette": "vivid", "features": [{"feature": "herd", "on": True, "variant": "bison", "count": 8},
        {"feature": "dwellings", "on": True, "variant": "tepee", "count": 5}, {"feature": "village", "on": True, "count": 5}]})
    st, raw, _ = call("POST", "/api/playtest/realms/" + slug + "/asks", json.dumps({"words": "brighter, and a waterfall"}).encode(), h)
    text = raw.decode()
    vivid = text.split()[1]
    spec = b_call("GET", "/api/playtest/realms/" + slug, key=herder)[1]
    got = b_call("GET", "/api/playtest/realms/" + slug + "/asks", key=herder)[1]
    check("an ADD that changes the palette is APPLIED, and what it named that no feature makes waits on the list",
          text.split("\n")[0].endswith(" applied") and "palette vivid\n" in spec and " waiting ask a waterfall\n" in got
          and '"palette": "pastel"' in str(builder_seen[-1]["contents"][0]["parts"][0]["text"]), (text, got))
    b_call("POST", "/api/playtest/realms/" + slug + "/asks/" + vivid + "/delete", key=herder)
    check("DELETE puts the pastel back", "palette pastel\n" in b_call("GET", "/api/playtest/realms/" + slug, key=herder)[1])

    # a design document (0080): every item a line, applied where it can be drawn, the rest waiting
    def drop(path, name, data, key):
        time.sleep(2.1)
        st, raw, _ = call("POST", path, json.dumps({"name": name, "data": base64.b64encode(data).decode()}).encode(), {"Content-Type": "application/json", "X-Verse-Player": key})
        return st, raw.decode()
    builder_mode["text"] = json.dumps({"asked": [{"thing": "deer grazing", "feature": "herd"}, {"thing": "a night sky", "feature": "setup"},
                                                 {"thing": "a castle on the hill", "feature": "none"}],
        "name": "x", "camera": "third", "style": "flat", "ground": "meadow", "time": "night", "palette": "pastel", "features": [
        {"feature": "herd", "on": True, "variant": "deer", "count": 6}, {"feature": "dwellings", "on": True, "variant": "tepee", "count": 5},
        {"feature": "village", "on": True, "count": 5}]})
    st, text = drop("/api/playtest/realms/" + slug + "/asks/doc", "Plains GDD.md", b"# Plains\n- deer grazing\n- night sky\n- a castle on the hill\n", herder)
    seen = builder_seen[-1]["contents"][0]["parts"][0]["text"]
    got = b_call("GET", "/api/playtest/realms/" + slug + "/asks", key=herder)[1]
    check("a dropped .md: the model reads its words, and each thing it asks for is a line, APPLIED or NOT APPLIED",
          st == 200 and text.startswith("doc ") and " waiting ask a castle on the hill\n" in got and " applied ask deer grazing\n" in got
          and " applied ask a night sky\n" in got and "a castle on the hill" in seen and "Plains GDD.md" in seen, (st, text, got))
    spec = b_call("GET", "/api/playtest/realms/" + slug, key=herder)[1]
    deer = [l.split()[1] for l in got.split("\n") if l.endswith(" deer grazing")][0]
    night = [l.split()[1] for l in got.split("\n") if l.endswith(" a night sky")][0]
    check("and the realm has what it could draw", "feature herd deer 6\n" in spec and "time night\n" in spec, spec)
    b_call("POST", "/api/playtest/realms/" + slug + "/asks/" + night + "/delete", key=herder)
    spec = b_call("GET", "/api/playtest/realms/" + slug, key=herder)[1]
    check("DELETE on one line takes back only what it changed", "time day\n" in spec and "feature herd deer 6\n" in spec, spec)
    b_call("POST", "/api/playtest/realms/" + slug + "/asks/" + deer + "/delete", key=herder)
    spec = b_call("GET", "/api/playtest/realms/" + slug, key=herder)[1]
    check("and the herd's line puts the bison back", "feature herd bison 8\n" in spec, spec)
    # a .docx: its paragraphs are read out of word/document.xml
    import io, zipfile
    z = io.BytesIO()
    with zipfile.ZipFile(z, "w", zipfile.ZIP_DEFLATED) as f:
        f.writestr("[Content_Types].xml", "<Types/>")
        f.writestr("word/document.xml", '<?xml version="1.0"?><w:document><w:body><w:p><w:r><w:t>Snowy hills &amp; foxes</w:t></w:r></w:p>'
                   '<w:p><w:r><w:t>A lighthouse</w:t></w:r></w:p></w:body></w:document>')
    builder_mode["text"] = json.dumps({"asked": [{"thing": "a lighthouse", "feature": "none"}], "name": "x", "camera": "third", "style": "flat",
        "ground": "meadow", "time": "day", "palette": "pastel", "features": [{"feature": "herd", "on": True, "variant": "bison", "count": 8},
        {"feature": "dwellings", "on": True, "variant": "tepee", "count": 5}, {"feature": "village", "on": True, "count": 5}]})
    st, text = drop("/api/playtest/realms/" + slug + "/asks/doc", "gdd.docx", z.getvalue(), herder)
    seen = builder_seen[-1]["contents"][0]["parts"][0]["text"]
    check("a dropped .docx: its paragraphs are what the model reads", st == 200 and "Snowy hills & foxes\nA lighthouse" in seen, (st, text, seen[-200:]))
    st, text = drop("/api/playtest/realms/" + slug + "/asks/doc", "gdd.pdf", b"%PDF-1.4 a pdf", herder)
    parts = builder_seen[-1]["contents"][0]["parts"]
    check("a dropped .pdf goes to the model as itself", st == 200 and parts[1]["inlineData"]["mimeType"] == "application/pdf"
          and base64.b64decode(parts[1]["inlineData"]["data"]) == b"%PDF-1.4 a pdf", (st, text))
    check("anything else is refused, and a visitor can't drop one",
          drop("/api/playtest/realms/" + slug + "/asks/doc", "gdd.exe", b"MZ", herder)[0] == 415
          and drop("/api/playtest/realms/" + slug + "/asks/doc", "gdd.md", b"x", visitor)[0] == 403)
    # the builder plans a board from one, what it can't draw kept as wanted
    builder_mode["text"] = json.dumps({"asked": [{"thing": "horses", "feature": "herd"}, {"thing": "a dragon", "feature": "none"}], "name": "Doc Realm",
        "camera": "third", "style": "lowpoly", "ground": "meadow", "time": "day", "palette": "natural", "features": [{"feature": "herd", "on": True, "variant": "horse", "count": 4}]})
    st, text = drop("/api/playtest/realms/plan/doc?as=lines", "idea.txt", b"Horses, and a dragon.", herder)
    check("the builder plans the board from a dropped document", st == 200 and "f herd 1 A herd of 4 horses\n" in text and "want a dragon\n" in text
          and "Horses, and a dragon." in builder_seen[-1]["contents"][0]["parts"][0]["text"], (st, text))
    fake.shutdown()
    fake.server_close()


realm_asks_checks()

# The Grid's board names what its module draws (0082): the walkers and the names, no robots
grid_rows = sql("select rows::text from verse.realm_rights where slug = 'the-grid'")
check("the Grid's rows are its walkers and its names, its robots row gone",
      '"walkers"' in grid_rows and '"names"' in grid_rows and "robots" not in grid_rows, grid_rows)
