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
reconnect. Tooling only; nothing here ships. Never points at a public relay.

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


def start(exe, log, store=None):
    args = [str(exe), '--listen', '127.0.0.1', '--port', '0'] + (['--store', str(store)] if store else []) + TLS_ARGS
    p = subprocess.Popen(args, stdout=subprocess.PIPE, stderr=log, text=True,
                         env={**os.environ, 'ASAN_OPTIONS': 'abort_on_error=0:halt_on_error=1:detect_leaks=0',
                              'UBSAN_OPTIONS': 'print_stacktrace=1:halt_on_error=1'})
    line = p.stdout.readline()
    if store and line.startswith('relay: replayed'):
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
          and lim['max_subid_length'] == 64 and doc['supported_nips'] == [1, 11, 40, 42], 'NIP-11: the Rust relay\'s limits, NIPs 1, 11, 40, 42')
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
