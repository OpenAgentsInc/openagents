#!/usr/bin/env python3
"""The stage-link wire (crates/stage-link, version 1) for Pylon's fake stages.

`stage_worker.py` imports this to send stage state straight to the next
stage. It is also a command for the interoperability test against the Rust
crate:

    stage_link.py listen --key HEX --group G --generation N --from A --to B
    stage_link.py send --address HOST:PORT --key HEX --group G --generation N
        --from A --to B --frames K --state-bytes S

`listen` prints {"port": P}, then one JSON line per frame it reads, then
{"end": REASON}. `send` prints {"sent": K, "bytes": B} or {"error": ...}.
"""

import hashlib
import hmac
import json
import os
import socket
import struct
import sys
import threading
import time

MAGIC = b"OASL"
VERSION = 1
CLIENT_HELLO, SERVER_HELLO, CLIENT_FINISH, ACCEPTED, REFUSED = 1, 2, 3, 4, 5
STATE, CANCEL = 1, 2
REFUSALS = {1: "version", 2: "group", 3: "generation", 4: "hop", 5: "auth", 6: "busy"}
CODES = {name: code for code, name in REFUSALS.items()}
BODY_FIXED = 8 + 1 + 8 + 4 + 8 + 1 + 4 + 4
MAC_LEN = 32
FRAME_OVERHEAD = 4 + BODY_FIXED + MAC_LEN


class LinkError(Exception):
    """A link failed; `kind` is a short word, the message says why."""

    def __init__(self, kind, message):
        super().__init__(message)
        self.kind = kind


def mac(key, *parts):
    digest = hmac.new(key, digestmod=hashlib.sha256)
    for part in parts:
        digest.update(part)
    return digest.digest()


def header(kind):
    return MAGIC + bytes([VERSION, kind])


def client_hello(group, generation, sender, receiver, nonce):
    group = group.encode()
    return (header(CLIENT_HELLO) + struct.pack(">QHHB", generation, sender, receiver, len(group))
            + group + nonce)


def read_exact(sock, n):
    data = bytearray()
    while len(data) < n:
        chunk = sock.recv(n - len(data))
        if not chunk:
            raise LinkError("closed", "the other stage closed the link")
        data += chunk
    return bytes(data)


def read_header(sock, want):
    head = read_exact(sock, 6)
    if head[:4] != MAGIC:
        raise LinkError("malformed", "not a stage link")
    if head[4] != VERSION:
        raise LinkError("refused", "refused: version")
    if head[5] == REFUSED:
        code = read_exact(sock, 1)[0]
        raise LinkError("refused", "refused: " + REFUSALS.get(code, "auth"))
    if head[5] != want:
        raise LinkError("malformed", f"handshake message {head[5]} where {want} was due")


def refuse(sock, why):
    try:
        sock.sendall(header(REFUSED) + bytes([CODES[why]]))
        sock.shutdown(socket.SHUT_RDWR)
    except OSError:
        pass


def encode(frame, seq, session):
    request = frame["request_id"].encode()
    meta, state = frame.get("meta", b""), frame.get("state", b"")
    body = (struct.pack(">QBQIQB", seq, frame.get("kind", STATE), frame["generation"],
                        frame["attempt"], frame["step"], len(request))
            + request + struct.pack(">I", len(meta)) + meta + struct.pack(">I", len(state)) + state)
    return struct.pack(">I", len(body) + MAC_LEN) + body + mac(session, body)


def parse(body):
    seq, kind, generation, attempt, step, n = struct.unpack_from(">QBQIQB", body, 0)
    at = 30
    request = body[at:at + n].decode()
    at += n
    (m,) = struct.unpack_from(">I", body, at)
    meta = body[at + 4:at + 4 + m]
    at += 4 + m
    (s,) = struct.unpack_from(">I", body, at)
    state = body[at + 4:at + 4 + s]
    if at + 4 + s != len(body) or len(state) != s or len(meta) != m:
        raise LinkError("malformed", "a frame ends early or runs long")
    return seq, {"kind": kind, "generation": generation, "request_id": request,
                 "attempt": attempt, "step": step, "meta": meta, "state": state}


def small_buffers(sock, size):
    if size:
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_SNDBUF, size)
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, size)


