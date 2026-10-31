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
          and f["rules"] == ["night", "waves"] and f["page"] is True, (st, f))
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
          set(enum) == {"flock", "floats", "undergrowth", "daycycle", "night", "waves", "nodestroy", "lowgrav"}
          and "gravity is a third of normal" in sent["systemInstruction"]["parts"][0]["text"], (enum, sent.get("systemInstruction")))
    m = line_map(b_call("GET", R + "doomglade/fork")[1])
    check("its answer is the fork now: low gravity on, night off; a row it hasn't is ignored",
          st == 200 and rv["changed"] == 2 and "lowgrav 1 Low gravity" in m["rule"] and "night 0 Always night" in m["rule"] and "waves 1 Hostile waves" in m["rule"], (st, rv, m))
    check("and Everglade is still unchanged", b_call("GET", R + "everglade")[1] == before_spec)

    # publishing: its owner lists it, with its own line
    check("a visitor can't publish it", b_call("POST", R + "doomglade/publish", {}, other)[0] == 403)
    check("a realm that isn't a fork has nothing to publish", b_call("POST", R + "moonlit-marsh/publish", {}, owner)[0] == 404)
    st, pub = b_call("POST", R + "doomglade/publish", {"description": "Always night in Everglade, and the watch comes in waves. <b>Run.</b>"}, owner)
    check("its owner publishes it with its own line, markup taken out", st == 200 and pub == {"published": True, "description": "Always night in Everglade, and the watch comes in waves. bRun./b"}, (st, pub))
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
    check("unpublishing takes it off again", b_call("POST", R + "doomglade/publish", {"published": False}, owner) == (200, {"published": False, "description": pub["description"]})
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

    sql("delete from verse.published where world = 'everglade' and published_by = 'test'")
    model.shutdown()
    model.server_close()
    gcs_server.shutdown()


realm_fork_checks()
