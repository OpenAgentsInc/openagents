# Firm reason: 2 (an independent oracle: a fake relay that recomputes each event's NIP-01 id in Python, for test.py and the auth checks)
"""A Nostr relay for the checks: it asks for AUTH (NIP-42), checks each event's id against its
NIP-01 serialization, keeps it, and answers REQs by author and kind. test_project_drops.py serves
it inside test.py; run alone (python3 fake_relay.py PORT FILE) it serves until killed and appends
each kept event to FILE as a JSON line (the auth checks' notices).
"""
import base64 as b64mod, hashlib, json, os, socketserver, struct, sys, threading
pub_relay = {"events": [], "auth": [], "reqs": []}
keep_file = None

class FakeRelay(socketserver.BaseRequestHandler):
    def frame(self, text):
        data = text.encode()
        head = bytes([0x81]) + (bytes([len(data)]) if len(data) < 126 else bytes([126]) + struct.pack(">H", len(data)) if len(data) < 65536
                                else bytes([127]) + struct.pack(">Q", len(data)))
        self.request.sendall(head + data)

    def read(self, n):
        out = b""
        while len(out) < n:
            got = self.request.recv(n - len(out))
            if not got:
                raise EOFError
            out += got
        return out

    def handle(self):
        req = b""
        while b"\r\n\r\n" not in req:
            req += self.request.recv(4096)
        key = [l.split(":", 1)[1].strip() for l in req.decode().split("\r\n") if l.lower().startswith("sec-websocket-key:")][0]
        accept = b64mod.b64encode(hashlib.sha1((key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()).decode()
        self.request.sendall(f"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n".encode())
        self.frame('["AUTH","challenge-1"]')
        try:
            while True:
                h = self.read(2)
                n = h[1] & 0x7F
                if n == 126:
                    n = struct.unpack(">H", self.read(2))[0]
                elif n == 127:
                    n = struct.unpack(">Q", self.read(8))[0]
                mask = self.read(4) if h[1] & 0x80 else b"\0\0\0\0"
                data = bytes(c ^ mask[i % 4] for i, c in enumerate(self.read(n)))
                if h[0] & 0x0F == 8:
                    return
                msg = json.loads(data)
                if msg[0] == "CLOSE":
                    continue
                if msg[0] == "REQ":   # rc-later-drops: the project's work, as a relay answers a filter
                    f = msg[2]
                    pub_relay["reqs"].append(f)
                    for e in pub_relay["events"]:
                        if e["pubkey"] in f.get("authors", [e["pubkey"]]) and e["kind"] in f.get("kinds", [e["kind"]]):
                            self.frame(json.dumps(["EVENT", msg[1], e]))
                    self.frame(json.dumps(["EOSE", msg[1]]))
                    continue
                ev = msg[1]
                serial = json.dumps([0, ev["pubkey"], ev["created_at"], ev["kind"], ev["tags"], ev["content"]], separators=(",", ":"), ensure_ascii=False)
                ok = hashlib.sha256(serial.encode()).hexdigest() == ev["id"] and len(ev["sig"]) == 128 and h[1] & 0x80
                if msg[0] == "AUTH":
                    pub_relay["auth"].append(ev)
                    ok = ok and ev["kind"] == 22242 and ["challenge", "challenge-1"] in ev["tags"]
                elif not pub_relay["auth"]:
                    self.frame(json.dumps(["OK", ev["id"], False, "auth-required: first"]))
                    continue
                else:
                    pub_relay["events"].append(ev)
                    if keep_file and ok:
                        with open(keep_file, "a") as f:
                            f.write(json.dumps(ev) + "\n")
                self.frame(json.dumps(["OK", ev["id"], bool(ok), "" if ok else "invalid: id"]))
        except EOFError:
            return




def serve(port):
    server = socketserver.ThreadingTCPServer(("127.0.0.1", port), FakeRelay)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


if __name__ == "__main__":
    keep_file = sys.argv[2]
    socketserver.ThreadingTCPServer.allow_reuse_address = True
    server = socketserver.ThreadingTCPServer(("127.0.0.1", int(sys.argv[1])), FakeRelay)
    server.serve_forever()
