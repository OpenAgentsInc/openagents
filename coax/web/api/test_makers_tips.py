# Tips for a realm's maker (makers.c, pay/makerpay-policy.coax, migrations/0154_maker_direct_pay.sql;
# owner decision D6, #13210): a tip goes from the player's wallet straight to the maker's own
# Lightning Address. The fake Spark wallet test_inference.py serves also plays every Lightning
# Address service here (test.sh builds the API so its LNURL fetches go to it, https://host/path as
# /host/path; the URL rules stay the real ones). Its invoices are real BOLT 11, signed below, so the
# API's own checks (the amount, the description hash, the expiry) read them.
# test.py runs this file after test_makers.py (the name sorts after it) (Orbit League's maker: tips@getalby.com).
import time
import urllib.parse

# ---- BOLT 11 for the fake services: Bech32 words and a recoverable secp256k1 signature
B32 = "qpzry9x8gf2tvdw0s3jn54khce6mua7l"
EC_P = 2**256 - 2**32 - 977
EC_N = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141
EC_G = (0x79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798, 0x483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8)


def ec_add(a, b):
    if a is None: return b
    if b is None: return a
    if a[0] == b[0] and (a[1] + b[1]) % EC_P == 0: return None
    m = (3 * a[0] * a[0] * pow(2 * a[1], -1, EC_P) if a == b else (b[1] - a[1]) * pow(b[0] - a[0], -1, EC_P)) % EC_P
    x = (m * m - a[0] - b[0]) % EC_P
    return x, (m * (a[0] - x) - a[1]) % EC_P


def ec_mul(k, pt):
    out = None
    while k:
        if k & 1: out = ec_add(out, pt)
        pt = ec_add(pt, pt); k >>= 1
    return out


def b32_words(data):
    acc = n = 0; out = []
    for x in data:
        acc = (acc << 8) | x; n += 8
        while n >= 5: n -= 5; out.append((acc >> n) & 31)
    if n: out.append((acc << (5 - n)) & 31)
    return out


def b32_number(v, width=0):
    out = []
    while v: out.insert(0, v & 31); v >>= 5
    return [0] * (width - len(out)) + out


def b32_text(hrp, words):
    chk = 1
    for v in [ord(x) >> 5 for x in hrp] + [0] + [ord(x) & 31 for x in hrp] + words + [0] * 6:
        top = chk >> 25; chk = (chk & 0x1ffffff) << 5 ^ v
        for i, g in enumerate((0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3)):
            if (top >> i) & 1: chk ^= g
    chk ^= 1
    return hrp + "1" + "".join(B32[w] for w in words + [(chk >> 5 * (5 - i)) & 31 for i in range(6)])


def ln_invoice(msat, description_hash, payment_hash, stamp, expiry):
    """A mainnet invoice for msat committing to description_hash, signed by a throwaway node key."""
    hrp = f"lnbc{msat * 10}p"
    words = b32_number(stamp, 7)
    for kind, value in ((1, b32_words(payment_hash)), (16, b32_words(b"\x09" * 32)), (23, b32_words(description_hash)), (6, b32_number(expiry))):
        words += [kind, len(value) >> 5, len(value) & 31] + value
    packed = bytearray(); acc = n = 0
    for w in words:
        acc = (acc << 5) | w; n += 5
        while n >= 8: n -= 8; packed.append((acc >> n) & 255)
    if n: packed.append((acc << (8 - n)) & 255)
    z = int.from_bytes(hashlib.sha256(hrp.encode() + bytes(packed)).digest(), "big")
    d, k = 0x1F1E1D1C1B1A191817161514131211100F0E0D0C0B0A09080706050403020100 + 7, z % EC_N or 1
    R = ec_mul(k, EC_G); r = R[0] % EC_N; s = pow(k, -1, EC_N) * (z + r * d) % EC_N; rec = R[1] & 1
    if s > EC_N // 2: s, rec = EC_N - s, rec ^ 1
    return b32_text(hrp, words + b32_words(r.to_bytes(32, "big") + s.to_bytes(32, "big") + bytes([rec])))


