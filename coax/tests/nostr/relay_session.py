#!/usr/bin/env python3
"""Scripted sessions against the Coax relay (R1), under ASan and UBSan.

Builds coax/relay with --sanitize, starts it on 127.0.0.1 with a free port,
signs test events with relay_sign.ts (@noble/curves, a TEST-ONLY key) and
drives it with a standard-library WebSocket client: NIP-11, publish and OK,
REQ with stored events then EOSE then live events, CLOSE, duplicates, bad
signatures, replaceable and addressable replacement with the same-second
tie, the kinds and tag filters openagents.com uses, NIP-40 expiration,
created_at limit, subscription/filter/frame limits, ping, a restart with
--store (the same events come back) and one without (empty), and
NIP-42: a challenge on every connection, AUTH answers (right, wrong
challenge, relay, time, kind, tags, signature, id), writes refused until
AUTH, kind 22242 never stored or delivered, and a new challenge after a
reconnect; NIP-77 syncs against negentropy_ref.py as the client; NIP-86
and NIP-29 against --admin and --groups, and with --store a ban and a group
surviving a restart (FILE.nip86 and the rebuilt groups), a torn last
FILE.nip86 frame cut and a bad one refused. Tooling only; nothing here
ships. Never points at a public relay.

With --tls every session above runs over wss:// (and NIP-11 over https://)
against the relay started with --tls-cert/--tls-key, with a one-day test CA
and leaf that openssl generates into the temp dir (none is committed); NIP-42
answers then name wss://127.0.0.1:PORT. It adds the TLS cases: the key file's
mode, a mismatched key and a ws:// --url refused at start; a client that
trusts another CA, or expects another name, refuses the relay; a plaintext
client on the TLS port is closed without an answer while others are served; a
stalled handshake does not hold up another client and is closed after the
host's handshake limit; and with --client EXE (coax/tests/net/tls.sh builds
relay_tls_client.coax) the Coax client transport relay_conn talks to it, and
refuses it when it trusts another CA.

  python3 coax/tests/nostr/relay_session.py      # COAX=..., BUN=..., NOBLE=... override
  python3 coax/tests/nostr/relay_session.py --tls [--client EXE]
"""
import base64
import hashlib
import json
import os
import shutil
import socket
import ssl
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
SCHEME = 'ws'       # 'wss' with --tls
TLS_CTX = None      # the client context that trusts the test CA, with --tls
TLS_ARGS = []       # --tls-cert/--tls-key for start()


def check(ok, what):
    print(('  ok   ' if ok else '  FAIL ') + what)
    if not ok:
        failures.append(what)


def sign(templates, label=None):
    lines = '\n'.join(json.dumps(t) for t in templates) + '\n'
    env = {**os.environ, 'NODE_PATH': NOBLE}
    if label:
        env['RELAY_TEST_KEY_LABEL'] = label
    out = subprocess.run([BUN, str(HERE / 'relay_sign.ts')], input=lines, capture_output=True, text=True,
                         env=env, check=True, timeout=60)
    return [json.loads(l) for l in out.stdout.splitlines() if l.strip()]


def connect(port, ctx=None, name='localhost'):
    s = socket.create_connection(('127.0.0.1', port), timeout=5)
    ctx = ctx or TLS_CTX
    return ctx.wrap_socket(s, server_hostname=name) if ctx else s


