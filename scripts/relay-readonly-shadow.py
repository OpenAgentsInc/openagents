#!/usr/bin/env python3
"""Compare bounded read-only Nostr queries across two relays."""

import argparse
import base64
import hashlib
import json
import os
import socket
import ssl
import struct
import sys
from pathlib import Path
from urllib.parse import urlsplit


MAX_URL_BYTES = 2048
MAX_HANDSHAKE_BYTES = 16384
MAX_FRAME_BYTES = 1048576
MAX_QUERIES = 64
MAX_FILTERS = 16
MAX_EVENTS = 10000


class WebSocket:
    def __init__(self, url: str, timeout: float) -> None:
        if len(url.encode()) > MAX_URL_BYTES:
            raise ValueError("relay URL exceeds 2048 bytes")
        parsed = urlsplit(url)
        if (
            parsed.scheme not in ("ws", "wss")
            or parsed.hostname is None
            or parsed.username is not None
            or parsed.password is not None
            or parsed.query
            or parsed.fragment
        ):
            raise ValueError("relay URL must be credential-free ws:// or wss://")
        port = parsed.port or (443 if parsed.scheme == "wss" else 80)
        raw = socket.create_connection((parsed.hostname, port), timeout=timeout)
        if parsed.scheme == "wss":
            raw = ssl.create_default_context().wrap_socket(
                raw, server_hostname=parsed.hostname
            )
        raw.settimeout(timeout)
        self.connection = raw
        self.stream = raw.makefile("rb")
        path = parsed.path or "/"
        host = parsed.hostname if parsed.port is None else f"{parsed.hostname}:{port}"
        key = base64.b64encode(os.urandom(16)).decode()
        raw.sendall(
            (
                f"GET {path} HTTP/1.1\r\n"
                f"Host: {host}\r\n"
                "Upgrade: websocket\r\n"
                "Connection: Upgrade\r\n"
                f"Sec-WebSocket-Key: {key}\r\n"
                "Sec-WebSocket-Version: 13\r\n\r\n"
            ).encode()
        )
        status = self._readline().decode("ascii", errors="strict").strip()
        if status != "HTTP/1.1 101 Switching Protocols":
            raise RuntimeError(f"WebSocket upgrade failed: {status}")
        headers = {}
        consumed = len(status) + 2
        while True:
            line = self._readline()
            consumed += len(line)
            if consumed > MAX_HANDSHAKE_BYTES:
                raise RuntimeError("WebSocket response headers exceed 16384 bytes")
            if line in (b"\r\n", b""):
                break
            name, separator, value = line.decode("ascii", errors="strict").partition(":")
            if not separator:
                raise RuntimeError("malformed WebSocket response header")
            headers[name.lower()] = value.strip()
        expected = base64.b64encode(
            hashlib.sha1((key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()
        ).decode()
        if headers.get("sec-websocket-accept") != expected:
            raise RuntimeError("WebSocket accept digest mismatch")

    def _readline(self) -> bytes:
        line = self.stream.readline(MAX_HANDSHAKE_BYTES + 1)
        if len(line) > MAX_HANDSHAKE_BYTES:
            raise RuntimeError("WebSocket response line exceeds 16384 bytes")
        return line

    def _read_exact(self, length: int) -> bytes:
        value = self.stream.read(length)
        if value is None or len(value) != length:
            raise RuntimeError("WebSocket closed before a complete frame arrived")
        return value

    def send(self, value: object) -> None:
        payload = json.dumps(value, separators=(",", ":")).encode()
        if len(payload) > MAX_FRAME_BYTES:
            raise RuntimeError("outbound WebSocket frame exceeds 1048576 bytes")
        mask = os.urandom(4)
        header = bytearray([0x81])
        if len(payload) < 126:
            header.append(0x80 | len(payload))
        elif len(payload) <= 0xFFFF:
            header.append(0x80 | 126)
            header.extend(struct.pack("!H", len(payload)))
        else:
            header.append(0x80 | 127)
            header.extend(struct.pack("!Q", len(payload)))
        header.extend(mask)
        masked = bytes(byte ^ mask[index % 4] for index, byte in enumerate(payload))
        self.connection.sendall(header + masked)

    def read(self) -> object:
        while True:
            first, second = self._read_exact(2)
            if first & 0x80 == 0:
                raise RuntimeError("fragmented WebSocket responses are unsupported")
            opcode = first & 0x0F
            length = second & 0x7F
            if length == 126:
                length = struct.unpack("!H", self._read_exact(2))[0]
            elif length == 127:
                length = struct.unpack("!Q", self._read_exact(8))[0]
            if length > MAX_FRAME_BYTES:
                raise RuntimeError("inbound WebSocket frame exceeds 1048576 bytes")
            mask = self._read_exact(4) if second & 0x80 else None
            payload = self._read_exact(length)
            if mask is not None:
                payload = bytes(
                    byte ^ mask[index % 4] for index, byte in enumerate(payload)
                )
            if opcode == 0x8:
                raise RuntimeError("relay closed the WebSocket during shadow query")
            if opcode == 0x9:
                self._send_pong(payload)
                continue
            if opcode != 0x1:
                raise RuntimeError(f"unexpected WebSocket opcode {opcode}")
            return json.loads(payload)

    def _send_pong(self, payload: bytes) -> None:
        if len(payload) > 125:
            raise RuntimeError("oversized WebSocket ping")
        mask = os.urandom(4)
        masked = bytes(byte ^ mask[index % 4] for index, byte in enumerate(payload))
        self.connection.sendall(bytes([0x8A, 0x80 | len(payload)]) + mask + masked)

    def close(self) -> None:
        try:
            self.connection.sendall(b"\x88\x80\x00\x00\x00\x00")
        finally:
            self.stream.close()
            self.connection.close()


def canonical(value: object) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()


def load_workload(path: Path) -> tuple[dict, bytes]:
    raw = path.read_bytes()
    if len(raw) > MAX_FRAME_BYTES:
        raise ValueError("shadow workload exceeds 1048576 bytes")
    workload = json.loads(raw)
    if set(workload) != {"schema", "queries"}:
        raise ValueError("shadow workload must contain exactly schema and queries")
    if workload["schema"] != "openagents.nostr-relay.relay-shadow-workload.v1":
        raise ValueError("unexpected shadow workload schema")
    queries = workload["queries"]
    if not isinstance(queries, list) or not 1 <= len(queries) <= MAX_QUERIES:
        raise ValueError("shadow workload must contain between 1 and 64 queries")
    names = set()
    for query in queries:
        if set(query) != {"name", "type", "filters"}:
            raise ValueError("each shadow query must contain name, type, and filters")
        name = query["name"]
        if (
            not isinstance(name, str)
            or not 1 <= len(name) <= 64
            or name in names
            or query["type"] not in ("req", "count")
            or not isinstance(query["filters"], list)
            or not 1 <= len(query["filters"]) <= MAX_FILTERS
            or not all(isinstance(item, dict) for item in query["filters"])
        ):
            raise ValueError(f"invalid bounded shadow query {name!r}")
        names.add(name)
    return workload, raw


def read_query(url: str, query: dict, timeout: float) -> dict:
    client = WebSocket(url, timeout)
    subscription = f"shadow-{query['name']}"
    try:
        if query["type"] == "count":
            client.send(["COUNT", subscription, *query["filters"]])
            while True:
                message = client.read()
                if is_auth_challenge(message):
                    continue
                if (
                    isinstance(message, list)
                    and len(message) == 3
                    and message[:2] == ["COUNT", subscription]
                    and isinstance(message[2], dict)
                    and isinstance(message[2].get("count"), int)
                ):
                    return {"count": message[2]["count"]}
                if isinstance(message, list) and message[:2] == ["CLOSED", subscription]:
                    raise RuntimeError(f"COUNT was closed: {message}")
        client.send(["REQ", subscription, *query["filters"]])
        events = {}
        while True:
            message = client.read()
            if is_auth_challenge(message):
                continue
            if message == ["EOSE", subscription]:
                digest = hashlib.sha256(
                    b"\n".join(events[event_id] for event_id in sorted(events))
                ).hexdigest()
                return {
                    "event_count": len(events),
                    "event_sha256": digest,
                    "events": events,
                }
            if isinstance(message, list) and message[:2] == ["CLOSED", subscription]:
                raise RuntimeError(f"REQ was closed: {message}")
            if (
                not isinstance(message, list)
                or len(message) != 3
                or message[:2] != ["EVENT", subscription]
                or not isinstance(message[2], dict)
            ):
                raise RuntimeError(f"unexpected shadow response: {message}")
            event_id = message[2].get("id")
            if (
                not isinstance(event_id, str)
                or len(event_id) != 64
                or any(character not in "0123456789abcdef" for character in event_id)
            ):
                raise RuntimeError("shadow response has an invalid event id")
            encoded = canonical(message[2])
            previous = events.get(event_id)
            if previous is not None and previous != encoded:
                raise RuntimeError(f"relay returned conflicting bytes for event {event_id}")
            events[event_id] = encoded
            if len(events) > MAX_EVENTS:
                raise RuntimeError("shadow response exceeds 10000 unique events")
    finally:
        client.close()


def is_auth_challenge(message: object) -> bool:
    return (
        isinstance(message, list)
        and len(message) == 2
        and message[0] == "AUTH"
        and isinstance(message[1], str)
        and len(message[1]) <= 128
    )


def compare_query(name: str, incumbent: dict, candidate: dict) -> dict:
    if "count" in incumbent or "count" in candidate:
        matched = incumbent == candidate
        return {
            "name": name,
            "type": "count",
            "incumbent_count": incumbent.get("count"),
            "candidate_count": candidate.get("count"),
            "matched": matched,
        }
    incumbent_events = incumbent.pop("events")
    candidate_events = candidate.pop("events")
    incumbent_ids = set(incumbent_events)
    candidate_ids = set(candidate_events)
    changed = sorted(
        event_id
        for event_id in incumbent_ids & candidate_ids
        if incumbent_events[event_id] != candidate_events[event_id]
    )
    only_incumbent = sorted(incumbent_ids - candidate_ids)
    only_candidate = sorted(candidate_ids - incumbent_ids)
    matched = not changed and not only_incumbent and not only_candidate
    return {
        "name": name,
        "type": "req",
        "incumbent": incumbent,
        "candidate": candidate,
        "only_incumbent": only_incumbent,
        "only_candidate": only_candidate,
        "changed": changed,
        "matched": matched,
    }


def write_output(path: Path, result: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.tmp-{os.getpid()}")
    temporary.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    temporary.replace(path)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--incumbent", required=True)
    parser.add_argument("--candidate", required=True)
    parser.add_argument("--workload", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--timeout-seconds", type=float, default=10.0)
    arguments = parser.parse_args()
    if not 0.1 <= arguments.timeout_seconds <= 120:
        raise ValueError("timeout must be between 0.1 and 120 seconds")
    workload, raw_workload = load_workload(arguments.workload)
    comparisons = []
    for query in workload["queries"]:
        incumbent = read_query(arguments.incumbent, query, arguments.timeout_seconds)
        candidate = read_query(arguments.candidate, query, arguments.timeout_seconds)
        comparisons.append(compare_query(query["name"], incumbent, candidate))
    result = {
        "schema": "openagents.nostr-relay.relay-shadow-result.v1",
        "read_only": True,
        "incumbent": arguments.incumbent,
        "candidate": arguments.candidate,
        "workload_sha256": hashlib.sha256(raw_workload).hexdigest(),
        "queries": comparisons,
        "matched": all(comparison["matched"] for comparison in comparisons),
    }
    write_output(arguments.output, result)
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0 if result["matched"] else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, RuntimeError, json.JSONDecodeError) as error:
        print(f"relay-readonly-shadow: {error}", file=sys.stderr)
        sys.exit(2)
