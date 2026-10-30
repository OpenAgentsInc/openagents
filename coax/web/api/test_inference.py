# /api/v1/inference (gw-ours): a signed Lab request reaches a fake Vertex that
# test.sh points the service at; the answer comes back as Vertex gave it and
# the call is metered by account. test.py runs this file.
import http.server
import threading

infer_seen = []
infer_mode = {"status": 200}


class FakeVertex(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        infer_seen.append({"auth": self.headers.get("Authorization"), "body": body})
        if infer_mode["status"] != 200:
            out, kind = json.dumps({"error": {"code": infer_mode["status"], "message": "Resource exhausted"}}).encode(), "application/json"
        elif body.get("stream"):
            chunks = [{"choices": [{"index": 0, "delta": {"role": "assistant", "content": "Par"}}]},
                      {"choices": [{"index": 0, "delta": {"content": "is."}, "finish_reason": "stop"}]},
                      {"choices": [], "usage": {"prompt_tokens": 2000, "completion_tokens": 400, "total_tokens": 2400}}]
            out, kind = ("".join("data: " + json.dumps(c) + "\n\n" for c in chunks) + "data: [DONE]\n\n").encode(), "text/event-stream"
        else:
            out, kind = json.dumps({"choices": [{"index": 0, "message": {"role": "assistant", "content": "Paris."}, "finish_reason": "stop"}],
                                    "usage": {"prompt_tokens": 1000, "completion_tokens": 200, "total_tokens": 1200}}).encode(), "application/json"
        self.send_response(infer_mode["status"])
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(out)))
        self.end_headers()
        self.wfile.write(out)

    def log_message(self, *a):
        pass


infer_fake = http.server.ThreadingHTTPServer(("127.0.0.1", int(os.environ["INFER_PORT"])), FakeVertex)
threading.Thread(target=infer_fake.serve_forever, daemon=True).start()


def infer_sign(keyfile, url=None):
    return subprocess.run([os.environ["SIGN"], "--key", keyfile, "POST", url or API + "/api/v1/inference"], capture_output=True, text=True, check=True).stdout.strip()


def infer(body, keyfile=os.environ["INFER_KEY"], auth=None):
    headers = {"Content-Type": "application/json"}
    if keyfile or auth:
        headers["Authorization"] = auth or "Nostr " + infer_sign(keyfile)
    return call("POST", "/api/v1/inference", json.dumps(body).encode(), headers)


def infer_pubkey(keyfile):
    return subprocess.run([os.environ["SIGN"], "--key", keyfile, "--pubkey"], capture_output=True, text=True, check=True).stdout.strip()


infer_ask = {"model": "google/gemini-3.8-flash", "messages": [{"role": "user", "content": "What is the capital of France?"}]}
infer_user = infer_pubkey(os.environ["INFER_KEY"])

check("inference: unsigned is a 401", infer(infer_ask, keyfile=None)[0] == 401)
check("inference: a GET is a 405", call("GET", "/api/v1/inference")[0] == 405)
check("inference: a signature for another URL is a 401", infer(infer_ask, auth="Nostr " + infer_sign(os.environ["INFER_KEY"], API + "/v1/other"))[0] == 401)
check("inference: a key not on the list, with no credit, is a 402", infer(infer_ask, keyfile=os.environ["OTHER_KEY"])[0] == 402)
s, b, _ = infer({"model": "gpt-6", "messages": infer_ask["messages"]})
check("inference: a model the route doesn't serve is a 400", s == 400 and b"does not serve" in b, b)
check("inference: no messages is a 400", infer({"model": infer_ask["model"]})[0] == 400)
check("inference: bad requests never reach Vertex", infer_seen == [])

