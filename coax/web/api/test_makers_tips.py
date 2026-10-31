# Tips for a realm's maker (ev-maker-tips, makers.c, migrations/0073_maker_tips.sql) against the fake
# Spark wallet test_inference.py serves: a paid invoice credits the maker once and queues a payout;
# the payout pays the maker's address once; a failed payout retries and never pays twice.
# test.py runs this file after test_makers.py (the name sorts after it) (Orbit League's maker: tips@getalby.com).
import time

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
    h = {"Content-Type": "application/json", "Origin": API}
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


s, b = tw_post("/api/playtest/makers/belt-2200/tip", {"sats": 100})
check("tips: an unclaimed realm takes no tips", s == 404, (s, b))
s, b = tw_post("/api/playtest/makers/orbit-league/tip", {"sats": 5})
check("tips: a tip has a floor", s == 400 and "10" in b["error"], (s, b))
s, b = tw_post("/api/playtest/makers/orbit-league/tip", {"sats": "100"})
check("tips: sats is a number", s == 400, (s, b))
s, b = tw_post("/api/playtest/makers/orbit-league/tip", {"sats": 100})
tip1 = b.get("payment_hash", "")
check("tips: a tip is our wallet's invoice", s == 200 and b["invoice"].startswith("lnbc") and b["sats"] == 100 and len(tip1) == 64, (s, b))
check("tips: the invoice was asked of our wallet for the tip's sats", wallet["last"]["sats"] == 100 and "Topher" in wallet["last"]["memo"], wallet.get("last"))
inv1 = b.get("invoice", "")

s, b = tw_post(f"/api/playtest/makers/orbit-league/tip/{tip1}", {})
check("tips: unpaid, it is open", s == 200 and b["state"] == "open" and b["payout"] == "", (s, b))
wallet["paid"][inv1].update(state="completed", amount_msat=99000)
s, b = tw_post(f"/api/playtest/makers/orbit-league/tip/{tip1}", {})
check("tips: paid the wrong amount, it stays open", s == 200 and b["state"] == "open", (s, b))
wallet["paid"][inv1].pop("amount_msat"); wallet["paid"][inv1]["say_pre"] = "00" * 32
s, b = tw_post(f"/api/playtest/makers/orbit-league/tip/{tip1}", {})
check("tips: a preimage that doesn't hash to the invoice is no payment", s == 200 and b["state"] == "open", (s, b))
wallet["paid"][inv1].pop("say_pre")
s, b = tw_post(f"/api/playtest/makers/orbit-league/tip/{tip1}", {})
check("tips: paid, it is settled and credited to the maker", s == 200 and b["state"] == "paid" and b["to"] == "Topher", (s, b))
check("tips: the payment is kept once in the replay ledger, as the maker's",
      sql(f"select count(*) || '|' || min(pubkey) || '|' || min(msat) from inference.used_payments where replay_key like '%:{tip1}'") == f"1|{mk_maker_pk}|100000")
check("tips: the first tip carries the first-paid bonus", tw_payout(tip1).split("|")[1:] == ["1100", "1000"], tw_payout(tip1))
check("tips: the payout is sent to the maker's address", tw_wait(lambda: tw_payout(tip1).startswith("sent")), tw_payout(tip1))
check("tips: it was quoted for the address and the tip with the bonus, and sent once",
      tw["quotes"][-1]["address"] == "tips@getalby.com" and tw["quotes"][-1]["sats"] == 1100 and len(tw["sends"]) == 1, (tw["quotes"], tw["sends"]))
check("tips: the routing fee is kept with it", sql(f"select fee_sats || '|' || amount_sats from makers.payouts where tip = '{tip1}'") == "2|1098")
s, b = tw_post(f"/api/playtest/makers/orbit-league/tip/{tip1}", {})
check("tips: asked again, it stays paid and is settled once", s == 200 and b["state"] == "paid" and b["payout"] == "sent"
      and sql(f"select count(*) from makers.payouts where tip = '{tip1}'") == "1" and len(tw["sends"]) == 1, (s, b, tw["sends"]))