# ---- the fake Lightning Address services: /<host>/.well-known/lnurlp/<name>, its callback and its LUD-21 verify
ln = {"asked": [], "made": {}, "settled": set(), "verify": True, "mode": "ok", "hook": None}   # made: payment hash -> {pre, pr, msat, address}
LN_META = '[["text/plain","Sats for the maker"]]'


def ln_get(req):
    path, _, query = req.path.partition("?")
    parts = path.strip("/").split("/")
    if len(parts) < 3: return req.reply(502, {"error": "the wallet failed"})   # the wallet's own GETs (/health): as before
    host = parts[0]
    ln["asked"].append({"path": req.path, "headers": {k.lower(): v for k, v in req.headers.items()}})
    if ln["mode"] == "down": return req.reply(500, {"error": "down"})
    if ln["mode"] == "redirect":
        req.send_response(302); req.send_header("Location", "https://example.org/elsewhere"); req.send_header("Content-Length", "0"); req.end_headers(); return
    if ln["mode"] == "huge": return req.reply(200, {"tag": "payRequest", "pad": "x" * 70000})
    if parts[1:3] == [".well-known", "lnurlp"] and len(parts) == 4:
        return req.reply(200, {"tag": "payRequest", "callback": f"https://{host}/lnurlp/{parts[3]}/callback", "minSendable": 1000, "maxSendable": 1000000000, "metadata": LN_META})
    if parts[1] == "lnurlp" and len(parts) == 4 and parts[3] == "callback":
        if ln["hook"]: ln["hook"]()
        msat = int(urllib.parse.parse_qs(query)["amount"][0])
        if ln["mode"] == "replay":   # the same invoice again, whatever was asked
            old = next(iter(ln["made"].values()))
            return req.reply(200, {"pr": old["pr"], "routes": []})
        pre = hashlib.sha256(f"{req.path}{time.time()}{len(ln['made'])}".encode()).digest()
        ph = hashlib.sha256(pre).digest()
        dh = hashlib.sha256((LN_META + ("x" if ln["mode"] == "other_description" else "")).encode()).digest()
        stamp = int(time.time()) - (7200 if ln["mode"] == "expired" else 0)
        pr = ln_invoice(msat + (1000 if ln["mode"] == "more" else 0), dh, ph, stamp, 600)
        ln["made"][ph.hex()] = {"pre": pre.hex(), "pr": pr, "msat": msat, "address": f"{parts[2]}@{host}"}
        out = {"pr": pr, "routes": []}
        if ln["verify"]: out["verify"] = f"https://{host}/lnurlp/{parts[2]}/verify/{ph.hex()}"
        return req.reply(200, out)
    if parts[1] == "lnurlp" and len(parts) == 5 and parts[3] == "verify":
        made = ln["made"].get(parts[4])
        if not made: return req.reply(404, {"status": "ERROR", "reason": "Not found"})
        if ln["mode"] == "lie": return req.reply(200, {"status": "OK", "settled": True, "preimage": "00" * 32, "pr": made["pr"]})
        paid = parts[4] in ln["settled"]
        return req.reply(200, {"status": "OK", "settled": paid, "preimage": made["pre"] if paid else None, "pr": made["pr"]})
    req.reply(404, {"status": "ERROR", "reason": "Not found"})


wallet["get"] = ln_get

tw = {"quotes": [], "sends": [], "paid": {}, "send_mode": ["completed"]}   # send_mode: what each /send does, in turn


def tw_quote(q):
    tw["quotes"].append(q)
    if tw.get("quote_fails"):
        return 502, {"error": "the address did not answer"}
    pre = hashlib.sha256(json.dumps(q).encode() + str(time.time()).encode()).hexdigest()
    h = hashlib.sha256(bytes.fromhex(pre)).hexdigest()
    fee = 2
    return 200, {"invoice": f"lnbc{q['sats'] - fee}n1maker{h[:20]}{len(tw['quotes'])}", "payment_hash": h, "amount_sats": q["sats"] - fee, "fee_sats": fee, "expiry": int(time.time()) + 600}


def tw_send(q):
    tw["sends"].append(q["invoice"])
    if tw["paid"].get(q["invoice"]) in ("completed", "pending"):   # the real wallet never sends twice either
        return 200, {"state": tw["paid"][q["invoice"]]}
    mode = tw["send_mode"].pop(0) if len(tw["send_mode"]) > 1 else tw["send_mode"][0]
    if mode == "lost":   # paid, but the answer never came back
        tw["paid"][q["invoice"]] = "completed"
        return 500, {"error": "the wallet failed"}
    tw["paid"][q["invoice"]] = mode
    return 200, {"state": mode, "fee_sats": 2}