s, b, h = infer(dict(infer_ask, max_tokens=100000, store=True, user="someone"))
check("inference: a signed request gets Vertex's answer", s == 200 and json.loads(b)["choices"][0]["message"]["content"] == "Paris.", b)
infer_sent = infer_seen[-1]
check("inference: the service's token goes to Vertex, not the caller's signature", infer_sent["auth"] == "Bearer test-token", infer_sent["auth"])
check("inference: Vertex gets the messages, its model name, the output capped at 8192, nothing stored",
      infer_sent["body"]["messages"] == infer_ask["messages"] and infer_sent["body"]["model"] == "google/gemini-3.8-flash" and infer_sent["body"]["max_tokens"] == 8192
      and "store" not in infer_sent["body"] and "user" not in infer_sent["body"], infer_sent["body"])
check("inference: the call is kept with its tokens and its cost at the card's rates",
      sql(f"select status||'|'||in_tokens||'|'||out_tokens||'|'||usd_micros from inference.calls where pubkey = '{infer_user}' order by id desc limit 1") == "200|1000|200|1500")

s, b, h = infer(dict(infer_ask, stream=True))
infer_text = b.decode()
check("inference: a stream comes back as server-sent events", s == 200 and h.get("Content-Type") == "text/event-stream" and infer_text.endswith("data: [DONE]\n\n")
      and '"content": "Par"' in infer_text, (h, infer_text))
check("inference: a stream asks Vertex for its usage", infer_seen[-1]["body"].get("stream_options") == {"include_usage": True}, infer_seen[-1]["body"])
check("inference: a stream's usage, from its last chunk, is metered",
      sql(f"select in_tokens||'|'||out_tokens||'|'||usd_micros from inference.calls where pubkey = '{infer_user}' order by id desc limit 1") == "2000|400|3000")

infer_mode["status"] = 429
s, b, _ = infer(infer_ask)
infer_mode["status"] = 200
check("inference: Vertex's refusal comes back as its status, so the Lab falls through to its next gateway", s == 429 and b"Resource exhausted" in b, (s, b))
check("inference: a refused call costs nothing", sql(f"select status||'|'||usd_micros from inference.calls where pubkey = '{infer_user}' order by id desc limit 1") == "429|0")

s, _, _ = infer(infer_ask, keyfile=os.environ["ADMIN_KEY"])
check("inference: an admin's key may use the route", s == 200)

infer_codes = [infer(infer_ask)[0] for _ in range(3)]
check("inference: an account's day is five requests here; the sixth is a 429 and never reaches Vertex", infer_codes == [200, 200, 429], infer_codes)
check("inference: the day's count and spend are the account's",
      sql(f"select n||'|'||usd_micros from inference.account_day where pubkey = '{infer_user}'") == "6|7500")
infer_admin = infer_pubkey(os.environ["ADMIN_KEY"])
sql(f"update inference.account_day set usd_micros = 2000000 where pubkey = '{infer_admin}'")
s, b, _ = infer(infer_ask, keyfile=os.environ["ADMIN_KEY"])
check("inference: an account whose day's spend is used up gets a 429", s == 429 and b"spend" in b, b)

# /api/v1/inference/credit (NIP-X402 openagents:spark-receiver:1): credit bought over Lightning
# from a fake Spark wallet on SPARK_PORT, then spent on the route by a key that isn't listed.
import base64
import hashlib
import time

wallet = {"paid": {}, "rate": 100000, "asked": []}   # invoice -> what /paid says; USD per BTC (0: the rate fails)


