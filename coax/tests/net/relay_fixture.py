#!/usr/bin/env python3
"""A scripted Nostr relay for the native relay client's end-to-end test.

Tooling only, standard library only, bound to 127.0.0.1. It speaks RFC 6455
(server side) and serves seven already-signed "status" events captured in
coax/lib/nostr/fixtures/coax_relay.coax, so nothing is signed here. It is not
a general relay: every connection follows the script that relay_e2e.coax
expects, and every observation is recorded so relay_e2e.py can check what the
client actually sent (mask keys, handshake keys, REQ filters, publishes).

Connection 1: handshake; REQ -> E1, E2, E3, EOSE, then a ping; A -> OK true;
B -> OK false; C -> the connection is dropped (no OK, no close frame).
Connection 2: the request head is read and the socket closed (a refused
handshake, so the client's next backoff doubles).
Connection 3: handshake; REQ with since -> the stored events at or after
since (E3 again, and E4, which "arrived" during the outage), EOSE; C -> OK
true; CLOSE; the client's close frame is echoed and the socket closed.
"""
import base64
import hashlib
import json
import re
import socket
import struct
import threading
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
FIXTURE = ROOT / 'coax/lib/nostr/fixtures/coax_relay.coax'
GUID = b'258EAFA5-E914-47DA-95CA-C5AB0DC85B11'
PREFIX = '["EVENT","status",'
REFUSAL = 'blocked: fixture policy'
RELAY_PING = b'rx-relay-ping'


def fixture_events():
    """The seven events (raw JSON text, parsed) the driver's adapter names: lines 241 down to 235."""
    text = FIXTURE.read_text()
    body = re.search(r'COAX_RELAY :: "(.*)";', text, re.S).group(1)
    lines = body.encode().decode('unicode_escape').split('\n')
    out = []
    for k in range(7):
        line = lines[241 - k]
        assert line.startswith(PREFIX) and line.endswith(']'), k
        raw = line[len(PREFIX):-1]
        out.append((raw, json.loads(raw)))
    return out


class Closed(Exception):
    pass


class Conn:
    def __init__(self, sock, relay, number):
        self.sock, self.relay, self.number = sock, relay, number
        self.buf = b''

    def need(self, n):
        while len(self.buf) < n:
            chunk = self.sock.recv(65536)
            if not chunk:
                raise Closed()
            self.buf += chunk
        out, self.buf = self.buf[:n], self.buf[n:]
        return out

    def head(self):
        while b'\r\n\r\n' not in self.buf:
            chunk = self.sock.recv(65536)
            if not chunk:
                raise Closed()
            self.buf += chunk
            if len(self.buf) > 8192:
                raise ValueError('request head too long')
        head, self.buf = self.buf.split(b'\r\n\r\n', 1)
        lines = head.decode('ascii').split('\r\n')
        headers = {}
        for line in lines[1:]:
            name, value = line.split(':', 1)
            headers[name.strip().lower()] = value.strip()
        return lines[0], headers

    def handshake(self, request, headers):
        key = headers.get('sec-websocket-key', '')
        self.relay.note('handshake', request=request, key=key, host=headers.get('host'),
                        upgrade=headers.get('upgrade'), connection=headers.get('connection'),
                        version=headers.get('sec-websocket-version'))
        return key

    def accept(self, key):
        accept = base64.b64encode(hashlib.sha1(key.encode() + GUID).digest()).decode()
        self.sock.sendall(('HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n'
                           'Connection: Upgrade\r\nSec-WebSocket-Accept: %s\r\n\r\n' % accept).encode())

    def send(self, op, payload):
        n = len(payload)
        if n < 126:
            header = struct.pack('!BB', 0x80 | op, n)
        elif n < 65536:
            header = struct.pack('!BBH', 0x80 | op, 126, n)
        else:
            header = struct.pack('!BBQ', 0x80 | op, 127, n)
        self.sock.sendall(header + payload)

    def text(self, s):
        self.send(1, s.encode())

    def frame(self):
        b0, b1 = self.need(2)
        fin, op, masked, n = b0 & 0x80, b0 & 15, b1 & 0x80, b1 & 127
        if b0 & 0x70 or not fin:
            raise ValueError('unexpected RSV bits or fragment from the client')
        if n == 126:
            n = struct.unpack('!H', self.need(2))[0]
        elif n == 127:
            n = struct.unpack('!Q', self.need(8))[0]
        if not masked:
            raise ValueError('client frame not masked')
        mask = self.need(4)
        self.relay.note('mask', mask=mask.hex())
        data = bytes(c ^ mask[i % 4] for i, c in enumerate(self.need(n)))
        return op, data


