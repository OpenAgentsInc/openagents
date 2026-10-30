# Logging in with the words account's key (POST /api/auth/key, migration 0067,
# docs/web-log-in.md): a NIP-98 event (kind 27235, u, method, payload) on the
# body {"kind":"login"} from this site's page gets the poll's oa_session cookie
# and moves the page's Verse player; a replay, a wrong u or payload, a stale
# event, another site or no signature are refused; it is rate limited.
# test.py runs it after test_private_phone.py, whose helpers it uses.
WORDS_KEY, WORDS = new_key("wk-words.key")
LOGIN = json.dumps({"kind": "login"}, separators=(",", ":")).encode()


def key_login(auth, body=LOGIN, origin=API, xff="10.2.0.1", player=None):
    h = {"Content-Type": "application/json", "X-Forwarded-For": xff}
    if origin: h["Origin"] = origin
    if auth: h["Authorization"] = auth
    if player: h["X-Verse-Player"] = player
    s, b, hd = call("POST", "/api/auth/key", body, h)
    try: return s, json.loads(b), hd
    except ValueError: return s, b, hd


time.sleep(1.05)
auth = signed(WORDS_KEY, "POST", "/api/auth/key", LOGIN)
s, b, h = key_login(auth)
wsess = cookie_of(h, "oa_session")
check("a signed login gets the poll's oa_session cookie (HttpOnly, Secure, Lax, 30 days) for that key",
      s == 200 and b["state"] == "signed-in" and b["npub"] == bech32_npub(WORDS) and len(wsess) > 20
      and "HttpOnly" in h["Set-Cookie"] and "Secure" in h["Set-Cookie"] and "SameSite=Lax" in h["Set-Cookie"] and "Max-Age=2592000" in h["Set-Cookie"], (s, b, h))
s, b, _ = site("GET", "me", cookie=wsess)
check("the session is that key's", s == 200 and ("K|" + bech32_npub(WORDS)).encode() in b, (s, b))
s, b, _ = key_login(auth, xff="10.2.0.2")
check("the same event again (a replay) is refused", s == 401 and not cookie_of(_, "oa_session"), (s, b))

time.sleep(1.05)
s, b, _ = key_login(signed(WORDS_KEY, "POST", "/api/auth/approve", LOGIN), xff="10.2.0.3")
check("an event for another URL (wrong u) is refused", s == 401, (s, b))
time.sleep(1.05)
s, b, _ = key_login(signed(WORDS_KEY, "POST", "/api/auth/key", b'{"kind":"other"}'), xff="10.2.0.4")
check("an event whose payload is another body is refused", s == 401, (s, b))
time.sleep(1.05)
os.environ["SIGN_AGE"] = "120"
stale = signed(WORDS_KEY, "POST", "/api/auth/key", LOGIN)
del os.environ["SIGN_AGE"]
s, b, _ = key_login(stale, xff="10.2.0.5")
check("an event two minutes old (stale) is refused", s == 401, (s, b))
s, b, _ = key_login(None, xff="10.2.0.6")
check("no signature is refused", s == 403, (s, b))
time.sleep(1.05)
s, b, _ = key_login(signed(WORDS_KEY, "POST", "/api/auth/key", LOGIN), origin="https://evil.example", xff="10.2.0.7")
check("another site's page is refused", s == 403, (s, b))
time.sleep(1.05)
s, b, _ = key_login(signed(WORDS_KEY, "POST", "/api/auth/key", b'{"kind":"login","x":1}'), body=b'{"kind":"login","x":1}', xff="10.2.0.8")
check("a body other than {\"kind\":\"login\"} is refused", s == 400, (s, b))

# the page's Verse player moves to the key's account, as the poll's does
s, g2 = pt("POST", "player", {})
time.sleep(1.05)
s, b, _ = key_login(signed(WORDS_KEY, "POST", "/api/auth/key", LOGIN), xff="10.2.0.9", player=g2["key"])
check("a login with the page's player key moves that player to the key's account",
      s == 200 and b.get("player") == {"key": g2["key"], "id": g2["id"], "account": "n-" + WORDS}, (s, b))

# rate limited per address, like approve: ten a minute
codes = []
for i in range(11):
    codes.append(key_login("Nostr x", xff="10.2.0.10")[0])
check("the eleventh login in a minute from one address gets 429", codes[:10] == [401] * 10 and codes[10] == 429, codes)