def tw_sent(q):
    st = tw["paid"].get(q["invoice"])
    return 200, ({"state": st, "fee_sats": 2} if st else {"state": "none"})


wallet["extra"] = {"/quote": tw_quote, "/send": tw_send, "/sent": tw_sent}


def tw_post(path, obj, keyfile=None):
    body = json.dumps(obj).encode()
    h = {"Content-Type": "application/json", "Origin": API, "X-Forwarded-For": "203.0.113.77, 130.211.0.1", "Cookie": "oa_player=secret"}
    if keyfile: h["Authorization"] = mk_signed(keyfile, "POST", path, body)
    s, b, _ = call("POST", path, body, h)
    return s, json.loads(b or b"{}")


def tw_wait(cond, seconds=8):
    end = time.time() + seconds
    while time.time() < end:
        if cond(): return True
        time.sleep(0.1)
    return False


def tw_payout(tip):
    return sql(f"select state || '|' || sats || '|' || bonus_sats from makers.payouts where tip = '{tip}'")


def tw_tip(hash_):
    return sql(f"select (paid is not null)::text || '|' || coalesce(confirmed_by, '') || '|' || direct::text from makers.tips where payment_hash = '{hash_}'")


TIP = "/api/playtest/makers/orbit-league/tip"
tw_invoices = sum(1 for p, _ in wallet["asked"] if p == "/invoice")
s, b = tw_post("/api/playtest/makers/belt-2200/tip", {"sats": 100})
check("tips: an unclaimed realm takes no tips", s == 404, (s, b))
s, b = tw_post(TIP, {"sats": 5})
check("tips: a tip has a floor", s == 400 and "10" in b["error"], (s, b))
s, b = tw_post(TIP, {"sats": "100"})
check("tips: sats is a number", s == 400, (s, b))
check("tips: none of those asked the maker's address", ln["asked"] == [], ln["asked"])

# the maker's own invoice, asked by our server
s, b = tw_post(TIP, {"sats": 100})
tip1 = b.get("payment_hash", "")
check("tips: a tip is the maker's own invoice, for exactly the tip", s == 200 and tip1 in ln["made"] and b["invoice"] == ln["made"][tip1]["pr"]
      and ln["made"][tip1]["msat"] == 100000 and ln["made"][tip1]["address"] == "tips@getalby.com" and b["sats"] == 100 and b["to"] == "Topher", (s, b))
check("tips: the page learns the maker's service can confirm it, never where", b.get("confirm") == "verify" and "verify/" not in json.dumps(b) and "getalby" not in json.dumps(b), b)
check("tips: our wallet made no invoice: nothing of the player's comes to us", sum(1 for p, _ in wallet["asked"] if p == "/invoice") == tw_invoices)
check("tips: the maker's service was asked for its payRequest, then its invoice",
      [a["path"].split("?")[0] for a in ln["asked"]] == ["/getalby.com/.well-known/lnurlp/tips", "/getalby.com/lnurlp/tips/callback"], ln["asked"])
seen = json.dumps([a["headers"] for a in ln["asked"]])
check("tips: it saw our server and nothing of the player: no network address, no cookie, no origin",
      "203.0.113.77" not in seen and "oa_player" not in seen and "x-forwarded-for" not in seen and "cookie" not in seen and "origin" not in seen
      and all(a["headers"].get("user-agent") == "OpenAgents-LNURL/1" for a in ln["asked"]), seen)
check("tips: kept as a direct payment, unpaid", tw_tip(tip1) == "false||true", tw_tip(tip1))

