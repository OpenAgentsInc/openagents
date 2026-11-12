#!/usr/bin/env python3
# Firm reason: 2 (independent reader: exports and compares relay.openagents.com and the Coax relay with code that shares nothing with the Coax Nostr library under test)
"""Chapter 103's export, shadow period and equality against the live relay.

Reads relay.openagents.com only through the public Nostr protocol, as any
client can: REQ, COUNT and EOSE over wss://. It never reads the production
database, its backups or its host, and never writes to it. The local relay it
compares with is a Coax relay on 127.0.0.1.

Python rather than Coax: this is the independent side of the comparison. The
export, the shadow answers and the equality are read by code that shares
nothing with the Coax relay under test (the importer re-verifies every line in
Coax), so a bug in the Coax client can't hide the same bug in the relay.

  relay_live_cutover.py export OUT.jsonl [--url wss://relay.openagents.com]
      Every event the relay serves to an anonymous reader, oldest first, one
      NIP-01 object a line. Prints lines, SHA-256 and the newest created_at.
  relay_live_cutover.py ids URL OUT.txt
      The sorted ids an anonymous reader gets from URL (ws:// or wss://).
  relay_live_cutover.py pages [OTHER_URL] --url URL
      The website's live subscriptions (AUTH, REQ, EOSE) through URL, compared
      with OTHER_URL's answers when given.
  relay_live_cutover.py rw --url URL
      NIP-11, AUTH, a 10-minute note written and read back through URL.
  relay_live_cutover.py publish EXPORT.jsonl --url URL
      Each line sent as an EVENT on one NIP-42 connection (AUTH_URL names
      the relay's URL): the cutover's catch-up of writes since the export.
  relay_live_cutover.py shadow LOCAL SECONDS LOG.jsonl [--url ...]
      LOCAL is a port on 127.0.0.1 or the Coax relay's ws:// or wss:// URL;
      AUTH_URL (environment) is the URL its NIP-42 answers name.
      The shadow period: every new event the live relay delivers is sent to
      the local relay too, and every PERIOD seconds the same REQs go to both;
      each answer pair is classified match, order, limit, known or
      unexplained (chapter 94's method).
"""
import base64
import hashlib
import json
import os
import socket
import ssl
import struct
import sys
import time
import urllib.parse

LIVE = 'wss://relay.openagents.com'
PACE = float(os.environ.get('PACE', '1.0'))
PAGE = 50           # under the live relay's max_limit of 127; smaller pages keep each answer small
# NIP-89 handler announcements. relay.openagents.com holds at least one 31990
# its own read check now refuses ("a platform URL contains the bech32
# placeholder"), and reading it stops the whole relay process (exit 1, every
# connection dropped). COUNT reads ids only, so the export finds every 31990
# by COUNT alone and reads around those seconds; the 31990s are counted, not
# exported.
AVOID_KIND = 31990


def Ws(url, timeout=20, tries=8):
    """A connection, retried with backoff while the relay answers 429."""
    for i in range(tries):
        try:
            return Conn(url, timeout)
        except ConnectionError as e:
            if '429' not in str(e) or i == tries - 1:
                raise
            time.sleep(10 * (i + 1))


# --- BIP-340, for the test client's events and NIP-42 answers only ----------
P = 2**256 - 2**32 - 977
N = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141
G = (0x79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798,
     0x483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8)


def padd(a, b):
    if a is None:
        return b
    if b is None:
        return a
    if a[0] == b[0] and (a[1] + b[1]) % P == 0:
        return None
    if a == b:
        lam = 3 * a[0] * a[0] * pow(2 * a[1], P - 2, P) % P
    else:
        lam = (b[1] - a[1]) * pow(b[0] - a[0], P - 2, P) % P
    x = (lam * lam - a[0] - b[0]) % P
    return x, (lam * (a[0] - x) - a[1]) % P


