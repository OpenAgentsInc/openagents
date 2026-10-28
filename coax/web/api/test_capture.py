# Capture (pt-capture): a page's batch is kept as the playtest keeps it. test.py runs this file.
def batch(sid, events, auto=None):
    b = {"sid": sid, "events": events}
    if auto:
        b["auto"] = auto
    return json.dumps(b).encode()


def post(body, headers=None):
    h = {"Content-Type": "text/plain", "User-Agent": "Mozilla/5.0 (Macintosh) Chrome/152.0.0.0 Safari/537.36"}
    h.update(headers or {})
    return call("POST", "/api/analytics/events", body, h)


ev = lambda name, ct, p=None: {"ev": name, "page": "haunt", "ct": ct, "p": p or {}}
sid = "c00000000000000a1"
status, body, _ = post(batch(sid, [
    ev("session-start", 1, {"pid": "v1", "rev": "coaxial", "os": "macOS"}),
    ev("world-enter", 2, {"world": "haunted-house"}),
    ev("area", 3, {"area": "Foyer", "world": "haunted-house", "x": 0.123456}),
    ev("survey-sent", 5, {"q": "how it went", "text": "It was spooky", "consent": True}),
    ev("prompt", 4, {"text": "no consent here", "long": "y" * 200, "bad key": 1, "obj": {"a": 1}}),
    {"ev": "Not-An-Event", "ct": 6},
]), {"X-Forwarded-For": "203.0.113.9, 10.0.0.1", "Host": "openagents.com", "X-Client-City": "chicago", "X-Client-Region": "US"})
check("a batch is kept, all but the malformed event", status == 200 and json.loads(body) == {"kept": 5}, body)
check("the session has the visitor's address, host and city",
      sql(f"select ip||' '||host||' '||city||' '||events from analytics.sessions where sid='{sid}'") == "203.0.113.9 openagents.com chicago 5")
check("a session replays in the page's order",
      sql(f"select string_agg(ev, ',' order by ct) from analytics.events where sid='{sid}'") == "session-start,world-enter,area,prompt,survey-sent")
check("words with consent are kept", sql(f"select p->>'text' from analytics.events where sid='{sid}' and ev='survey-sent'") == "It was spooky")
p = json.loads(sql(f"select p from analytics.events where sid='{sid}' and ev='prompt'"))
check("words without consent, bad keys and objects are dropped; strings cut to 80", p == {"long": "y" * 80}, p)
check("floats keep three places", sql(f"select p->>'x' from analytics.events where sid='{sid}' and ev='area'") == "0.123")

bot = "c00000000000000b2"
post(batch(bot, [ev("session-start", 1)]), {"User-Agent": "Mozilla/5.0 HeadlessChrome/152.0.0.0"})
check("a headless agent is noted on its session-start",
      sql(f"select p->>'agent' from analytics.events where sid='{bot}'") == "headlesschrome" and
      sql(f"select agent from analytics.sessions where sid='{bot}'") == "headlesschrome")
auto = "c00000000000000c3"
post(batch(auto, [ev("click", 1)]), {"X-Verse-Automation": "release-smoke"})
post(batch(auto, [ev("click", 2)], auto="webdriver"), {"Cookie": "a=b; verse-automation=probe"})
check("automation marks from the header, then the cookie",
      sql(f"select string_agg(auto, ',' order by ct) from analytics.events where sid='{auto}'") == "release-smoke,probe")

many = "c00000000000000d4"
status, body, _ = post(batch(many, [ev("key", i) for i in range(60)]))
check("at most 50 events a batch", json.loads(body) == {"kept": 50}, body)
check("bad JSON is a 400", post(b"{not json")[0] == 400)
check("a batch without a session is a 400", post(json.dumps({"events": []}).encode())[0] == 400)
check("a GET is a 405", call("GET", "/api/analytics/events")[0] == 405)
check("a bad NIP-98 signature is refused", post(batch(sid, [ev("click", 9)]), {"Authorization": "Nostr e30="})[0] == 401)