s, b = tw_post(f"{TIP}/{tip1}", {})
check("tips: unpaid, it is open and nothing is counted", s == 200 and b["state"] == "open" and b["bonus"] == "" and tw_tip(tip1) == "false||true", (s, b))
s, b = tw_post(f"{TIP}/{tip1}", {"preimage": "00" * 32})
check("tips: a fake preimage is refused and counts nothing", s == 402 and "preimage" in b["error"] and tw_tip(tip1) == "false||true", (s, b))
s, b = tw_post(f"{TIP}/{tip1}", {"preimage": "zz"})
check("tips: so is one that isn't a preimage at all", s == 402 and tw_tip(tip1) == "false||true", (s, b))
ln["mode"] = "lie"
s, b = tw_post(f"{TIP}/{tip1}", {})
check("tips: a service that says settled with a preimage that doesn't hash to the invoice is not believed", s == 200 and b["state"] == "open" and tw_tip(tip1) == "false||true", (s, b))
ln["mode"] = "ok"
check("tips: nothing was credited or queued so far", sql("select count(*) from makers.payouts") == "0"
      and sql(f"select count(*) from inference.used_payments where replay_key like '%:{tip1}'") == "0")

# confirmed by the maker's service (LUD-21)
ln["settled"].add(tip1)
s, b = tw_post(f"{TIP}/{tip1}", {})
check("tips: the maker's service says settled, with the preimage: paid", s == 200 and b["state"] == "paid" and b["to"] == "Topher" and b["sats"] == 100, (s, b))
check("tips: counted as confirmed by the service, with its proof", tw_tip(tip1) == "true|verify|true"
      and sql(f"select preimage from makers.tips where payment_hash = '{tip1}'") == ln["made"][tip1]["pre"], tw_tip(tip1))
check("tips: the payment is kept once in the replay ledger, as the maker's",
      sql(f"select count(*) || '|' || min(pubkey) || '|' || min(msat) from inference.used_payments where replay_key like '%:{tip1}'") == f"1|{mk_maker_pk}|100000")
check("tips: the tip itself has no payout: our wallet never held it", sql(f"select count(*) from makers.payouts where tip = '{tip1}'") == "0")
bonus1 = sql(f"select 'bonus:' || md5('{tip1}')")
check("tips: the first confirmed tip's bonus is a payment of its own, from our wallet",
      sql(f"select source || '|' || sats || '|' || bonus_sats || '|' || lightning from makers.payouts where tip = '{bonus1}'") == "bonus|1000|1000|tips@getalby.com", tw_payout(bonus1))
check("tips: the bonus is sent to the maker's address", tw_wait(lambda: tw_payout(bonus1).startswith("sent")), tw_payout(bonus1))
check("tips: our wallet quoted the address for the bonus alone, and sent once",
      len(tw["quotes"]) == 1 and tw["quotes"][-1]["address"] == "tips@getalby.com" and tw["quotes"][-1]["sats"] == 1000 and "bonus" in tw["quotes"][-1]["comment"]
      and len(tw["sends"]) == 1, (tw["quotes"], tw["sends"]))

# a replayed confirmation
s, b = tw_post(f"{TIP}/{tip1}", {})
s2, b2 = tw_post(f"{TIP}/{tip1}", {"preimage": ln["made"][tip1]["pre"]})
check("tips: confirmed again, by the service or by its preimage, it stays paid and is counted once",
      s == 200 and b["state"] == "paid" and b["bonus"] == "sent" and s2 == 200 and b2["state"] == "paid"
      and sql(f"select count(*) from inference.used_payments where replay_key like '%:{tip1}'") == "1"
      and sql("select count(*) from makers.payouts") == "1" and len(tw["sends"]) == 1, (b, b2, tw["sends"]))

# confirmed by the preimage the player's wallet reports; a maker whose service has no LUD-21
ln["verify"] = False
s, b = tw_post(TIP, {"sats": 200})
tip2 = b.get("payment_hash", "")
check("tips: a maker's service without LUD-21: only the paying wallet can confirm", s == 200 and b["confirm"] == "wallet"
      and sql(f"select (verify_url is null)::text from makers.tips where payment_hash = '{tip2}'") == "true", (s, b))
asked = len(ln["asked"])
s, b = tw_post(f"{TIP}/{tip2}", {})
check("tips: with no confirmation it stays open: sent from the wallet, nothing credited", s == 200 and b["state"] == "open" and tw_tip(tip2) == "false||true"
      and len(ln["asked"]) == asked, (s, b))
