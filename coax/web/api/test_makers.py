# A realm's maker (ev-claim-realm, makers.c, migrations/0072_realm_makers.sql): an admin's claim
# code works once, binds the logged-in key, a name and a Lightning address to its realm, and is
# refused after. test.py runs this file.
import tempfile


def mk_signed(keyfile, method, path, body=None):
    url = API + path
    args = [os.environ["SIGN"], "--key", keyfile, method, url]
    tmp = None
    if body is not None:
        tmp = tempfile.NamedTemporaryFile(delete=False)
        tmp.write(body); tmp.close()
        args.append(tmp.name)
    auth = subprocess.run(args, capture_output=True, text=True, check=True).stdout.strip()
    if tmp: os.unlink(tmp.name)
    return "Nostr " + auth


def mk_post(path, obj, keyfile=None, origin=True, auth=None):
    body = json.dumps(obj).encode()
    h = {"Content-Type": "application/json"}
    if origin: h["Origin"] = API
    if auth: h["Authorization"] = auth
    elif keyfile: h["Authorization"] = mk_signed(keyfile, "POST", path, body)
    s, b, _ = call("POST", path, body, h)
    return s, b, h.get("Authorization")


OTHER_KEY = os.environ["OTHER_KEY"]
mk_maker_key = tempfile.mktemp()
mk_maker_pk = subprocess.run([os.environ["SIGN"], "--new", mk_maker_key], capture_output=True, text=True, check=True).stdout.strip()

s, b, _ = call("GET", "/api/playtest/makers/orbit-league")
check("makers: an unclaimed realm says so, with no maker", s == 200 and json.loads(b) == {"realm": "orbit-league", "state": "unclaimed"}, (s, b))

s, b, _ = mk_post("/api/admin/makers/orbit-league/code", {})
check("makers: a code needs an admin (nobody signed in)", s == 401, (s, b))
s, b, _ = mk_post("/api/admin/makers/orbit-league/code", {}, keyfile=OTHER_KEY)
check("makers: a code needs an admin (someone else)", s == 403, (s, b))

s, b, _ = mk_post("/api/admin/makers/orbit-league/code", {}, keyfile=os.environ["ADMIN_KEY"])
mk_code = json.loads(b).get("code", "") if s == 200 else ""
check("makers: an admin makes a code and its link", s == 200 and len(mk_code) == 32 and json.loads(b)["link"].endswith("/realms/orbit-league/claim#code=" + mk_code), (s, b))
check("makers: only the code's hash is kept", sql(f"select count(*) from makers.codes where code_sha256 = '{mk_code}'") == "0"
      and sql("select count(*) from makers.codes where realm = 'orbit-league' and used is null") == "1")
check("makers: the code is never logged", mk_code and mk_code not in open(os.environ["API_LOG"]).read())

mk_claim = {"code": mk_code, "name": "Topher", "lightning": "Topher@Example.com"}
s, b, _ = mk_post("/api/playtest/makers/orbit-league/claim", mk_claim)
check("makers: a claim needs a login", s == 401, (s, b))
s, b, _ = mk_post("/api/playtest/makers/orbit-league/claim", dict(mk_claim, lightning="not an address"), keyfile=mk_maker_key)
check("makers: a bad Lightning address is refused", s == 400 and "Lightning" in json.loads(b)["error"], (s, b))
s, b, _ = mk_post("/api/playtest/makers/orbit-league/claim", dict(mk_claim, name="<b>" + "x" * 50), keyfile=mk_maker_key)
check("makers: a bad name is refused", s == 400, (s, b))
check("makers: a refused claim leaves the code unspent", sql("select count(*) from makers.codes where realm = 'orbit-league' and used is null") == "1")
s, b, _ = mk_post("/api/playtest/makers/hold-the-stair/claim", mk_claim, keyfile=mk_maker_key)
check("makers: another realm's code is refused", s == 404, (s, b))
s, b, _ = mk_post("/api/playtest/makers/orbit-league/claim", mk_claim, keyfile=mk_maker_key, origin=False)
s2, b2, _ = call("POST", "/api/playtest/makers/orbit-league/claim", json.dumps(mk_claim).encode(), {"Origin": "https://evil.example", "Content-Type": "application/json"})
check("makers: another site's page is refused", s2 == 403, (s2, b2))
check("makers: the claim binds the signed key, the name and the address", s == 200 and json.loads(b) == {"realm": "orbit-league", "name": "Topher", "lightning": "topher@example.com"}, (s, b))
check("makers: the maker is kept", sql("select pubkey || '|' || name || '|' || lightning from makers.realms where realm = 'orbit-league'") == f"{mk_maker_pk}|Topher|topher@example.com")
s, b, _ = mk_post("/api/playtest/makers/orbit-league/claim", mk_claim, keyfile=OTHER_KEY)
check("makers: the code is refused after", s == 409, (s, b))
check("makers: and the maker stays", sql("select pubkey from makers.realms where realm = 'orbit-league'") == mk_maker_pk)

s, b, _ = call("GET", "/api/playtest/makers/orbit-league")
check("makers: anyone sees the maker's name, never the address", s == 200 and json.loads(b) == {"realm": "orbit-league", "name": "Topher", "pubkey": mk_maker_pk}, (s, b))
s, b, _ = call("GET", "/api/playtest/makers/orbit-league", None, {"Authorization": mk_signed(mk_maker_key, "GET", "/api/playtest/makers/orbit-league")})
check("makers: the maker sees their own address", s == 200 and json.loads(b).get("lightning") == "topher@example.com" and json.loads(b).get("mine") is True, (s, b))

s, b, _ = mk_post("/api/playtest/makers/orbit-league", {"lightning": "x@y.org"}, keyfile=OTHER_KEY)
check("makers: only the maker changes the address", s == 403, (s, b))
s, b, mk_auth = mk_post("/api/playtest/makers/orbit-league", {"lightning": "tips@getalby.com"}, keyfile=mk_maker_key)
check("makers: the maker changes the address", s == 200 and json.loads(b)["lightning"] == "tips@getalby.com", (s, b))
s, b, _ = mk_post("/api/playtest/makers/orbit-league", {"lightning": "tips@getalby.com"}, auth=mk_auth)
check("makers: a signed request is good once", s == 401, (s, b))
s, b, _ = call("POST", "/api/playtest/makers/orbit-league", b"{}", {"Origin": API, "Authorization": mk_signed(mk_maker_key, "POST", "/api/playtest/makers/orbit-league")})
check("makers: a signed write must bind its body", s == 401, (s, b))
s, b, _ = call("GET", "/api/playtest/makers/Bad_Slug")
check("makers: a bad slug is a 404", s == 404, (s, b))
os.unlink(mk_maker_key)