class Ws:
    def __init__(self, port):
        self.s = connect(port)
        key = base64.b64encode(os.urandom(16)).decode()
        self.s.sendall(('GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n'
                        'Sec-WebSocket-Key: %s\r\nSec-WebSocket-Version: 13\r\n\r\n' % key).encode())
        head = b''
        while b'\r\n\r\n' not in head:
            head += self.s.recv(4096)
        head, self.buf = head.split(b'\r\n\r\n', 1)
        want = base64.b64encode(hashlib.sha1((key + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').encode()).digest()).decode()
        self.ok = head.startswith(b'HTTP/1.1 101') and ('Sec-WebSocket-Accept: ' + want).encode() in head
        self.port = port
        # NIP-42: the relay's challenge comes first on every connection
        m = self.recv() if self.ok else None
        self.challenge = m[1] if isinstance(m, list) and len(m) == 2 and m[0] == 'AUTH' else None

    def auth_event(self, **change):
        t = {"kind": 22242, "created_at": int(time.time()), "content": "",
             "tags": [["relay", "%s://127.0.0.1:%d" % (SCHEME, self.port)], ["challenge", self.challenge]]}
        label = change.pop('label', None)
        t.update(change)
        return sign([t], label)[0]

    def auth(self, e=None):
        e = e or self.auth_event()
        self.send(['AUTH', e])
        return self.recv()

    def send_frame(self, op, payload, cut=None):
        mask = os.urandom(4)
        n = len(payload)
        head = bytes([0x80 | op])
        if n < 126:
            head += bytes([0x80 | n])
        elif n < 65536:
            head += bytes([0x80 | 126]) + struct.pack('>H', n)
        else:
            head += bytes([0x80 | 127]) + struct.pack('>Q', n)
        data = head + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(payload))
        self.s.sendall(data if cut is None else data[:cut])

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
        except (socket.timeout, ConnectionError, ssl.SSLError):
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


STARTED = []        # the lines the last start() read before 'listening'


def start(exe, log, store=None, rate='1000,1000,1000', extra=()):
    args = [str(exe), '--listen', '127.0.0.1', '--port', '0'] + (['--rate', rate] if rate else []) + (['--store', str(store)] if store else []) + list(extra) + TLS_ARGS
    p = subprocess.Popen(args, stdout=subprocess.PIPE, stderr=log, text=True,
                         env={**os.environ, 'ASAN_OPTIONS': 'abort_on_error=0:halt_on_error=1:detect_leaks=0',
                              'UBSAN_OPTIONS': 'print_stacktrace=1:halt_on_error=1'})
    STARTED.clear()
    line = p.stdout.readline()
    while store and line.startswith('relay: ') and 'listening on port' not in line:
        STARTED.append(line.strip())
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
    check(bool(a.challenge and b.challenge) and len(a.challenge) == 64 and a.challenge != b.challenge
          and all(c in '0123456789abcdef' for c in a.challenge), 'NIP-42: each connection gets its own 64-hex challenge first')
    nip42(port, a, b, ev[0])
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
    # signed now, so a loaded machine cannot eat the window
    soon_at = int(time.time()) + 8
    ev[12] = sign([{"kind": 1, "created_at": T, "tags": [["expiration", str(soon_at)]], "content": "soon"}])[0]
    r = publish(b, ev[12])
    check(r[2] is True, 'an event that expires in 8 s is accepted %r' % (r,))
    b.send(['REQ', 'soon', {'ids': [ev[12]['id']]}])
    got, _ = b.until_eose('soon')
    check([e['id'] for e in got] == [ev[12]['id']], 'and served before it expires')
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
    check(a.recv() == ['OK', ev[16]['id'], False, 'invalid: authentication event must have exactly one relay tag and one challenge tag'],
          'AUTH without a relay tag: refused')
    check(publish(a, ev[16]) == ['OK', ev[16]['id'], False, 'invalid: authentication events cannot be published'], 'kind 22242 cannot be published')
    a.send(['HELLO'])
    check(a.recv() == ['NOTICE', 'invalid: unsupported message verb'], 'unknown verb: NOTICE')
    a.send_frame(9, b'ping!')
    check(a.frame() == (10, b'ping!'), 'ping: pong with the same payload')
    a.send_frame(2, b'\x00')
    check(a.recv() == ['NOTICE', 'invalid: binary messages are not supported'], 'binary: NOTICE')
    # expiration at read time
    print('  note session reached the expiry wait %.1f s after start' % (time.time() - now))
    time.sleep(max(0.0, soon_at + 1 - time.time()))
    b.send(['REQ', 'exp', {'ids': [ev[12]['id']]}])
    got, _ = b.until_eose('exp')
    check(got == [], 'an expired event is no longer served')
    # Only the header and 1 KiB: the relay refuses on the header, and writing
    # the rest after its close would race the reset (EPIPE, seen over TLS).
    a.send_frame(1, b'[' + b' ' * 131072 + b']', cut=1024)
    f = a.frame()
    check(f is not None and f[0] == 8 and f[1][:2] == b'\x03\xf1', 'a 131074-byte frame: close 1009 on its header')
    b.send_frame(8, b'\x03\xe8')
    f = b.frame()
    check(f is not None and f[0] == 8, 'close 1000 is answered')
    a.close(); b.close()


def nip42(port, a, b, first):
    """NIP-42 on connections a (stays anonymous) and b (authenticates)."""
    check(publish(b, first) == ['OK', first['id'], False, 'auth-required: authenticate before publishing'],
          'NIP-42: a write before AUTH gets auth-required')
    a.send(['REQ', 'auth', {'kinds': [22242]}])
    check(a.until_eose('auth') == ([], ['EOSE', 'auth']), 'NIP-42: reading needs no AUTH')
    now = int(time.time())
    url = "%s://127.0.0.1:%d" % (SCHEME, port)
    for what, e, text in [
            ('a wrong challenge', b.auth_event(tags=[["relay", url], ["challenge", a.challenge]]),
             'authentication challenge does not match'),
            ('another relay', b.auth_event(tags=[["relay", "%s://127.0.0.1:%d" % (SCHEME, port + 1)], ["challenge", b.challenge]]),
             'authentication relay URL does not match'),
            ('created 601 s ago', b.auth_event(created_at=now - 601),
             "the authentication event's timestamp is more than 10 minutes from the relay's clock"),
            ('kind 1', b.auth_event(kind=1), 'authentication event must have kind 22242'),
            ('two challenge tags', b.auth_event(tags=[["relay", url], ["challenge", b.challenge], ["challenge", b.challenge]]),
             'authentication event must have exactly one relay tag and one challenge tag')]:
        check(b.auth(e) == ['OK', e['id'], False, 'invalid: ' + text], 'NIP-42: AUTH with %s is refused' % what)
    good = b.auth_event(tags=[["relay", "%s://127.0.0.1:%d/" % (SCHEME.upper(), port)], ["challenge", b.challenge]])
    bad = dict(good); bad['sig'] = ('0' if bad['sig'][0] != '0' else '1') + bad['sig'][1:]
    check(b.auth(bad) == ['OK', bad['id'], False, 'invalid: bad signature'], 'NIP-42: AUTH with a bad signature is refused')
    forged = dict(good); forged['content'] = 'x'
    check(b.auth(forged) == ['OK', forged['id'], False, 'invalid: event id does not match its content'], 'NIP-42: AUTH with a wrong id is refused')
    check(publish(b, first)[2] is False, 'NIP-42: refused AUTH answers authenticate nothing')
    check(b.auth(good) == ['OK', good['id'], True, ''], 'NIP-42: the right answer (relay URL in another case, with a slash) gets OK true')
    check(a.recv(0.3) is None, 'NIP-42: the AUTH event is not delivered to a kind 22242 subscription')
    check(b.auth(good) == ['OK', good['id'], True, ''], 'NIP-42: the same key again is still OK true')
    other = b.auth_event(label='coax relay r1 second test key')
    check(b.auth(other) == ['OK', other['id'], True, ''] and other['pubkey'] != good['pubkey'], 'NIP-42: a second key on one connection')
    a.send(['CLOSE', 'auth'])
    b.send(['REQ', 'auth2', {'kinds': [22242]}])
    check(b.until_eose('auth2') == ([], ['EOSE', 'auth2']), 'NIP-42: kind 22242 is never stored')
    # a reconnect gets a new challenge; the old answer and the old key do not carry over
    c = Ws(port)
    check(bool(c.challenge) and c.challenge not in (a.challenge, b.challenge), 'NIP-42: a new connection gets a new challenge')
    check(c.auth(good) == ['OK', good['id'], False, 'invalid: authentication challenge does not match'],
          "NIP-42: an answer to another connection's challenge is refused")
    check(publish(c, first)[3] == 'auth-required: authenticate before publishing', 'NIP-42: a new connection is anonymous until it answers')
    c.close()


def deletion(port):
    """NIP-09 and NIP-62 (store_delete.coax): what a stored kind 5 or kind 62
    covers is retired and can't come back. Returns an event that stays
    deleted, for the check after a restart."""
    now = int(time.time())
    w = Ws(port)
    check(w.auth()[2] is True, 'NIP-09: AUTH for the deletion session')
    ev = sign([{"kind": 1, "created_at": now - 50, "tags": [], "content": "to be deleted"},          # 0
               {"kind": 30023, "created_at": now - 50, "tags": [["d", "post"]], "content": "v1"},   # 1
               {"kind": 1, "created_at": now - 50, "tags": [], "content": "kept"}])                 # 2
    for e in ev:
        check(publish(w, e)[2] is True, 'NIP-09: publish %s' % e['content'])
    pk = ev[0]['pubkey']
    req = sign([{"kind": 5, "created_at": now - 40, "content": "",
                 "tags": [["e", ev[0]['id']], ["a", "30023:%s:post" % pk], ["e", "9" * 64]]}])[0]
    check(publish(w, req) == ['OK', req['id'], True, ''], 'NIP-09: a kind 5 naming an e, an a and an unknown id is stored')
    w.send(['REQ', 'gone', {'ids': [ev[0]['id'], ev[1]['id'], ev[2]['id']]}])
    got, _ = w.until_eose('gone')
    check([e['id'] for e in got] == [ev[2]['id']], 'NIP-09: the e and a targets are no longer served; the other note is')
    w.send(['COUNT', 'n', {'ids': [ev[0]['id'], ev[1]['id'], ev[2]['id']]}, {'ids': [ev[2]['id']], 'limit': 0}])
    check(w.recv() == ['COUNT', 'n', {'count': 1}], 'NIP-45: COUNT over two filters counts the one visible match once')
    w.send(['COUNT', 'n2', {'kinds': [5], 'authors': [pk]}])
    check(w.recv() == ['COUNT', 'n2', {'count': 1}], 'NIP-45: COUNT of the deletion requests')
    w.send(['COUNT', 'n3', {'kinds': []}])
    check(w.recv() == ['CLOSED', 'n3', 'invalid: filter arrays must not be empty'], 'NIP-45: a bad filter is CLOSED')
    w.send(['REQ', 'k5', {'kinds': [5], 'authors': [pk]}])
    got, _ = w.until_eose('k5')
    check([e['id'] for e in got] == [req['id']], 'NIP-09: the deletion request itself is served')
    check(publish(w, ev[0]) == ['OK', ev[0]['id'], False, 'blocked: a deletion request covers this event'],
          'NIP-09: the deleted event published again is refused')
    newer = sign([{"kind": 30023, "created_at": now - 30, "tags": [["d", "post"]], "content": "v2"}])[0]
    check(publish(w, newer)[2] is True, 'NIP-09: a version of the address newer than the request is accepted')
    # NIP-62 from a second key: every one of its events up to the request goes
    other = sign([{"kind": 1, "created_at": now - 50, "tags": [], "content": "second key"}], 'coax relay r1 second test key')[0]
    check(publish(w, other)[2] is True, 'NIP-62: an event by a second key')
    url = '%s://127.0.0.1:%d' % (SCHEME, port)
    wrong = sign([{"kind": 62, "created_at": now - 45, "tags": [["relay", url + '0']], "content": ""}], 'coax relay r1 second test key')[0]
    check(publish(w, wrong)[2] is True, 'NIP-62: a request naming another relay is stored')
    w.send(['REQ', 'v0', {'ids': [other['id']]}])
    check(len(w.until_eose('v0')[0]) == 1, 'NIP-62: and changes nothing here')
    # ALL_RELAYS, not this relay's URL: the restart below listens on another
    # port (the URL match is relay_store.coax's check)
    vanish = sign([{"kind": 62, "created_at": now - 40, "tags": [["relay", "ALL_RELAYS"]], "content": ""}], 'coax relay r1 second test key')[0]
    check(publish(w, vanish)[2] is True, 'NIP-62: a request naming ALL_RELAYS is stored')
    w.send(['REQ', 'v1', {'authors': [other['pubkey']]}])
    got, _ = w.until_eose('v1')
    check(sorted(e['kind'] for e in got) == [62, 62], 'NIP-62: only the requests remain from that key')
    check(publish(w, other)[3] == 'blocked: a deletion request covers this event', 'NIP-62: its old event is refused')
    w.close()
    return ev[0], ev[2]


def rate_limits(exe, log):
    """relay_rate.coax: --rate 3,2,2 on a fresh relay, and the defaults."""
    refused_start(exe, ['--rate', '0,1,1'], 'a zero rate limit', 'three limits')
    refused_start(exe, ['--rate', '1,2'], 'two rate limits', 'EVENTS,REQS,KEY_EVENTS')
    p, port = start(exe, log, rate='3,2,2')
    try:
        doc = info(port)
        lim = doc['limitation']
        check(lim['events_per_minute'] == 3 and lim['reqs_per_minute'] == 2 and lim['events_per_minute_per_key'] == 2,
              'rate: NIP-11 carries the configured limits')
        now = int(time.time())
        ev = sign([{"kind": 1, "created_at": now - 10 + i, "tags": [], "content": "r%d" % i} for i in range(3)])
        other = sign([{"kind": 1, "created_at": now - 10, "tags": [], "content": "other key"}], 'coax relay r1 second test key')[0]
        w = Ws(port)
        check(w.auth()[2] is True, 'rate: AUTH')
        check(publish(w, ev[0])[2] is True and publish(w, ev[1])[2] is True, 'rate: two EVENTs from one key are accepted')
        check(publish(w, ev[2]) == ['OK', ev[2]['id'], False, 'rate-limited: event rate exceeded for this key'],
              'rate: the third from that key in a minute is refused')
        check(publish(w, other) == ['OK', other['id'], False, 'rate-limited: event rate exceeded'], 'rate: the connection has used its 3 EVENTs')
        w2 = Ws(port)
        check(w2.auth()[2] is True and publish(w2, other)[2] is True, 'rate: another connection, another key: accepted')
        check(publish(w2, ev[2])[3] == 'rate-limited: event rate exceeded for this key', 'rate: the key window holds across connections')
        for i in range(2):
            w2.send(['REQ', 'q%d' % i, {'kinds': [1]}])
            w2.until_eose('q%d' % i)
        w2.send(['REQ', 'q2', {'kinds': [1]}])
        check(w2.recv() == ['CLOSED', 'q2', 'rate-limited: REQ rate exceeded'], 'rate: the third REQ in a minute is CLOSED')
        w.close(); w2.close()
    finally:
        p.terminate(); p.wait(10)
    p, port = start(exe, log, rate=None)
    try:
        lim = info(port)['limitation']
        check(lim['events_per_minute'] == 120 and lim['reqs_per_minute'] == 120 and lim['events_per_minute_per_key'] == 60,
              "rate: without --rate, the Rust relay's 120, 120 and 60")
    finally:
        p.terminate(); p.wait(10)


def protected(port):
    """NIP-70 (nip70.coax): a ["-"] event only from a connection authenticated as its author."""
    now = int(time.time())
    mine = sign([{"kind": 1, "created_at": now - 5, "tags": [["-"]], "content": "protected"}])[0]
    theirs = sign([{"kind": 1, "created_at": now - 5, "tags": [["-"]], "content": "protected 2"}], 'coax relay r1 second test key')[0]
    w = Ws(port)
    check(w.auth()[2] is True, 'NIP-70: AUTH as the first key')
    check(publish(w, theirs) == ['OK', theirs['id'], False, 'restricted: this event may only be published by its author'],
          "NIP-70: another key's protected event is refused")
    check(publish(w, mine) == ['OK', mine['id'], True, ''], 'NIP-70: its own protected event is accepted')
    check(w.auth(w.auth_event(label='coax relay r1 second test key'))[2] is True and publish(w, theirs)[2] is True,
          'NIP-70: after AUTH as the second key too, its protected event is accepted')
    w.close()


def search(port):
    """NIP-50 (nip50.coax via relay_store.coax): stored matches best first, live ones by term, never kind 4."""
    now = int(time.time())
    ev = sign([{"kind": 1, "created_at": now - 30, "tags": [], "content": "zyxwv search once"},
               {"kind": 1, "created_at": now - 31, "tags": [], "content": "Zyxwv search: zyxwv twice"},
               {"kind": 4, "created_at": now - 29, "tags": [], "content": "zyxwv ciphertext"},
               {"kind": 1, "created_at": now - 28, "tags": [], "content": "zyxwvu is another word"}])
    w = Ws(port)
    check(w.auth()[2] is True and all(publish(w, e)[2] is True for e in ev), 'NIP-50: four events stored')
    w.send(['REQ', 'q', {'search': 'zyxwv'}])
    got, end = w.until_eose('q')
    check(end == ['EOSE', 'q'] and [e['id'] for e in got] == [ev[1]['id'], ev[0]['id']],
          'NIP-50: two whole-word matches, the one with two first; kind 4 and "zyxwvu" left out')
    w.send(['REQ', 'q2', {'search': 'ZYXWV once'}, {'search': 'zyxwv', 'limit': 1}])
    got, _ = w.until_eose('q2')
    check([e['id'] for e in got] == [ev[0]['id'], ev[1]['id']], 'NIP-50: every term must match; the limit keeps the best')
    w.send(['REQ', 'bad', {'search': 7}])
    check(w.recv() == ['CLOSED', 'bad', 'invalid: malformed filter'], 'NIP-50: a search that is not a string is CLOSED')
    w.send(['REQ', 'live', {'search': 'qwerty live'}])
    check(w.until_eose('live')[0] == [], 'NIP-50: a live search subscription opens empty')
    w2 = Ws(port)
    check(w2.auth()[2] is True, 'NIP-50: a second connection authenticates')
    later = sign([{"kind": 1, "created_at": now - 5, "tags": [], "content": "qwerty only"},
                  {"kind": 4, "created_at": now - 4, "tags": [], "content": "qwerty live"},
                  {"kind": 1, "created_at": now - 3, "tags": [], "content": "Live: QWERTY"}])
    for e in later:
        check(publish(w2, e)[2] is True, 'NIP-50: a later event is stored')
    check(w.recv() == ['EVENT', 'live', later[2]], 'NIP-50: only the kind 1 event with both terms is delivered live')
    w.send(['CLOSE', 'live'])
    w.close(); w2.close()


def negentropy(port):
    """NIP-77 (nip77.coax over relay_store.coax's rd_neg_items): a sync names what each side lacks; limits answer NEG-ERR."""
    sys.path.insert(0, str(HERE))
    import negentropy_ref as nr
    now = int(time.time())
    ev = sign([{"kind": 9999, "created_at": now - 200 + i, "tags": [], "content": "negentropy %d" % i} for i in range(40)])
    w = Ws(port)
    check(w.auth()[2] is True and all(publish(w, e)[2] is True for e in ev), 'NIP-77: 40 kind 9999 events stored')
    fakes = [(now - 150 + i, hashlib.sha256(b'not on the relay %d' % i).digest()) for i in range(5)]
    mine = [(e['created_at'], bytes.fromhex(e['id'])) for e in ev[:30]] + fakes
    c = nr.Side(mine, True)
    w.send(['NEG-OPEN', 'n1', {'kinds': [9999]}, c.initiate().hex()])
    have, need, rounds = [], [], 0
    while True:
        m = w.recv()
        if not (isinstance(m, list) and m[:2] == ['NEG-MSG', 'n1']):
            check(False, 'NIP-77: the relay answers NEG-MSG (got %r)' % (m,))
            break
        rounds += 1
        nxt, h, n = c.reconcile(bytes.fromhex(m[2]))
        have += h
        need += n
        if nxt is None:
            break
        w.send(['NEG-MSG', 'n1', nxt.hex()])
    check(sorted(have) == sorted(i for _, i in fakes) and sorted(need) == sorted(bytes.fromhex(e['id']) for e in ev[30:]),
          'NIP-77: after %d rounds the client has the 5 the relay lacks and needs the 10 it lacks' % rounds)
    w.send(['NEG-CLOSE', 'n1'])
    w.send(['NEG-MSG', 'n1', '61'])
    check(w.recv() == ['NEG-ERR', 'n1', 'closed: no sync is open with this id'], 'NIP-77: NEG-MSG after NEG-CLOSE is NEG-ERR closed')
    # an equal set: the relay's first answer settles it
    c = nr.Side([(e['created_at'], bytes.fromhex(e['id'])) for e in ev], True)
    w.send(['NEG-OPEN', 'eq', {'kinds': [9999]}, c.initiate().hex()])
    m = w.recv()
    check(m[:2] == ['NEG-MSG', 'eq'] and c.reconcile(bytes.fromhex(m[2])) == (None, [], []), 'NIP-77: the same 40 events agree in one round')
    w.send(['NEG-CLOSE', 'eq'])
    # at most 4 syncs on a connection; the same id again replaces its sync
    for sub in ['s2', 's3', 's4', 's5', 's2']:
        w.send(['NEG-OPEN', sub, {'kinds': [9999]}, nr.Side([], True).initiate().hex()])
        check(w.recv()[:2] == ['NEG-MSG', sub], 'NIP-77: sync %s opened' % sub)
    w.send(['NEG-OPEN', 's6', {'kinds': [9999]}, '61'])
    check(w.recv() == ['NEG-ERR', 's6', 'blocked: 4 syncs are already open on this connection'], 'NIP-77: a fifth sync is NEG-ERR blocked')
    w2 = Ws(port)
    w2.send(['NEG-OPEN', 's6', {'kinds': [9999]}, '61'])
    check(w2.recv()[:2] == ['NEG-MSG', 's6'], 'NIP-77: the limit is per connection')
    w.send(['NEG-CLOSE', 's3'])
    w.send(['NEG-OPEN', 's6', {'kinds': [9999]}, '61'])
    check(w.recv()[:2] == ['NEG-MSG', 's6'], 'NIP-77: after NEG-CLOSE a slot is free again')
    w.send(['NEG-CLOSE', 's6'])
    # refusals
    w.send(['NEG-OPEN', 'bad', {'search': 'x'}, '61'])
    check(w.recv() == ['NEG-ERR', 'bad', 'invalid: malformed filter'], 'NIP-77: a search filter is NEG-ERR invalid')
    w.send(['NEG-MSG', 's2', '6g'])
    check(w.recv() == ['NEG-ERR', 's2', 'invalid: the NEG frame does not read'], 'NIP-77: bad hex is NEG-ERR invalid')
    w.send(['NEG-MSG', 's2', '61'])
    check(w.recv() == ['NEG-ERR', 's2', 'closed: no sync is open with this id'], 'NIP-77: and that sync is gone')
    w.send(['NEG-MSG', 's4', '00'])
    check(w.recv() == ['NEG-ERR', 's4', 'invalid: the Negentropy message does not read'], 'NIP-77: a message that is not Negentropy is NEG-ERR')
    w.send(['NEG-MSG', 's5', '62'])
    check(w.recv() == ['NEG-MSG', 's5', '61'], 'NIP-77: version 2 is answered with the version the relay speaks')
    # 32768 bytes are read (and refused as Negentropy: ranges after infinity)
    w.send(['NEG-MSG', 's5', '61' + '00' * 32767])
    check(w.recv() == ['NEG-ERR', 's5', 'invalid: the Negentropy message does not read'], 'NIP-77: a message of 32768 bytes is read')
    w.send(['NEG-MSG', 's5', '61' + '00' * 32768])
    check(w.recv() == ['NOTICE', 'invalid: malformed message'], 'NIP-77: one of 32769 bytes is longer than a JSON string the relay reads: NOTICE')
    w.send(['NEG-ERR', 'x', 'blocked: no'])
    check(w.recv() == ['NOTICE', 'invalid: malformed message'], 'NIP-77: a NEG-ERR from a client is a NOTICE')
    w.send(['REQ', 'still', {'kinds': [9999], 'limit': 1}])
    check(len(w.until_eose('still')[0]) == 1, 'NIP-77: the connection still serves REQ')
    w.close(); w2.close()


def post(port, body, auth, ctype='application/nostr+json+rpc', head=None):
    """One HTTP POST to the relay; (status, parsed JSON or raw body)."""
    s = connect(port)
    data = body.encode() if isinstance(body, str) else body
    h = head if head is not None else ('POST / HTTP/1.1\r\nHost: x\r\nContent-Type: %s\r\nAuthorization: %s\r\nContent-Length: %d\r\n\r\n'
                                      % (ctype, auth, len(data)))
    s.sendall(h.encode() + data)
    got = b''
    while True:
        c = s.recv(65536)
        if not c:
            break
        got += c
    s.close()
    head_, _, rest = got.partition(b'\r\n\r\n')
    code = int(head_.split(b' ')[1]) if head_ else 0
    try:
        return code, json.loads(rest)
    except ValueError:
        return code, rest


def nip98(port, body, label=None, url=None, method='POST', payload=True, at=None):
    """A NIP-98 Authorization header for a POST of body to the relay's URL."""
    tags = [["u", url or '%s://127.0.0.1:%d' % (SCHEME, port)], ["method", method]]
    if payload:
        tags.append(["payload", hashlib.sha256(body.encode()).hexdigest()])
    e = sign([{"kind": 27235, "created_at": at or int(time.time()), "tags": tags, "content": ""}], label)[0]
    return 'Nostr ' + base64.b64encode(json.dumps(e, separators=(',', ':')).encode()).decode()


def admin(exe, log):
    """NIP-86 (nip86.coax through relay.coax's ry_admin): --admin KEY, NIP-98 auth, lists that admission consults."""
    refused_start(exe, ['--admin', 'abc'], 'a short --admin key', '64-digit hex')
    refused_start(exe, ['--admin', 'g' * 64], 'a non-hex --admin key', '64-digit hex')
    me = sign([{"kind": 1, "created_at": 1, "tags": [], "content": ""}])[0]['pubkey']
    other_key = 'coax relay r1 second test key'
    other = sign([{"kind": 1, "created_at": 1, "tags": [], "content": ""}], other_key)[0]['pubkey']
    p, port = start(exe, log)
    try:
        code, _ = post(port, '{"method":"supportedmethods","params":[]}', 'Nostr x')
        check(code == 400 and 86 not in info(port)['supported_nips'], 'NIP-86: without --admin a POST is a 400 and NIP-11 lists no 86')
    finally:
        p.terminate(); p.wait(10)
    p = subprocess.Popen([str(exe), '--listen', '127.0.0.1', '--port', '0', '--rate', '1000,1000,1000', '--admin', me] + TLS_ARGS,
                         stdout=subprocess.PIPE, stderr=log, text=True, env={**os.environ, 'ASAN_OPTIONS': 'detect_leaks=0'})
    try:
        port = int(p.stdout.readline().split()[-1])
        check(info(port)['supported_nips'] == [1, 9, 11, 40, 42, 45, 50, 62, 70, 77, 86], 'NIP-86: with --admin NIP-11 lists 86')
        body = '{"method":"supportedmethods","params":[]}'
        code, r = post(port, body, nip98(port, body))
        check(code == 200 and 'banpubkey' in r['result'] and 'supportedmethods' not in r['result'], 'NIP-86: supportedmethods from the admin key')
        code, r = post(port, body, nip98(port, body, label=other_key))
        check(code == 403 and r['error'] == 'not an admin key', 'NIP-86: another key is 403')
        code, r = post(port, body, nip98(port, body, url='ws://elsewhere:1'))
        check(code == 401 and r['error'].startswith('unauthorized: '), 'NIP-86: a NIP-98 event for another URL is 401 (%r)' % (r,))
        code, r = post(port, body, nip98(port, body, payload=False))
        check(code == 401 and r['error'] == 'the NIP-98 event has no payload tag', 'NIP-86: no payload tag is 401')
        code, r = post(port, body, nip98(port, body), ctype='application/json')
        check(code == 415, 'NIP-86: another Content-Type is 415')
        auth = nip98(port, body)
        check(post(port, body, auth)[0] == 200 and post(port, body, auth) == (401, {'result': None, 'error': 'event id already used'}),
              'NIP-86: a NIP-98 event is good once')
        code, _ = post(port, 'x' * 4097, 'Nostr x')
        check(code == 413, 'NIP-86: a body over 4096 bytes is 413')
        code, _ = post(port, '', '', head='POST / HTTP/1.1\r\nContent-Length: 1x\r\n\r\n')
        check(code == 400, 'NIP-86: a bad Content-Length is 400')
        now = int(time.time())
        theirs = sign([{"kind": 1, "created_at": now - 5, "tags": [], "content": "before the ban"},
                       {"kind": 1, "created_at": now - 4, "tags": [], "content": "after the ban"}], other_key)
        mine = sign([{"kind": 1, "created_at": now - 5, "tags": [], "content": "mine"},
                     {"kind": 7, "created_at": now - 5, "tags": [], "content": "+"}])
        w = Ws(port)
        check(w.auth()[2] is True and publish(w, theirs[0])[2] is True, 'NIP-86: the other key publishes before any ban')
        ban = '{"method":"banpubkey","params":["%s","spam"]}' % other
        check(post(port, ban, nip98(port, ban)) == (200, {'result': True}), 'NIP-86: banpubkey')
        check(publish(w, theirs[1]) == ['OK', theirs[1]['id'], False, 'blocked: this key is banned on this relay'], 'NIP-86: a banned key is refused')
        lst = '{"method":"listbannedpubkeys","params":[]}'
        check(post(port, lst, nip98(port, lst)) == (200, {'result': [{'pubkey': other, 'reason': 'spam'}]}), 'NIP-86: listbannedpubkeys')
        dis = '{"method":"disallowkind","params":[7]}'
        check(post(port, dis, nip98(port, dis))[0] == 200 and publish(w, mine[1])[3] == 'blocked: this relay does not accept this kind'
              and publish(w, mine[0])[2] is True, 'NIP-86: a disallowed kind is refused, others pass')
        unban = '{"method":"unbanpubkey","params":["%s"]}' % other
        check(post(port, unban, nip98(port, unban))[0] == 200 and publish(w, theirs[1])[2] is True, 'NIP-86: after unbanpubkey the key publishes again')
        allow = '{"method":"allowpubkey","params":["%s"]}' % other
        later = sign([{"kind": 1, "created_at": now - 3, "tags": [], "content": "allow list"}])[0]
        check(post(port, allow, nip98(port, allow))[0] == 200 and publish(w, later) == ['OK', later['id'], False, "restricted: this key is not on this relay's allow list"],
              'NIP-86: with an allow list only its keys publish')
        bad = '{"method":"nosuchmethod","params":[]}'
        check(post(port, bad, nip98(port, bad)) == (400, {'result': None, 'error': 'unknown method'}), 'NIP-86: an unknown method is 400')
        w.close()
    finally:
        p.terminate(); p.wait(10)


def groups(exe, log):
    """NIP-29 (nip29.coax through relay.coax's ry_event): --groups, a group created, joined, posted to and refused."""
    p = subprocess.Popen([str(exe), '--listen', '127.0.0.1', '--port', '0', '--rate', '1000,1000,1000', '--groups'] + TLS_ARGS,
                         stdout=subprocess.PIPE, stderr=log, text=True, env={**os.environ, 'ASAN_OPTIONS': 'detect_leaks=0'})
    try:
        port = int(p.stdout.readline().split()[-1])
        check(info(port)['supported_nips'] == [1, 9, 11, 29, 40, 42, 45, 50, 62, 70, 77], 'NIP-29: with --groups NIP-11 lists 29')
        now = int(time.time())
        b_key = 'coax relay r1 second test key'
        bk = sign([{"kind": 1, "created_at": 1, "tags": [], "content": ""}], b_key)[0]['pubkey']
        a = sign([{"kind": 9007, "created_at": now - 9, "tags": [["h", "g1"]], "content": ""},
                  {"kind": 9, "created_at": now - 8, "tags": [["h", "g1"]], "content": "hello group"},
                  {"kind": 9007, "created_at": now - 7, "tags": [["h", "g1"]], "content": "again"},
                  {"kind": 39000, "created_at": now - 7, "tags": [["d", "g1"]], "content": ""},
                  {"kind": 9, "created_at": now - 7, "tags": [["h", "nope"]], "content": "nowhere"},
                  {"kind": 1, "created_at": now - 7, "tags": [], "content": "no group at all"}])
        b = sign([{"kind": 9021, "created_at": now - 6, "tags": [["h", "g1"]], "content": ""},
                  {"kind": 9021, "created_at": now - 5, "tags": [["h", "g1"]], "content": "twice"},
                  {"kind": 9000, "created_at": now - 5, "tags": [["h", "g1"], ["p", bk, "admin"]], "content": ""},
                  {"kind": 9, "created_at": now - 4, "tags": [["h", "g1"]], "content": "a member posts"}], b_key)
        w = Ws(port)
        check(w.auth()[2] is True, 'NIP-29: AUTH')
        check(publish(w, a[0]) == ['OK', a[0]['id'], True, ''], 'NIP-29: a create-group is stored')
        check(publish(w, a[1])[2] is True, 'NIP-29: its creator posts to it')
        check(publish(w, a[2])[3] == 'duplicate: the group already exists', 'NIP-29: a second create of g1 is refused')
        check(publish(w, a[3])[3] == 'restricted: group metadata is signed only by the relay', 'NIP-29: a 39000 not from the relay is refused')
        check(publish(w, a[4])[3] == 'invalid: no such group', 'NIP-29: a post to a group that does not exist is refused')
        check(publish(w, a[5])[2] is True, 'NIP-29: an event without an h tag is not the groups\' business')
        check(publish(w, b[0])[2] is True, 'NIP-29: another key joins the open group')
        check(publish(w, b[1])[3] == 'duplicate: already a member', 'NIP-29: and cannot join twice')
        check(publish(w, b[2])[3] == "restricted: your roles in this group don't allow this", 'NIP-29: a plain member cannot put a user')
        check(publish(w, b[3])[2] is True, 'NIP-29: the new member posts')
        w.close()
    finally:
        p.terminate(); p.wait(10)


def persistence(exe, log, d):
    """With --store, NIP-86 changes (FILE.nip86) and NIP-29 groups (rebuilt from the stored events) survive a restart; a torn
    last journal record is cut, and other journal damage refuses to start."""
    store = d / 'persist.log'
    journal = d / 'persist.log.nip86'
    me = sign([{"kind": 1, "created_at": 1, "tags": [], "content": ""}])[0]['pubkey']
    other_key = 'coax relay r1 second test key'
    third_key = 'coax relay r1 third test key'
    other = sign([{"kind": 1, "created_at": 1, "tags": [], "content": ""}], other_key)[0]['pubkey']
    extra = ['--admin', me, '--groups']
    now = int(time.time())
    mine = sign([{"kind": 9007, "created_at": now - 20, "tags": [["h", "kept"]], "content": ""},
                 {"kind": 9, "created_at": now - 19, "tags": [["h", "kept"]], "content": "before the restart"},
                 {"kind": 9007, "created_at": now - 18, "tags": [["h", "kept"]], "content": "again"},
                 {"kind": 9, "created_at": now - 17, "tags": [["h", "kept"]], "content": "after the restart"},
                 {"kind": 7, "created_at": now - 17, "tags": [], "content": "+"}])
    theirs = sign([{"kind": 1, "created_at": now - 16, "tags": [], "content": "banned"}], other_key)
    third = sign([{"kind": 9021, "created_at": now - 15, "tags": [["h", "kept"]], "content": ""},
                  {"kind": 9021, "created_at": now - 14, "tags": [["h", "kept"]], "content": "twice"},
                  {"kind": 9, "created_at": now - 14, "tags": [["h", "kept"]], "content": "a member after the restart"}], third_key)
    p, port = start(exe, log, store, extra=extra)
    try:
        check(not journal.exists(), 'persistence: no FILE.nip86 before the first policy change')
        ban = '{"method":"banpubkey","params":["%s","spam"]}' % other
        dis = '{"method":"disallowkind","params":[7]}'
        lst = '{"method":"listbannedpubkeys","params":[]}'
        check(post(port, ban, nip98(port, ban)) == (200, {'result': True}) and post(port, dis, nip98(port, dis))[0] == 200,
              'persistence: banpubkey and disallowkind')
        size = journal.stat().st_size if journal.exists() else -1
        check(size == 8 + len(ban) + len(dis) and journal.stat().st_mode & 0o777 == 0o600,
              'persistence: both changes are framed in FILE.nip86 (0600), %d bytes' % size)
        check(post(port, lst, nip98(port, lst))[0] == 200 and journal.stat().st_size == size, 'persistence: a listing is not saved')
        w = Ws(port)
        check(w.auth()[2] is True and publish(w, mine[0])[2] is True and publish(w, mine[1])[2] is True and publish(w, third[0])[2] is True,
              'persistence: a group is created, posted to and joined')
        w.close()
    finally:
        p.terminate(); p.wait(10)
    p, port = start(exe, log, store, extra=extra)
    try:
        check('relay: replayed 2 policy changes' in STARTED and 'relay: rebuilt groups from 3 stored group events (3 changed a group)' in STARTED,
              'persistence: the restart replays the journal and rebuilds the groups (%r)' % STARTED)
        check(post(port, lst, nip98(port, lst)) == (200, {'result': [{'pubkey': other, 'reason': 'spam'}]}), 'persistence: the ban survives the restart')
        w = Ws(port)
        check(w.auth()[2] is True and publish(w, theirs[0])[3] == 'blocked: this key is banned on this relay', 'persistence: the banned key is still refused')
        check(publish(w, mine[4])[3] == 'blocked: this relay does not accept this kind', 'persistence: the disallowed kind is still refused')
        check(publish(w, mine[2])[3] == 'duplicate: the group already exists', 'persistence: the group survives: a second create is refused')
        check(publish(w, third[1])[3] == 'duplicate: already a member', 'persistence: its member survives')
        check(publish(w, third[2])[2] is True and publish(w, mine[3])[2] is True, 'persistence: the member and the creator post after the restart')
        w.close()
    finally:
        p.terminate(); p.wait(10)
    whole = journal.read_bytes()
    with open(journal, 'ab') as f:
        f.write(struct.pack('>I', 100) + b'{"method":"ban')
    p, port = start(exe, log, store, extra=extra)
    try:
        check('relay: replayed 2 policy changes, a partial last write cut' in STARTED and journal.read_bytes() == whole,
              'persistence: a torn last journal record is cut (%r)' % STARTED)
        check(post(port, lst, nip98(port, lst)) == (200, {'result': [{'pubkey': other, 'reason': 'spam'}]}), 'persistence: the ban survives the cut')
    finally:
        p.terminate(); p.wait(10)
    bad = b'{"method":"nosuchmethod","params":[]}'
    with open(journal, 'ab') as f:
        f.write(struct.pack('>I', len(bad)) + bad)
    out = subprocess.run([str(exe), '--listen', '127.0.0.1', '--port', '0', '--store', str(store)] + extra + TLS_ARGS,
                         capture_output=True, text=True, timeout=60, env={**os.environ, 'ASAN_OPTIONS': 'detect_leaks=0'})
    check('policy journal is damaged at byte %d' % len(whole) in out.stdout and 'listening' not in out.stdout
          and journal.read_bytes() == whole + struct.pack('>I', len(bad)) + bad,
          'persistence: a whole journal record the relay would not have saved refuses to start, the file unchanged')


def checkpoint(exe, log, d):
    """--checkpoint (store_fresh.coax through durable.coax, #12631): a first start, the checkpoint following a stored event
    and a NIP-86 change, a fresh restart, and a rolled-back policy journal refused before the relay listens."""
    store, journal, cp = d / 'fresh.log', d / 'fresh.log.nip86', d / 'fresh.cp'
    me = sign([{"kind": 1, "created_at": 1, "tags": [], "content": ""}])[0]['pubkey']
    other = sign([{"kind": 1, "created_at": 1, "tags": [], "content": ""}], 'coax relay r1 second test key')[0]['pubkey']
    extra = ['--admin', me, '--checkpoint', str(cp)]
    p, port = start(exe, log, store, extra=extra)
    try:
        check(any('a first start' in l for l in STARTED), 'checkpoint: a first start with empty files (%r)' % STARTED[:1])
        w = Ws(port)
        e = sign([{"kind": 1, "created_at": int(time.time()) - 5, "tags": [], "content": "checkpointed"}])[0]
        check(w.auth()[2] is True and publish(w, e)[2] is True, 'checkpoint: an event is stored')
        w.close()
        ban = '{"method":"banpubkey","params":["%s","spam"]}' % other
        check(post(port, ban, nip98(port, ban)) == (200, {'result': True}), 'checkpoint: a NIP-86 change is saved')
        line = cp.read_text()
        sb, jb = store.read_bytes(), journal.read_bytes()
        want = 'coax-relay-fresh 1 %018d %s %018d %s\n' % (len(sb), hashlib.sha256(sb).hexdigest(), len(jb), hashlib.sha256(jb).hexdigest())
        check(line == want, 'checkpoint: it names both files, sizes and SHA-256, after each commit (%r)' % line[:60])
    finally:
        p.terminate(); p.wait(10)
    p, port = start(exe, log, store, extra=extra)
    p.terminate(); p.wait(10)
    check(any(': fresh' in l for l in STARTED), 'checkpoint: the restart is fresh (%r)' % STARTED[:1])
    whole = journal.read_bytes()
    journal.write_bytes(b'')
    r = subprocess.run([str(exe), '--listen', '127.0.0.1', '--port', '0', '--store', str(store)] + extra, capture_output=True,
                       text=True, timeout=60, env={**os.environ, 'ASAN_OPTIONS': 'detect_leaks=0'})
    check(r.returncode == 3 and 'older than the checkpoint' in r.stdout and 'listening' not in r.stdout and journal.read_bytes() == b'',
          'checkpoint: an emptied policy journal (the ban rolled back) refuses to start (%r)' % r.stdout.strip()[:120])
    journal.write_bytes(whole)
    p, port = start(exe, log, store, extra=extra)
    p.terminate(); p.wait(10)
    check(any(': fresh' in l for l in STARTED), 'checkpoint: the journal put back starts fresh again')


def proof_of_work(exe, log):
    """NIP-13 (nip13.coax): --pow 8 asks for 8 leading zero bits and a nonce committing to 8."""
    refused_start(exe, ['--pow', '257'], 'a difficulty over 256', '0 to 256')
    p, port = start(exe, log)
    try:
        check('min_pow_difficulty' not in info(port)['limitation'] and 13 not in info(port)['supported_nips'], 'pow: off by default, not in NIP-11')
    finally:
        p.terminate(); p.wait(10)
    p = subprocess.Popen([str(exe), '--listen', '127.0.0.1', '--port', '0', '--pow', '8'] + TLS_ARGS,
                         stdout=subprocess.PIPE, stderr=log, text=True, env={**os.environ, 'ASAN_OPTIONS': 'detect_leaks=0'})
    try:
        port = int(p.stdout.readline().split()[-1])
        check(info(port)['limitation'].get('min_pow_difficulty') == 8, 'pow: --pow 8 is advertised as min_pow_difficulty')
        check(info(port)['supported_nips'] == [1, 9, 11, 13, 40, 42, 45, 50, 62, 70, 77], 'pow: --pow 8 lists NIP-13')
        w = Ws(port)
        check(w.auth()[2] is True, 'pow: AUTH')
        now = int(time.time())
        plain = sign([{"kind": 1, "created_at": now, "tags": [], "content": "no work"}])[0]
        r = publish(w, plain)
        check(r[2] is False and r[3].startswith('pow: '), 'pow: an event without a nonce is refused with pow: (%r)' % (r[3],))
        pk = plain['pubkey']
        def mined(target):
            for nonce in range(1 << 20):
                tags = [["nonce", str(nonce), str(target)]]
                i = hashlib.sha256(json.dumps([0, pk, now, 1, tags, "work"], separators=(',', ':'), ensure_ascii=False).encode()).digest()
                if i[0] == 0:
                    return tags
        low = sign([{"kind": 1, "created_at": now, "tags": mined(4), "content": "work"}])[0]
        check(publish(w, low)[3] == 'pow: committed target 4 is less than 8', 'pow: a lucky id that commits to 4 is refused')
        good = sign([{"kind": 1, "created_at": now, "tags": mined(8), "content": "work"}])[0]
        check(publish(w, good) == ['OK', good['id'], True, ''], 'pow: 8 zero bits committing to 8 are accepted')
        w.close()
    finally:
        p.terminate(); p.wait(10)


def info(port):
    s = connect(port)
    s.sendall(b'GET / HTTP/1.1\r\nHost: x\r\nAccept: application/nostr+json\r\n\r\n')
    data = b''
    while True:
        c = s.recv(65536)
        if not c:
            break
        data += c
    return json.loads(data.split(b'\r\n\r\n', 1)[1])


def nip11(port):
    s = connect(port)
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
          and lim['max_subid_length'] == 64 and doc['supported_nips'] == [1, 9, 11, 40, 42, 45, 50, 62, 70, 77], 'NIP-11: the Rust relay\'s limits, NIPs 1, 9, 11, 40, 42, 45, 50, 62, 70, 77')
    check(lim['auth_required'] is False and lim['restricted_writes'] is True, 'NIP-11: reads open, writes need AUTH')


def certs(d):
    """A one-day test CA, a CA nobody trusts, and a leaf for localhost and
    127.0.0.1 from the test CA, in d, plus a second key. Key files are 0600."""
    def run(*args):
        subprocess.run(['openssl', *args], check=True, capture_output=True)
    for ca in ('ca', 'rogue'):
        run('req', '-x509', '-newkey', 'ec', '-pkeyopt', 'ec_paramgen_curve:prime256v1', '-nodes', '-days', '1',
            '-keyout', str(d / (ca + '.key')), '-out', str(d / (ca + '.pem')), '-subj', '/CN=Coax relay TLS test ' + ca,
            '-addext', 'basicConstraints=critical,CA:TRUE', '-addext', 'keyUsage=critical,keyCertSign,cRLSign')
    for leaf in ('leaf', 'other'):
        run('req', '-newkey', 'ec', '-pkeyopt', 'ec_paramgen_curve:prime256v1', '-nodes',
            '-keyout', str(d / (leaf + '.key')), '-out', str(d / (leaf + '.csr')), '-subj', '/CN=localhost')
    (d / 'leaf.ext').write_text('subjectAltName=DNS:localhost,IP:127.0.0.1\nbasicConstraints=critical,CA:FALSE\n'
                                'keyUsage=critical,digitalSignature\nextendedKeyUsage=serverAuth\n')
    run('x509', '-req', '-in', str(d / 'leaf.csr'), '-CA', str(d / 'ca.pem'), '-CAkey', str(d / 'ca.key'),
        '-set_serial', '0x' + os.urandom(8).hex(), '-days', '1', '-extfile', str(d / 'leaf.ext'), '-out', str(d / 'leaf.pem'))
    for k in d.glob('*.key'):
        k.chmod(0o600)


def refused_start(exe, args, what, want):
    """The relay must refuse to start with args, saying want, printing no key."""
    p = subprocess.run([str(exe), '--listen', '127.0.0.1', '--port', '0', *args], capture_output=True, text=True, timeout=30,
                       env={**os.environ, 'ASAN_OPTIONS': 'detect_leaks=0'})
    out = p.stdout + p.stderr
    # The C host exits 2; a URL the Coax side refuses ends main without listening.
    check(want in out and 'PRIVATE KEY' not in out and 'listening' not in out,
          'TLS: %s refused at start (%r)' % (what, out.strip()[:120]))


def closed_within(s, seconds):
    """True when the relay closes s (EOF or reset, nothing sent) within seconds."""
    s.settimeout(seconds)
    try:
        return s.recv(4096) == b''
    except ConnectionError:
        return True
    except socket.timeout:
        return False


def tls_cases(exe, d, log, store, client):
    key0 = d / 'open.key'
    shutil.copy(d / 'leaf.key', key0); key0.chmod(0o644)
    refused_start(exe, ['--tls-cert', str(d / 'leaf.pem'), '--tls-key', str(key0)], 'a group- or world-readable key (0644)', '0600')
    (d / 'link.key').symlink_to(d / 'leaf.key')
    refused_start(exe, ['--tls-cert', str(d / 'leaf.pem'), '--tls-key', str(d / 'link.key')], 'a key path that is a symbolic link', '0600')
    refused_start(exe, ['--tls-cert', str(d / 'leaf.pem'), '--tls-key', str(d / 'other.key')], "a key that is not the certificate's", "certificate's key")
    refused_start(exe, ['--tls-cert', str(d / 'leaf.pem')], 'a certificate without a key', 'go together')
    refused_start(exe, TLS_ARGS + ['--url', 'ws://127.0.0.1:7447'], 'a ws:// --url on a TLS relay', 'wss://')
    p, port = start(exe, log, store)
    try:
        rogue = ssl.create_default_context(cafile=str(d / 'rogue.pem'))
        try:
            connect(port, rogue).close(); ok = False
        except ssl.SSLCertVerificationError:
            ok = True
        check(ok, 'TLS: a client that trusts another CA refuses the relay (certificate verify failed)')
        try:
            connect(port, name='other.test').close(); ok = False
        except ssl.SSLCertVerificationError:
            ok = True
        check(ok, 'TLS: a client that expects another name refuses the relay (hostname mismatch)')
        # A plaintext WebSocket client on the TLS port: closed, no HTTP answer.
        plain = socket.create_connection(('127.0.0.1', port), timeout=5)
        plain.sendall(b'GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n'
                      b'Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n')
        check(closed_within(plain, 3), 'TLS: a plaintext client on the TLS port is closed without an answer')
        plain.close()
        # Two stalled handshakes: one sends nothing, one the first bytes of a ClientHello.
        t0 = time.monotonic()
        quiet = socket.create_connection(('127.0.0.1', port), timeout=5)
        part = socket.create_connection(('127.0.0.1', port), timeout=5)
        part.sendall(b'\x16\x03\x01\x02\x00\x01\x00\x01\xfc\x03')
        time.sleep(0.2)
        t1 = time.monotonic()
        w = Ws(port)
        w.send(['REQ', 'k1', {'kinds': [1]}])
        k1, end = w.until_eose('k1')
        took = time.monotonic() - t1
        check(w.ok and bool(w.challenge) and end == ['EOSE', 'k1'] and took < 2.0,
              'TLS: with two stalled handshakes open, another client connects, gets its challenge and REQ to EOSE in %.2f s' % took)
        w.close()
        q, r = closed_within(quiet, 14), closed_within(part, 2)
        gone = time.monotonic() - t0
        check(q and r and 9.0 <= gone <= 14.0, 'TLS: the stalled handshakes are closed after the 10 s handshake limit (%.1f s)' % gone)
        quiet.close(); part.close()
        w = Ws(port)
        w.send(['REQ', 'after', {'kinds': [1]}])
        again, end = w.until_eose('after')
        check(end == ['EOSE', 'after'] and again == k1, 'TLS: the relay still serves after the refused and timed-out connections')
        w.close()
        if client:
            env = {**os.environ, 'RX_PORT': str(port), 'CXTLS_TEST_CA': str(d / 'ca.pem'),
                   'ASAN_OPTIONS': 'detect_leaks=1:abort_on_error=1', 'UBSAN_OPTIONS': 'print_stacktrace=1:halt_on_error=1'}
            out = subprocess.run([str(client)], capture_output=True, text=True, timeout=60, env=env)
            want = 'relay tls client: challenge 1 events %d eose 1 close 1' % len(k1)
            check(out.returncode == 0 and want in out.stdout and len(k1) > 0,
                  'TLS: the Coax client (relay_conn) over wss://: challenge, %d stored kind 1 events, EOSE, close 1000 (%r)'
                  % (len(k1), (out.stdout + out.stderr).strip()[-200:]))
            env['CXTLS_TEST_CA'] = str(d / 'rogue.pem')
            out = subprocess.run([str(client)], capture_output=True, text=True, timeout=60, env=env)
            check(out.returncode == 0 and 'relay tls client: refused' in out.stdout,
                  'TLS: the Coax client refuses the relay when it trusts another CA (%r)' % (out.stdout + out.stderr).strip()[-200:])
    finally:
        p.terminate(); p.wait(10)


def main():
    global SCHEME, TLS_CTX, TLS_ARGS
    args = sys.argv[1:]
    tls = '--tls' in args
    client = Path(args[args.index('--client') + 1]).resolve() if '--client' in args else None
    with tempfile.TemporaryDirectory(prefix='coax-relay-session-') as tmp:
        d = Path(tmp)
        if tls:
            certs(d)
            SCHEME = 'wss'
            TLS_CTX = ssl.create_default_context(cafile=str(d / 'ca.pem'))
            TLS_ARGS = ['--tls-cert', str(d / 'leaf.pem'), '--tls-key', str(d / 'leaf.key')]
        exe = d / 'relay'
        t0 = time.monotonic()
        subprocess.run([str(ROOT / 'coax/relay/build.sh'), str(exe), '--sanitize'], check=True, stdout=subprocess.DEVNULL)
        print('build %.1f s' % (time.monotonic() - t0))
        with open(d / 'relay.err', 'w') as log:
            store = d / 'events.log'
            p, port = start(exe, log, store)
            try:
                nip11(port)
                session(port, int(time.time()))
                deleted, kept = deletion(port)
                protected(port)
                search(port)
                negentropy(port)
                w = Ws(port)
                w.send(['REQ', 'before', {}])
                before, _ = w.until_eose('before')
                w.close()
            finally:
                p.terminate(); p.wait(10)
            p, port = start(exe, log, store)
            try:
                w = Ws(port)
                w.send(['REQ', 'after', {}])
                got, _ = w.until_eose('after')
                check(len(before) > 0 and got == before,
                      'restart with --store: the same %d events come back, in order' % len(before))
                check(w.auth()[2] is True and publish(w, deleted)[3] == 'blocked: a deletion request covers this event'
                      and publish(w, kept)[3].startswith('duplicate'),
                      'restart with --store: the tombstones come back from the saved requests')
                w.close()
            finally:
                p.terminate(); p.wait(10)
            p, port = start(exe, log)
            try:
                w = Ws(port)
                w.send(['REQ', 'mem', {}])
                got, _ = w.until_eose('mem')
                check(got == [], 'without --store the store is in memory: a new relay starts empty')
                w.close()
            finally:
                p.terminate(); p.wait(10)
            rate_limits(exe, log)
            proof_of_work(exe, log)
            admin(exe, log)
            groups(exe, log)
            persistence(exe, log, d)
            checkpoint(exe, log, d)
            if tls:
                tls_cases(exe, d, log, store, client)
        err = (d / 'relay.err').read_text()
        check('runtime error' not in err and 'AddressSanitizer' not in err, 'no sanitizer report')
        if err.strip():
            sys.stderr.write(err[-4000:])
    print('relay_session: ' + ('ok' if not failures else '%d failures' % len(failures)))
    return 1 if failures else 0


if __name__ == '__main__':
    sys.exit(main())