s, b = tw_post(f"{TIP}/{tip2}", {"preimage": ln["made"][tip1]["pre"]})
check("tips: another payment's preimage confirms nothing", s == 402 and tw_tip(tip2) == "false||true", (s, b))
s, b = tw_post(f"{TIP}/{tip2}", {"preimage": ln["made"][tip2]["pre"]})
check("tips: the wallet's preimage hashes to the invoice: paid", s == 200 and b["state"] == "paid" and tw_tip(tip2) == "true|wallet|true", (s, b))
check("tips: a second tip carries no bonus, and no payout at all", b["bonus"] == "" and sql("select count(*) from makers.payouts") == "1", b)
sql(f"update makers.tips set paid = null, confirmed_by = null, preimage = null where payment_hash = '{tip2}'")
s, b = tw_post(f"{TIP}/{tip2}", {"preimage": ln["made"][tip2]["pre"]})
check("tips: a payment hash already in the replay ledger is never counted again", s == 409 and tw_tip(tip2) == "false||true", (s, b))
sql(f"update makers.tips set paid = now(), confirmed_by = 'wallet', preimage = '{ln['made'][tip2]['pre']}' where payment_hash = '{tip2}'")
s, b = tw_post(TIP, {"sats": 300})
tip3 = b["payment_hash"]
sql(f"update makers.tips set expires = now() - interval '1 minute' where payment_hash = '{tip3}'")
s, b = tw_post(f"{TIP}/{tip3}", {})
check("tips: never confirmed, the invoice runs out: expired, nothing credited", s == 200 and b["state"] == "expired" and tw_tip(tip3) == "false||true", (s, b))
s, b = tw_post(f"{TIP}/{tip3}", {"preimage": ln["made"][tip3]["pre"]})
check("tips: a proof that comes after the invoice's time still counts", s == 200 and b["state"] == "paid", (s, b))
ln["verify"] = True

# an invoice that doesn't check is never shown
tw_rows = sql("select count(*) from makers.tips")
for mode, why in (("more", "a wrong amount"), ("other_description", "another description hash"), ("expired", "an expired invoice"),
                  ("replay", "an invoice seen before (a swapped one)"), ("down", "a service that fails"), ("redirect", "a redirect"), ("huge", "an answer over 64 KB")):
    ln["mode"] = mode
    sql(f"update makers.realms set lightning = '{mode.replace('_', '')}@wallet.example' where realm = 'orbit-league'")   # each its own address: the payRequest is cached a minute
    s, b = tw_post(TIP, {"sats": 100})
    check(f"tips: {why} is refused, and nothing is shown or kept", s == 502 and "invoice" not in b and sql("select count(*) from makers.tips") == tw_rows, (mode, s, b))
ln["mode"] = "ok"
check("tips: the redirect was not followed", not any("elsewhere" in a["path"] for a in ln["asked"]))

# where our server will not fetch (lnurl.coax's URL rules; the connect-time address check is proved on staging)
asked = len(ln["asked"])
for addr in ("a@localhost", "a@metadata.google.internal", "a@printer.local", "a@10.0.0.1", "a@169.254.169.254", "a@hidden.onion", "a@intranet"):
    sql(f"update makers.realms set lightning = '{addr}' where realm = 'orbit-league'")
    s, b = tw_post(TIP, {"sats": 100})
    check(f"tips: {addr} is never fetched", s == 502 and len(ln["asked"]) == asked and sql("select count(*) from makers.tips") == tw_rows, (addr, s, b))

# a maker who changes address mid-payment
sql("update makers.realms set lightning = 'first@wallet.example' where realm = 'orbit-league'")
ln["hook"] = lambda: sql("update makers.realms set lightning = 'second@wallet.example' where realm = 'orbit-league'")
s, b = tw_post(TIP, {"sats": 100})
ln["hook"] = None
check("tips: the maker changed address while we asked the old one: no invoice, ask again", s == 503 and "invoice" not in b and "changed" in b["error"]
      and sql("select count(*) from makers.tips") == tw_rows, (s, b))
s, b = tw_post(TIP, {"sats": 100})
check("tips: asked again, the invoice is the new address's", s == 200 and ln["made"][b["payment_hash"]]["address"] == "second@wallet.example", (s, b))

