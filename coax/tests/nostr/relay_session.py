#!/usr/bin/env python3
"""Scripted sessions against the Coax relay (R1), under ASan and UBSan.

Builds coax/relay with --sanitize, starts it on 127.0.0.1 with a free port,
signs test events with relay_sign.ts (@noble/curves, a TEST-ONLY key) and
drives it with a standard-library WebSocket client: NIP-11, publish and OK,
REQ with stored events then EOSE then live events, CLOSE, duplicates, bad
signatures, replaceable and addressable replacement with the same-second
tie, the kinds and tag filters openagents.com uses, NIP-40 expiration,
created_at limit, subscription/filter/frame limits, AUTH, ping, and a
restart. Tooling only; nothing here ships. Never points at a public relay.

  python3 coax/tests/nostr/relay_session.py      # COAX=..., BUN=..., NOBLE=... override
"""
import base64
import hashlib
import json
import os
import socket
import struct
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
NOBLE = os.environ.get('NOBLE', '/home/christopherdavid/oa-board/coax/web/project/node_modules')
BUN = os.environ.get('BUN', 'bun')
failures = []


def check(ok, what):
    print(('  ok   ' if ok else '  FAIL ') + what)
    if not ok:
        failures.append(what)


def sign(templates):
    lines = '\n'.join(json.dumps(t) for t in templates) + '\n'
    out = subprocess.run([BUN, str(HERE / 'relay_sign.ts')], input=lines, capture_output=True, text=True,
                         env={**os.environ, 'NODE_PATH': NOBLE}, check=True, timeout=60)
    return [json.loads(l) for l in out.stdout.splitlines() if l.strip()]