class FakeWallet(http.server.BaseHTTPRequestHandler):
    def reply(self, status, obj):
        out = json.dumps(obj).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(out)))
        self.end_headers()
        self.wfile.write(out)

    def do_GET(self):
        wallet["asked"].append((self.path, self.headers.get("Authorization")))
        if self.path == "/rate" and wallet["rate"]:
            return self.reply(200, {"usd_per_btc": wallet["rate"]})
        self.reply(502, {"error": "the wallet failed"})

    def do_POST(self):
        q = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        wallet["asked"].append((self.path, self.headers.get("Authorization")))
        if self.path == "/invoice":
            pre = hashlib.sha256(q["memo"].encode() + str(time.time()).encode()).hexdigest()
            h = hashlib.sha256(bytes.fromhex(pre)).hexdigest()
            inv = f"lnbc{q['sats']}n1p{h}"
            wallet["paid"][inv] = {"state": "none", "pre": pre, "hash": h, "sats": q["sats"]}
            wallet["last"] = dict(q, invoice=inv)
            return self.reply(200, {"invoice": inv, "payment_hash": h, "amount_msat": q["sats"] * 1000, "expiry": int(time.time()) + q["expiry"], "pay_to": "02" + "ab" * 32})
        if self.path == "/paid":
            w = wallet["paid"].get(q["invoice"])
            if not w or w["state"] == "none":
                return self.reply(200, {"state": "none"})
            return self.reply(200, {"state": w["state"], "amount_msat": w.get("amount_msat", w["sats"] * 1000), "payment_hash": w["hash"],
                                    "preimage": w.get("say_pre", w["pre"]) if w["state"] == "completed" else ""})
        self.reply(404, {})

    def log_message(self, *a):
        pass


wallet_fake = http.server.ThreadingHTTPServer(("127.0.0.1", int(os.environ["SPARK_PORT"])), FakeWallet)
threading.Thread(target=wallet_fake.serve_forever, daemon=True).start()
other_key = os.environ["OTHER_KEY"]
other = infer_pubkey(other_key)
credit_url = API + "/api/v1/inference/credit"


def topup(body, sig=None):
    headers = {"Content-Type": "application/json"}
    if sig is not None:
        headers["PAYMENT-SIGNATURE"] = sig
    return call("POST", "/api/v1/inference/credit", json.dumps(body).encode() if not isinstance(body, bytes) else body, headers)


def b64json(obj):
    return base64.b64encode(json.dumps(obj).encode()).decode()


def header(h, name):
    return next((v for k, v in h.items() if k.lower() == name.lower()), None)


check("credit: an unsigned GET is a 401", call("GET", "/api/v1/inference/credit")[0] == 401)
check("credit: a PUT is a 405", call("PUT", "/api/v1/inference/credit", b"{}")[0] == 405)
check("credit: not JSON is a 400", topup(b"{nope")[0] == 400)
check("credit: a pubkey that isn't 64 lowercase hex digits is a 400", topup({"pubkey": other.upper(), "msat": "50000"})[0] == 400)
s, b, _ = topup({"pubkey": other, "msat": "9000"})
check("credit: below the smallest top-up (10 sats) is a 400", s == 400 and b"10 to 100000 sats" in b, b)
check("credit: above the largest is a 400", topup({"pubkey": other, "msat": "100001000"})[0] == 400)
check("credit: msat that isn't whole sats is a 400", topup({"pubkey": other, "msat": "50001"})[0] == 400)
check("credit: refused top-ups never reach the wallet", not any(p == "/invoice" for p, _ in wallet["asked"]))

s, b, h = topup({"pubkey": other, "msat": "50000"})
req = json.loads(base64.b64decode(header(h, "PAYMENT-REQUIRED") or "e30="))
acc = (req.get("accepts") or [{}])[0]
ex = acc.get("extra", {})
digest = hashlib.sha256(f"openagents:spark-receiver:1|POST|{credit_url}|{other}|50000".encode()).hexdigest()
check("credit: a top-up is a 402 with PAYMENT-REQUIRED, x402 v2 exact on Lightning mainnet in msat",
      s == 402 and req.get("x402Version") == 2 and acc.get("scheme") == "exact" and acc.get("network") == "lnbtc:000000000019d6689c085ae165831e93"
      and acc.get("amount") == "50000" and acc.get("asset") == "BTC" and ex.get("receiverProfile") == "openagents:spark-receiver:1"
      and ex.get("method") == "bolt11" and ex.get("paymentFlow") == "upfront", (s, req))
check("credit: the body repeats the challenge", json.loads(b) == req, b)
check("credit: the invoice is our wallet's, asked for the sats with the request digest in its memo",
      ex.get("invoice") == wallet["last"]["invoice"] and wallet["last"]["sats"] == 50 and digest in wallet["last"]["memo"] and ex.get("requestDigest") == digest, (ex, wallet["last"]))