# an address of ours forwards: asked through our own rows, exactly as its callback would
tw_acct = sql("select auth.new_person(NULL)")
sql(f"insert into pay.addresses (name, skeleton, account, mode, forward_to) values ('makerpal', 'makerpal', '{tw_acct}', 'forward', 'pal@wallet.example')")
sql("update makers.realms set lightning = 'makerpal@openagents.com' where realm = 'orbit-league'")
s, b = tw_post(TIP, {"sats": 150})
check("tips: a maker's name@openagents.com is paid where it forwards", s == 200 and ln["made"][b["payment_hash"]]["address"] == "pal@wallet.example"
      and sql(f"select name || '|' || amount_msat from pay.invoices where payment_hash = '{b.get('payment_hash')}'") == "makerpal|150000", (s, b))
sql("update pay.addresses set state = 'off' where name = 'makerpal'")
s, b = tw_post(TIP, {"sats": 150})
check("tips: an address of ours that is turned off gives no invoice", s == 502, (s, b))
sql("update makers.realms set lightning = 'tips@getalby.com' where realm = 'orbit-league'")

# our wallet pays out only our own money
r = subprocess.run(PSQL + [f"insert into makers.payouts (tip, realm, maker, lightning, sats) values ('{tip2}', 'orbit-league', '{mk_maker_pk}', 'tips@getalby.com', 200)"], capture_output=True, text=True)
check("tips: a direct tip can never get a payout of the player's money", r.returncode != 0 and "no payout from our wallet" in r.stderr, r.stderr)
r = subprocess.run(PSQL + [f"update makers.tips set paid = now() where payment_hash = (select payment_hash from makers.tips where direct and paid is null limit 1)"], capture_output=True, text=True)
check("tips: a direct tip can never be marked paid without its proof", r.returncode != 0 and "tips_direct_proof" in r.stderr, r.stderr)

# a tip paid into our wallet before D6 is still settled and paid out; none is made after
old_pre = hashlib.sha256(b"before D6").hexdigest(); old_hash = hashlib.sha256(bytes.fromhex(old_pre)).hexdigest(); old_inv = f"lnbc400n1p{old_hash}"
sql(f"insert into makers.tips (payment_hash, invoice, realm, maker, lightning, sats, expires) values ('{old_hash}', '{old_inv}', 'orbit-league', '{mk_maker_pk}', 'tips@getalby.com', 400, now() + interval '10 minutes')")
wallet["paid"][old_inv] = {"state": "completed", "pre": old_pre, "hash": old_hash, "sats": 400}
sends = len(tw["sends"])
s, b = tw_post(f"{TIP}/{old_hash}", {})
check("tips: an invoice of ours from before is settled from our wallet's record", s == 200 and b["state"] == "paid" and tw_tip(old_hash) == "true||false", (s, b))
check("tips: and what it paid us is paid out to the maker, once", tw_wait(lambda: tw_payout(old_hash).startswith("sent")) and len(tw["sends"]) == sends + 1
      and sql(f"select source from makers.payouts where tip = '{old_hash}'") == "tip", tw_payout(old_hash))
s, b = tw_post(f"{TIP}/{old_hash}", {"preimage": old_pre})
check("tips: it is never confirmed the new way", s == 200 and b["state"] == "paid" and tw_tip(old_hash) == "true||false", (s, b))

# a failed bonus retries with a new invoice, and is paid once; one with no answer is asked about, never sent again
sql("delete from makers.payouts where source = 'bonus'")   # this run's database only: the next confirmed tip is a first again
tw["send_mode"] = ["failed", "completed"]
s, b = tw_post(TIP, {"sats": 100})
tip5 = b["payment_hash"]; ln["settled"].add(tip5); bonus5 = sql(f"select 'bonus:' || md5('{tip5}')")
s, b = tw_post(f"{TIP}/{tip5}", {})
check("tips: a failed bonus waits to try again", s == 200 and b["state"] == "paid" and tw_wait(lambda: tw_payout(bonus5).startswith("failed")) and
      sql(f"select (next_try > now())::text || '|' || (invoice is null)::text from makers.payouts where tip = '{bonus5}'") == "true|true", tw_payout(bonus5))