class Ws:
    def __init__(self, port):
        self.s = socket.create_connection(('127.0.0.1', port), timeout=5)
        key = base64.b64encode(os.urandom(16)).decode()
        self.s.sendall(('GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n'
                        'Sec-WebSocket-Key: %s\r\nSec-WebSocket-Version: 13\r\n\r\n' % key).encode())
        head = b''
        while b'\r\n\r\n' not in head:
            head += self.s.recv(4096)
        head, self.buf = head.split(b'\r\n\r\n', 1)
        want = base64.b64encode(hashlib.sha1((key + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').encode()).digest()).decode()
        self.ok = head.startswith(b'HTTP/1.1 101') and ('Sec-WebSocket-Accept: ' + want).encode() in head

    def send_frame(self, op, payload):
        mask = os.urandom(4)
        n = len(payload)
        head = bytes([0x80 | op])
        if n < 126:
            head += bytes([0x80 | n])
        elif n < 65536:
            head += bytes([0x80 | 126]) + struct.pack('>H', n)
        else:
            head += bytes([0x80 | 127]) + struct.pack('>Q', n)
        self.s.sendall(head + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(payload)))

    def send(self, msg):
        self.send_frame(1, json.dumps(msg, separators=(',', ':')).encode())

    def frame(self, timeout=3.0):
        self.s.settimeout(timeout)
        try:
            while True:
                if len(self.buf) >= 2:
                    op, n, at = self.buf[0] & 15, self.buf[1] & 127, 2
                    if n == 126 and len(self.buf) >= 4:
                        n, at = struct.unpack('>H', self.buf[2:4])[0], 4
                    elif n == 127 and len(self.buf) >= 10:
                        n, at = struct.unpack('>Q', self.buf[2:10])[0], 10
                    if n < 126 or at > 2:
                        if len(self.buf) >= at + n:
                            payload, self.buf = self.buf[at:at + n], self.buf[at + n:]
                            return op, payload
                chunk = self.s.recv(65536)
                if not chunk:
                    return None
                self.buf += chunk
        except (socket.timeout, ConnectionError):
            return None

    def recv(self, timeout=3.0):
        f = self.frame(timeout)
        if f is None or f[0] != 1:
            return f
        return json.loads(f[1])

    def until_eose(self, sub):
        events = []
        while True:
            m = self.recv()
            if m is None or m[0] == 'EOSE' or m[0] == 'CLOSED':
                return events, m
            if m[0] == 'EVENT' and m[1] == sub:
                events.append(m[2])

    def close(self):
        self.s.close()


def start(exe, log):
    p = subprocess.Popen([str(exe), '--listen', '127.0.0.1', '--port', '0'], stdout=subprocess.PIPE, stderr=log, text=True,
                         env={**os.environ, 'ASAN_OPTIONS': 'abort_on_error=0:halt_on_error=1:detect_leaks=0',
                              'UBSAN_OPTIONS': 'print_stacktrace=1:halt_on_error=1'})
    line = p.stdout.readline()
    if 'listening on port' not in line:
        raise SystemExit('relay did not start: %r' % line)
    return p, int(line.split()[-1])


def publish(ws, e):
    ws.send(['EVENT', e])
    while True:
        m = ws.recv()
        if not (isinstance(m, list) and m and m[0] == 'EVENT'):
            return m


def session(port, now):
    T = now - 100
    ev = sign([
        {"kind": 1, "created_at": T, "tags": [], "content": "first"},                                   # 0
        {"kind": 0, "created_at": T, "tags": [], "content": "{\"name\":\"v1\"}"},                      # 1
        {"kind": 0, "created_at": T + 1, "tags": [], "content": "{\"name\":\"v2\"}"},                  # 2
        {"kind": 30617, "created_at": T, "tags": [["d", "repo"]], "content": "a"},                     # 3
        {"kind": 30617, "created_at": T, "tags": [["d", "repo"]], "content": "b"},                     # 4
        {"kind": 30621, "created_at": T, "tags": [["d", "coax"], ["a", "30617:x:repo"]], "content": ""},  # 5
        {"kind": 30623, "created_at": T, "tags": [["d", "coax"]], "content": ""},                      # 6
        {"kind": 1621, "created_at": T, "tags": [["a", "30617:x:repo"], ["p", "0" * 64]], "content": "issue"},  # 7
        {"kind": 1630, "created_at": T, "tags": [["e", "1" * 64]], "content": ""},                     # 8
        {"kind": 1633, "created_at": T, "tags": [["e", "1" * 64]], "content": ""},                     # 9
        {"kind": 1111, "created_at": T, "tags": [["E", "2" * 64], ["q", "3" * 64]], "content": "c"},   # 10
        {"kind": 1, "created_at": T, "tags": [["expiration", str(now - 1)]], "content": "gone"},       # 11
        {"kind": 1, "created_at": T, "tags": [["expiration", str(now + 15)]], "content": "soon"},       # 12
        {"kind": 1, "created_at": now + 1000, "tags": [], "content": "future"},                       # 13
        {"kind": 1, "created_at": T + 5, "tags": [], "content": "live"},                               # 14
        {"kind": 1, "created_at": T + 6, "tags": [], "content": "after close"},                        # 15
        {"kind": 22242, "created_at": now, "tags": [["challenge", "x"]], "content": ""},              # 16
    ])
    a, b = Ws(port), Ws(port)
    check(a.ok and b.ok, 'two WebSocket handshakes with the right Sec-WebSocket-Accept')
    a.send(['REQ', 'all', {'kinds': [1]}])
    got, end = a.until_eose('all')
    check(got == [] and end == ['EOSE', 'all'], 'REQ on an empty relay: EOSE only')
    check(publish(b, ev[0]) == ['OK', ev[0]['id'], True, ''], 'publish a kind 1: OK true')
    m = a.recv()
    check(m == ['EVENT', 'all', ev[0]], 'the open subscription gets it live, bytes as published')
    check(publish(b, ev[0]) == ['OK', ev[0]['id'], True, 'duplicate: already have this event'], 'the same event again: duplicate')
    bad = dict(ev[14]); bad['sig'] = ('0' if bad['sig'][0] != '0' else '1') + bad['sig'][1:]
    check(publish(b, bad) == ['OK', bad['id'], False, 'invalid: bad signature'], 'a bad signature: OK false invalid')
    forged = dict(ev[14]); forged['content'] = 'changed'
    r = publish(b, forged)
    check(r == ['OK', forged['id'], False, 'invalid: event id does not match its content'], 'a changed content: id refused %r' % (r,))
    check(a.recv(0.5) is None, 'refused events are not delivered')
    # replaceable
    check(publish(b, ev[2])[2] is True and publish(b, ev[1]) == ['OK', ev[1]['id'], True, 'duplicate: newer replaceable event already stored'],
          'kind 0: an older version after a newer one is superseded')
    b.send(['REQ', 'p', {'kinds': [0]}])
    got, _ = b.until_eose('p')
    check([e['id'] for e in got] == [ev[2]['id']], 'kind 0: only the newest is served')
    # addressable, same second: lowest id wins
    hi, lo = sorted([ev[3], ev[4]], key=lambda e: e['id'], reverse=True)
    check(publish(b, hi)[2] is True and publish(b, lo) == ['OK', lo['id'], True, ''], 'addressable: the lower id at the same second replaces')
    check(publish(b, hi) == ['OK', hi['id'], True, 'duplicate: already have this event'], 'addressable: the retired id is a duplicate')
    b.send(['REQ', 'r', {'kinds': [30617], '#d': ['repo']}])
    got, _ = b.until_eose('r')
    check([e['id'] for e in got] == [lo['id']], 'addressable: one version, the lowest id')
    # the website's kinds and tag filters
    oks = [publish(b, ev[i]) for i in range(5, 11)]
    check(all(o[2] is True for o in oks), 'kinds 30621, 30623, 1621, 1630, 1633, 1111 accepted')
    for sub, flt, want in [('a', {'#a': ['30617:x:repo']}, {ev[5]['id'], ev[7]['id']}),
                           ('d', {'kinds': [30621, 30623], '#d': ['coax']}, {ev[5]['id'], ev[6]['id']}),
                           ('e', {'#e': ['1' * 64], 'kinds': [1630, 1631, 1632, 1633]}, {ev[8]['id'], ev[9]['id']}),
                           ('E', {'#E': ['2' * 64]}, {ev[10]['id']}), ('q', {'#q': ['3' * 64]}, {ev[10]['id']}),
                           ('pp', {'#p': ['0' * 64]}, {ev[7]['id']}),
                           ('au', {'authors': [ev[0]['pubkey']], 'kinds': [1621], 'limit': 5}, {ev[7]['id']}),
                           ('ids', {'ids': [ev[6]['id']]}, {ev[6]['id']}),
                           ('lim', {'kinds': [1630, 1633], 'limit': 1}, {max([ev[8], ev[9]], key=lambda e: (e['created_at'], ''.join(chr(255 - ord(c)) for c in e['id'])))['id']}),
                           ('zero', {'kinds': [1], 'limit': 0}, set()),
                           ('since', {'kinds': [1], 'since': T + 1}, set()), ('until', {'kinds': [1], 'until': T}, {ev[0]['id']})]:
        b.send(['REQ', sub, flt])
        got, end = b.until_eose(sub)
        check({e['id'] for e in got} == want and end == ['EOSE', sub], 'REQ %s %s' % (sub, json.dumps(flt)[:60]))
    # NIP-40 and created_at
    check(publish(b, ev[11]) == ['OK', ev[11]['id'], False, 'invalid: event has expired'], 'an expired event is refused')
    r = publish(b, ev[12])
    check(r[2] is True, 'an event that expires in 15 s is accepted %r' % (r,))
    check(publish(b, ev[13]) == ['OK', ev[13]['id'], False, "invalid: the event's created_at timestamp is too far in the past or the future for this relay"],
          'created_at 1000 s ahead is refused')
    # CLOSE and live delivery
    a.recv(0.3)  # the expiring kind 1 went live to 'all'
    check(publish(b, ev[14])[2] is True and a.recv() == ['EVENT', 'all', ev[14]], 'another live event')
    a.send(['CLOSE', 'all'])
    time.sleep(0.2)
    r = publish(b, ev[15]); r2 = a.recv(0.5)
    check(r[2] is True and r2 is None, 'after CLOSE nothing more arrives %r %r' % (r, r2))
    # limits
    a.send(['REQ', 'x' * 65, {}])
    check(a.recv() == ['NOTICE', 'invalid: subscription id must contain 1 to 64 characters'], '65-character sub id refused')
    a.send(['REQ', 'f17'] + [{'kinds': [1]}] * 17)
    check(a.recv() == ['CLOSED', 'f17', 'restricted: too many filters'], '17 filters: CLOSED restricted')
    a.send(['REQ', 'empty', {'kinds': []}])
    check(a.recv() == ['CLOSED', 'empty', 'invalid: filter arrays must not be empty'], 'an empty list: CLOSED invalid')
    a.send(['REQ', 'cost'] + [{}] * 8)
    check(a.recv() == ['CLOSED', 'cost', 'restricted: query cost exceeds the configured limit'], 'eight open filters: query cost')
    a.send(['REQ', 'hex', {'#e': ['abc']}])
    check(a.recv() == ['CLOSED', 'hex', 'invalid: #e values must be 64 lowercase hexadecimal characters'], '#e not hex: CLOSED')
    for i in range(32):
        a.send(['REQ', 's%d' % i, {'kinds': [7]}])
        a.until_eose('s%d' % i)
    a.send(['REQ', 's32', {'kinds': [7]}])
    check(a.recv() == ['CLOSED', 's32', 'restricted: too many active subscriptions'], 'a 33rd subscription is refused')
    a.send(['REQ', 's0', {'kinds': [8]}])
    check(a.until_eose('s0')[1] == ['EOSE', 's0'], 'reusing a sub id replaces it within the limit')
    a.send(['AUTH', ev[16]])
    check(a.recv() == ['OK', ev[16]['id'], False, 'restricted: NIP-42 is not configured on this relay'], 'AUTH: NIP-42 not configured yet')
    check(publish(a, ev[16]) == ['OK', ev[16]['id'], False, 'invalid: authentication events cannot be published'], 'kind 22242 cannot be published')
    a.send(['HELLO'])
    check(a.recv() == ['NOTICE', 'invalid: unsupported message verb'], 'unknown verb: NOTICE')
    a.send_frame(9, b'ping!')
    check(a.frame() == (10, b'ping!'), 'ping: pong with the same payload')
    a.send_frame(2, b'\x00')
    check(a.recv() == ['NOTICE', 'invalid: binary messages are not supported'], 'binary: NOTICE')
    # expiration at read time
    print('  note session reached the expiry wait %.1f s after start' % (time.time() - now))
    time.sleep(max(0.0, now + 16 - time.time()))
    b.send(['REQ', 'exp', {'ids': [ev[12]['id']]}])
    got, _ = b.until_eose('exp')
    check(got == [], 'an expired event is no longer served')
    a.send_frame(1, b'[' + b' ' * 131072 + b']')
    f = a.frame()
    check(f is not None and f[0] == 8 and f[1][:2] == b'\x03\xf1', 'a 131074-byte frame: close 1009')
    b.send_frame(8, b'\x03\xe8')
    f = b.frame()
    check(f is not None and f[0] == 8, 'close 1000 is answered')
    a.close(); b.close()


def nip11(port):
    s = socket.create_connection(('127.0.0.1', port), timeout=5)
    s.sendall(b'GET / HTTP/1.1\r\nHost: x\r\nAccept: application/nostr+json\r\n\r\n')
    data = b''
    while True:
        c = s.recv(65536)
        if not c:
            break
        data += c
    head, body = data.split(b'\r\n\r\n', 1)
    doc = json.loads(body)
    check(head.startswith(b'HTTP/1.1 200') and b'Access-Control-Allow-Origin: *' in head, 'NIP-11: 200 with CORS')
    lim = doc['limitation']
    check(lim['max_message_length'] == 131072 and lim['max_subscriptions'] == 32 and lim['max_limit'] == 127
          and lim['max_subid_length'] == 64 and doc['supported_nips'] == [1, 11, 40], 'NIP-11: the Rust relay\'s limits, NIPs 1, 11, 40')


def main():
    with tempfile.TemporaryDirectory(prefix='coax-relay-session-') as tmp:
        d = Path(tmp)
        exe = d / 'relay'
        t0 = time.monotonic()
        subprocess.run([str(ROOT / 'coax/relay/build.sh'), str(exe), '--sanitize'], check=True, stdout=subprocess.DEVNULL)
        print('build %.1f s' % (time.monotonic() - t0))
        with open(d / 'relay.err', 'w') as log:
            p, port = start(exe, log)
            try:
                nip11(port)
                session(port, int(time.time()))
            finally:
                p.terminate(); p.wait(10)
            p, port = start(exe, log)
            try:
                w = Ws(port)
                w.send(['REQ', 'after', {}])
                got, _ = w.until_eose('after')
                print('  note restart: %d events after restart (the test-only store double is not durable)' % len(got))
                w.close()
            finally:
                p.terminate(); p.wait(10)
        err = (d / 'relay.err').read_text()
        check('runtime error' not in err and 'AddressSanitizer' not in err, 'no sanitizer report')
        if err.strip():
            sys.stderr.write(err[-4000:])
    print('relay_session: ' + ('ok' if not failures else '%d failures' % len(failures)))
    return 1 if failures else 0


if __name__ == '__main__':
    sys.exit(main())