class FixtureRelay:
    def __init__(self):
        self.events = fixture_events()
        self.ids = [e[1]['id'] for e in self.events]
        self.log = []
        self.errors = []
        self.strays = 0
        self.number = 0
        self.lock = threading.Lock()
        self.server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.server.bind(('127.0.0.1', 0))
        self.server.listen(4)
        self.port = self.server.getsockname()[1]
        self.thread = threading.Thread(target=self.serve, daemon=True)
        self.stopped = False

    def note(self, what, **fields):
        with self.lock:
            self.log.append(dict(what=what, conn=self.number, **fields))

    def start(self):
        self.thread.start()
        return self

    def stop(self):
        self.stopped = True
        try:
            self.server.close()
        except OSError:
            pass
        self.thread.join(timeout=5)

    def serve(self):
        while not self.stopped and self.number < 3:
            try:
                sock, _ = self.server.accept()
            except OSError:
                return
            sock.settimeout(20)
            conn = Conn(sock, self, self.number + 1)
            try:
                request, headers = conn.head()
                if not request.startswith('GET ') or headers.get('upgrade', '').lower() != 'websocket':
                    # Something else on this host probing the port (seen: a
                    # HEAD request from a local health checker). Not the
                    # client under test: answer 400 and don't count it.
                    self.strays += 1
                    sock.sendall(b'HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n')
                    continue
                self.number += 1
                self.script(conn, request, headers)
            except Closed:
                if conn.number == self.number:
                    self.note('eof')
            except Exception as e:  # recorded, checked by the harness
                self.errors.append('connection %d: %r' % (self.number, e))
            finally:
                try:
                    sock.close()
                except OSError:
                    pass

    def script(self, c, request, headers):
        key = c.handshake(request, headers)
        if c.number == 2:
            self.note('refused-handshake')
            c.sock.shutdown(socket.SHUT_RDWR)
            return
        c.accept(key)
        store = self.events[:3] if c.number == 1 else self.events[:4]
        while True:
            op, data = c.frame()
            if op == 9:
                self.note('ping', payload=data.decode())
                c.send(10, data)
            elif op == 10:
                self.note('pong', payload=data.decode())
            elif op == 8:
                code = struct.unpack('!H', data[:2])[0] if len(data) >= 2 else None
                self.note('close', code=code)
                c.send(8, struct.pack('!H', 1000))
                c.sock.shutdown(socket.SHUT_RDWR)
                return
            elif op == 1:
                msg = json.loads(data.decode())
                if msg[0] == 'REQ':
                    sub, filt = msg[1], msg[2]
                    self.note('req', sub=sub, filter=filt, text=data.decode())
                    since = filt.get('since')
                    for raw, ev in sorted(store, key=lambda e: e[1]['created_at']):
                        if ev['kind'] in filt.get('kinds', []) and (since is None or ev['created_at'] >= since):
                            c.text('["EVENT",%s,%s]' % (json.dumps(sub), raw))
                    c.text('["EOSE",%s]' % json.dumps(sub))
                    if c.number == 1:
                        c.send(9, RELAY_PING)
                elif msg[0] == 'EVENT':
                    ev = msg[1]
                    k = self.ids.index(ev['id']) if ev['id'] in self.ids else -1
                    exact = k >= 0 and data.decode() == '["EVENT",%s]' % self.events[k][0]
                    self.note('event', k=k, exact=exact)
                    if k == 4:
                        c.text('["OK","%s",true,""]' % ev['id'])
                    elif k == 5:
                        c.text('["OK","%s",false,"%s"]' % (ev['id'], REFUSAL))
                    elif k == 6 and c.number == 1:
                        self.note('drop')
                        c.sock.shutdown(socket.SHUT_RDWR)
                        return
                    elif k == 6:
                        c.text('["OK","%s",true,""]' % ev['id'])
                    else:
                        raise ValueError('unexpected publish %r' % ev['id'])
                elif msg[0] == 'CLOSE':
                    self.note('unsubscribe', sub=msg[1])
                else:
                    raise ValueError('unexpected message %r' % msg[0])
            else:
                raise ValueError('unexpected opcode %d' % op)

    def failures(self):
        """What the client should have done, against what this relay saw."""
        bad = list(self.errors)
        log = self.log
        def of(what):
            return [e for e in log if e['what'] == what]
        hs = of('handshake')
        keys = [e['key'] for e in hs]
        if len(hs) != 3:
            bad.append('expected 3 handshakes, saw %d' % len(hs))
        for e in hs:
            if e['request'] != 'GET / HTTP/1.1' or e['upgrade'] != 'websocket' or e['version'] != '13' \
                    or e['connection'] != 'Upgrade' or e['host'] != 'localhost:%d' % self.port:
                bad.append('handshake request: %r' % e)
            if len(base64.b64decode(e['key'])) != 16:
                bad.append('Sec-WebSocket-Key is not 16 bytes')
        if len(set(keys)) != len(keys):
            bad.append('a handshake nonce repeated across connections')
        masks = [e['mask'] for e in of('mask')]
        # conn 1: REQ, pong, A, B, ping, C; conn 3: REQ, C, CLOSE, close.
        if len(masks) != 10:
            bad.append('%d masked frames, expected 10' % len(masks))
        if any(m == '00000000' for m in masks):
            bad.append('an all-zero mask key')
        if len(set(masks)) != len(masks):
            bad.append('a mask key repeated (%d frames, %d distinct)' % (len(masks), len(set(masks))))
        reqs = of('req')
        if [r['conn'] for r in reqs] != [1, 3]:
            bad.append('REQs on connections %r, expected [1, 3]' % [r['conn'] for r in reqs])
        else:
            if 'since' in reqs[0]['filter']:
                bad.append('the first REQ had since')
            if reqs[1]['filter'].get('since') != self.events[2][1]['created_at']:
                bad.append('the resubscribe since was %r' % reqs[1]['filter'].get('since'))
            if reqs[0]['sub'] != reqs[1]['sub']:
                bad.append('the resubscribe used another id')
        pubs = [(e['conn'], e['k'], e['exact']) for e in of('event')]
        if pubs != [(1, 4, True), (1, 5, True), (1, 6, True), (3, 6, True)]:
            bad.append('publishes seen %r' % pubs)
        if [e['payload'] for e in of('pong')] != [RELAY_PING.decode()]:
            bad.append('pongs to the relay: %r' % of('pong'))
        if [e['payload'] for e in of('ping')] != ['rx-client-ping']:
            bad.append('pings from the client: %r' % of('ping'))
        if [(e['conn'], e['sub']) for e in of('unsubscribe')] != [(3, reqs[0]['sub'] if reqs else None)]:
            bad.append('CLOSE seen %r' % of('unsubscribe'))
        if [(e['conn'], e['code']) for e in of('close')] != [(3, 1000)]:
            bad.append('close frames %r' % of('close'))
        if len(of('drop')) != 1 or len(of('refused-handshake')) != 1:
            bad.append('the drop and the refused handshake did not both happen once')
        return bad