def pmul(k, pt=G):
    r = None
    while k:
        if k & 1:
            r = padd(r, pt)
        pt, k = padd(pt, pt), k >> 1
    return r


def tagged(tag, m):
    t = hashlib.sha256(tag.encode()).digest()
    return hashlib.sha256(t + t + m).digest()


def schnorr(msg, d0):
    pub = pmul(d0)
    d = d0 if pub[1] % 2 == 0 else N - d0
    px = pub[0].to_bytes(32, 'big')
    k0 = int.from_bytes(tagged('BIP0340/nonce', d.to_bytes(32, 'big') + px + msg), 'big') % N
    r = pmul(k0)
    k = k0 if r[1] % 2 == 0 else N - k0
    e = int.from_bytes(tagged('BIP0340/challenge', r[0].to_bytes(32, 'big') + px + msg), 'big') % N
    return px.hex(), (r[0].to_bytes(32, 'big') + ((k + e * d) % N).to_bytes(32, 'big')).hex()


SECRET = 0x103      # TEST ONLY: never a real identity


def signed(kind, tags, content, created_at, secret=SECRET):
    pub = pmul(secret)[0].to_bytes(32, 'big').hex()
    body = [0, pub, created_at, kind, tags, content]
    eid = hashlib.sha256(json.dumps(body, separators=(',', ':'), ensure_ascii=False).encode()).digest()
    _, sig = schnorr(eid, secret)
    return {'id': eid.hex(), 'pubkey': pub, 'created_at': created_at, 'kind': kind,
            'tags': tags, 'content': content, 'sig': sig}


def event(content, created_at):
    return signed(1, [['t', 'rehearsal']], content, created_at)



