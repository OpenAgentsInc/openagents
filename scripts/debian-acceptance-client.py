#!/usr/bin/env python3
"""Publish and read one pinned event over a dependency-free WebSocket client."""

import json
import os
import socket
import struct
from pathlib import Path


HOST = os.environ.get("NOSTR_RELAY_ACCEPTANCE_HOST", "127.0.0.1")
PORT = int(os.environ.get("NOSTR_RELAY_ACCEPTANCE_PORT", "18080"))


def send_text(connection: socket.socket, value: object) -> None:
    payload = json.dumps(value, separators=(",", ":")).encode()
    mask = os.urandom(4)
    header = bytearray([0x81])
    length = len(payload)
    if length < 126:
        header.append(0x80 | length)
    elif length <= 0xFFFF:
        header.append(0x80 | 126)
        header.extend(struct.pack("!H", length))
    else:
        header.append(0x80 | 127)
        header.extend(struct.pack("!Q", length))
    header.extend(mask)
    masked = bytes(byte ^ mask[index % 4] for index, byte in enumerate(payload))
    connection.sendall(header + masked)


def read_exact(stream, length: int) -> bytes:
    value = stream.read(length)
    if value is None or len(value) != length:
        raise RuntimeError("WebSocket closed before a complete frame arrived")
    return value


def read_text(connection: socket.socket, stream) -> object:
    while True:
        first, second = read_exact(stream, 2)
        opcode = first & 0x0F
        length = second & 0x7F
        if length == 126:
            length = struct.unpack("!H", read_exact(stream, 2))[0]
        elif length == 127:
            length = struct.unpack("!Q", read_exact(stream, 8))[0]
        mask = read_exact(stream, 4) if second & 0x80 else None
        payload = read_exact(stream, length)
        if mask is not None:
            payload = bytes(
                byte ^ mask[index % 4] for index, byte in enumerate(payload)
            )
        if opcode == 0x8:
            raise RuntimeError("relay closed the WebSocket during acceptance")
        if opcode == 0x9:
            connection.sendall(bytes([0x8A, len(payload)]) + payload)
            continue
        if opcode == 0x1:
            return json.loads(payload)


def main() -> None:
    fixture_path = Path("tests/fixtures/nip01/events.json")
    event = json.loads(fixture_path.read_text())["event"]
    connection = socket.create_connection((HOST, PORT), timeout=5)
    connection.settimeout(5)
    stream = connection.makefile("rb")
    connection.sendall(
        (
            "GET / HTTP/1.1\r\n"
            f"Host: {HOST}:{PORT}\r\n"
            "Upgrade: websocket\r\n"
            "Connection: Upgrade\r\n"
            "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n"
            "Sec-WebSocket-Version: 13\r\n\r\n"
        ).encode()
    )
    status = stream.readline().decode().strip()
    if status != "HTTP/1.1 101 Switching Protocols":
        raise RuntimeError(f"WebSocket upgrade failed: {status}")
    while stream.readline() not in (b"\r\n", b""):
        pass

    send_text(connection, ["EVENT", event])
    while True:
        message = read_text(connection, stream)
        if message[0] == "OK" and message[1] == event["id"]:
            if message[2] is not True:
                raise RuntimeError(f"event was refused: {message}")
            break

    send_text(connection, ["REQ", "acceptance", {"ids": [event["id"]]}])
    saw_event = False
    while True:
        message = read_text(connection, stream)
        if message[0] == "EVENT" and message[1] == "acceptance":
            saw_event = message[2]["id"] == event["id"]
        if message[:2] == ["EOSE", "acceptance"]:
            break
    if not saw_event:
        raise RuntimeError("accepted event was absent from the historical query")

    connection.sendall(b"\x88\x80\x00\x00\x00\x00")
    connection.close()


if __name__ == "__main__":
    main()
