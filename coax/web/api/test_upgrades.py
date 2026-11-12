# Listed upgrades (makers.c, makerpay-policy.coax, migrations/0077_fairy_upgrades.sql and 0154_maker_direct_pay.sql)
# against the fake Lightning Address services test_makers_tips.py adds: a maker lists a fairy, an admin
# approves it, it shows under UPGRADE, a player buys it by paying the maker's own invoice, wears it
# once the payment is confirmed, and everyone in the room sees its look. Service credit never buys one
# (owner decision D6, #13210).
# test.py runs this file after test_makers_tips.py (the name sorts after it) and uses its helpers.
import tempfile

up_dir = tempfile.mkdtemp()
up_maker_key = os.path.join(up_dir, "maker.key")
up_maker = subprocess.run([os.environ["SIGN"], "--new", up_maker_key], capture_output=True, text=True, check=True).stdout.strip()
up_buyer_key = os.path.join(up_dir, "buyer.key")
up_buyer_pk = subprocess.run([os.environ["SIGN"], "--new", up_buyer_key], capture_output=True, text=True, check=True).stdout.strip()
UP_LOOK = "120140210" + "ff3399" + "33ccff" + "ffcc00" + "6633cc" + "00ffaa" + "ff66cc" + "ffffff"


def up_mint():
    return json.loads(call("POST", "/api/playtest/player", b"{}", {"Content-Type": "application/json"})[1])


def up_call(method, path, obj=None, player=None, keyfile=None, origin=True):
    body = json.dumps(obj).encode() if obj is not None else None
    h = {"Content-Type": "application/json"}
    if origin and method == "POST": h["Origin"] = API
    if player: h["X-Verse-Player"] = player
    if keyfile: h["Authorization"] = mk_signed(keyfile, method, path, body)
    s, b, _ = call(method, path, body, h)
    try: return s, json.loads(b or b"{}")
    except ValueError: return s, b.decode()


up_ann, up_bo = up_mint(), up_mint()
fairy = {"name": "Moonmoth Queen", "by": "Ted", "look": UP_LOOK, "sats": 500, "lightning": "ted@getalby.com"}

s, b = up_call("POST", "/api/playtest/upgrades/fairy", fairy)
check("upgrades: listing needs a login", s == 401, (s, b))
s, b = up_call("POST", "/api/playtest/upgrades/fairy", dict(fairy, look="999999999"), keyfile=up_maker_key)
check("upgrades: a look code Fairy Forest can't draw is refused", s == 400 and "look" in b["error"], (s, b))
s, b = up_call("POST", "/api/playtest/upgrades/fairy", dict(fairy, name="<b>x</b>"), keyfile=up_maker_key)
check("upgrades: a name with markup is refused", s == 400, (s, b))
s, b = up_call("POST", "/api/playtest/upgrades/fairy", dict(fairy, name="x" * 41), keyfile=up_maker_key)
check("upgrades: a name is at most 40 letters", s == 400, (s, b))
s, b = up_call("POST", "/api/playtest/upgrades/fairy", dict(fairy, sats=5), keyfile=up_maker_key)
check("upgrades: a price has a floor", s == 400, (s, b))
s, b = up_call("POST", "/api/playtest/upgrades/fairy", fairy, keyfile=up_maker_key)
up_id = b.get("id")
check("upgrades: a maker lists a fairy; it waits", s == 200 and b["state"] == "pending" and isinstance(up_id, int), (s, b))
s, b = up_call("POST", "/api/playtest/upgrades/fairy", dict(fairy, name="Plain Jane", look="000000000", sats=50), keyfile=up_maker_key)
up_id2 = b.get("id")

