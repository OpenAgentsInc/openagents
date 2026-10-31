# Reply with a realm (ev-reply-realm; replies.c, migrations/0076_realm_replies.sql): a line or a
# post's link put on a house in Everglade and smashed, kept as typed (never fetched), length-limited,
# escaped on its page, which is written into the fake bucket (test_project_drops.py serves it); its
# card is its maker's alone. test.py runs this file with its helpers (call, check, sql).
import threading
import time


def r_player():
    _, raw, _ = call("POST", "/api/playtest/player", b"{}", {"Content-Type": "application/json"})
    return json.loads(raw)["key"]


def r_call(method, path, body=None, key=None, origin=None, raw=None):
    h = {"Content-Type": "application/json"}
    if key:
        h["X-Verse-Player"] = key
    if origin:
        h["Origin"] = origin
    data = raw if raw is not None else (None if body is None else json.dumps(body).encode())
    status, out, _ = call(method, path, data, h)
    try:
        return status, json.loads(out)
    except Exception:
        return status, out.decode()


def reply_realm_checks():
    threading.Thread(target=gcs_server.serve_forever, daemon=True).start()
    maker, other = r_player(), r_player()
    line = {"realm": "everglade", "text": "ok", "seed": 7}

    check("a reply needs a player", r_call("POST", "/api/playtest/replies", line)[0] == 401)
    check("a reply from another site's page is refused", r_call("POST", "/api/playtest/replies", line, maker, origin="https://evil.example")[0] == 403)
    check("only Everglade takes replies", r_call("POST", "/api/playtest/replies", dict(line, realm="haunted-house"), maker)[0] == 400
          and r_call("POST", "/api/playtest/replies", {"text": "ok", "seed": 7}, maker)[0] == 400)
    check("a text or a link, never both or neither",
          r_call("POST", "/api/playtest/replies", {"realm": "everglade", "seed": 7}, maker)[0] == 400
          and r_call("POST", "/api/playtest/replies", dict(line, link="https://x.com/a/status/1"), maker)[0] == 400)
    check("a line is 1 to 140 characters", r_call("POST", "/api/playtest/replies", dict(line, text="x" * 141), maker)[0] == 400
          and r_call("POST", "/api/playtest/replies", dict(line, text=" \n\t "), maker)[0] == 400
          and r_call("POST", "/api/playtest/replies", dict(line, text=5), maker)[0] == 400
          and r_call("POST", "/api/playtest/replies", dict(line, text="é" * 140), maker)[0] == 200)
    check("a link is an http or https address of 280 characters at most, kept as text",
          r_call("POST", "/api/playtest/replies", {"realm": "everglade", "link": "javascript:alert(1)", "seed": 7}, maker)[0] == 400
          and r_call("POST", "/api/playtest/replies", {"realm": "everglade", "link": "https://x.com/a b", "seed": 7}, maker)[0] == 400
          and r_call("POST", "/api/playtest/replies", {"realm": "everglade", "link": 'https://x.com/"><script>', "seed": 7}, maker)[0] == 400
          and r_call("POST", "/api/playtest/replies", {"realm": "everglade", "link": "https://x.com/" + "a" * 267, "seed": 7}, maker)[0] == 400
          and r_call("POST", "/api/playtest/replies", {"realm": "everglade", "link": "https://x.com/" + "a" * 266, "seed": 7}, maker)[0] == 200)
    check("the seed is a whole number from 1 to 2147483647",
          all(r_call("POST", "/api/playtest/replies", dict(line, seed=s), maker)[0] == 400 for s in (0, -3, 2.5, "7", 2147483648, None))
          and r_call("POST", "/api/playtest/replies", dict(line, seed=2147483647), maker)[0] == 200)
    check("a body past 4 KB is refused", r_call("POST", "/api/playtest/replies", None, maker, raw=b'{"realm":"everglade","text":"' + b"x" * 5000 + b'","seed":7}')[0] == 413)
    check("a body that isn't JSON is a 400", r_call("POST", "/api/playtest/replies", None, maker, raw=b"{nope")[0] == 400)

    # a line with every character a page could be broken with: kept as one line, escaped everywhere
    words = 'Ship it <script>alert("x")</script> & \'quote\' |bar|\n\r\x07 next'
    st, made = r_call("POST", "/api/playtest/replies", {"realm": "everglade", "text": words, "seed": 424242}, maker)
    rid = made.get("id", "") if st == 200 else ""
    check("a reply is made: its id and its page's path", st == 200 and len(rid) == 12 and made["path"] == "/realms/everglade/reply/" + rid and made["page"] is True, made)
    kept = sql("select words from verse.replies where id = '%s'" % rid)
    check("its words are kept as one line of plain characters: no line breaks, bars or control characters",
          kept == 'Ship it <script>alert("x")</script> & \'quote\' bar next', kept)
    check("only what was typed is kept: no author, no fetched page",
          sql("select string_agg(column_name, ',' order by ordinal_position) from information_schema.columns where table_schema = 'verse' and table_name = 'replies'")
          == "id,realm,house,kind,words,seed,player,made_ms,card_ms,card_kb")
    st, lines = r_call("GET", "/api/playtest/replies/" + rid)
    check("the realm reads it back a line each, the words last and on one line",
          st == 200 and lines == "id=%s\nrealm=everglade\nhouse=12\nseed=424242\nkind=text\ntext=%s\n" % (rid, kept), lines)
    time.sleep(0.3)
    site = bucket["site"]
    check("its page, bare path and text are in the bucket",
          all(n in site for n in ("realms/everglade/reply/" + rid, "realms/everglade/reply/%s/index.html" % rid, "realms/everglade/reply/%s/index.md" % rid))
          and site["realms/everglade/reply/" + rid]["meta"]["cacheControl"] == "no-cache", sorted(n for n in site if "reply" in n))
    page = site.get("realms/everglade/reply/" + rid, {}).get("data", b"").decode()
    check("every word on its page is escaped: no tag, quote or ampersand of the player's survives",
          "<script>alert" not in page and 'alert("x")' not in page
          and "&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt; &amp; 'quote'" in page, page[:2400])
    check("its page has every preview tag and the realm's card until its own",
          '<meta property="og:title" content="“Ship it &lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt; &amp; \'quote\' bar next”">' in page
          and '<meta name="twitter:card" content="summary_large_image">' in page and '<meta property="og:image:width" content="1200">' in page
          and '/realms/everglade/og.jpg"' in page and '<meta property="og:url" content="' in page and "/realms/everglade/reply/" + rid + '">' in page, page[:1800])
    check("its page opens Everglade with the reply: its words come from the API, never from the page",
          'data-reply="%s"' % rid in page and 's.dataset.lines = "620:/api/playtest/replies/" + id' in page
          and 's.dataset.cardOn = "impact"' in page and 'fetch("/realms/everglade"' in page and page.count("Ship it") == 4, page[-1200:])
    md = site.get("realms/everglade/reply/%s/index.md" % rid, {}).get("data", b"").decode()
    check("its text quotes the words with markup escaped", "> Ship it \\<script\\>alert" in md and "<script>" not in md, md)
    check("an unknown or malformed id is a 404", r_call("GET", "/api/playtest/replies/zzzzzzzzzzzz")[0] == 404
          and r_call("GET", "/api/playtest/replies/../../x")[0] == 404 and r_call("GET", "/api/playtest/replies/" + rid + "/x")[0] == 404)

    # a link is kept as the text it is
    st, linked = r_call("POST", "/api/playtest/replies", {"realm": "everglade", "link": "https://x.com/someone/status/123?s=20", "seed": 9}, maker)
    lid = linked.get("id", "") if st == 200 else ""
    check("a link is kept as its text and marked a link", st == 200 and r_call("GET", "/api/playtest/replies/" + lid)[1].endswith("kind=link\ntext=https://x.com/someone/status/123?s=20\n"))
    time.sleep(0.3)
    lpage = site.get("realms/everglade/reply/" + lid, {}).get("data", b"").decode()
    check("a link on its page is words, never a link to follow", 'href="https://x.com' not in lpage and "“https://x.com/someone/status/123?s=20”" in lpage, lpage[:900])

    # its card: its maker's alone, a 1200x630 JPEG
    check("its maker is asked for its card; anyone else isn't",
          r_call("GET", "/api/playtest/replies/%s/card" % rid, key=maker)[1] == "want" and r_call("GET", "/api/playtest/replies/%s/card" % rid, key=other)[1] == "no"
          and r_call("GET", "/api/playtest/replies/%s/card" % rid)[1] == "no")
    jpeg = b"\xff\xd8\xff\xe0\x00\x04ab\xff\xc0\x00\x11\x08\x02\x76\x04\xb0\x03\x01\x22\x00\x02\x11\x01\x03\x11\x01\xff\xda\x00\x02" + b"\x00--x" * 10 + b"\xff\xd9"
    def put_card(body, key, which=rid):
        hh = {"Content-Type": "image/jpeg"}
        if key:
            hh["X-Verse-Player"] = key
        return call("POST", "/api/playtest/replies/%s/card" % which, body, hh)
    check("only its maker keeps its card", put_card(jpeg, other)[0] == 403 and put_card(jpeg, None)[0] == 403 and put_card(jpeg, maker, "zzzzzzzzzzzz")[0] == 404)
    check("a card that isn't a 1200x630 JPEG under 300 KB is refused",
          put_card(jpeg.replace(b"\x02\x76\x04\xb0", b"\x01\x00\x01\x00"), maker)[0] == 400 and put_card(b"GIF89a....", maker)[0] == 400
          and put_card(b"\xff\xd8" + b"\x00" * (301 * 1024), maker)[0] == 413)
    st, raw, _ = put_card(jpeg, maker)
    got = json.loads(raw) if st == 200 else {}
    time.sleep(0.3)
    page = site.get("realms/everglade/reply/" + rid, {}).get("data", b"").decode()
    check("its card is kept in the bucket and its page names it",
          st == 200 and site.get("realms/everglade/reply/%s/card.jpg" % rid, {}).get("data") == jpeg
          and got.get("image", "").startswith("/realms/everglade/reply/%s/card.jpg?v=" % rid) and got.get("page") is True
          and "/realms/everglade/reply/%s/card.jpg?v=" % rid in page and "/realms/everglade/og.jpg" not in page, (st, raw))
    check("once it has its card its maker isn't asked again", r_call("GET", "/api/playtest/replies/%s/card" % rid, key=maker)[1] == "no")

    # a player makes so many a day
    pid = sql("select player from verse.replies where id = '%s'" % rid)
    made_today = int(sql("select count(*) from verse.replies where player = '%s'" % pid))
    check("a player's replies a day are capped", made_today > 0 and
          sql("select verse.reply_make('{\"realm\":\"everglade\",\"text\":\"one more\",\"seed\":3}'::jsonb, '%s', %d)->>'status'" % (maker, made_today)) == "429"
          and sql("select verse.reply_make('{\"realm\":\"everglade\",\"text\":\"one more\",\"seed\":3}'::jsonb, '%s', %d)->>'status'" % (maker, made_today + 1)) == "200")
    gcs_server.shutdown()


reply_realm_checks()
