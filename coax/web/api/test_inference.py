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
            usage = {"prompt_tokens": 1000, "completion_tokens": 200, "total_tokens": 1200}
            if infer_mode.get("cached_tokens"):
                usage["prompt_tokens_details"] = {"cached_tokens": infer_mode["cached_tokens"]}
            out, kind = json.dumps({"choices": [{"index": 0, "message": {"role": "assistant", "content": "Paris."}, "finish_reason": "stop"}],
                                    "usage": usage}).encode(), "application/json"
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


infer_ask = {"model": "google/gemini-4-hydrogen", "messages": [{"role": "user", "content": "What is the capital of France?"}]}
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
check("inference: Vertex gets the messages, its model name, the output capped at 65536, nothing stored",
      infer_sent["body"]["messages"] == infer_ask["messages"] and infer_sent["body"]["model"] == "google/gemini-4-hydrogen" and infer_sent["body"]["max_tokens"] == 65536
      and "store" not in infer_sent["body"] and "user" not in infer_sent["body"], infer_sent["body"])
check("inference: the call is kept with its tokens and OpenAgents retail (+25%) and upstream costs",
      sql(f"select status||'|'||in_tokens||'|'||cached_in_tokens||'|'||out_tokens||'|'||usd_micros||'|'||upstream_usd_micros from inference.calls where pubkey = '{infer_user}' order by id desc limit 1") == "200|1000|0|200|3750|3000")
check("inference: the old Gemini 3.8 Flash name is still served, asked of Gemini 4 Hydrogen at OpenAgents retail price (#13479, #14069)",
      sql("select (inference.request('alias-check', '{\"model\":\"google/gemini-3.8-flash\",\"messages\":[{\"role\":\"user\",\"content\":\"hi\"}]}', 5, 1000000))->'request'->>'model'") == "google/gemini-4-hydrogen"
      and sql("select upstream||'|'||input_micros||'|'||output_micros||'|'||upstream_input_micros||'|'||upstream_output_micros from inference.models where name = 'google/gemini-3.8-flash'") == "google/gemini-4-hydrogen|1875000|9375000|1500000|7500000")

s, b, h = infer(dict(infer_ask, stream=True))
infer_text = b.decode()
check("inference: a stream comes back as server-sent events", s == 200 and h.get("Content-Type") == "text/event-stream" and infer_text.endswith("data: [DONE]\n\n")
      and '"content": "Par"' in infer_text, (h, infer_text))
check("inference: a stream asks Vertex for its usage", infer_seen[-1]["body"].get("stream_options") == {"include_usage": True}, infer_seen[-1]["body"])
check("inference: a stream's usage, from its last chunk, is metered",
      sql(f"select in_tokens||'|'||out_tokens||'|'||usd_micros||'|'||upstream_usd_micros from inference.calls where pubkey = '{infer_user}' order by id desc limit 1") == "2000|400|7500|6000")

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
      sql(f"select n||'|'||usd_micros from inference.account_day where pubkey = '{infer_user}'") == "6|18750")
infer_words = [sql("select coalesce(x->>'error', x->>'status') from inference.request('limit-words-check', '{\"model\":\"google/gemini-4-hydrogen\",\"messages\":[{\"role\":\"user\",\"content\":\"hi\"}]}', 2, 1000000) x") for _ in range(3)]
check("inference: the day's request limit is said with its number", infer_words == ["200", "200", "this account's 2 requests for today are used up"], infer_words)
infer_admin = infer_pubkey(os.environ["ADMIN_KEY"])
sql(f"update inference.account_day set usd_micros = 2000000 where pubkey = '{infer_admin}'")
s, b, _ = infer(infer_ask, keyfile=os.environ["ADMIN_KEY"])
check("inference: an account whose day's spend is used up gets a 429 that says the limit", s == 429 and json.loads(b) == {"error": "this account's $2.00 spend for today is used up"}, b)

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
        if "get" in wallet:   # other files' GET routes (test_makers_tips.py: the fake Lightning Address services)
            return wallet["get"](self)
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
        if self.path in wallet.get("extra", {}):   # other files' routes (test_maker_tips.py: /quote, /send, /sent)
            return self.reply(*wallet["extra"][self.path](q))
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
check("drawdown: the call's cost ($0.00375) is taken from its credit in msat at the kept price (3750 msat at $100,000)",
      sql(f"select msat from inference.credit where pubkey = '{other}'") == "66250"
      and sql(f"select usd_micros||'|'||upstream_usd_micros||'|'||msat from inference.calls where pubkey = '{other}' order by id desc limit 1") == "3750|3000|3750")