s, b = up_call("GET", "/api/playtest/upgrades/fairy", player=up_ann["key"])
check("upgrades: a waiting listing isn't under UPGRADE", s == 200 and b["top"] == [] and b["wearing"] is None and b["pending"] == [] and b["admin"] is False, (s, b))
s, b = up_call("GET", "/api/playtest/upgrades/fairy", keyfile=os.environ["ADMIN_KEY"])
check("upgrades: an admin sees what waits", s == 200 and b["admin"] and [x["id"] for x in b["pending"]] == [up_id, up_id2], (s, b))
check("upgrades: nobody sees the maker's address", "getalby" not in json.dumps(b), b)
s, b = up_call("POST", f"/api/admin/upgrades/{up_id}/approve", {}, keyfile=up_maker_key)
check("upgrades: only an admin approves", s == 403, (s, b))
s, b = up_call("POST", f"/api/admin/upgrades/{up_id}/approve", {}, keyfile=os.environ["ADMIN_KEY"])
check("upgrades: an admin approves", s == 200 and b["state"] == "approved", (s, b))
s, b = up_call("POST", f"/api/admin/upgrades/{up_id2}/approve", {}, keyfile=os.environ["ADMIN_KEY"])
s, b = up_call("GET", "/api/admin/upgrades", keyfile=os.environ["ADMIN_KEY"])
check("upgrades: the admin's lines", s == 200 and f"{up_id}|approved|500|0|Moonmoth Queen|Ted|{UP_LOOK}" in b, (s, b))

s, b = up_call("GET", "/api/playtest/upgrades/fairy", player=up_ann["key"])
check("upgrades: an approved listing shows under UPGRADE", s == 200 and [x["id"] for x in b["top"]] == [up_id2, up_id]
      and b["top"][1]["name"] == "Moonmoth Queen" and b["top"][1]["by"] == "Ted" and b["top"][1]["sats"] == 500
      and b["top"][1]["look"] == UP_LOOK and b["top"][1]["owned"] is False, (s, b))

# sats: the maker's own invoice, confirmed, counted once
up_invoices = sum(1 for p, _ in wallet["asked"] if p == "/invoice"); up_payouts = sql("select count(*) from makers.payouts")
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id}/buy", {"pay": "sats"})
check("upgrades: buying needs a player", s == 403, (s, b))
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id}/buy", {"pay": "sats"}, player=up_ann["key"])
up_hash = b.get("payment_hash", ""); up_inv = b.get("invoice", "")
check("upgrades: buying is the maker's own invoice for the price", s == 200 and up_hash in ln["made"] and up_inv == ln["made"][up_hash]["pr"] and b["sats"] == 500
      and ln["made"][up_hash]["msat"] == 500000 and ln["made"][up_hash]["address"] == "ted@getalby.com" and b["confirm"] == "verify", (s, b))
check("upgrades: the buyer never learns the maker's address or its service", "getalby" not in json.dumps(b) and "verify/" not in json.dumps(b), b)
check("upgrades: our wallet made no invoice", sum(1 for p, _ in wallet["asked"] if p == "/invoice") == up_invoices)
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id}/buy/{up_hash}", {}, player=up_bo["key"])
check("upgrades: someone else's purchase isn't theirs to see", s == 404, (s, b))
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id}/buy/{up_hash}", {"preimage": ln["made"][up_hash]["pre"]}, player=up_bo["key"])
check("upgrades: or to confirm", s == 404, (s, b))
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id}/buy/{up_hash}", {}, player=up_ann["key"])
check("upgrades: unpaid, it is open", s == 200 and b["state"] == "open", (s, b))
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id}/buy/{up_hash}", {"preimage": "11" * 32}, player=up_ann["key"])
check("upgrades: a fake preimage buys nothing", s == 402, (s, b))
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id}/wear", {}, player=up_ann["key"])
check("upgrades: not bought, not worn", s == 403, (s, b))
ln["settled"].add(up_hash)
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id}/buy/{up_hash}", {}, player=up_ann["key"])
check("upgrades: the maker's service says settled: it is bought and its look comes back", s == 200 and b["state"] == "paid" and b["look"] == UP_LOOK and b["id"] == up_id, (s, b))
check("upgrades: the purchase is kept once in the replay ledger", sql(f"select count(*) from inference.used_payments where replay_key like '%{up_hash}'") == "1")
check("upgrades: kept as paid straight to the maker, with its proof",
      sql(f"select direct::text || '|' || confirmed_by || '|' || paid_with || '|' || lightning from makers.tips where payment_hash = '{up_hash}'") == "true|verify|sats|ted@getalby.com")
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id}/buy/{up_hash}", {}, player=up_ann["key"])
check("upgrades: asked again, it is counted once", s == 200 and b["state"] == "paid"
      and sql(f"select count(*) from inference.used_payments where replay_key like '%{up_hash}'") == "1", (s, b))