s, b = tw_post(f"{TIP}/{tip5}", {})
check("tips: the tip is paid whatever became of the bonus", s == 200 and b["state"] == "paid" and b["bonus"] == "failed", (s, b))
sql(f"update makers.payouts set next_try = now() where tip = '{bonus5}'")
s, b = tw_post("/api/admin/makers/payouts/run", {}, keyfile=os.environ["ADMIN_KEY"])
check("tips: an admin runs the due payouts", s == 200, (s, b))
check("tips: tried again, the bonus is sent", tw_wait(lambda: tw_payout(bonus5).startswith("sent")), tw_payout(bonus5))
check("tips: one payment completed for it", sum(1 for i, v in tw["paid"].items() if v == "completed" and "maker" in i and i in tw["sends"][-2:]) == 1, tw["paid"])
sql("delete from makers.payouts where source = 'bonus'")
tw["send_mode"] = ["lost"]
s, b = tw_post(TIP, {"sats": 100})
tip6 = b["payment_hash"]; ln["settled"].add(tip6); bonus6 = sql(f"select 'bonus:' || md5('{tip6}')")
sends = len(tw["sends"])
s, b = tw_post(f"{TIP}/{tip6}", {})
check("tips: a bonus with no answer keeps its invoice", tw_wait(lambda: tw_payout(bonus6).startswith("failed")) and
      sql(f"select (invoice is not null)::text from makers.payouts where tip = '{bonus6}'") == "true", tw_payout(bonus6))
sql(f"update makers.payouts set next_try = now() where tip = '{bonus6}'")
time.sleep(1.1)   # a NIP-98 event is good once: the same request in the same second is the same event
s, b = tw_post("/api/admin/makers/payouts/run", {}, keyfile=os.environ["ADMIN_KEY"])
check("tips: the admin's second run is let in", s == 200, (s, b))
check("tips: asked again, our wallet's record says it was paid: sent, without paying twice",
      tw_wait(lambda: tw_payout(bonus6).startswith("sent")) and len(tw["sends"]) == sends + 1, (tw_payout(bonus6), tw["sends"][sends:]))

# the owner's gift from our wallet
tw["send_mode"] = ["completed"]
s, b = tw_post("/api/admin/makers/orbit-league/gift", {"sats": 50}, keyfile=OTHER_KEY)
check("tips: only an admin gives a gift", s == 403, (s, b))
s, b = tw_post("/api/admin/makers/orbit-league/gift", {"sats": 50}, keyfile=os.environ["ADMIN_KEY"])
gift = b.get("tip", "")
check("tips: an admin's gift is a paid tip from our wallet, flagged team", s == 200 and gift.startswith("gift:") and
      sql(f"select team::text || '|' || (invoice is null)::text from makers.tips where payment_hash = '{gift}'") == "true|true", (s, b))
check("tips: the gift is paid out to the maker, with no bonus, named a gift", tw_wait(lambda: tw_payout(gift).startswith("sent")) and tw_payout(gift).endswith("|50|0")
      and sql(f"select source from makers.payouts where tip = '{gift}'") == "gift", tw_payout(gift))
s, b = tw_post("/api/admin/makers/orbit-league/gift", {"sats": 999999}, keyfile=os.environ["ADMIN_KEY"])
check("tips: a gift has a ceiling", s == 400, (s, b))
s, b, _ = call("GET", "/api/admin/makers/payouts", None, {"Authorization": mk_signed(os.environ["ADMIN_KEY"], "GET", "/api/admin/makers/payouts")})
check("tips: an admin reads the payouts, each with what it is", s == 200 and b.decode().count("orbit-league|sent") == 3 and b.decode().count("|bonus") == 1
      and b.decode().count("|gift") == 1 and b.decode().count("|tip") == 1, b)
check("tips: every payout our wallet sent was our own money or owed from before",
      sql("select count(*) from makers.payouts p join makers.tips t on t.payment_hash = p.tip where t.direct") == "0")
s, b = tw_post(f"{TIP}/" + "ab" * 32, {})
check("tips: an unknown tip is a 404", s == 404, (s, b))
s, b, _ = call("POST", TIP, b'{"sats": 100}', {"Origin": "https://evil.example", "Content-Type": "application/json"})
check("tips: another site's page can't ask for one", s == 403, (s, b))
wallet["extra"] = {}
