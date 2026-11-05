#!/usr/bin/env python3
# Firm reason: 2 (independent TLS/wire observation oracle using Python's ssl reference).
"""Finite loopback TLS endpoint; no host or game admission policy lives here."""
import base64
import hashlib
import json
import signal
import socket
import ssl
import sys
import time

port, cert, key, output = sys.argv[1:]
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.minimum_version = ssl.TLSVersion.TLSv1_2
context.load_cert_chain(cert, key)
rows = []
running = True


def stop(*_):
    global running
    running = False


signal.signal(signal.SIGTERM, stop)
signal.signal(signal.SIGINT, stop)


def save():
    with open(output, "w", encoding="utf-8") as stream:
        json.dump(rows, stream, separators=(",", ":"))


save()
deadline = time.monotonic() + 120
with socket.socket() as server:
    server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    server.bind(("127.0.0.1", int(port)))
    server.listen(4)
    server.settimeout(0.2)
    print("READY", flush=True)
    while running and time.monotonic() < deadline:
        try:
            connection, _ = server.accept()
        except socket.timeout:
            continue
        connection.settimeout(3)
        try:
            with context.wrap_socket(connection, server_side=True) as tls:
                header = b""
                while b"\r\n\r\n" not in header and len(header) <= 16384:
                    part = tls.recv(4096)
                    if not part:
                        break
                    header += part
                if b"\r\n\r\n" not in header or len(header) > 16384:
                    continue
                lines = header.split(b"\r\n")
                fields = {}
                for line in lines[1:]:
                    name, _, value = line.partition(b":")
                    fields[name.strip().lower()] = value.strip()
                rows.append({"path": lines[0].split(b" ")[1].decode("ascii"),
                             "origin": fields.get(b"origin", b"").decode("ascii"),
                             "cookie": fields.get(b"cookie", b"").decode("ascii"),
                             "version": tls.version()})
                save()
                accept = base64.b64encode(hashlib.sha1(fields[b"sec-websocket-key"] + b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11").digest())
                tls.sendall(b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: " + accept + b"\r\n\r\n")
                payload = b"tls-authenticated"
                tls.sendall(bytes([0x81, len(payload)]) + payload + b"\x88\x00")
                # Read a close acknowledgment before the concrete TLS owner exits.
                try:
                    tls.recv(4096)
                except (socket.timeout, ssl.SSLError):
                    pass
        except (ssl.SSLError, OSError):
            connection.close()
save()