sql(f"delete from inference.account_day where pubkey = '{infer_user}'")
s, _, _ = infer(infer_ask)
time.sleep(0.5)
check("drawdown: a listed key is never debited", s == 200 and sql(f"select msat from inference.calls where pubkey = '{infer_user}' order by id desc limit 1") == "0")
sql(f"update inference.credit set msat = 1 where pubkey = '{other}'")
s, _, _ = infer(infer_ask, keyfile=other_key)
time.sleep(0.5)
check("drawdown: the last call may take credit below zero; then the key gets a 402",
      s == 200 and int(sql(f"select msat from inference.credit where pubkey = '{other}'")) < 0 and infer(infer_ask, keyfile=other_key)[0] == 402)

# Vertex's two endpoints (inference.c upstream_ask, inference-upstream-policy.coax): us first, global when us
# is slow or failing, global first for a while after, and the call counted once. A second API on this
# database (INFER_PORT + 1) asks two fake endpoints on INFER_PORT + 2 (/us/..., /global/...): a race after
# 1 second, global first for 2 seconds after us loses. test.sh's own API keeps its one endpoint.
import atexit

up_mode = {"us": {}, "global": {}}   # status (0: the connection closes with no answer), delay in seconds before the first byte
up_seen = []


class FakeEndpoints(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        which = "global" if self.path.startswith("/global/") else "us"
        raw = self.rfile.read(int(self.headers["Content-Length"]))
        mode = dict(up_mode[which])
        up_seen.append({"which": which, "auth": self.headers.get("Authorization"), "raw": raw})
        time.sleep(mode.get("delay", 0))
        status = mode.get("status", 200)
        tokens = {"prompt_tokens": 1000, "completion_tokens": 200} if which == "us" else {"prompt_tokens": 2000, "completion_tokens": 400}
        if status == 0:
            self.close_connection = True
            return
        if status != 200:
            out, kind = json.dumps({"error": {"code": status, "message": which + " is down"}}).encode(), "application/json"
        elif json.loads(raw).get("stream"):
            chunks = [{"choices": [{"index": 0, "delta": {"role": "assistant", "content": "from "}}]},
                      {"choices": [{"index": 0, "delta": {"content": which}, "finish_reason": "stop"}]},
                      {"choices": [], "usage": tokens}]
            out, kind = ("".join("data: " + json.dumps(c) + "\n\n" for c in chunks) + "data: [DONE]\n\n").encode(), "text/event-stream"
        else:
            out, kind = json.dumps({"choices": [{"index": 0, "message": {"role": "assistant", "content": "from " + which}, "finish_reason": "stop"}], "usage": tokens}).encode(), "application/json"
        try:
            self.send_response(status)
            self.send_header("Content-Type", kind)
            self.send_header("Content-Length", str(len(out)))
            self.end_headers()
            self.wfile.write(out[:20])
            self.wfile.flush()
            time.sleep(mode.get("middle", 0))
            self.wfile.write(out[20:])
        except OSError:
            pass   # the API dropped this endpoint: the other one answered

    def log_message(self, *a):
        pass


up_port = int(os.environ["INFER_PORT"])
up_fake = http.server.ThreadingHTTPServer(("127.0.0.1", up_port + 2), FakeEndpoints)
threading.Thread(target=up_fake.serve_forever, daemon=True).start()
UP_API = f"http://127.0.0.1:{up_port + 1}"
up_key = os.path.join(os.path.dirname(os.environ["INFER_KEY"]), "infer-upstream.key")
up_user = subprocess.run([os.environ["SIGN"], "--new", up_key], capture_output=True, text=True, check=True).stdout.strip()
up_api = subprocess.Popen([os.environ["API_BIN"], "--listen", f"127.0.0.1:{up_port + 1}"], stderr=subprocess.PIPE, text=True,
                          env=dict(os.environ, OA_API_DATABASE=os.environ["PGURL"], OA_API_ORIGIN=UP_API, OA_API_WORKERS="2", OA_INFERENCE_PUBKEYS=up_user,
                                   OA_INFERENCE_URL=f"http://127.0.0.1:{up_port + 2}/us/chat/completions", OA_INFERENCE_FALLBACK_URL=f"http://127.0.0.1:{up_port + 2}/global/chat/completions",
                                   OA_INFERENCE_TOKEN="test-token", OA_INFERENCE_HEDGE_MS="1000", OA_INFERENCE_BREAKER_MS="2000", OA_INFERENCE_DAILY_REQUESTS="500"))
atexit.register(up_api.kill)
up_log = []
threading.Thread(target=lambda: [up_log.append(l) for l in up_api.stderr], daemon=True).start()


def up_call(method, path, body=None, headers=None):
    req = urllib.request.Request(UP_API + path, method=method, data=body, headers=headers or {})
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return r.status, r.read(), dict(r.headers)
    except urllib.error.HTTPError as e:
        return e.code, e.read(), dict(e.headers)
    except OSError as e:
        return 0, str(e).encode(), {}


for _ in range(300):
    if up_call("GET", "/api/playtest/health")[0] == 200:
        break
    time.sleep(0.1)


def up_ask(us=None, glob=None, stream=False, settle=0.4):
    """One signed ask of the second API with the endpoints set so: (status, body, headers, seconds, the endpoints asked in order)."""
    up_mode["us"], up_mode["global"] = us or {}, glob or {}
    before = len(up_seen)
    t0 = time.monotonic()
    s, b, h = up_call("POST", "/api/v1/inference", json.dumps(dict(infer_ask, stream=True) if stream else infer_ask).encode(),
                      {"Content-Type": "application/json", "Authorization": "Nostr " + infer_sign(up_key, UP_API + "/api/v1/inference")})
    took = time.monotonic() - t0
    time.sleep(settle)   # the child keeps the call after it answers
    return s, b, h, took, [x["which"] for x in up_seen[before:]]


def up_kept():
    """The account's calls kept, its last one, and its day: 'calls|status|in|out|usd_micros|day's requests'."""
    return sql(f"select (select count(*) from inference.calls where pubkey = '{up_user}')||'|'||status||'|'||in_tokens||'|'||out_tokens||'|'||usd_micros||'|'||"
               f"(select n from inference.account_day where pubkey = '{up_user}') from inference.calls where pubkey = '{up_user}' order by id desc limit 1")


def up_said(b):
    return json.loads(b)["choices"][0]["message"]["content"]


def up_close():
    """Past the while global is first, then one fast answer from us, which ends its run of losses."""
    time.sleep(2.2)
    s, b, h, _, asked = up_ask()
    return s == 200 and up_said(b) == "from us" and h.get("X-OA-Upstream") == "us" and asked == ["us"], (s, b, asked)


s, b, h, took, asked = up_ask()
check("upstream: us fast: its answer, and global is never asked", s == 200 and up_said(b) == "from us" and h.get("X-OA-Upstream") == "us" and asked == ["us"], (s, b, h, asked))
check("upstream: us fast: the call is kept once with us's usage", up_kept() == "1|200|1000|200|3750|1", up_kept())

s, b, h, took, asked = up_ask(us={"delay": 5})
check("upstream: us slow: global is asked as well after the bound, and its answer is the answer",
      s == 200 and up_said(b) == "from global" and h.get("X-OA-Upstream") == "global" and asked == ["us", "global"] and 0.9 < took < 4, (s, b, h, took, asked))
check("upstream: us slow: both endpoints got the same body and the service's token",
      up_seen[-1]["raw"] == up_seen[-2]["raw"] and json.loads(up_seen[-1]["raw"])["max_tokens"] == 8192 and {x["auth"] for x in up_seen[-2:]} == {"Bearer test-token"}, up_seen[-2:])
check("upstream: us slow: one call kept and one request of the day, with the usage of the answer the caller got", up_kept() == "2|200|2000|400|7500|2", up_kept())

s, b, h, took, asked = up_ask(us={"delay": 5})
check("upstream: the breaker is open: global is asked first and us is not asked, with no wait",
      s == 200 and up_said(b) == "from global" and h.get("X-OA-Upstream") == "global" and asked == ["global"] and took < 3, (s, b, took, asked))
ok, why = up_close()
check("upstream: the breaker closes: after its while, us is asked first again and answers", ok, why)

s, b, h, took, asked = up_ask(us={"status": 503})
check("upstream: us failing (503): global is asked once and the caller sees its answer, not an error",
      s == 200 and up_said(b) == "from global" and asked == ["us", "global"] and took < 3, (s, b, took, asked))
up_count = up_kept()
s, b, h, took, asked = up_ask(us={}, glob={"status": 503})
check("upstream: global first and failing: us is asked once", s == 200 and up_said(b) == "from us" and h.get("X-OA-Upstream") == "us" and asked == ["global", "us"], (s, b, asked))
check("upstream: a failed endpoint is never a call of its own", up_count == "5|200|2000|400|7500|5" and up_kept() == "6|200|1000|200|3750|6", (up_count, up_kept()))
ok, why = up_close()
check("upstream: the breaker closes after a failure too", ok, why)

s, b, h, took, asked = up_ask(us={"status": 429})
check("upstream: us failing (429): global answers", s == 200 and up_said(b) == "from global" and asked == ["us", "global"], (s, b, asked))
ok, why = up_close()
s, b, h, took, asked = up_ask(us={"status": 0})
check("upstream: us not answering at all: global answers", ok and s == 200 and up_said(b) == "from global" and asked == ["us", "global"], (why, s, b, asked))
ok, why = up_close()

up_count = int(up_kept().split("|")[0])
s, b, h, took, asked = up_ask(us={"status": 503}, glob={"status": 429})
check("upstream: both failing: the caller's error is the first endpoint's status and body, as one endpoint's always was",
      ok and s == 503 and json.loads(b) == {"error": {"code": 503, "message": "us is down"}} and h.get("Content-Type") == "application/json" and asked == ["us", "global"], (why, s, b, h, asked))
check("upstream: both failing: one call kept, at no cost", up_kept().startswith(f"{up_count + 1}|503|0|0|0|"), up_kept())
s, b, h, took, asked = up_ask(us={"status": 0}, glob={"status": 0})
check("upstream: neither reached: the same 502 in the same words", s == 502 and json.loads(b) == {"error": "the model was not reached"} and sorted(asked) == ["global", "us"], (s, b, asked))
s, b, h, took, asked = up_ask(us={"status": 400}, glob={"status": 400})
check("upstream: a 400 is the answer from whichever endpoint is first: the other would say the same", s == 400 and len(asked) == 1, (s, b, asked))
ok, why = up_close()

s, b, h, took, asked = up_ask(us={"delay": 5}, stream=True)
up_text = b.decode()
check("upstream: a stream whose first byte is late: the whole stream is global's, never two answers joined",
      ok and s == 200 and h.get("Content-Type") == "text/event-stream" and h.get("X-OA-Upstream") == "global" and asked == ["us", "global"]
      and up_text.count("data: ") == 4 and up_text.endswith("data: [DONE]\n\n") and '"content": "global"' in up_text and '"content": "us"' not in up_text, (why, s, h, asked, up_text))
check("upstream: a stream's usage is the answering endpoint's, once", up_kept().split("|")[1:5] == ["200", "2000", "400", "7500"], up_kept())
ok, why = up_close()
s, b, h, took, asked = up_ask(us={"middle": 2.5}, stream=True)
check("upstream: a stream that has begun is never left: us's stream to its end though the bound passes, global not asked",
      ok and s == 200 and h.get("X-OA-Upstream") == "us" and asked == ["us"] and b.decode().endswith("data: [DONE]\n\n") and took > 2.4, (why, s, h, took, asked))

up_count = int(up_kept().split("|")[0])
s, b, h, took, asked = up_ask(us={"delay": 2}, glob={"delay": 6}, settle=0)   # the next ask is inside the race's rest (1 second)
up_long = (s, b, h, took, asked)
s, b, h, took, asked = up_ask(us={"delay": 1.3}, glob={"delay": 6})
up_next = (s, b, took, asked)
s, b, h, took, asked = up_long
check("upstream: a long answer: us wins the race it was first in, and global is dropped",
      s == 200 and up_said(b) == "from us" and h.get("X-OA-Upstream") == "us" and asked == ["us", "global"] and took < 4, (s, b, took, asked))
s, b, took, asked = up_next
check("upstream: after us won a race, the next long answer starts none (a long answer is not paid for twice in a row)",
      s == 200 and up_said(b) == "from us" and asked == ["us"], (s, b, took, asked))
check("upstream: every ask was one call, whatever was tried", up_kept().startswith(f"{up_count + 2}|200|1000|200|3750|"), up_kept())
time.sleep(1.2)   # past the race's rest
s, b, h, took, asked = up_ask(us={"delay": 2.5}, glob={"status": 403})
check("upstream: global's own refusal in a race is not the answer: us's answer still comes",
      s == 200 and up_said(b) == "from us" and h.get("X-OA-Upstream") == "us" and asked == ["us", "global"], (s, b, took, asked))
time.sleep(0.3)
up_lines = [l for l in up_log if "inference: upstream:" in l]
check("upstream: the log says which endpoint answered and how long each took, and no token",
      any("global answered 200" in l and "us asked first, the other when it was slow" in l and "us: dropped after" in l for l in up_lines)
      and any("the other when it failed" in l and "us: 503 in" in l for l in up_lines) and any("neither answered" in l for l in up_lines)
      and not any("test-token" in l or "Bearer" in l for l in up_log), up_lines[:6])
up_api.kill()
up_fake.shutdown()

# OpenAgents Cloud retail pricing, Gemini 4 Neon, cached tokens, pricing/usage APIs & decision metering (#14069).
s, b, _ = call("GET", "/api/v1/inference/pricing")
pricing = json.loads(b) if s == 200 else {}
p_models = {m["name"]: m for m in pricing.get("models", [])}
check("pricing: public GET /api/v1/inference/pricing returns authoritative OpenAgents retail schedule",
      s == 200 and pricing.get("currency") == "USD" and pricing.get("local_cost_usd_micros") == 0
      and p_models.get("google/gemini-4-neon", {}).get("input_micros") == 3750000
      and p_models.get("google/gemini-4-neon", {}).get("cached_input_micros") == 187500
      and p_models.get("google/gemini-4-neon", {}).get("output_micros") == 18750000
      and p_models.get("google/gemini-4-neon", {}).get("markup_bps") == 2500
      and p_models.get("gemini-4-neon", {}).get("input_micros") == 3750000
      and p_models.get("openagents/router-decision", {}).get("kind") == "decision"
      and p_models.get("openagents/router-decision", {}).get("per_call_micros") == 250, (s, pricing))
check("pricing: POST is 405", call("POST", "/api/v1/inference/pricing", b"{}")[0] == 405)

# Gemini 4 Neon + alias + cached input tokens metering & billing (+25% retail vs upstream).
sql(f"delete from inference.account_day where pubkey = '{infer_user}'")
infer_mode["cached_tokens"] = 500
s, b, _ = infer({"model": "google/gemini-4-neon", "max_tokens": 32768, "messages": infer_ask["messages"]})
infer_mode["cached_tokens"] = 0
time.sleep(0.4)
# 500 uncached * 3.75 + 500 cached * 0.1875 + 200 out * 18.75 = 1875 + 93.75 + 3750 = 5718.75 -> ceil 5719 micros
# upstream: 500 * 3.00 + 500 * 0.15 + 200 * 15.00 = 1500 + 75 + 3000 = 4575 micros
check("neon: google/gemini-4-neon is served with high max_tokens and meters cached vs uncached input + upstream cost",
      s == 200 and infer_seen[-1]["body"]["model"] == "google/gemini-4-neon" and infer_seen[-1]["body"]["max_tokens"] == 32768
      and sql(f"select model||'|'||in_tokens||'|'||cached_in_tokens||'|'||out_tokens||'|'||usd_micros||'|'||upstream_usd_micros||'|'||kind from inference.calls where pubkey = '{infer_user}' order by id desc limit 1")
          == "google/gemini-4-neon|1000|500|200|5719|4575|chat", (s, b))

s, b, _ = infer({"model": "gemini-4-neon", "messages": infer_ask["messages"]})
time.sleep(0.4)
check("neon: short alias gemini-4-neon maps to upstream google/gemini-4-neon",
      s == 200 and infer_seen[-1]["body"]["model"] == "google/gemini-4-neon", (s, b))
check("neon: decision model cannot be invoked via chat completions",
      infer({"model": "openagents/router-decision", "messages": infer_ask["messages"]})[0] == 400)

# Usage API (NIP-98 authenticated) + decision-model metering.
check("usage: unauthenticated GET /api/v1/inference/usage is 401", call("GET", "/api/v1/inference/usage")[0] == 401)
sql(f"insert into inference.credit (pubkey, msat) values ('{other}', 20000) on conflict (pubkey) do update set msat = 20000")
sql(f"select inference.decision_record('{other}', 200, 145, false)")
check("decisions: inference.decision_record meters decision call and debits credit in msat",
      sql(f"select model||'|'||kind||'|'||usd_micros||'|'||upstream_usd_micros||'|'||msat from inference.calls where pubkey = '{other}' and kind = 'decision' order by id desc limit 1")
          == "openagents/router-decision|decision|250|200|250"
      and sql(f"select msat from inference.credit where pubkey = '{other}'") == "19750")

usage_url = API + "/api/v1/inference/usage"
usage_sig = subprocess.run([os.environ["SIGN"], "--key", other_key, "GET", usage_url], capture_output=True, text=True, check=True).stdout.strip()
s, b, _ = call("GET", "/api/v1/inference/usage", None, {"Authorization": "Nostr " + usage_sig})
u = json.loads(b) if s == 200 else {}
check("usage: authenticated GET /api/v1/inference/usage returns credit, total usage, decision usage, and $0 local",
      s == 200 and u.get("who") == other and u.get("credit_msat") == 19750 and u.get("local_usd_micros") == 0
      and u.get("decisions", {}).get("calls", 0) >= 1 and u.get("decisions", {}).get("usd_micros", 0) >= 250
      and u.get("total", {}).get("calls", 0) >= 1, (s, u))