check("credit: the wallet is asked with the service's token", wallet["asked"][-1] == ("/invoice", "Bearer test-spark"), wallet["asked"][-1])
check("credit: the binding is kept in our database",
      sql(f"select pubkey||'|'||msat||'|'||digest from inference.topups where payment_hash = '{ex.get('paymentHash')}'") == f"{other}|50000|{digest}")

inv = ex["invoice"]
w = wallet["paid"][inv]
proof = {"x402Version": 2, "accepted": acc, "payload": {"paymentHash": ex["paymentHash"]}}
s, b, h = topup({"pubkey": other, "msat": "50000"}, b64json(proof))
check("credit: before the wallet sees the payment, the retry is a 402 payment_pending with the challenge again",
      s == 402 and b"payment_pending" in b and json.loads(base64.b64decode(header(h, "PAYMENT-REQUIRED")))["accepts"][0]["extra"]["invoice"] == inv, (s, b))
w["state"] = "pending"
check("credit: a pending receive is still payment_pending", topup({"pubkey": other, "msat": "50000"}, b64json(proof))[1].find(b"payment_pending") >= 0)
w["state"] = "completed"
check("credit: a proof for another body (another key, same invoice) refuses", topup({"pubkey": infer_user, "msat": "50000"}, b64json(proof))[0] == 402)
check("credit: a proof for another amount refuses", topup({"pubkey": other, "msat": "60000"}, b64json(proof))[0] == 402)
check("credit: accepted terms changed (the amount) refuse", topup({"pubkey": other, "msat": "50000"}, b64json(dict(proof, accepted=dict(acc, amount="1000"))))[0] == 402)
check("credit: an invoice we didn't issue refuses", topup({"pubkey": other, "msat": "50000"}, b64json(dict(proof, payload={"paymentHash": "00" * 32})))[0] == 402)
check("credit: a payload that isn't base64 JSON is a 400", topup({"pubkey": other, "msat": "50000"}, "!!!")[0] == 400)
check("credit: x402 version 1 is a 400", topup({"pubkey": other, "msat": "50000"}, b64json(dict(proof, x402Version=1)))[0] == 400)
w["say_pre"] = "11" * 32
s, b, _ = topup({"pubkey": other, "msat": "50000"}, b64json(proof))
check("credit: a wallet preimage that doesn't hash to the payment hash refuses", s == 402 and b"preimage" in b, b)
del w["say_pre"]
w["amount_msat"] = 49000
check("credit: a receive for less than the invoice refuses", topup({"pubkey": other, "msat": "50000"}, b64json(proof))[0] == 402)
del w["amount_msat"]
s, b, _ = topup({"pubkey": other, "msat": "50000"}, b64json(dict(proof, payload={"paymentHash": ex["paymentHash"], "preimage": "22" * 32})))
check("credit: a payer's preimage that isn't the invoice's refuses", s == 402 and b"payer" in b, b)
check("credit: nothing refused added credit", sql(f"select count(*) from inference.credit where pubkey = '{other}'") == "0")

s, b, h = topup({"pubkey": other, "msat": "50000"}, b64json(dict(proof, payload={"paymentHash": ex["paymentHash"], "preimage": w["pre"]})))
resp = json.loads(base64.b64decode(header(h, "PAYMENT-RESPONSE") or "e30="))
check("credit: our wallet's record of the payment credits the key: 200 with PAYMENT-RESPONSE naming the payment hash",
      s == 200 and json.loads(b)["credit_msat"] == 50000 and resp.get("success") is True and resp.get("transaction") == ex["paymentHash"]
      and resp.get("network") == "lnbtc:000000000019d6689c085ae165831e93" and w["pre"] not in json.dumps(resp), (s, b, resp))
s, b, _ = topup({"pubkey": other, "msat": "50000"}, b64json(proof))
check("credit: the same payment again is duplicate_settlement and adds nothing",
      s == 409 and b"duplicate_settlement" in b and sql(f"select msat from inference.credit where pubkey = '{other}'") == "50000", (s, b))
