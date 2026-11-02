# FORK on a realm (ev-fork-realm; realms.c, migrations/0075_realm_forks.sql, docs/verse/realm-fork.md):
# anyone forks Everglade with rule sets its code implements; the fork is a made realm at its own
# address whose page loads Everglade's module with the fork's slug and rules, credits and links what
# it forks; the original never changes. Its owner edits it (switch, revise by voice through a fake
# model, held to its row ids) and publishes it. test.py runs this file after test_realm_edit.py with
# its helpers (call, check, sql) and test_realm_builder.py's (b_player, b_call, the fake model and
# the fake bucket).
import json
import threading
import time


def realm_fork_checks():
    global gcs_server
    gcs_server.server_close()   # test_realm_builder.py stopped it; this file serves the same fake bucket again
    gcs_server = http.server.ThreadingHTTPServer(("127.0.0.1", int(os.environ["GCS_PORT"])), FakeGcs)
    threading.Thread(target=gcs_server.serve_forever, daemon=True).start()
    builder_fake.server_close()
    model = http.server.ThreadingHTTPServer(("127.0.0.1", int(os.environ["BUILDER_PORT"])), FakeBuilderModel)
    threading.Thread(target=model.serve_forever, daemon=True).start()
    site = bucket["site"]
    R = "/api/playtest/realms/"

    def lines(raw):
        return raw if isinstance(raw, str) else json.dumps(raw)

    def line_map(raw):
        out = {}
        for ln in lines(raw).splitlines():
            k, _, v = ln.partition(" ")
            out.setdefault(k, []).append(v)
        return out

    # Everglade's published city (as its admins left it): a fork starts from it
    sql("""insert into verse.published (world, doc, version, published_by, published_at) values ('everglade', '{"everglade": 1, "edits": "eg-edit 1\\nt 1 4.50 -10.00 0.79 1.00\\n"}', 3, 'test', 1)
           on conflict (world) do nothing""")
    before_spec = b_call("GET", R + "everglade")[1]
    before_world = call("GET", "/api/playtest/world/everglade")[1]
    before_edit = sql("select coalesce((select switched::text || tools::text || placed::text from verse.realm_edit where slug = 'everglade'), '')")
    before_pages = sorted(n for n in site if n == "realms/everglade" or n.startswith("realms/everglade/"))

    st, info = b_call("GET", R + "everglade/fork")
    m = line_map(info)
    check("Everglade can be forked: its title, its maker credited, and the rule sets its code implements, all off",
          st == 200 and m.get("base") == ["everglade"] and m.get("fork") == ["0"] and m.get("maker") == ["OpenAgents"]
          and m.get("rule") == ["night 0 Always night", "waves 0 Hostile waves", "nodestroy 0 No destruction", "lowgrav 0 Low gravity"], info)
    check("a made realm that isn't a fork, and no realm at all, can't be forked",
          b_call("GET", R + "moonlit-marsh/fork")[0] == 404 and b_call("GET", R + "no-such-realm/fork")[0] == 404
          and b_call("POST", R + "the-grid/fork", {"rules": []}, b_player())[0] == 404)

    owner, other = b_player(), b_player()
    check("forking needs a player", b_call("POST", R + "everglade/fork", {"rules": ["night"]})[0] == 401)
    st, bad = b_call("POST", R + "everglade/fork", {"rules": ["night", "dragons"]}, owner)
    check("a rule Everglade doesn't implement is refused, by name", st == 400 and bad == {"error": "not a rule here: dragons"}, (st, bad))
    check("rules not as a list of words is refused too", b_call("POST", R + "everglade/fork", {"rules": [7]}, owner)[0] == 400)
    check("a name of no plain words is refused", b_call("POST", R + "everglade/fork", {"rules": [], "name": "<>"}, owner)[0] == 400)
    check("a fork from another site's page is refused", b_call("POST", R + "everglade/fork", {"rules": []}, owner, origin="https://evil.example")[0] == 403)

    st, f = b_call("POST", R + "everglade/fork", {"rules": ["waves", "night", "night"], "name": "Doomglade"}, owner)
    check("FORK makes a realm of its own at /realms/<slug>, with the rules chosen (once each, in Everglade's order), and writes its page",
          st == 200 and f["slug"] == "doomglade" and f["path"] == "/realms/doomglade" and f["base"] == "everglade" and f["from"] == "everglade"
          and f["rules"] == ["night", "waves"] and f["page"] is False, (st, f))
    page = site.get("realms/doomglade", {}).get("data", b"").decode()
    check("its page loads Everglade's module with this fork's slug, slot and rules",
          'data-fork="doomglade"' in page and 'data-base="everglade"' in page and 'data-rules="night,waves"' in page
          and 'fetch("/realms/" + me.dataset.base' in page and 's.dataset.slot = "made-" + slug' in page
          and "/api/playtest/realms/\" + slug + \"/card" in page, page[-900:])
    check("its page says what it was forked from, links to it and credits its maker",
          'Forked from <a href="/realms/everglade">Everglade</a>, made by OpenAgents.' in page, page[:2400])
    check("its line says what it forks and its rules, until its owner writes one",
          'content="A fork of Everglade on the Verse by OpenAgents: always night, hostile waves."' in page, page[:1600])
    check("unpublished, it is noindex and not on /og's rows",
          '<meta name="robots" content="noindex">' in page and "/realms/doomglade|" not in b_call("GET", R + "made")[1], page[:600])
    md = site.get("realms/doomglade/index.md", {}).get("data", b"").decode()
    check("its text names what it forks and its rules", "A fork of Everglade (https://openagents.com/realms/everglade), made by OpenAgents" in md
          and "- Always night\n- Hostile waves" in md and "No destruction" not in md, md)
    check("it is on the list of made pages the site keeps", b"doomglade" in site.get("realms/made-slugs.txt", {}).get("data", b""))

    st, ln = b_call("POST", R + "everglade/fork?as=lines", {"rules": ["lowgrav"], "name": "Doom <script>alert(1)</script>"}, other)
    check("?as=lines answers the slug and path as lines; a name's markup is taken out",
          st == 200 and lines(ln).startswith("slug doom-scriptalert-1-script\npath /realms/doom-scriptalert-1-script\n"), (st, ln))
    escaped = site.get("realms/doom-scriptalert-1-script", {}).get("data", b"").decode()
    check("and never reaches its page as markup", "<script>alert" not in escaped and "<h1>Doom scriptalert(1)/script</h1>" in escaped, escaped[2000:2600])
    st, plain = b_call("POST", R + "everglade/fork", {"rules": []}, other)
    check("no name: the base's name and Fork", st == 200 and plain["name"] == "Everglade Fork" and plain["slug"] == "everglade-fork" and plain["rules"] == [], plain)

    # the original never changes
    check("Everglade itself is unchanged: its spec, its world, its edits and its page",
          b_call("GET", R + "everglade")[1] == before_spec and call("GET", "/api/playtest/world/everglade")[1] == before_world
          and sql("select coalesce((select switched::text || tools::text || placed::text from verse.realm_edit where slug = 'everglade'), '')") == before_edit
          and sorted(n for n in site if n == "realms/everglade" or n.startswith("realms/everglade/")) == before_pages, sorted(n for n in site if n == "realms/everglade" or n.startswith("realms/everglade/")))

    st, city = b_call("GET", "/api/playtest/world/made-doomglade")
    check("its city starts as Everglade's published one, in its own world", st == 200 and city["doc"] == json.loads(before_world)["doc"]
          and city["doc"] is not None and city["version"] == 1, (city, before_world))
    st, _ = b_call("POST", "/api/playtest/world/made-doomglade", {"doc": {"everglade": 1, "edits": "eg-edit 1\n"}, "summary": "Took the barrel away"}, owner)
    check("its owner publishes the fork's own city; anyone else can't", st == 200
          and b_call("POST", "/api/playtest/world/made-doomglade", {"doc": {"everglade": 1, "edits": "eg-edit 1\n"}, "summary": "x"}, other)[0] == 403)
    check("a visitor can't edit the fork by voice", b_call("POST", "/api/playtest/realm/doomglade/edit", {"sentence": "a crate here"}, other)[0] == 403)
    check("its owner may (an empty sentence is a 400, not a 403)", b_call("POST", "/api/playtest/realm/doomglade/edit", {"sentence": ""}, owner)[0] == 400)

    # editing: the shared edit layer's spec and switches, by its owner only
    st, spec = b_call("GET", R + "doomglade", key=owner)
    check("the fork's spec gives its owner the board: Everglade's rows and its rules, on as chosen",
          st == 200 and "role owner" in spec and "row flock 1 " in spec and "row night 1 Always night" in spec and "row waves 1 Hostile waves" in spec
          and "row nodestroy 0 No destruction" in spec and "row lowgrav 0 Low gravity" in spec, spec)
    check("anyone else is a visitor there", "role visitor" in b_call("GET", R + "doomglade", key=other)[1])
    m = line_map(b_call("GET", R + "doomglade/fork", key=owner)[1])
    check("its FORK pane says it is a fork, of what, its rules and that this is its owner",
          m.get("fork") == ["1"] and m.get("from") == ["everglade|Everglade"] and m.get("owner") == ["1"] and m.get("published") == ["0"]
          and "night 1 Always night" in m.get("rule", []) and "lowgrav 0 Low gravity" in m.get("rule", []), m)
    check("and not anyone else's", line_map(b_call("GET", R + "doomglade/fork", key=other)[1]).get("owner") == ["0"])
    check("a visitor can't switch its rules", b_call("POST", R + "doomglade/switch", {"row": "nodestroy", "on": True}, other)[0] == 403)
    st, sw = b_call("POST", R + "doomglade/switch", {"row": "nodestroy", "on": True}, owner)
    check("its owner switches a rule on", st == 200 and sw == {"row": "nodestroy", "on": True}
          and "nodestroy 1 No destruction" in line_map(b_call("GET", R + "doomglade/fork")[1]).get("rule", []), (st, sw))
    st, sw = b_call("POST", R + "doomglade/switch", {"row": "flock", "on": False}, owner)
    check("and one of Everglade's rows off", st == 200 and "row flock 0 " in b_call("GET", R + "doomglade", key=owner)[1], sw)
    check("a row it hasn't is a 404", b_call("POST", R + "doomglade/switch", {"row": "robots", "on": True}, owner)[0] == 404)

    # talking revises it: the model answers under the fork's row ids, and only those are kept
    builder_seen.clear()
    builder_mode["status"] = 200
    builder_mode["text"] = json.dumps({"features": [{"id": "lowgrav", "on": True}, {"id": "night", "on": False}, {"id": "dragons", "on": True}]})
    time.sleep(2.1)
    check("a visitor can't revise it", b_call("POST", R + "doomglade/revise", {"prompt": "moon gravity"}, other)[0] == 403)
    st, rv = b_call("POST", R + "doomglade/revise", {"prompt": "moon gravity, and let the sun come up"}, owner)
    sent = builder_seen[-1] if builder_seen else {}
    enum = sent.get("generationConfig", {}).get("responseSchema", {}).get("properties", {}).get("features", {}).get("items", {}).get("properties", {}).get("id", {}).get("enum", [])
    check("the model is asked under a schema of the fork's row ids, told what each does",
          set(enum) == {"flock", "floats", "undergrowth", "daycycle", "guards", "invincible", "night", "waves", "nodestroy", "lowgrav"}
          and "gravity is a third of normal" in sent["systemInstruction"]["parts"][0]["text"], (enum, sent.get("systemInstruction")))
    m = line_map(b_call("GET", R + "doomglade/fork")[1])
    check("its answer is the fork now: low gravity on, night off; a row it hasn't is ignored",
          st == 200 and rv["changed"] == 2 and "lowgrav 1 Low gravity" in m["rule"] and "night 0 Always night" in m["rule"] and "waves 1 Hostile waves" in m["rule"], (st, rv, m))
    check("and Everglade is still unchanged", b_call("GET", R + "everglade")[1] == before_spec)

    # publishing: its owner lists it, with its own line
    check("a visitor can't publish it", b_call("POST", R + "doomglade/publish", {}, other)[0] == 403)
    check("only a made realm's owner publishes it (0093)", b_call("POST", R + "moonlit-marsh/publish", {"rev": 1}, owner)[0] == 403)
    st, pub = b_call("POST", R + "doomglade/publish", {"description": "Always night in Everglade, and the watch comes in waves. <b>Run.</b>"}, owner)
    check("its owner publishes it with its own line, markup taken out", st == 200 and pub == {"published": True, "description": "Always night in Everglade, and the watch comes in waves. bRun./b", "visibility": "public", "listed": True}, (st, pub))
    page = ""
    for _ in range(40):
        page = site.get("realms/doomglade", {}).get("data", b"").decode()
        if "noindex" not in page:
            break
        time.sleep(0.1)
    check("published, its page is written again: indexed, with its own line and still crediting Everglade",
          "noindex" not in page and 'content="Always night in Everglade, and the watch comes in waves. bRun./b"' in page
          and 'Forked from <a href="/realms/everglade">Everglade</a>' in page, page[:1200])
    rows = b_call("GET", R + "made")[1]
    check("and it is on /og's rows with its line, an unpublished fork still not", "/realms/doomglade|made||Doomglade|Always night in Everglade" in rows
          and "/realms/everglade-fork|" not in rows and "/realms/moonlit-marsh|" in rows, rows[:400])
    check("unpublishing takes it off again", b_call("POST", R + "doomglade/publish", {"published": False}, owner) == (200, {"published": False, "description": pub["description"], "visibility": "unlisted", "listed": False})
          and "/realms/doomglade|" not in b_call("GET", R + "made")[1])
    b_call("POST", R + "doomglade/publish", {}, owner)

    # a fork of a fork: it credits both
    st, ff = b_call("POST", R + "doomglade/fork", {"rules": ["waves"]}, other)
    check("a fork can be forked again: its base stays Everglade, it comes from the fork", st == 200 and ff["base"] == "everglade" and ff["from"] == "doomglade"
          and ff["name"] == "Doomglade Fork" and ff["rules"] == ["waves"], (st, ff))
    page2 = site.get("realms/" + ff.get("slug", "x"), {}).get("data", b"").decode()
    check("its page links what it was forked from and the realm that came from",
          'Forked from <a href="/realms/doomglade">Doomglade</a>, a fork of <a href="/realms/everglade">Everglade</a> made by OpenAgents.' in page2
          and 'data-base="everglade"' in page2 and "row flock 0 " in b_call("GET", R + ff.get("slug", "x"), key=other)[1], page2[2000:2600])
    check("the fork it came from is unchanged", "waves 1 Hostile waves" in line_map(b_call("GET", R + "doomglade/fork")[1])["rule"]
          and "lowgrav 1 Low gravity" in line_map(b_call("GET", R + "doomglade/fork")[1])["rule"])

    # a made realm is a document with revisions (0093): save from the revision you read, publish one, fork it
    maker, rival = b_player(), b_player()
    pid = sql("select id from verse.players where key_sha256 = encode(sha256(convert_to('" + maker + "', 'UTF8')), 'hex')")
    sql("insert into verse.worlds (id, owner_player, created_ms) values ('made-forge-court', '" + pid + "', 1)")
    sql("""insert into verse.made_realms (slug, world, name, style, ground, time, features, tools, made_ms)
           values ('forge-court', 'made-forge-court', 'Forge Court', 'lowpoly', 'stone', 'day',
                   '[{"feature": "trees", "variant": "round", "count": 6, "label": "Six round trees"}]', '{"build": false}', 1)""")
    sql("update verse.made_realms set visibility = 'public' where slug = 'forge-court'")   # made before 0099: as visitors had it
    st, d0 = b_call("GET", R + "forge-court/doc")
    check("a made realm's document: revision 1, its setup and features, and a hash", st == 200 and d0["doc"]["rev"] == 1 and d0["doc"]["doc"] == "aethyr.realm"
          and d0["doc"]["setup"]["ground"] == "stone" and d0["doc"]["moved"] == [] and len(d0["sha256"]) == 64, (st, d0))
    spec = line_map(b_call("GET", R + "forge-court", key=maker)[1])
    check("its spec says its revision, published as it was made (0094), listed, nothing moved and nothing to undo", spec.get("rev") == ["1"] and spec.get("published") == ["1"]
          and spec.get("listed") == ["1"] and "moved" not in spec and spec.get("undo") == ["0"], spec)
    mv = [{"entity": "trees:2", "x": 3.256, "z": -4.5}]
    check("only its owner saves", b_call("POST", R + "forge-court/save", {"rev": 1, "moved": mv}, rival)[0] == 403)
    check("a save names a revision and holds its moves", b_call("POST", R + "forge-court/save", {"moved": mv}, maker)[0] == 400
          and b_call("POST", R + "forge-court/save", {"rev": 1, "moved": [{"entity": "Trees 2", "x": 0, "z": 0}]}, maker)[0] == 400
          and b_call("POST", R + "forge-court/save", {"rev": 1, "moved": [{"entity": "trees:2", "x": 41, "z": 0}]}, maker)[0] == 400
          and b_call("POST", R + "forge-court/save", {"rev": 1, "moved": [{"entity": "trees:2", "x": 0, "z": 0}] * 65}, maker)[0] == 400)
    st, sv = b_call("POST", R + "forge-court/save", {"rev": 1, "moved": mv}, maker)
    check("a save from the revision there now is kept as the next revision, rounded to the centimetre", st == 200 and sv["rev"] == 2 and len(sv["sha256"]) == 64
          and "moved trees:2 3.26 -4.50" in b_call("GET", R + "forge-court", key=maker)[1], (st, sv))
    vis = line_map(b_call("GET", R + "forge-court", key=rival)[1])
    check("a visitor sees the revision published, not the owner's save (0094)", vis.get("rev") == ["1"] and vis.get("published") == ["1"] and "moved" not in vis
          and vis.get("role") == ["visitor"] and vis.get("owner") == ["0"] and "undo" not in vis and vis.get("row") == ["trees 1 Six round trees"], vis)
    check("and so does the visitor's document; its owner's is the save", b_call("GET", R + "forge-court/doc", key=rival)[1] == d0
          and b_call("GET", R + "forge-court/doc", key=maker)[1]["doc"]["rev"] == 2)
    st, stale = b_call("POST", R + "forge-court/save", {"rev": 1, "moved": []}, maker)
    check("a save from an older revision is refused with the revision there now: newer never just wins", st == 409 and stale == {"error": "changed since", "rev": 2}
          and "moved trees:2 3.26 -4.50" in b_call("GET", R + "forge-court", key=maker)[1], (st, stale))
    check("any change to what it is makes a new revision (a row switched off)", b_call("POST", R + "forge-court/switch", {"row": "trees", "on": False}, maker)[0] == 200
          and line_map(b_call("GET", R + "forge-court", key=maker)[1]).get("rev") == ["3"])
    b_call("POST", R + "forge-court/switch", {"row": "trees", "on": True}, maker)
    check("so a save read before it is refused too", b_call("POST", R + "forge-court/save", {"rev": 2, "moved": []}, maker) == (409, {"error": "changed since", "rev": 4}))
    check("a publish names its revision", b_call("POST", R + "forge-court/publish", {"rev": 3}, maker) == (409, {"error": "changed since", "rev": 4})
          and b_call("POST", R + "forge-court/publish", {}, maker)[0] == 400 and b_call("POST", R + "forge-court/publish", {"rev": 4}, rival)[0] == 403)
    st, pb = b_call("POST", R + "forge-court/publish", {"rev": 4, "listed": False}, maker)
    d4 = b_call("GET", R + "forge-court/doc")[1]
    check("publishing keeps the revision and its document's hash; unlisted, it is off /og's rows", st == 200 and pb["rev"] == 4 and pb["listed"] is False
          and pb["sha256"] == d4["sha256"] and line_map(b_call("GET", R + "forge-court")[1]).get("published") == ["4"]
          and line_map(b_call("GET", R + "forge-court")[1]).get("rev") == ["4"]
          and "/realms/forge-court|" not in b_call("GET", R + "made")[1]
          and sql("select doc->>'rev' from verse.realm_revisions where slug = 'forge-court' and why = 'publish'") == "4", (st, pb))
    st, fk = b_call("POST", R + "forge-court/fork", {"name": "Forge Court Copy"}, rival)
    check("anyone forks a made realm: a realm of its own that records its parent and the revision it came from", st == 200 and fk["from"] == "forge-court" and fk["rev"] == 4
          and fk["slug"] == "forge-court-copy", (st, fk))
    fs = line_map(b_call("GET", R + "forge-court-copy", key=rival)[1])
    check("the fork's spec names its parent and revision, starts at revision 1 and keeps the moves", fs.get("from") == ["forge-court 4|Forge Court"]
          and fs.get("rev") == ["1"] and fs.get("moved") == ["trees:2 3.26 -4.50"] and fs.get("owner") == ["1"], fs)
    fd = b_call("GET", R + "forge-court-copy/doc")[1]["doc"]
    check("and its document says so", fd["forked_from"] == {"slug": "forge-court", "rev": 4} and fd["features"] == d4["doc"]["features"], fd)
    check("its owner saves the fork", b_call("POST", R + "forge-court-copy/save", {"rev": 1, "moved": [{"entity": "trees:2", "x": -9, "z": 9}]}, rival)[0] == 200)
    check("the original is unchanged: the same revision and the same bytes", b_call("GET", R + "forge-court/doc")[1] == d4)
    check("the fork's owner can't save the original", b_call("POST", R + "forge-court/save", {"rev": 4, "moved": []}, rival)[0] == 403)
    page3 = ""
    for _ in range(50):
        page3 = site.get("realms/forge-court-copy", {}).get("data", b"").decode()
        if page3:
            break
        time.sleep(0.1)
    check("the fork's page names its parent and plays the made-realm template", 'Forked from <a href="/realms/forge-court">Forge Court</a> at revision 4.' in page3
          and 'data-made="forge-court-copy"' in page3, page3[:1600])
    st, auto, _ = call("POST", R + "forge-court/fork", json.dumps({"name": "Forge Court Probe"}).encode(),
                    {"Content-Type": "application/json", "X-Verse-Player": rival, "X-Verse-Automation": "forge-journey"})
    check("a realm a tool makes is private, like every new realm (0099): off /og's rows", st == 200
          and sql("select visibility from verse.made_realms where slug = 'forge-court-probe'") == "private"
          and sql("select unlisted from verse.made_realms where slug = 'forge-court-probe'") == "t"
          and "/realms/forge-court-probe|" not in b_call("GET", R + "made")[1], (st, auto))
    call("POST", R + "forge-court-probe/visibility", b'{"visibility":"unlisted"}', {"Content-Type": "application/json", "X-Verse-Player": rival})
    page4 = site.get("realms/forge-court-probe", {}).get("data", b"").decode()
    check("unlisted, its page says noindex", '<meta name="robots" content="noindex">' in page4, page4[:400])

    # visitors see the revision published; every edit is one undo step (0094)
    mv5 = [{"entity": "trees:2", "x": 1, "z": 1}, {"entity": "rocks:0", "x": -2, "z": 3}]
    st, sv5 = b_call("POST", R + "forge-court/save", {"rev": 4, "moved": mv5}, maker)
    om, vm = line_map(b_call("GET", R + "forge-court", key=maker)[1]), line_map(b_call("GET", R + "forge-court", key=rival)[1])
    check("after a save its owner sees it and a visitor still sees the revision published", st == 200 and sv5["rev"] == 5
          and om.get("rev") == ["5"] and om.get("moved") == ["trees:2 1.00 1.00", "rocks:0 -2.00 3.00"] and om.get("published") == ["4"]
          and vm.get("rev") == ["4"] and vm.get("moved") == ["trees:2 3.26 -4.50"], (st, sv5, om, vm))
    check("a visitor's document is the one published, its hash the publish's", b_call("GET", R + "forge-court/doc")[1] == d4
          and b_call("GET", R + "forge-court/doc", key=maker)[1]["doc"]["rev"] == 5)
    third = b_player()
    st, f3 = b_call("POST", R + "forge-court/fork", {"name": "Forge Court Live"}, third)
    fdoc = b_call("GET", R + "forge-court-live/doc", key=third)[1]["doc"]
    check("a visitor's fork copies the revision published", st == 200 and f3["rev"] == 4 and fdoc["moved"] == d4["doc"]["moved"]
          and fdoc["forked_from"] == {"slug": "forge-court", "rev": 4} and line_map(b_call("GET", R + "forge-court-live", key=third)[1]).get("published") == ["1"], (st, f3, fdoc))
    check("only its owner undoes, from the revision there now", b_call("POST", R + "forge-court/undo", {"rev": 5}, rival)[0] == 403
          and b_call("POST", R + "forge-court/undo", {"rev": 4}, maker) == (409, {"error": "changed since", "rev": 5})
          and b_call("POST", R + "forge-court/undo", {}, maker)[0] == 400)
    steps = int(line_map(b_call("GET", R + "forge-court", key=maker)[1])["undo"][0])
    st, u = b_call("POST", R + "forge-court/undo", {"rev": 5}, maker)
    check("UNDO takes the save back as a new revision, with the moves before it", st == 200 and u == {"rev": 6, "undo": steps - 1}
          and b_call("GET", R + "forge-court/doc", key=maker)[1]["doc"]["moved"] == d4["doc"]["moved"], (st, u, steps))
    st, pl = b_call("POST", R + "forge-court/place", {"thing": "crate", "x": 2, "z": -3, "yaw": 0}, maker)
    placed = b_call("GET", R + "forge-court/doc", key=maker)[1]["doc"]["placed"]
    st2, u = b_call("POST", R + "forge-court/undo", {"rev": 7}, maker)
    check("a thing placed is one step: UNDO takes it out", st == 200 and len(placed) == 1 and st2 == 200 and u["rev"] == 8
          and b_call("GET", R + "forge-court/doc", key=maker)[1]["doc"]["placed"] == [], (st, pl, placed, st2, u))
    b_call("POST", R + "forge-court/switch", {"row": "trees", "on": False}, maker)
    off = line_map(b_call("GET", R + "forge-court", key=maker)[1]).get("row")
    st, u = b_call("POST", R + "forge-court/undo", {"rev": 9}, maker)
    check("a row switched is one step: UNDO switches it back", off == ["trees 0 Six round trees"] and st == 200
          and line_map(b_call("GET", R + "forge-court", key=maker)[1]).get("row") == ["trees 1 Six round trees"], (off, st, u))
    b_call("POST", R + "forge-court/settings", {"tools": {"build": True}}, maker)
    rev = int(line_map(b_call("GET", R + "forge-court", key=maker)[1])["rev"][0])
    pid = b_call("POST", R + "forge-court/place", {"thing": "barrel", "x": 1, "z": 1, "yaw": 0}, rival)[1].get("proposed")
    st, dec = b_call("POST", R + "forge-court/proposals/" + str(pid) + "/accept", {}, maker)
    acc = b_call("GET", R + "forge-court/doc", key=maker)[1]["doc"]
    st2, u = b_call("POST", R + "forge-court/undo", {"rev": rev + 1}, maker)
    cards = b_call("GET", R + "forge-court/proposals", key=maker)[1]
    check("a card accepted is one step: UNDO takes its thing out and the card waits again", st == 200 and acc["rev"] == rev + 1 and len(acc["placed"]) == 1
          and st2 == 200 and b_call("GET", R + "forge-court/doc", key=maker)[1]["doc"]["placed"] == [] and ("P " + str(pid) + "|") in cards, (st, dec, acc, st2, u, cards))
    rev = u["rev"]
    st, dec = b_call("POST", R + "forge-court/proposals/" + str(pid) + "/decline", {}, maker)
    gone = b_call("GET", R + "forge-court/proposals", key=maker)[1]
    st2, u = b_call("POST", R + "forge-court/undo", {"rev": rev}, maker)
    check("a card declined is a step too, though nothing in the realm changes", st == 200 and ("P " + str(pid) + "|") not in gone and st2 == 200
          and u["rev"] == rev and ("P " + str(pid) + "|") in b_call("GET", R + "forge-court/proposals", key=maker)[1], (st, dec, st2, u))
    check("every revision is kept, an edit's and an undo's too", int(sql("select count(*) from verse.realm_revisions where slug = 'forge-court' and why in ('edit', 'undo')")) >= 6
          and sql("select why from verse.realm_revisions where slug = 'forge-court' and rev = 6") == "undo")
    check("a visitor still sees revision 4 through all of it", line_map(b_call("GET", R + "forge-court")[1]).get("rev") == ["4"])

    sql("delete from verse.published where world = 'everglade' and published_by = 'test'")
    model.shutdown()
    model.server_close()
    gcs_server.shutdown()


realm_fork_checks()