# a failed payout retries with a new invoice, and is paid once
tw["send_mode"] = ["failed", "completed"]
s, b = tw_post("/api/playtest/makers/orbit-league/tip", {"sats": 200})
tip2 = b["payment_hash"]; wallet["paid"][b["invoice"]]["state"] = "completed"
s, b = tw_post(f"/api/playtest/makers/orbit-league/tip/{tip2}", {})
check("tips: a second tip has no bonus", s == 200 and tw_payout(tip2).split("|")[1:] == ["200", "0"], tw_payout(tip2))
check("tips: a failed payout waits to try again", tw_wait(lambda: tw_payout(tip2).startswith("failed")) and
      sql(f"select (next_try > now())::text || '|' || (invoice is null)::text from makers.payouts where tip = '{tip2}'") == "true|true", tw_payout(tip2))
sql(f"update makers.payouts set next_try = now() where tip = '{tip2}'")
s, b = tw_post("/api/admin/makers/payouts/run", {}, keyfile=os.environ["ADMIN_KEY"])
check("tips: an admin runs the due payouts", s == 200, (s, b))
check("tips: tried again, the payout is sent", tw_wait(lambda: tw_payout(tip2).startswith("sent")), tw_payout(tip2))
check("tips: one payment completed for it", sum(1 for i, v in tw["paid"].items() if v == "completed" and "maker" in i and i in tw["sends"][-2:]) == 1, tw["paid"])

# a payout whose answer was lost is asked about, never sent again
tw["send_mode"] = ["lost"]
s, b = tw_post("/api/playtest/makers/orbit-league/tip", {"sats": 300})
tip3 = b["payment_hash"]; wallet["paid"][b["invoice"]]["state"] = "completed"
sends = len(tw["sends"])
s, b = tw_post(f"/api/playtest/makers/orbit-league/tip/{tip3}", {})
check("tips: a payout with no answer keeps its invoice", tw_wait(lambda: tw_payout(tip3).startswith("failed")) and
      sql(f"select (invoice is not null)::text from makers.payouts where tip = '{tip3}'") == "true", tw_payout(tip3))
sql(f"update makers.payouts set next_try = now() where tip = '{tip3}'")
time.sleep(1.1)   # a NIP-98 event is good once: the same request in the same second is the same event
s, b = tw_post("/api/admin/makers/payouts/run", {}, keyfile=os.environ["ADMIN_KEY"])
check("tips: the admin's second run is let in", s == 200, (s, b))
check("tips: asked again, our wallet's record says it was paid: sent, without paying twice",
      tw_wait(lambda: tw_payout(tip3).startswith("sent")) and len(tw["sends"]) == sends + 1, (tw_payout(tip3), tw["sends"][sends:]))

# the owner's gift from our wallet
tw["send_mode"] = ["completed"]
s, b = tw_post("/api/admin/makers/orbit-league/gift", {"sats": 50}, keyfile=OTHER_KEY)
check("tips: only an admin gives a gift", s == 403, (s, b))
s, b = tw_post("/api/admin/makers/orbit-league/gift", {"sats": 50}, keyfile=os.environ["ADMIN_KEY"])
gift = b.get("tip", "")
check("tips: an admin's gift is a paid tip from our wallet, flagged team", s == 200 and gift.startswith("gift:") and
      sql(f"select team::text || '|' || (invoice is null)::text from makers.tips where payment_hash = '{gift}'") == "true|true", (s, b))
check("tips: the gift is paid out to the maker, with no bonus", tw_wait(lambda: tw_payout(gift).startswith("sent")) and tw_payout(gift).endswith("|50|0"), tw_payout(gift))
s, b = tw_post("/api/admin/makers/orbit-league/gift", {"sats": 999999}, keyfile=os.environ["ADMIN_KEY"])
check("tips: a gift has a ceiling", s == 400, (s, b))
s, b, _ = call("GET", "/api/admin/makers/payouts", None, {"Authorization": mk_signed(os.environ["ADMIN_KEY"], "GET", "/api/admin/makers/payouts")})
check("tips: an admin reads the payouts", s == 200 and b.decode().count("orbit-league|sent") == 4, b)
s, b = tw_post("/api/playtest/makers/orbit-league/tip/" + "ab" * 32, {})
check("tips: an unknown tip is a 404", s == 404, (s, b))
s, b, _ = call("POST", "/api/playtest/makers/orbit-league/tip", b'{"sats": 100}', {"Origin": "https://evil.example", "Content-Type": "application/json"})
check("tips: another site's page can't ask for one", s == 403, (s, b))
wallet["extra"] = {}
