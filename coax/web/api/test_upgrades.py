# Listed upgrades (ev-asset-upgrades, makers.c, migrations/0077_fairy_upgrades.sql) against the fake Spark
# wallet test_inference.py serves and the payout fakes test_makers_tips.py adds: a maker lists a fairy,
# an admin approves it, it shows under UPGRADE, a player buys it with sats or with credit, wears it,
# everyone in the room sees its look, and the maker's share is paid out once.
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


wallet["extra"] = {"/quote": tw_quote, "/send": tw_send, "/sent": tw_sent}   # test_makers_tips.py took them off at its end
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

# sats: our wallet's invoice, paid, settled once
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id}/buy", {"pay": "sats"})
check("upgrades: buying needs a player", s == 403, (s, b))
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id}/buy", {"pay": "sats"}, player=up_ann["key"])
up_hash = b.get("payment_hash", ""); up_inv = b.get("invoice", "")
check("upgrades: buying with sats is our wallet's invoice for the price", s == 200 and up_inv.startswith("lnbc") and b["sats"] == 500 and len(up_hash) == 64
      and wallet["last"]["sats"] == 500 and "Moonmoth Queen" in wallet["last"]["memo"], (s, b))
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id}/buy/{up_hash}", {}, player=up_bo["key"])
check("upgrades: someone else's purchase isn't theirs to see", s == 404, (s, b))
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id}/buy/{up_hash}", {}, player=up_ann["key"])
check("upgrades: unpaid, it is open", s == 200 and b["state"] == "open", (s, b))
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id}/wear", {}, player=up_ann["key"])
check("upgrades: not bought, not worn", s == 403, (s, b))
tw["send_mode"] = ["completed"]; up_sends = len(tw["sends"])
wallet["paid"][up_inv]["state"] = "completed"
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id}/buy/{up_hash}", {}, player=up_ann["key"])
check("upgrades: paid, it is bought and its look comes back", s == 200 and b["state"] == "paid" and b["look"] == UP_LOOK, (s, b))
check("upgrades: the purchase is kept once in the replay ledger", sql(f"select count(*) from inference.used_payments where replay_key like '%{up_hash}'") == "1")
check("upgrades: the maker's payout is sent to their address, the price less the fee",
      tw_wait(lambda: tw_payout(up_hash) == "sent|500|0") and sql(f"select source || '|' || lightning from makers.payouts where tip = '{up_hash}'") == "upgrade|ted@getalby.com",
      tw_payout(up_hash))
check("upgrades: the quote asked the maker's address for the price", any(q["address"] == "ted@getalby.com" and q["sats"] == 500 for q in tw["quotes"]), tw["quotes"][-2:])
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id}/buy/{up_hash}", {}, player=up_ann["key"])
time.sleep(0.5)
check("upgrades: asked again, nothing is paid twice", s == 200 and b["state"] == "paid" and len(tw["sends"]) == up_sends + 1
      and sql(f"select count(*) from makers.payouts where tip = '{up_hash}'") == "1", (s, b, len(tw["sends"]) - up_sends))
check("upgrades: the first purchase carries no first-tip bonus", sql(f"select bonus_sats from makers.payouts where tip = '{up_hash}'") == "0")

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

# credit: the inference credit ledger
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id}/buy", {"pay": "credit"}, player=up_bo["key"])
check("upgrades: credit needs a login", s == 401, (s, b))
sql(f"insert into inference.credit (pubkey, msat) values ('{up_buyer_pk}', 400000) on conflict (pubkey) do update set msat = 400000")
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id}/buy", {"pay": "credit"}, player=up_bo["key"], keyfile=up_buyer_key)
check("upgrades: not enough credit is a 402, and nothing is taken", s == 402 and b["credit_msat"] == 400000 and sql(f"select msat from inference.credit where pubkey = '{up_buyer_pk}'") == "400000", (s, b))
sql(f"update inference.credit set msat = 600000 where pubkey = '{up_buyer_pk}'")
up_sends = len(tw["sends"])
time.sleep(1.1)   # a NIP-98 event is its second and its body: the same one again is a replay
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id}/buy", {"pay": "credit"}, player=up_bo["key"], keyfile=up_buyer_key)
check("upgrades: bought with credit, the price comes out of it", s == 200 and b["state"] == "paid" and b["look"] == UP_LOOK and b["credit_msat"] == 100000
      and sql(f"select msat from inference.credit where pubkey = '{up_buyer_pk}'") == "100000", (s, b))
up_ctip = sql(f"select payment_hash from makers.tips where listing = {up_id} and paid_with = 'credit'")
check("upgrades: a credit purchase pays the maker too", tw_wait(lambda: tw_payout(up_ctip) == "sent|500|0") and len(tw["sends"]) == up_sends + 1, tw_payout(up_ctip))
time.sleep(1.1)
s, b = up_call("POST", f"/api/playtest/upgrades/{up_id}/buy", {"pay": "credit"}, player=up_bo["key"], keyfile=up_buyer_key)
check("upgrades: what was bought with credit isn't sold again", s == 409, (s, b))
up_call("POST", RF, {"x": 2, "z": 2, "after": 0}, player=up_bo["key"])
s, b = up_call("POST", RF, {"x": 1, "z": 1, "after": 0}, player=up_ann["key"])
check("upgrades: and the credit buyer is seen in it too", {o["id"]: o for o in b.get("others", [])}.get(up_bo["id"], {}).get("look") == UP_LOOK, b)
s, b = up_call("POST", f"/api/admin/upgrades/{up_id}/reject", {}, keyfile=os.environ["ADMIN_KEY"])
s, b = up_call("GET", "/api/playtest/upgrades/fairy", player=up_ann["key"])
up_call("POST", RF, {"x": 1, "z": 1, "after": 0}, player=up_ann["key"])
s2, b2 = up_call("POST", RF, {"x": 2, "z": 2, "after": 0}, player=up_bo["key"])
check("upgrades: a rejected listing leaves UPGRADE and the room", up_id not in [x["id"] for x in b["top"]] and up_ann["id"] in {o["id"] for o in b2.get("others", [])}
      and "look" not in {o["id"]: o for o in b2["others"]}[up_ann["id"]], (b, b2))
check("upgrades: a page of another site can't buy",
      call("POST", f"/api/playtest/upgrades/{up_id2}/buy", b"{}", {"Origin": "https://evil.example", "X-Verse-Player": up_bo["key"]})[0] == 403)
wallet["extra"] = {}