class Sender:
    """The sending end: a bounded queue in front of a writer thread."""

    def __init__(self, sock, session, generation, queue, send_timeout):
        self.sock, self.session, self.generation = sock, session, generation
        self.room = threading.Semaphore(queue)
        self.send_timeout = send_timeout
        self.items = []
        self.cond = threading.Condition()
        self.seq = 0
        self.queued = self.peak = self.bytes = self.frames = 0
        self.failed = None
        self.closed = False
        threading.Thread(target=self._write, daemon=True).start()

    def _write(self):
        while True:
            with self.cond:
                while not self.items and not self.closed:
                    self.cond.wait()
                if not self.items:
                    break
                data = self.items.pop(0)
            try:
                self.sock.settimeout(self.send_timeout)
                self.sock.sendall(data)
                with self.cond:
                    self.bytes += len(data)
                    self.frames += 1
            except OSError as error:
                with self.cond:
                    if isinstance(error, (socket.timeout, TimeoutError)):
                        # The next stage stopped reading, as in Rust's writer.
                        failure = LinkError(
                            "backpressure",
                            f"the next stage isn't keeping up: {self.queued} frames are waiting")
                    else:
                        failure = LinkError("io", f"the link failed: {error}")
                    self.failed = self.failed or failure
                    self.queued -= 1
                self.room.release()
                return
            with self.cond:
                self.queued -= 1
            self.room.release()

    def send(self, frame):
        if self.failed:
            raise self.failed
        if frame["generation"] != self.generation:
            raise LinkError("generation", "a frame for another generation")
        data = encode(frame, self.seq, self.session)
        if not self.room.acquire(timeout=self.send_timeout):
            with self.cond:
                self.failed = LinkError(
                    "backpressure",
                    f"the next stage isn't keeping up: {self.queued} frames are waiting")
            raise self.failed
        with self.cond:
            if self.failed:
                raise self.failed
            self.queued += 1
            self.peak = max(self.peak, self.queued)
            self.items.append(data)
            self.cond.notify()
        self.seq += 1
        return len(data)

    def abort(self):
        with self.cond:
            self.closed = True
            self.items.clear()
            self.cond.notify()
        try:
            self.sock.shutdown(socket.SHUT_RDWR)
        except OSError:
            pass
        self.sock.close()


def connect(address, key, group, generation, sender, receiver, queue=4, send_timeout=10.0,
            buffers=0, timeout=5.0):
    host, port = address.rsplit(":", 1)
    sock = socket.create_connection((host, int(port)), timeout=timeout)
    small_buffers(sock, buffers)
    sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
    hello = client_hello(group, generation, sender, receiver, os.urandom(32))
    try:
        sock.sendall(hello)
        read_header(sock, SERVER_HELLO)
        nonce = read_exact(sock, 32)
        proof = read_exact(sock, 32)
        if not hmac.compare_digest(proof, mac(key, b"oasl/1 server", hello, nonce)):
            refuse(sock, "auth")
            raise LinkError("refused", "refused: auth")
        sock.sendall(header(CLIENT_FINISH) + mac(key, b"oasl/1 client", hello, nonce))
        read_header(sock, ACCEPTED)
    except (OSError, LinkError):
        sock.close()
        raise
    sock.settimeout(None)
    return Sender(sock, mac(key, b"oasl/1 session", hello, nonce), generation, queue, send_timeout)


class Receiver:
    def __init__(self, sock, session, hop, max_frame=64 << 20):
        self.sock, self.session, self.hop, self.max_frame = sock, session, hop, max_frame
        self.seq = 0
        self.bytes = 0

    def recv(self):
        (length,) = struct.unpack(">I", read_exact(self.sock, 4))
        if length > self.max_frame:
            raise LinkError("too-large", f"a frame of {length} bytes is over the limit")
        data = read_exact(self.sock, length)
        body, proof = data[:-MAC_LEN], data[-MAC_LEN:]
        if not hmac.compare_digest(proof, mac(self.session, body)):
            raise LinkError("mac", "a frame failed its authentication check")
        seq, frame = parse(body)
        if seq != self.seq:
            raise LinkError("sequence", f"frame {seq} arrived where frame {self.seq} was due")
        if frame["generation"] != self.hop["generation"]:
            raise LinkError("generation", "a frame from another generation")
        self.seq += 1
        self.bytes += 4 + length
        return frame

    def close(self):
        try:
            self.sock.shutdown(socket.SHUT_RDWR)
        except OSError:
            pass
        self.sock.close()