check("upgrades: a purchase has no payout from our wallet and no first-tip bonus", sql("select count(*) from makers.payouts") == up_payouts)

s, b = up_call("GET", "/api/playtest/upgrades/fairy", player=up_ann["key"])
check("upgrades: the buyer wears it, and owns it", s == 200 and b["wearing"]["id"] == up_id and b["wearing"]["look"] == UP_LOOK
      and [x["owned"] for x in b["top"] if x["id"] == up_id] == [True], (s, b))
check("upgrades: most bought goes first", [x["id"] for x in b["top"]][0] == up_id, b["top"])
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id}/buy", {"pay": "sats"}, player=up_ann["key"])
check("upgrades: what you own isn't sold to you again", s == 409, (s, b))

# what others see: the room carries the look
RF = "/api/playtest/presence/fairy-forest"
up_call("POST", RF, {"x": 1, "z": 1, "after": 0}, player=up_ann["key"])
s, b = up_call("POST", RF, {"x": 2, "z": 2, "after": 0}, player=up_bo["key"])
seen = {o["id"]: o for o in b.get("others", [])}
check("upgrades: everyone in the forest sees the bought fairy", s == 200 and seen.get(up_ann["id"], {}).get("look") == UP_LOOK, b)
s, b = up_call("POST", RF, {"x": 1, "z": 1, "after": 0}, player=up_ann["key"])
seen = {o["id"]: o for o in b.get("others", [])}
check("upgrades: someone who bought nothing carries no look", s == 200 and up_bo["id"] in seen and "look" not in seen[up_bo["id"]], b)
s, b = up_call("POST", "/api/playtest/upgrades/0/wear", {}, player=up_ann["key"])
s2, b2 = up_call("POST", RF, {"x": 2, "z": 2, "after": 0}, player=up_bo["key"])
check("upgrades: back in the fairy you made, the room carries no look", s == 200 and "look" not in {o["id"]: o for o in b2.get("others", [])}.get(up_ann["id"], {"look": 1}), (b, b2))
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id}/wear", {}, player=up_ann["key"])
check("upgrades: what you bought you wear again", s == 200 and b["look"] == UP_LOOK, (s, b))

# service credit never buys an upgrade (decision D4: credit pays only for our own services)
sql(f"insert into inference.credit (pubkey, msat) values ('{up_buyer_pk}', 600000) on conflict (pubkey) do update set msat = 600000")
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id}/buy", {"pay": "credit"}, player=up_bo["key"], keyfile=up_buyer_key)
check("upgrades: credit is refused by name, and nothing is taken", s == 400 and "credit" in b["error"] and "Bitcoin" in b["error"]
      and sql(f"select msat from inference.credit where pubkey = '{up_buyer_pk}'") == "600000"
      and sql(f"select count(*) from makers.tips where listing = {up_id} and buyer = '{up_bo['id']}'") == "0", (s, b))
