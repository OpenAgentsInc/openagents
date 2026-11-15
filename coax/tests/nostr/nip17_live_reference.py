# Firm reason: 2 (independent NIP-17 cryptographic and installed-client interoperability oracle).
"""Opt-in, disposable-account phone/web/independent-client live oracle.

Run only against a loopback development API. Login codes come from that
server's private log, never from production email. The loopback fixture
serves short-lived test sessions to the simulator and headless browser.
It never prints sessions, secret keys, codes or opened message content.
The independent NIP-01/44/59/17 implementation is nip17_reference.py.
Requires the tooling-only websocket-client package for the real relay.
"""
import argparse
import copy
import http.server
import json
import pathlib
import re
import secrets
import threading
import time
import urllib.request
import urllib.error

import nip17_reference as n
import websocket


def run():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--api", required=True)
    p.add_argument("--log", required=True)
    p.add_argument("--upstream", help="Loopback API tunnel; defaults to --api")
    p.add_argument("--port", type=int, default=9183)
    p.add_argument("--lose-seal-response", action="store_true", help="Return one 503 after a real native seal commits")
    args = p.parse_args()
    assert re.fullmatch(r"http://127\.0\.0\.1:[0-9]+", args.api)
    upstream = args.upstream or args.api
    assert re.fullmatch(r"http://127\.0\.0\.1:[0-9]+", upstream)
    relay = "wss://relay.openagents.com"
    progress, lock = [], threading.Lock()
    uncertain = {"lost": False, "same_retry": False, "operation": "", "body": b""}
    nonce = secrets.token_hex(8)
    text, reply = "NIP17 phone " + nonce, "NIP17 web " + nonce

    def api(path, value=None, token=None):
        headers = {"Origin": args.api, "Content-Type": "application/json"}
        if token:
            headers["Authorization"] = "Bearer " + token
        req = urllib.request.Request(upstream + path, headers=headers,
                                     data=None if value is None else json.dumps(value).encode())
        with urllib.request.urlopen(req, timeout=20) as response:
            return json.load(response)

    def account(name):
        email = "nostr-live-" + name + "-" + nonce + "@example.com"
        begin = api("/api/auth/email/start", dict(email=email, client="app"))
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            found = re.findall(r"dev server login code for " + re.escape(email) + r": ([0-9]{6})", pathlib.Path(args.log).read_text())
            if found:
                break
            threading.Event().wait(.1)
        assert found, "Disposable login code did not reach the private fixture log"
        tokens = api("/api/auth/email/verify", dict(pending=begin["pending"], code=found[-1]))
        token = tokens["access_token"]
        identity = api("/api/auth/identity/v1", token=token)["pubkey"]
        settings = api("/api/auth/settings/v1", token=token)
        settings["relays"] = [relay]
        api("/api/auth/settings/v1", settings, token)
        return dict(tokens=tokens, identity=identity)

    native, web = account("native"), account("web")
    secret = secrets.token_bytes(32)
    reference = n.r.xonly(secret).hex()
    state = dict(origin=args.api, text=text, reply=reply, reference=reference, uncertain=args.lose_seal_response)
    native.update(state, peer=web["identity"])
    web.update(state, peer=native["identity"])
    assert native["identity"] != web["identity"] != reference

    class Fixture(http.server.BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def answer(self, value):
            body = json.dumps(value).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Cache-Control", "no-store")
            self.send_header("Access-Control-Allow-Origin", args.api)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_GET(self):
            if self.path.startswith("/api/"):
                self.proxy()
                return
            if self.path == "/native":
                self.answer(native)
            elif self.path == "/web":
                self.answer(web)
            elif self.path == "/state":
                with lock:
                    self.answer(dict(stages=progress, lost_seal_response=uncertain["lost"], exact_seal_retry=uncertain["same_retry"]))
            else:
                self.send_error(404)

        def do_POST(self):
            if self.path.startswith("/api/"):
                self.proxy()
                return
            if self.path != "/progress":
                self.send_error(404)
                return
            stage = self.rfile.read(min(int(self.headers.get("Content-Length", "0")), 128)).decode()
            assert re.fullmatch(r"[a-z-]{1,80}", stage)
            with lock:
                progress.append(stage)
            print("live oracle:", stage, flush=True)
            self.answer({"ok": True})

        def proxy(self):
            # The browser begins with its disposable real session on its very
            # first request. Explicit native credentials retain their owner.
            body = self.rfile.read(int(self.headers.get("Content-Length", "0"))) if self.command == "POST" else None
            headers = {"Origin": args.api, "Content-Type": "application/json", "Authorization": self.headers.get("Authorization") or "Bearer " + web["tokens"]["access_token"]}
            request = urllib.request.Request(upstream + self.path, data=body, headers=headers, method=self.command)
            try:
                response = urllib.request.urlopen(request, timeout=30)
            except urllib.error.HTTPError as error:
                response = error
            result = response.read()
            if self.path == "/api/auth/nostr/custody/v1alpha1" and body and args.lose_seal_response:
                request_body = json.loads(body)
                if request_body.get("action") == "seal" and headers["Authorization"] == "Bearer " + native["tokens"]["access_token"]:
                    with lock:
                        if not uncertain["lost"] and response.code == 200 and json.loads(result).get("status") == "ok":
                            uncertain.update(lost=True, operation=request_body["operation"], body=body)
                            result = b'{"error":"Injected uncertain seal response"}'
                            response.code = 503
                        elif request_body["operation"] == uncertain["operation"]:
                            assert body == uncertain["body"], "Uncertain send changed its original request"
                            uncertain["same_retry"] = True
            self.send_response(response.code)
            self.send_header("Content-Type", response.headers.get("Content-Type", "application/json"))
            self.send_header("Cache-Control", "no-store")
            self.send_header("Content-Length", str(len(result)))
            self.end_headers()
            self.wfile.write(result)

    def independent():
        try:
            ws = websocket.create_connection(relay, timeout=15)
            def send(value):
                ws.send(json.dumps(value, separators=(",", ":")))
            def receive():
                frame = json.loads(ws.recv())
                if frame[0] == "AUTH":
                    auth = json.loads(n.r.signed(secret, secrets.token_bytes(32), int(time.time()), 22242,
                                                [["relay", relay], ["challenge", frame[1]]], ""))
                    send(["AUTH", auth])
                return frame
            own = json.loads(n.r.signed(secret, secrets.token_bytes(32), int(time.time()), 10050, [["relay", relay]], ""))
            # Join AUTH before publishing/subscribing on the reference socket.
            for _ in range(10):
                frame = receive()
                if frame[0] == "OK" and frame[2] is True:
                    break
            send(["EVENT", own])
            for _ in range(20):
                frame = receive()
                if frame[:2] == ["OK", own["id"]]:
                    assert frame[2] is True
                    break
            else:
                raise AssertionError("Reference inbox list was not accepted")
            send(["REQ", "reference", {"kinds": [1059], "#p": [reference]}])
            ws.settimeout(600)
            print("live oracle: independent-client-ready", flush=True)
            with lock:
                progress.append("independent-client-ready")
            seen = set()
            while True:
                frame = receive()
                if frame[0] != "EVENT":
                    continue
                event = frame[2]
                _, rumor, _ = n.open_message(json.dumps(event), secret)
                if rumor["id"] in seen:
                    continue
                seen.add(rumor["id"])
                assert rumor["pubkey"] == native["identity"] and rumor["content"] == text + " reference"
                assert abs(event["created_at"] - rumor["created_at"]) <= 172800
                forged = copy.deepcopy(event)
                forged["sig"] = "0" * 128
                try:
                    n.open_message(json.dumps(forged), secret)
                except (AssertionError, ValueError):
                    pass
                else:
                    raise AssertionError("Forged wrap was admitted by reference")
                try:
                    n.open_message(json.dumps(event), n.r.k(91))
                except (AssertionError, ValueError):
                    pass
                else:
                    raise AssertionError("Wrong recipient opened a wrap")
                at = int(time.time())
                tags = [["p", native["identity"]], ["e", rumor["id"]]]
                for recipient in (native["identity"], reference):
                    fresh = dict(wrap_key=secrets.token_bytes(32), seal_nonce=secrets.token_bytes(32), wrap_nonce=secrets.token_bytes(32), seal_aux=secrets.token_bytes(32), wrap_aux=secrets.token_bytes(32), seal_back=secrets.randbelow(172800), wrap_back=secrets.randbelow(172800))
                    _, raw = n.wrap_one(secret, bytes.fromhex(recipient), at, 14, tags, reply + " reference", fresh)
                    send(["EVENT", json.loads(raw)])
                with lock:
                    progress.append("independent-client-replied")
                print("live oracle: independent-client-replied", flush=True)
                return
        except Exception as error:
            # Do not include exception bodies: an HTTP or SDK error may contain
            # credentials. Public stage + exception type is sufficient here.
            with lock:
                progress.append("independent-client-failed")
            print("live oracle: independent-client-failed", type(error).__name__, flush=True)

    threading.Thread(target=independent, daemon=True).start()
    print("live oracle: disposable-accounts-ready", flush=True)
    http.server.ThreadingHTTPServer(("127.0.0.1", args.port), Fixture).serve_forever()


if __name__ == "__main__":
    run()
