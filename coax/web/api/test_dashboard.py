# The admin dashboard's routes (pt-dashboard): only a signed-in admin sees anything. test.py runs this file.
import base64


def signed(key, method, url):
    return subprocess.run([os.environ["SIGN"], "--key", key, method, url], capture_output=True, text=True, check=True).stdout.strip()


def login(key, origin=API):
    auth = urllib.parse.quote(signed(key, "GET", origin + "/api/admin/login"), safe="")
    return call("GET", "/api/admin/login?auth=" + auth)


import urllib.parse


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *a, **k):
        return None


urllib.request.install_opener(urllib.request.build_opener(NoRedirect))
admin, other = os.environ["ADMIN_KEY"], os.environ["OTHER_KEY"]
reports = "/api/admin/analytics?view=overview&days=0&space=all&auto=1"

status, body, _ = call("GET", reports)
check("signed out: a 401 and no numbers", status == 401 and b"Sessions" not in body, body)
status, body, _ = call("GET", reports, headers={"Cookie": "oa_admin=" + "0" * 64})
check("a made-up session: a 401", status == 401)
status, body, _ = login(other)
check("someone not on the list can't sign in", status == 403 and b"Set-Cookie" not in body, body)
status, body, _ = call("GET", reports, headers={"Authorization": "Nostr " + signed(other, "GET", API + reports)})
check("someone not on the list can't sign a request either", status == 403 and b"Sessions" not in body, body)

auth = urllib.parse.quote(signed(admin, "GET", API + "/api/admin/login"), safe="")
status, body, headers = call("GET", "/api/admin/login?auth=" + auth)
cookie = headers.get("Set-Cookie", "")
check("an admin signs in: a session cookie and on to the dashboard",
      status == 303 and headers.get("Location") == "/admin/analytics" and cookie.startswith("oa_admin=")
      and "HttpOnly" in cookie and "Secure" in cookie and "SameSite=Strict" in cookie and "Path=/api/admin" in cookie, (status, headers))
check("the same signed sign-in can't be used twice", call("GET", "/api/admin/login?auth=" + auth)[0] == 401)
check("a sign-in signed for another site is refused", login(admin, "https://evil.example")[0] == 401)
token = cookie.split(";")[0]
check("only the token's hash is kept", sql("select count(*) from analytics.admin_sessions where token_sha256 = '" + token.split("=")[1] + "'") == "0")

status, body, _ = call("GET", reports, headers={"Cookie": token})
text = body.decode()
check("the admin sees the reports", status == 200 and "#SUMMARY||WHAT|COUNT" in text and "#FUNNEL|" in text and "#WHAT THEY SAID AND TYPED|s|" in text, text[:300])
check("the reports count Hecate's Cemetery's first visit step by step",
      "#HECATE'S CEMETERY||STEP|SESSIONS" in text and "\nChose it on the galaxy|" in text and "\nPet made|" in text and "\nShared a memorial|" in text, text[:300])
check("the reports count the captured sessions", "Sessions|4" in text and "It was spooky" in text, text[:600])
sql("INSERT INTO analytics.playtesters (id, at, email, sid, how, auto, space) VALUES "
    "('pt1', extract(epoch from now()), 'ann@example.com', 's-a', 'email', NULL, 'kiki'), "
    "('pt2', extract(epoch from now()), 'Ann@example.com', 's-b', 'email', NULL, 'kiki'), "
    "('pt3', extract(epoch from now()), 'bo@example.com', 's-c', 'form', NULL, 'kiki'), "
    "('pt4', extract(epoch from now()), 'e2e@example.com', 's-d', 'form', 'kiki-e2e', 'kiki'), "
    "('pt5', extract(epoch from now()), 'bot@example.com', '" + bot + "', 'form', NULL, 'kiki')")
real = call("GET", reports.replace("auto=1", "auto=0"), headers={"Cookie": token})[1].decode()
check("playtesters: real people by default, each email once, visits apart",
      "Playtesters|2" in real and "Playtester visits|3" in real and "@example.com" not in real, real[:600])
every = call("GET", reports, headers={"Cookie": token})[1].decode()
check("with automated runs counted, theirs too", "Playtesters|4" in every and "Playtester visits|5" in every, every[:600])
sql("DELETE FROM analytics.playtesters WHERE id IN ('pt1', 'pt2', 'pt3', 'pt4', 'pt5')")
check("automated sessions can be left out", "Sessions|2" in call("GET", reports.replace("auto=1", "auto=0"), headers={"Cookie": token})[1].decode())
status, body, _ = call("GET", "/api/admin/analytics?view=sessions&days=0", headers={"Cookie": token})
check("the sessions list", status == 200 and "#SESSIONS|s|" in body.decode() and sid in body.decode(), body[:300])
status, body, _ = call("GET", "/api/admin/analytics?view=session&sid=" + sid, headers={"Cookie": token})
lines = body.decode().split("#TIMELINE")[1].splitlines()[1:]
check("a session's timeline, in order, with its words", [l.split("|")[2] for l in lines] == ["session-start", "world-enter", "area", "prompt", "survey-sent"]
      and "text It was spooky" in lines[-1] and "Address|203.0.113.9" in body.decode(), body.decode())
check("a bad session id is a 400", call("GET", "/api/admin/analytics?view=session&sid=x'y", headers={"Cookie": token})[0] == 400)
signed_url = API + "/api/admin/analytics?view=overview"
h = {"Authorization": "Nostr " + signed(admin, "GET", signed_url)}
check("an admin's signed request works once", call("GET", "/api/admin/analytics?view=overview", headers=h)[0] == 200
      and call("GET", "/api/admin/analytics?view=overview", headers=h)[0] == 401)
call("GET", "/api/admin/logout", headers={"Cookie": token})
check("signed out: the session is gone", call("GET", reports, headers={"Cookie": token})[0] == 401)

# ---- logged in on the site with the phone (/login, oa_session): an admin's key opens the dashboard, anyone else's doesn't
admin_hex = os.environ["OA_ADMIN_PUBKEYS"].strip()
for tok, pk in (("a" * 64, admin_hex), ("b" * 64, "c" * 64)):
    sql(f"insert into signin.sessions (token_sha256, pubkey, created_ms, expires_ms) values (encode(sha256('{tok}'), 'hex'), '{pk}', 0, 9999999999999) on conflict do nothing")
status, body, _ = call("GET", reports, headers={"Cookie": "oa_session=" + "a" * 64})
check("an admin logged in on the site sees the dashboard", status == 200 and b"Sessions" in body, (status, body[:120]))
status, body, _ = call("GET", "/api/admin/reports", headers={"Cookie": "oa_session=" + "a" * 64})
check("and the review queue", status == 200, (status, body[:120]))
status, body, _ = call("GET", reports, headers={"Cookie": "oa_session=" + "b" * 64})
check("someone else logged in on the site: admins only", status == 403 and b"Sessions" not in body, (status, body[:120]))

# ---- a realm's old address (/realm/<slug>, before October 29) counts with its new one (/realms/<slug>)
post(batch("c00000000000000r9", [ev("page-view", 1, {"path": "/realm/everglade"}), ev("page-view", 2, {"path": "/realms/everglade"})]))
text = call("GET", reports, headers={"Cookie": "oa_session=" + "a" * 64})[1].decode()
pages = text.split("#PAGES|")[1].split("\n#")[0] if "#PAGES|" in text else ""
check("pages: /realm/everglade and /realms/everglade are one row", "\n/realms/everglade|2" in pages and "/realm/everglade|" not in pages, pages[:400])