class Conn:
    """A standard-library WebSocket client (RFC 6455), text frames only."""

    def __init__(self, url, timeout=20):
        u = urllib.parse.urlparse(url)
        port = u.port or (443 if u.scheme == 'wss' else 80)
        s = socket.create_connection((u.hostname, port), timeout=timeout)
        if u.scheme == 'wss':
            s = ssl.create_default_context().wrap_socket(s, server_hostname=u.hostname)
        self.s, self.buf, self.closed, self.challenge = s, b'', False, None
        key = base64.b64encode(os.urandom(16)).decode()
        s.sendall(('GET %s HTTP/1.1\r\nHost: %s\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n'
                   'Sec-WebSocket-Key: %s\r\nSec-WebSocket-Version: 13\r\n\r\n'
                   % (u.path or '/', u.hostname, key)).encode())
        head = b''
        while b'\r\n\r\n' not in head:
            chunk = s.recv(4096)
            if not chunk:
                raise ConnectionError('closed during the handshake')
            head += chunk
        head, self.buf = head.split(b'\r\n\r\n', 1)
        want = base64.b64encode(hashlib.sha1((key + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').encode()).digest())
        if not head.startswith(b'HTTP/1.1 101') or want not in head:
            raise ConnectionError('no WebSocket upgrade: %r' % head[:80])

    def send(self, msg):
        p = json.dumps(msg, separators=(',', ':')).encode()
        n, mask = len(p), os.urandom(4)
        h = bytes([0x81])
        h += bytes([0x80 | n]) if n < 126 else (bytes([0x80 | 126]) + struct.pack('>H', n) if n < 65536
                                               else bytes([0x80 | 127]) + struct.pack('>Q', n))
        self.s.sendall(h + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(p)))

    def recv(self, timeout=20.0):
        self.s.settimeout(timeout)
        try:
            while True:
                if len(self.buf) >= 2:
                    op, n, at = self.buf[0] & 15, self.buf[1] & 127, 2
                    if n == 126 and len(self.buf) >= 4:
                        n, at = struct.unpack('>H', self.buf[2:4])[0], 4
                    elif n == 127 and len(self.buf) >= 10:
                        n, at = struct.unpack('>Q', self.buf[2:10])[0], 10
                    if (n < 126 or at > 2) and len(self.buf) >= at + n:
                        payload, self.buf = self.buf[at:at + n], self.buf[at + n:]
                        if op == 9:                              # ping: answer, keep reading
                            self.s.sendall(bytes([0x8A, 0x80 | len(payload)]) + b'\0\0\0\0' + payload)
                            continue
                        if op == 8:
                            self.closed = True
                            return None
                        if op == 1:
                            m = json.loads(payload)
                            if isinstance(m, list) and len(m) == 2 and m[0] == 'AUTH':
                                self.challenge = m[1]
                            return m
                        continue
                chunk = self.s.recv(65536)
                if not chunk:
                    self.closed = True
                    return None
                self.buf += chunk
        except socket.timeout:
            return None
        except (ConnectionError, ssl.SSLError, OSError):
            self.closed = True
            return None

    def auth(self, relay_url):
        """NIP-42 with the test key: the relays take writes only from an
        authenticated connection (any key; the event's author may differ)."""
        while self.challenge is None:
            if self.recv(timeout=5) is None:
                return False
        self.send(['AUTH', signed(22242, [['relay', relay_url], ['challenge', self.challenge]], '', int(time.time()))])
        m = self.recv()
        while isinstance(m, list) and m[0] != 'OK':
            m = self.recv()
        return bool(m and m[2])

    def req(self, sub, flt):
        """Stored events for one filter, until EOSE. Returns (events, end message).
        Paced to one REQ per PACE seconds: the live relay rate-limits REQs."""
        time.sleep(PACE)
        self.send(['REQ', sub, flt])
        out = []
        while True:
            # 60 s: the live relay can take longer than 20 s on a page of large
            # events, and an EOSE or CLOSED only ends this REQ when it names
            # this sub (a late answer to an earlier one must not end it).
            m = self.recv(timeout=60)
            if isinstance(m, list) and m[0] in ('EOSE', 'CLOSED') and len(m) > 1 and m[1] != sub:
                continue
            if m is None or m[0] in ('EOSE', 'CLOSED'):
                if m is not None and m[0] == 'EOSE':
                    self.send(['CLOSE', sub])
                return out, m
            if m[0] == 'EVENT' and m[1] == sub:
                out.append(m[2])

    def close(self):
        try:
            self.s.close()
        except OSError:
            pass


def crawl(url, base=None, log=sys.stderr):
    """Every event `url` serves for `base` (default every event), by pages
    walking `until` down. A page whose events all share one second is split
    by kind so ties can't loop. The live relay drops the connection for some
    full pages of large events, so a failed page is asked again with half
    the limit (down to 8)."""
    base = dict(base or {})
    ws, seen, until, n, lim = Ws(url), {}, None, 0, PAGE
    try:
        while True:
            f = dict(base, limit=lim)
            if until is not None:
                f['until'] = until
            page, end = ws.req('x%d' % n, f)
            n += 1
            if end is None or end[0] != 'EOSE':
                print('  page %d (until %s, limit %d) ended with %r; asking again' % (n, until, lim, end), file=log)
                ws.close()
                if lim == 8:
                    raise RuntimeError('page %d (until %s) fails at limit 8' % (n, until))
                lim = max(8, lim // 2)
                ws = Ws(url)
                continue
            new = [e for e in page if e['id'] not in seen]
            for e in page:
                seen[e['id']] = e
            if len(page) < lim:
                return seen
            low = min(e['created_at'] for e in page)
            if all(e['created_at'] == low for e in page) or not new:
                # one second holds a full page: read that second kind by kind
                second = dict(base, since=low, until=low, limit=PAGE)
                for k in sorted({e['kind'] for e in page}):
                    got, end = ws.req('k%d' % n, dict(second, kinds=[k]))
                    n += 1
                    if end is None or end[0] != 'EOSE' or len(got) == PAGE:
                        raise RuntimeError('the second %d, kind %d, does not read whole: %r' % (low, k, end))
                    for e in got:
                        seen[e['id']] = e
                until = low - 1
            else:
                until = low       # the next page repeats `low`; seen drops repeats
            if n % 50 == 0:
                print('  %d pages, %d events, until %s' % (n, len(seen), until), file=log)
    finally:
        ws.close()


def count(url, flt, ws=None):
    own = ws is None
    ws = ws or Ws(url)
    try:
        time.sleep(PACE)
        ws.send(['COUNT', 'c', flt])
        m = ws.recv()
        while isinstance(m, list) and m[0] not in ('COUNT', 'CLOSED', 'NOTICE'):
            m = ws.recv()
        if not (m and m[0] == 'COUNT'):
            raise RuntimeError('COUNT %r answered %r' % (flt, m))
        return m[2].get('count')
    finally:
        if own:
            ws.close()


def seconds_of(url, kind, lo, hi, log=sys.stderr):
    """Every second in [lo, hi] holding an event of `kind`, by COUNT bisection
    (no event body is read)."""
    ws, out = Ws(url), {}

    def walk(a, b):
        n = count(url, {'kinds': [kind], 'since': a, 'until': b}, ws)
        if n == 0:
            return
        if a == b:
            if n is None:
                raise RuntimeError('COUNT over its maximum at one second, %d' % a)
            out[a] = n
            return
        m = (a + b) // 2
        walk(a, m)
        walk(m + 1, b)
    try:
        walk(lo, hi)
    finally:
        ws.close()
    print('  kind %d: %d events in %d seconds' % (kind, sum(out.values()), len(out)), file=log)
    return out


def crawl_around(url, log=sys.stderr):
    """crawl() over the windows between the seconds holding AVOID_KIND. At
    those seconds every other event must be readable by kind; a second that
    holds others is refused unless COUNT says AVOID_KIND is all of it."""
    now = int(time.time()) + 900
    bad = seconds_of(url, AVOID_KIND, 0, now, log)
    edges = [-1] + sorted(bad) + [now + 1]
    seen = {}
    for a, b in zip(edges, edges[1:]):
        if b - a > 1:
            seen.update(crawl(url, {'since': a + 1, 'until': b - 1}, log))
    for t, k in sorted(bad.items()):
        total = count(url, {'since': t, 'until': t})
        if total != k:
            raise RuntimeError('second %d holds %d events, %d of kind %d' % (t, total, k, AVOID_KIND))
    return seen, bad


def export(out, url):
    t0 = time.time()
    events, bad = crawl_around(url)
    rows = sorted(events.values(), key=lambda e: (e['created_at'], e['id']))
    data = b''.join(json.dumps({k: e[k] for k in ('id', 'pubkey', 'created_at', 'kind', 'tags', 'content', 'sig')},
                               separators=(',', ':'), ensure_ascii=False).encode() + b'\n' for e in rows)
    with open(out, 'wb') as f:
        f.write(data)
    print('export %s lines %d bytes %d sha256 %s newest_created_at %d seconds %.1f count_all %s not_exported_kind_%d %d'
          % (url, len(rows), len(data), hashlib.sha256(data).hexdigest(),
             max(e['created_at'] for e in rows), time.time() - t0, count(url, {}), AVOID_KIND, sum(bad.values())))


def ids(url, out):
    got = sorted(crawl_around(url)[0])
    with open(out, 'w') as f:
        f.write(''.join(i + '\n' for i in got))
    print('ids %s %d digest %s' % (url, len(got), hashlib.sha256(''.join(got).encode()).hexdigest()))


# The shadow's REQs: the filters openagents.com and the agents send most
# (kinds named in the relay's NIP-11 extensions and chapter 98-102 consumers),
# plus an unfiltered recent page.
SHADOW_FILTERS = [
    {'limit': 50}, {'kinds': [0], 'limit': 50}, {'kinds': [1], 'limit': 50},
    {'kinds': [3], 'limit': 20}, {'kinds': [5], 'limit': 20}, {'kinds': [7], 'limit': 50},
    {'kinds': [10002], 'limit': 20}, {'kinds': [30023], 'limit': 20}, {'kinds': [1111], 'limit': 50},
    {'kinds': [5000, 5001, 5050, 6000, 6001, 6050, 7000], 'limit': 50},
    {'kinds': [9, 10, 11, 12], 'limit': 50}, {'kinds': [30617, 1617, 1621], 'limit': 20},
    # the project pages' kinds (coax/nostr.md section 17): projects, issues' statuses
    {'kinds': [30621, 30623], 'limit': 20}, {'kinds': [1630, 1631, 1632, 1633], 'limit': 50},
]


def classify(a, b, limit):
    """Chapter 94's classes for one REQ answered by A (live) and B (local)."""
    ia, ib = [e['id'] for e in a], [e['id'] for e in b]
    if ia == ib:
        return 'match', []
    if sorted(ia) == sorted(ib):
        return 'order', []
    sa, sb = set(ia), set(ib)
    if limit and len(ia) == limit and len(ib) == limit:
        # both full: a difference only at the oldest second is the limit case
        low = min(min(e['created_at'] for e in a), min(e['created_at'] for e in b))
        diff = [e for e in a + b if (e['id'] in sa) != (e['id'] in sb)]
        if all(e['created_at'] == low for e in diff):
            return 'limit', []
    return 'unexplained', sorted(sa ^ sb)


def shadow(local, seconds, log_path, url, period=300):
    # LOCAL is a port on 127.0.0.1 or a ws:// or wss:// URL (the Coax relay on
    # its VM); AUTH_URL is the relay URL its NIP-42 answers name (its --url).
    local = local if '://' in local else 'ws://127.0.0.1:%d' % int(local)
    start = int(time.time())
    live = Ws(url)
    # From the export's newest created_at (SINCE), so what arrived between the
    # export and this start is mirrored too.
    live.send(['REQ', 'mirror', {'since': int(os.environ.get('SINCE', start))}])
    lw = Ws(local)
    if not lw.auth(os.environ.get('AUTH_URL', local)):
        raise SystemExit('the local relay refused NIP-42')
    log = open(log_path, 'a')
    stats = {'mirrored': 0, 'mirror_refused': 0, 'rounds': 0, 'match': 0, 'order': 0, 'limit': 0,
             'unexplained': 0}
    next_round = time.time()

    def note(rec):
        log.write(json.dumps(rec, separators=(',', ':')) + '\n')
        log.flush()

    note({'start': start, 'url': url, 'local': local, 'period': period})
    last = int(os.environ.get('SINCE', start))
    while time.time() < start + seconds:
        m = live.recv(timeout=5)
        if live.closed:
            # Cloud Run ends a WebSocket after its request timeout: reopen
            # from the newest mirrored second (duplicates answer "duplicate:")
            live.close()
            live = Ws(url)
            live.send(['REQ', 'mirror', {'since': last}])
            note({'t': int(time.time()), 'reconnect': last})
            continue
        if isinstance(m, list) and m[0] == 'EVENT' and m[1] == 'mirror':
            e = m[2]
            last = max(last, e['created_at'])
            lw.send(['EVENT', e])
            ok = lw.recv(timeout=10)
            while isinstance(ok, list) and ok[0] != 'OK':
                ok = lw.recv(timeout=10)
            stats['mirrored'] += 1
            if not (isinstance(ok, list) and (ok[2] or str(ok[3]).startswith('duplicate:'))):
                stats['mirror_refused'] += 1
            note({'t': int(time.time()), 'mirror': e['id'], 'kind': e['kind'], 'ok': ok})
        if time.time() >= next_round:
            next_round = time.time() + period
            stats['rounds'] += 1
            ra, rb = Ws(url), Ws(local)
            for i, flt in enumerate(SHADOW_FILTERS):
                a, ea = ra.req('s%d' % i, flt)
                b, eb = rb.req('s%d' % i, flt)
                cls, diff = classify(a, b, flt.get('limit'))
                stats[cls] += 1
                note({'t': int(time.time()), 'round': stats['rounds'], 'filter': flt, 'class': cls,
                      'live': len(a), 'local': len(b), 'diff': diff})
            ra.close()
            rb.close()
            print('round %d %s' % (stats['rounds'], json.dumps(stats)), flush=True)
    note({'end': int(time.time()), 'stats': stats})
    print('shadow %ds %s' % (seconds, json.dumps(stats)))


def rw(url, auth_url):
    """One read and write check through a name (the rollback rehearsal and the
    cutover's checks): NIP-11, then NIP-42 AUTH, a kind 1 note that expires in
    10 minutes (NIP-40, so neither relay keeps it), its OK, and a REQ by its id
    that must return it. Prints the relay's name and each step."""
    u = urllib.parse.urlparse(url)
    s = socket.create_connection((u.hostname, 443 if u.scheme == 'wss' else (u.port or 80)), timeout=15)
    if u.scheme == 'wss':
        s = ssl.create_default_context().wrap_socket(s, server_hostname=u.hostname)
    s.sendall(('GET / HTTP/1.1\r\nHost: %s\r\nAccept: application/nostr+json\r\nConnection: close\r\n\r\n' % u.hostname).encode())
    body = b''
    while True:
        chunk = s.recv(65536)
        if not chunk:
            break
        body += chunk
    s.close()
    info = json.loads(body.split(b'\r\n\r\n', 1)[1].decode(errors='replace').strip() or '{}')
    print('nip11 name=%r nips=%s' % (info.get('name'), info.get('supported_nips')), flush=True)
    w = Ws(url)
    ok_auth = w.auth(auth_url)
    now = int(time.time())
    e = signed(1, [['expiration', str(now + 600)], ['t', 'oa-relay-rehearsal']], 'relay rollback rehearsal (#13361)', now)
    w.send(['EVENT', e])
    ok = w.recv(timeout=20)
    while isinstance(ok, list) and ok[0] != 'OK':
        ok = w.recv(timeout=20)
    got, _ = Ws(url).req('rw', {'ids': [e['id']]})
    good = bool(ok_auth and isinstance(ok, list) and ok[2] and any(x['id'] == e['id'] for x in got))
    print('auth %s, write %s, read back %d: %s' % (ok_auth, ok, len(got), 'ok' if good else 'FAILED'), flush=True)
    if not good:
        sys.exit(1)


def pages(url, other=None):
    """The website's live subscriptions through url (landing-nostr.coax's
    filter shapes, chapter 103's post-cutover check): the AUTH challenge a
    page receives on connect, then for each public project its repository
    (30617 by d), project and status (30621, 30623 by d), issues (1621 by a),
    and the issues' comments (1111 by E) and statuses (1630 to 1633 by e), each
    answered and ended with EOSE. With other, the same REQs go there too and
    each answer's ids must match (the fallback, during the first hour)."""
    w = Ws(url)
    for _ in range(10):
        if w.challenge:
            break
        w.recv(timeout=3)
    print('AUTH challenge on connect: %s' % ('yes' if w.challenge else 'NO'), flush=True)
    bad = 0 if w.challenge else 1
    projects, _ = w.req('p', {'kinds': [30621], 'limit': 127})
    o = Ws(other) if other else None
    n = 0
    for p in projects:
        d = next((t[1] for t in p['tags'] if t and t[0] == 'd' and len(t) > 1), '')
        flts = [{'kinds': [30617], 'authors': [p['pubkey']], '#d': [d]},
                {'kinds': [30621, 30623], 'authors': [p['pubkey']], '#d': [d]},
                {'kinds': [1621], '#a': ['30621:%s:%s' % (p['pubkey'], d)], 'limit': 127}]
        issues = []
        for i, flt in enumerate(flts):
            got, end = w.req('q%d' % i, flt)
            n += 1
            if not (isinstance(end, list) and end[0] == 'EOSE'):
                bad += 1; print('no EOSE for %s %s: %s' % (d, flt, end), flush=True); continue
            if i == 2:
                issues = [e['id'] for e in got][:50]
            if o:
                theirs, _ = o.req('q%d' % i, flt)
                if sorted(e['id'] for e in got) != sorted(e['id'] for e in theirs):
                    bad += 1; print('differs for %s %s: %d here, %d there' % (d, flt, len(got), len(theirs)), flush=True)
        if issues:
            for i, flt in enumerate([{'kinds': [1111], '#E': issues}, {'kinds': [1630, 1631, 1632, 1633], '#e': issues}]):
                got, end = w.req('c%d' % i, flt)
                n += 1
                if not (isinstance(end, list) and end[0] == 'EOSE'):
                    bad += 1; print('no EOSE for %s comments/statuses: %s' % (d, end), flush=True)
                elif o:
                    theirs, _ = o.req('c%d' % i, flt)
                    if sorted(e['id'] for e in got) != sorted(e['id'] for e in theirs):
                        bad += 1; print('differs for %s %s: %d here, %d there' % (d, flt['kinds'], len(got), len(theirs)), flush=True)
    print('pages: %d projects, %d REQs, %s' % (len(projects), n, 'ok' if not bad else '%d FAILED' % bad), flush=True)
    if bad:
        sys.exit(1)


def publish(path, url, auth_url):
    """Every line of a JSON-lines export sent as an EVENT to url over one
    NIP-42-authenticated connection (the Coax relay takes writes from an
    authenticated connection whatever the author): the cutover's catch-up of
    the writes the Rust relay took after the export, private kinds included,
    which no subscription can mirror. Counts each OK: stored, duplicate, or
    refused with its reason."""
    w = Ws(url)
    if not w.auth(auth_url):
        raise SystemExit('the relay refused NIP-42')
    stats, reasons = {'lines': 0, 'stored': 0, 'duplicate': 0, 'refused': 0}, {}
    for raw in open(path, 'rb'):
        e = json.loads(raw)
        stats['lines'] += 1
        w.send(['EVENT', e])
        ok = w.recv(timeout=20)
        while isinstance(ok, list) and not (ok[0] == 'OK' and ok[1] == e['id']):
            ok = w.recv(timeout=20)
        if not isinstance(ok, list):
            raise SystemExit('no OK for %s' % e['id'])
        msg = str(ok[3])
        if msg.startswith('duplicate:'):
            stats['duplicate'] += 1
        elif ok[2]:
            stats['stored'] += 1
        else:
            stats['refused'] += 1
            r = msg.split(':')[0]
            reasons[r] = reasons.get(r, 0) + 1
            print('refused %s kind %d: %s' % (e['id'], e['kind'], msg), flush=True)
        time.sleep(0.012)   # under 6000 a minute
    print('publish %s' % json.dumps({'stats': stats, 'reasons': reasons}))


def main():
    a = sys.argv[1:]
    url = LIVE
    if '--url' in a:
        i = a.index('--url')
        url = a[i + 1]
        del a[i:i + 2]
    if a[:1] == ['export']:
        export(a[1], url)
    elif a[:1] == ['ids']:
        ids(a[1], a[2])
    elif a[:1] == ['pages']:
        pages(url, a[1] if len(a) > 1 else None)
    elif a[:1] == ['rw']:
        rw(url, os.environ.get('AUTH_URL', url))
    elif a[:1] == ['publish']:
        publish(a[1], url, os.environ.get('AUTH_URL', url))
    elif a[:1] == ['shadow']:
        period = int(os.environ.get('PERIOD', '300'))
        shadow(a[1], int(a[2]), a[3], url, period)
    else:
        sys.exit(__doc__)


if __name__ == '__main__':
    main()