check("upgrades: the old rule's function refuses too", sql(f"select makers.buy_credit({up_id}, '{up_bo['id']}', '{up_buyer_pk}', false)->>'status'") == "400"
      and sql(f"select msat from inference.credit where pubkey = '{up_buyer_pk}'") == "600000")
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id}/buy", {"pay": "card"}, player=up_bo["key"])
check("upgrades: sats is the only way to pay", s == 400, (s, b))

# a maker whose service can't confirm: offered only to a wallet that reports the payment
ln["verify"] = False
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id}/buy", {"pay": "sats"}, player=up_bo["key"])
check("upgrades: no way to confirm, no invoice: nobody pays for something we couldn't deliver", s == 422 and "invoice" not in b and "confirm" in b["error"]
      and sql(f"select count(*) from makers.tips where listing = {up_id} and buyer = '{up_bo['id']}'") == "0", (s, b))
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id}/buy", {"pay": "sats", "reports": True}, player=up_bo["key"])
up_hash2 = b.get("payment_hash", "")
check("upgrades: a wallet that reports the payment gets the maker's invoice", s == 200 and b["confirm"] == "wallet" and up_hash2 in ln["made"], (s, b))
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id}/buy/{up_hash2}", {}, player=up_bo["key"])
check("upgrades: unconfirmed, it is not theirs", s == 200 and b["state"] == "open" and up_call("POST", f"/api/playtest/upgrades/{up_id}/wear", {}, player=up_bo["key"])[0] == 403, (s, b))
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id}/buy/{up_hash2}", {"preimage": ln["made"][up_hash]["pre"]}, player=up_bo["key"])
check("upgrades: the first buyer's preimage doesn't buy it for the second", s == 402, (s, b))
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id}/buy/{up_hash2}", {"preimage": ln["made"][up_hash2]["pre"]}, player=up_bo["key"])
check("upgrades: the wallet's preimage hashes to the invoice: bought", s == 200 and b["state"] == "paid" and b["look"] == UP_LOOK, (s, b))
ln["verify"] = True
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id}/buy", {"pay": "sats"}, player=up_bo["key"])
check("upgrades: what was bought isn't sold again", s == 409, (s, b))
check("upgrades: still no payout of a buyer's money", sql("select count(*) from makers.payouts") == up_payouts)
up_call("POST", RF, {"x": 2, "z": 2, "after": 0}, player=up_bo["key"])
s, b = up_call("POST", RF, {"x": 1, "z": 1, "after": 0}, player=up_ann["key"])
check("upgrades: and the second buyer is seen in it too", {o["id"]: o for o in b.get("others", [])}.get(up_bo["id"], {}).get("look") == UP_LOOK, b)
# a listing whose maker changes address between the ask and the invoice
ln["hook"] = lambda: sql(f"update makers.listings set lightning = 'moved@wallet.example' where id = {up_id2}")
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id2}/buy", {"pay": "sats"}, player=up_bo["key"])
ln["hook"] = None
check("upgrades: the maker changed address while we asked: no invoice, ask again", s == 503 and "changed" in b["error"], (s, b))
s, b = up_call("POST", f"/api/admin/upgrades/{up_id}/reject", {}, keyfile=os.environ["ADMIN_KEY"])
s, b = up_call("GET", "/api/playtest/upgrades/fairy", player=up_ann["key"])
up_call("POST", RF, {"x": 1, "z": 1, "after": 0}, player=up_ann["key"])
s2, b2 = up_call("POST", RF, {"x": 2, "z": 2, "after": 0}, player=up_bo["key"])
check("upgrades: a rejected listing leaves UPGRADE and the room", up_id not in [x["id"] for x in b["top"]] and up_ann["id"] in {o["id"] for o in b2.get("others", [])}
      and "look" not in {o["id"]: o for o in b2["others"]}[up_ann["id"]], (b, b2))
check("upgrades: a page of another site can't buy",
      call("POST", f"/api/playtest/upgrades/{up_id2}/buy", b"{}", {"Origin": "https://evil.example", "X-Verse-Player": up_bo["key"]})[0] == 403)