class Listener:
    def __init__(self, host="127.0.0.1", port=0, buffers=0):
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        small_buffers(self.sock, buffers)
        self.sock.bind((host, port))
        self.sock.listen(16)
        self.port = self.sock.getsockname()[1]

    def offer(self, timeout=5.0):
        """Accept a connection and read the hop it claims."""
        sock, _ = self.sock.accept()
        sock.settimeout(timeout)
        try:
            head = read_exact(sock, 6)
            if head[:4] != MAGIC or head[5] != CLIENT_HELLO:
                raise LinkError("malformed", "not a client hello")
            if head[4] != VERSION:
                refuse(sock, "version")
                raise LinkError("refused", "refused: version")
            generation, sender, receiver, n = struct.unpack(">QHHB", read_exact(sock, 13))
            group = read_exact(sock, n).decode()
            nonce = read_exact(sock, 32)
        except (OSError, LinkError, UnicodeDecodeError):
            sock.close()
            raise
        hop = {"group": group, "generation": generation, "from": sender, "to": receiver}
        return Offer(sock, hop, client_hello(group, generation, sender, receiver, nonce))


class Offer:
    def __init__(self, sock, hop, hello):
        self.sock, self.hop, self.hello = sock, hop, hello

    def accept(self, key, expected):
        why = None
        if self.hop["group"] != expected["group"]:
            why = "group"
        elif self.hop["generation"] != expected["generation"]:
            why = "generation"
        elif (self.hop["from"], self.hop["to"]) != (expected["from"], expected["to"]):
            why = "hop"
        if why:
            self.refuse(why)
            raise LinkError("refused", "refused: " + why)
        nonce = os.urandom(32)
        try:
            self.sock.sendall(header(SERVER_HELLO) + nonce
                              + mac(key, b"oasl/1 server", self.hello, nonce))
            read_header(self.sock, CLIENT_FINISH)
            proof = read_exact(self.sock, 32)
            if not hmac.compare_digest(proof, mac(key, b"oasl/1 client", self.hello, nonce)):
                self.refuse("auth")
                raise LinkError("refused", "refused: auth")
            self.sock.sendall(header(ACCEPTED))
        except OSError as error:
            self.sock.close()
            raise LinkError("io", str(error)) from error
        self.sock.settimeout(None)
        return Receiver(self.sock, mac(key, b"oasl/1 session", self.hello, nonce), self.hop)

    def refuse(self, why):
        refuse(self.sock, why)
        self.sock.close()


def main():
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=["listen", "send"])
    parser.add_argument("--address")
    parser.add_argument("--key", required=True)
    parser.add_argument("--group", required=True)
    parser.add_argument("--generation", type=int, required=True)
    parser.add_argument("--from", dest="sender", type=int, required=True)
    parser.add_argument("--to", dest="receiver", type=int, required=True)
    parser.add_argument("--frames", type=int, default=3)
    parser.add_argument("--state-bytes", type=int, default=1000)
    args = parser.parse_args()
    key = bytes.fromhex(args.key)

    def out(value):
        print(json.dumps(value), flush=True)

    if args.mode == "listen":
        listener = Listener()
        out({"port": listener.port})
        try:
            receiver = listener.offer().accept(key, {
                "group": args.group, "generation": args.generation,
                "from": args.sender, "to": args.receiver})
        except LinkError as error:
            out({"end": str(error)})
            return
        while True:
            try:
                frame = receiver.recv()
            except LinkError as error:
                out({"end": error.kind if error.kind == "closed" else str(error),
                     "bytes": receiver.bytes})
                return
            out({"kind": frame["kind"], "step": frame["step"], "request_id": frame["request_id"],
                 "meta": frame["meta"].decode(), "len": len(frame["state"]),
                 "sha256": hashlib.sha256(frame["state"]).hexdigest()})
    try:
        sender = connect(args.address, key, args.group, args.generation, args.sender,
                         args.receiver)
        for step in range(args.frames):
            state = bytes((i + step) % 251 for i in range(args.state_bytes))
            sender.send({"generation": args.generation, "request_id": "py", "attempt": 1,
                         "step": step, "meta": b'{"from":"python"}', "state": state})
        deadline = time.monotonic() + 5
        while sender.queued and time.monotonic() < deadline:
            time.sleep(0.01)
        out({"sent": args.frames, "bytes": sender.bytes})
        sender.abort()
    except LinkError as error:
        out({"error": str(error)})


if __name__ == "__main__":
    sys.exit(main())
