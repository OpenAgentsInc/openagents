# What an owner asked for (realm-asks; migrations/0078_realm_asks.sql, realms.c): every realm's edit
# mode lists its features, each APPLIED or NOT APPLIED, with ADD and DELETE. An ADD goes through the
# board's revision (the model under the realm's rows); what it changed is kept, and DELETE puts that
# back and leaves nothing behind. test.py runs this file after test_realm_rights.py (call, check,
# sql, nip98, b_call, b_player, FakeBuilderModel, builder_mode and builder_seen come from before).


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
    fake.shutdown()
    fake.server_close()


realm_asks_checks()
