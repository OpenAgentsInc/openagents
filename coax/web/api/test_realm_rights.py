# Every realm edits the way Build your own realm does (byo-every-realm; realms.c,
# migrations/0050_realm_edit.sql): every realm in realms.json has an owner account and admins, and
# the server decides, for any slug, who may switch the board, change the settings, place at once and
# settle cards. test.py runs this file after test_realm_builder.py (call, check, sql, pt, nip98,
# b_call, b_player and builder_fake come from them).


def realm_rights_checks():
    here = os.path.dirname(os.path.abspath(__file__))
    realms = json.load(open(os.path.join(here, "..", "..", "..", "apps", "openagents-web", "realms.json")))
    hand = {w["realm"]["slug"]: w["realm"]["edit"] for w in realms["worlds"] if "realm" in w and "edit" in w["realm"]}
    seeded = {}
    for line in sql("select slug || '|' || owner_account || '|' || array_to_string(admins, ',') || '|' || array_to_string(tools, ',') || '|' "
                    "|| array_to_string(things, ',') || '|' || (select string_agg(r->>'id', ',' order by o) from jsonb_array_elements(rows) with ordinality t(r, o)) "
                    "from verse.realm_rights").split("\n"):
        if line:
            f = line.split("|")
            seeded[f[0]] = f[1:]
    want = {s: [e["owner"], ",".join(e.get("admins", [])), ",".join(e.get("tools", [])), ",".join(e.get("things", [])),
                ",".join(r["id"] for r in e["rows"])] for s, e in hand.items()}
    check("every hand-built realm in realms.json has its owner, admins, tools, things and rows on the server, the same",
          seeded == want and {"everglade", "the-grid", "hecates-cemetery", "fairy-forest", "dead-gears", "ice-cream-man", "haunted-house"} <= set(seeded),
          (seeded, want))

    # a hand-built realm of the test's own: its owner joins by email, an admin account, a site admin
    owner = b_player()
    pt("POST", "join", {"email": "realm-owner@example.com", "how": "email"}, key=owner)
    friend = b_player()
    pt("POST", "join", {"email": "realm-friend@example.com", "how": "email"}, key=friend)
    acct = lambda key: pt("GET", "player", key=key)[1]["account"]
    visitor = b_player()
    sql("insert into verse.realm_rights (slug, owner_account, admins, tools, things, rows) values ('test-hand', '" + acct(owner) + "', '{" + acct(friend)
        + "}', '{build,jump}', '{crate,lantern}', '[{\"id\":\"robots\",\"label\":\"Robots on their rounds\",\"says\":\"robots walking\"},"
        + "{\"id\":\"fog\",\"label\":\"Fog\",\"says\":\"a low fog\"}]')")
    roles = [b_call("GET", "/api/playtest/realms/test-hand", key=k)[1].split("\n")[1] for k in (owner, friend, visitor)]
    check("the owner, an admin account and anyone else", roles == ["role owner", "role admin", "role visitor"], roles)
    path = "/api/playtest/realms/test-hand/switch"
    body = json.dumps({"row": "robots", "on": False}).encode()
    st, raw, _ = call("POST", path, body, {"Content-Type": "application/json", "Authorization": nip98(os.environ["ADMIN_KEY"], "POST", "realms/test-hand/switch", body)})
    check("a site admin switches a row of a hand-built realm", st == 200 and json.loads(raw)["on"] is False, (st, raw))
    st, spec = b_call("GET", "/api/playtest/realms/test-hand", key=visitor)
    check("everyone's spec has it off: rows, the tools its code honours, what may be placed; no cards for a visitor",
          "row robots 0 Robots on their rounds\n" in spec and "row fog 1 Fog\n" in spec and "tool build 1\ntool jump 1\n" in spec
          and "thing crate\nthing lantern\n" in spec and "waiting" not in spec and "owner 0" in spec, spec)
    for who, k in (("a visitor", visitor),):
        check(who + " may not switch, change settings, see or settle cards",
              b_call("POST", path, {"row": "fog", "on": False}, k)[0] == 403
              and b_call("POST", "/api/playtest/realms/test-hand/settings", {"tools": {"jump": False}}, k)[0] == 403
              and b_call("GET", "/api/playtest/realms/test-hand/proposals", key=k)[1] == "")
    check("the owner and an admin account switch rows", b_call("POST", path, {"row": "fog", "on": False}, owner)[0] == 200
          and b_call("POST", path, {"row": "robots", "on": True}, friend)[0] == 200)
    check("an unknown row is a 404", b_call("POST", path, {"row": "dragons", "on": True}, owner)[0] == 404)
    st, got = b_call("POST", "/api/playtest/realms/test-hand/settings", {"tools": {"jump": False, "fly": True}}, friend)
    check("an admin changes the settings, only the tools its code honours", st == 200 and got["tools"] == {"build": True, "jump": False}, got)
    st, got = b_call("POST", "/api/playtest/realms/test-hand/place", {"thing": "crate", "x": 2, "z": -3}, visitor)
    check("a visitor's thing waits as a card", st == 200 and "proposed" in got, got)
    check("a thing the realm doesn't place is refused", b_call("POST", "/api/playtest/realms/test-hand/place", {"thing": "dragon", "x": 2, "z": 3}, visitor)[0] == 400)
    st, cards = b_call("GET", "/api/playtest/realms/test-hand/proposals", key=friend)
    pid = cards.split("|")[0][2:] if isinstance(cards, str) and cards.startswith("P ") else "0"
    check("an admin sees the card", "|crate|2|-3|0|0|1" in cards, cards)
    check("a visitor can't settle it; an admin accepts it once",
          b_call("POST", "/api/playtest/realms/test-hand/proposals/" + pid + "/accept", key=visitor)[0] == 403
          and b_call("POST", "/api/playtest/realms/test-hand/proposals/" + pid + "/accept", key=friend)[0] == 200
          and b_call("POST", "/api/playtest/realms/test-hand/proposals/" + pid + "/decline", key=owner)[0] == 409)
    st, spec = b_call("GET", "/api/playtest/realms/test-hand", key=owner)
    check("accepted, it is placed; the owner is told nothing waits", "placed crate 2 -3 0\n" in spec and "waiting 0" in spec and "tool jump 0" in spec, spec)
    check("the owner places at once", b_call("POST", "/api/playtest/realms/test-hand/place", {"thing": "lantern", "x": 1, "z": 1}, owner)[1].get("placed") is True)
    b_call("POST", "/api/playtest/realms/test-hand/settings", {"tools": {"build": False}}, owner)
    check("build off stops a visitor's card", b_call("POST", "/api/playtest/realms/test-hand/place", {"thing": "crate", "x": 2, "z": 3}, visitor)[0] == 403)
    check("a realm nobody has is a 404", b_call("GET", "/api/playtest/realms/no-such-realm")[0] == 404
          and b_call("POST", "/api/playtest/realms/no-such-realm/switch", {"row": "x", "on": True}, owner)[0] == 404)

    # a hand-built realm's admin account publishes its world (Everglade's edits) as a site admin does
    edits = {"everglade": 1, "edits": "eg-edit 1\nt 1 4.50 -10.00 0.79 1.00\n"}
    check("a realm's visitor can't publish its world", pt("POST", "world/everglade", {"doc": edits, "summary": "a visitor"}, key=friend)[0] == 403)
    vpath = "/api/playtest/realm/everglade/edit"
    check("a realm's visitor can't make a voice edit there", b_call("POST", vpath, {"sentence": ""}, friend)[0] == 403)
    sql("update verse.realm_rights set admins = admins || '{" + acct(friend) + "}' where slug = 'everglade'")
    check("Everglade's admin account makes voice edits there (past the gate: an empty sentence is 400), not on another realm's",
          b_call("POST", vpath, {"sentence": ""}, friend) == (400, {"error": "say what to change"})
          and b_call("POST", "/api/playtest/realm/the-grid/edit", {"sentence": ""}, friend)[0] == 403,
          (b_call("POST", vpath, {"sentence": ""}, friend), b_call("POST", "/api/playtest/realm/the-grid/edit", {"sentence": ""}, friend)))
    st, got = pt("POST", "world/everglade", {"doc": edits, "summary": "Placed a barrel"}, key=friend)
    check("Everglade's admin account publishes its edits, and puts the designed city back", st == 200 and "version" in got
          and pt("POST", "world/everglade/reset", {}, key=friend)[0] == 200 and pt("POST", "world/the-grid", {"doc": edits, "summary": "x"}, key=friend)[0] == 403, (st, got))
    sql("update verse.realm_rights set admins = array_remove(admins, '" + acct(friend) + "') where slug = 'everglade'")

    # talking revises the board: the model answers under the realm's rows
    builder_fake.server_close()
    fake = http.server.ThreadingHTTPServer(("127.0.0.1", int(os.environ["BUILDER_PORT"])), FakeBuilderModel)
    threading.Thread(target=fake.serve_forever, daemon=True).start()
    builder_seen.clear()
    builder_mode["status"] = 200
    builder_mode["text"] = json.dumps({"features": [{"id": "fog", "on": True}, {"id": "robots", "on": False}, {"id": "dragons", "on": True}]})
    check("a visitor can't revise", b_call("POST", "/api/playtest/realms/test-hand/revise", {"prompt": "fog back, no robots"}, visitor)[0] == 403)
    st, raw, _ = call("POST", "/api/playtest/realms/test-hand/revise?as=lines", json.dumps({"prompt": "fog back, no robots"}).encode(),
                      {"Content-Type": "application/json", "X-Verse-Player": owner})
    text = raw.decode()
    asked = builder_seen[-1] if builder_seen else {}
    enum = asked.get("generationConfig", {}).get("responseSchema", {}).get("properties", {}).get("features", {}).get("items", {}).get("properties", {}).get("id", {}).get("enum")
    check("the model is asked under an enum of the realm's rows, with the board as it is", enum == ["robots", "fog"]
          and '"on": false' in str(asked.get("contents")) and "fog back, no robots" in str(asked.get("contents")), asked)
    check("its answer is the realm now (an unknown row dropped), as lines", st == 200 and text.startswith("plan ")
          and "row robots 0 Robots on their rounds\nrow fog 1 Fog\n" in text, (st, text))

    # a made realm: an admin may as its owner does; its board's rows switch and come back
    time.sleep(2.1)
    maker = b_player()
    builder_mode["text"] = json.dumps({"name": "Rights Glade", "style": "lowpoly", "ground": "meadow", "time": "day", "features": [
        {"feature": "water", "on": True, "size": "small"}, {"feature": "rocks", "on": True, "count": 4},
        {"feature": "props", "on": True, "variant": "wagon"}, {"feature": "tool", "on": True, "tool": "build"}]})
    st, plan = b_call("POST", "/api/playtest/realms/plan", {"prompt": "a glade"}, maker)
    st, made = b_call("POST", "/api/playtest/realms", {"plan": plan.get("plan"), "on": [True, True, True, True]}, maker)
    slug = made.get("slug", "")
    st, spec = b_call("GET", "/api/playtest/realms/" + slug, key=maker)
    check("a made realm's spec has its role and its board's rows", "role owner\n" in spec and "row water 1 Small pond\n" in spec and "row props:wagon 1 A wagon\n" in spec, spec)
    check("a visitor can't switch a made realm's row", b_call("POST", "/api/playtest/realms/" + slug + "/switch", {"row": "rocks", "on": False}, visitor)[0] == 403)
    check("its owner switches one off", b_call("POST", "/api/playtest/realms/" + slug + "/switch", {"row": "rocks", "on": False}, maker)[0] == 200)
    st, spec = b_call("GET", "/api/playtest/realms/" + slug)
    check("off, it is gone from the realm and kept on the board", "feature rocks" not in spec and "row rocks 0 4 rocks\n" in spec and "role visitor" in spec, spec)
    adm = {"Content-Type": "application/json"}
    body = json.dumps({"row": "rocks", "on": True}).encode()
    st, raw, _ = call("POST", "/api/playtest/realms/" + slug + "/switch", body, dict(adm, Authorization=nip98(os.environ["ADMIN_KEY"], "POST", "realms/" + slug + "/switch", body)))
    st2, spec = b_call("GET", "/api/playtest/realms/" + slug)
    check("a site admin switches it back on", st == 200 and "feature rocks 4" in spec, (st, raw, spec))
    builder_mode["text"] = json.dumps({"name": "ignored", "style": "painterly", "ground": "snow", "time": "night", "features": [
        {"feature": "water", "on": False, "size": "small"}, {"feature": "lanterns", "on": True, "colour": "blue", "count": 6}]})
    time.sleep(2.1)
    st, got = b_call("POST", "/api/playtest/realms/" + slug + "/revise", {"prompt": "night, snow, blue lanterns, no pond"}, maker)
    st2, spec = b_call("GET", "/api/playtest/realms/" + slug)
    check("talking revises a made realm: its ground, time and features, its name and style kept",
          st == 200 and "ground snow" in spec and "time night" in spec and "feature lanterns blue 6" in spec and "row water 0" in spec
          and "name Rights Glade" in spec and "style lowpoly" in spec, (got, spec))
    fake.shutdown()
    fake.server_close()


realm_rights_checks()