check("credit: the replay key is kept once",
      sql(f"select count(*) from inference.used_payments where replay_key = 'lnbtc:000000000019d6689c085ae165831e93:{ex['paymentHash']}'") == "1")
check("credit: no preimage in the log", w["pre"] not in open(os.environ["API_LOG"]).read())

def balance(keyfile):
    sig = subprocess.run([os.environ["SIGN"], "--key", keyfile, "GET", credit_url], capture_output=True, text=True, check=True).stdout.strip()
    return call("GET", "/api/v1/inference/credit", None, {"Authorization": "Nostr " + sig})


s, b, _ = balance(other_key)
check("credit: a signed GET reads the signer's credit", s == 200 and json.loads(b) == {"credit_msat": 50000}, (s, b))
check("credit: a key with none reads 0", json.loads(balance(os.environ["INFER_KEY"])[1]) == {"credit_msat": 0})

# A second top-up paid from a QR wallet (no preimage from the payer): our wallet's record is the proof.
s, b, h = topup({"pubkey": other, "msat": "20000"})
acc2 = json.loads(base64.b64decode(header(h, "PAYMENT-REQUIRED")))["accepts"][0]
wallet["paid"][acc2["extra"]["invoice"]]["state"] = "completed"
s, b, _ = topup({"pubkey": other, "msat": "20000"}, b64json({"x402Version": 2, "accepted": acc2, "payload": {"paymentHash": acc2["extra"]["paymentHash"]}}))
check("credit: a payer with no preimage is credited from our wallet's record", s == 200 and json.loads(b)["credit_msat"] == 70000, (s, b))

# Draw-down: the key isn't an admin's or on the list; it pays from its credit.
sql(f"delete from inference.credit where pubkey = '{other}'")
s, b, _ = infer(infer_ask, keyfile=other_key)
check("drawdown: a key with no credit is a 402 that points at the top-up route",
      s == 402 and json.loads(b).get("credit") == credit_url and json.loads(b).get("credit_msat") == 0, (s, b))
sql(f"insert into inference.credit (pubkey, msat) values ('{other}', 70000)")
wallet["rate"] = 0
sql("delete from inference.btc_usd")
infer_before = len(infer_seen)
s, b, _ = infer(infer_ask, keyfile=other_key)
check("drawdown: with no bitcoin price, paid use is refused (503) and never reaches Vertex", s == 503 and len(infer_seen) == infer_before, (s, b))
wallet["rate"] = 100000
s, b, _ = infer(infer_ask, keyfile=other_key)
time.sleep(0.5)
check("drawdown: with credit and a price, the key's request is answered", s == 200 and json.loads(b)["choices"][0]["message"]["content"] == "Paris.", b)
check("drawdown: the price was asked of our wallet and kept", sql("select usd_per_btc from inference.btc_usd") == "100000")
check("drawdown: the call's cost ($0.0015) is taken from its credit in msat at the kept price (1500 msat at $100,000)",
      sql(f"select msat from inference.credit where pubkey = '{other}'") == "68500"
      and sql(f"select usd_micros||'|'||msat from inference.calls where pubkey = '{other}' order by id desc limit 1") == "1500|1500")
sql(f"delete from inference.account_day where pubkey = '{infer_user}'")
s, _, _ = infer(infer_ask)
time.sleep(0.5)
check("drawdown: a listed key is never debited", s == 200 and sql(f"select msat from inference.calls where pubkey = '{infer_user}' order by id desc limit 1") == "0")
sql(f"update inference.credit set msat = 1 where pubkey = '{other}'")
s, _, _ = infer(infer_ask, keyfile=other_key)
time.sleep(0.5)
check("drawdown: the last call may take credit below zero; then the key gets a 402",
      s == 200 and int(sql(f"select msat from inference.credit where pubkey = '{other}'")) < 0 and infer(infer_ask, keyfile=other_key)[0] == 402)
