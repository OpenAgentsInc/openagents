#!/usr/bin/env python3
"""Local TLS endpoint for coax/tests/net/tls.sh (test tooling, never shipped).

Serves TLS on 127.0.0.1 with the given certificate chain and prints
"PORT <n>" once listening. With --ws it is a minimal RFC 6455 echo server
behind TLS: it answers the opening handshake and echoes each text or binary
frame back unmasked, and answers a close frame with a close frame. With
--max 1.1 it offers nothing above TLS 1.1 (security level 0, so OpenSSL
still allows it), to prove the client refuses old versions; --max 1.2 caps
it at TLS 1.2. Stdlib only.
"""
import argparse
import base64
import hashlib
import socket
import ssl
import sys
import threading

GUID = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11"


def read_exact(conn, n):
    out = b""
    while len(out) < n:
        chunk = conn.recv(n - len(out))
        if not chunk:
            raise EOFError
        out += chunk
    return out


def frame(op, payload):
    n = len(payload)
    if n < 126:
        head = bytes([0x80 | op, n])
    elif n < 65536:
        head = bytes([0x80 | op, 126]) + n.to_bytes(2, "big")
    else:
        head = bytes([0x80 | op, 127]) + n.to_bytes(8, "big")
    return head + payload


def websocket(conn):
    head = b""
    while b"\r\n\r\n" not in head:
        chunk = conn.recv(4096)
        if not chunk or len(head) > 8192:
            return
        head += chunk
    key = None
    for line in head.split(b"\r\n")[1:]:
        name, _, value = line.partition(b":")
        if name.strip().lower() == b"sec-websocket-key":
            key = value.strip()
    if key is None:
        conn.sendall(b"HTTP/1.1 400 Bad Request\r\n\r\n")
        return
    accept = base64.b64encode(hashlib.sha1(key + GUID).digest())
    conn.sendall(b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n"
                 b"Connection: Upgrade\r\nSec-WebSocket-Accept: " + accept + b"\r\n\r\n")
    while True:
        b0, b1 = read_exact(conn, 2)
        op, n = b0 & 15, b1 & 127
        if n == 126:
            n = int.from_bytes(read_exact(conn, 2), "big")
        elif n == 127:
            n = int.from_bytes(read_exact(conn, 8), "big")
        mask = read_exact(conn, 4) if b1 & 128 else b"\0\0\0\0"
        data = bytes(c ^ mask[i % 4] for i, c in enumerate(read_exact(conn, n)))
        if op == 8:
            conn.sendall(frame(8, data[:2]))
            return
        if op in (1, 2):
            conn.sendall(frame(op, data))


def serve(conn, ctx, ws):
    try:
        with ctx.wrap_socket(conn, server_side=True) as tls:
            print(f"handshake {tls.version()}", file=sys.stderr, flush=True)
            if ws:
                websocket(tls)
            try:
                tls.unwrap()
            except (OSError, ssl.SSLError, ValueError):
                pass
    except (OSError, ssl.SSLError, EOFError) as e:
        print(f"refused {type(e).__name__}: {e}", file=sys.stderr, flush=True)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--cert", required=True, help="PEM chain: leaf first")
    ap.add_argument("--key", required=True)
    ap.add_argument("--ws", action="store_true")
    ap.add_argument("--max", choices=["1.1", "1.2"])
    a = ap.parse_args()
    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    if a.max == "1.1":
        ctx.set_ciphers("DEFAULT:@SECLEVEL=0")
        ctx.minimum_version = ssl.TLSVersion.TLSv1
        ctx.maximum_version = ssl.TLSVersion.TLSv1_1
    elif a.max == "1.2":
        ctx.maximum_version = ssl.TLSVersion.TLSv1_2
    ctx.load_cert_chain(a.cert, a.key)
    srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    srv.bind(("127.0.0.1", 0))
    srv.listen(8)
    print(f"PORT {srv.getsockname()[1]}", flush=True)
    while True:
        conn, _ = srv.accept()
        threading.Thread(target=serve, args=(conn, ctx, a.ws), daemon=True).start()


if __name__ == "__main__":
    main()
