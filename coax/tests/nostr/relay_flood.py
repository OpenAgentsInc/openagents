"""Flood the Coax relay (R1) and measure its memory. Tooling only (standard library).

Builds coax/relay (release, or ASan+UBSan with --sanitize), starts it with a
--store in a fresh scratch directory on ADDRESS (default 127.0.0.1; the
tailnet address with --listen), and drives it past every limit the R1 record
names, sampling VmRSS and VmHWM from /proc/PID/status every 0.2 s:

  connections   16 WebSockets, then EXTRA more (each closed, no 101)
  subscriptions 32 per connection on all 16 (each about 8 KiB of filters),
                the 33rd refused; 17 filters, over 8192 bytes, an 8-open-
                filter query cost, all refused
  frames        a 131074-byte text frame (close 1009), a header claiming
                2^40 bytes (close 1009), 16 connections each holding 140 KiB
                of an unfinished frame
  events        EVENTS valid signed events (a TEST-ONLY key, the sha256 of a
                fixed label), fanned out live to 16 subscriptions, until the
                store answers "relay storage is full"; then more are refused
  slowloris     16 connections trickling an HTTP head or a frame one byte at
                a time; a fresh client meanwhile

Never touches a public relay: the only relay is the one started here.

  python3 -I coax/tests/nostr/relay_flood.py [--sanitize] [--listen ADDRESS]
      [--events N] [--big] [--extra N] [--out DIR] [--relay BINARY]

--big signs 60 KiB events, so the 4 MiB store arena fills before the
2048-event count does (the ASan run uses it: verification there is slow).
"""
import argparse, base64, hashlib, json, os, pathlib, random, socket, struct, subprocess, sys, tempfile, threading, time
from concurrent.futures import ProcessPoolExecutor

ROOT = pathlib.Path(__file__).resolve().parents[3]
failed = []


def check(ok, what):
    print(('ok   ' if ok else 'FAIL ') + what, flush=True)
    if not ok:
        failed.append(what)


# --- BIP-340, for test keys only -------------------------------------------
P = 2**256 - 2**32 - 977
N = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141
G = (0x79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798,
     0x483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8)


def _add(a, b):  # Jacobian points; None is infinity
    if a is None: return b
    if b is None: return a
    x1, y1, z1 = a; x2, y2, z2 = b
    z1s, z2s = z1 * z1 % P, z2 * z2 % P
    u1, u2 = x1 * z2s % P, x2 * z1s % P
    s1, s2 = y1 * z2s * z2 % P, y2 * z1s * z1 % P
    if u1 == u2:
        if s1 != s2: return None
        return _dbl(a)
    h, r = (u2 - u1) % P, (s2 - s1) % P
    h2 = h * h % P; h3 = h2 * h % P
    x3 = (r * r - h3 - 2 * u1 * h2) % P
    return (x3, (r * (u1 * h2 - x3) - s1 * h3) % P, h * z1 * z2 % P)


def _dbl(a):
    if a is None: return None
    x, y, z = a
    if y == 0: return None
    s = 4 * x * y * y % P; m = 3 * x * x % P
    x3 = (m * m - 2 * s) % P
    return (x3, (m * (s - x3) - 8 * pow(y, 4, P)) % P, 2 * y * z % P)


def mul(k):
    r, q = None, (G[0], G[1], 1)
    while k:
        if k & 1: r = _add(r, q)
        q = _dbl(q); k >>= 1
    zi = pow(r[2], -1, P)
    return (r[0] * zi * zi % P, r[1] * zi * zi * zi % P)


def th(tag, data):
    t = hashlib.sha256(tag.encode()).digest()
    return hashlib.sha256(t + t + data).digest()


def keypair(label):
    d = int.from_bytes(hashlib.sha256(label.encode()).digest(), 'big') % N
    px, py = mul(d)
    return (d if py % 2 == 0 else N - d), px.to_bytes(32, 'big')


def schnorr(d, px, msg):
    aux = os.urandom(32)
    t = (d ^ int.from_bytes(th('BIP0340/aux', aux), 'big')).to_bytes(32, 'big')
    k0 = int.from_bytes(th('BIP0340/nonce', t + px + msg), 'big') % N
    rx, ry = mul(k0)
    k = k0 if ry % 2 == 0 else N - k0
    rb = rx.to_bytes(32, 'big')
    e = int.from_bytes(th('BIP0340/challenge', rb + px + msg), 'big') % N
    return rb + ((k + e * d) % N).to_bytes(32, 'big')


KEY_LABEL = 'coax relay flood test key (never a real key)'


def sign(kind, tags, content, created_at):
    d, px = keypair(KEY_LABEL)
    pub = px.hex()
    body = json.dumps([0, pub, created_at, kind, tags, content], separators=(',', ':'), ensure_ascii=False)
    eid = hashlib.sha256(body.encode()).digest()
    return {'id': eid.hex(), 'pubkey': pub, 'created_at': created_at, 'kind': kind, 'tags': tags,
            'content': content, 'sig': schnorr(d, px, eid).hex()}


def sign_note(args):
    i, size, now = args
    return sign(1, [['t', 'flood']], ('flood %d ' % i) + 'x' * size, now)


# --- a small WebSocket client ----------------------------------------------
class Ws:
    def __init__(self, addr, port, url_host, read=True, timeout=10):
        self.s = socket.create_connection((addr, port), timeout)
        key = base64.b64encode(os.urandom(16)).decode()
        self.s.sendall(('GET / HTTP/1.1\r\nHost: %s:%d\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n'
                        'Sec-WebSocket-Key: %s\r\nSec-WebSocket-Version: 13\r\n\r\n' % (url_host, port, key)).encode())
        self.buf = b''
        while b'\r\n\r\n' not in self.buf:
            d = self.s.recv(4096)
            if not d: raise ConnectionError('closed before 101')
            self.buf += d
        head, self.buf = self.buf.split(b'\r\n\r\n', 1)
        if not head.startswith(b'HTTP/1.1 101'): raise ConnectionError(head[:40])
        self.msgs, self.close_code, self.closed, self.lock = [], None, False, threading.Condition()
        self.challenge = None
        if read:
            self.s.settimeout(None)   # the reader blocks; a timeout there is not a close
            threading.Thread(target=self._reader, daemon=True).start()

    def frame(self, op, payload, fin=True):
        h = bytes([(0x80 if fin else 0) | op])
        n = len(payload)
        if n < 126: h += bytes([0x80 | n])
        elif n < 65536: h += bytes([0x80 | 126]) + struct.pack('>H', n)
        else: h += bytes([0x80 | 127]) + struct.pack('>Q', n)
        m = os.urandom(4)
        return h + m + bytes(b ^ m[i % 4] for i, b in enumerate(payload))

    def send(self, obj):
        self.s.sendall(self.frame(1, json.dumps(obj, separators=(',', ':')).encode()))

    def _reader(self):
        try:
            while True:
                while True:
                    m = self._parse()
                    if m is None: break
                    op, payload = m
                    with self.lock:
                        if op == 1:
                            v = json.loads(payload)
                            if v[0] == 'AUTH' and self.challenge is None: self.challenge = v[1]
                            self.msgs.append(v)
                        elif op == 8:
                            self.close_code = struct.unpack('>H', payload[:2])[0] if len(payload) >= 2 else 1005
                        self.lock.notify_all()
                d = self.s.recv(65536)
                if not d: break
                self.buf += d
        except (OSError, ValueError):
            pass
        with self.lock:
            self.closed = True; self.lock.notify_all()

    def _parse(self):
        b = self.buf
        if len(b) < 2: return None
        n, k = b[1] & 0x7F, 2
        if n == 126:
            if len(b) < 4: return None
            n, k = struct.unpack('>H', b[2:4])[0], 4
        elif n == 127:
            if len(b) < 10: return None
            n, k = struct.unpack('>Q', b[2:10])[0], 10
        if len(b) < k + n: return None
        self.buf = b[k + n:]
        return b[0] & 0x0F, b[k:k + n]

    def wait(self, pred, seconds=30):
        end = time.time() + seconds
        with self.lock:
            while True:
                for i, m in enumerate(self.msgs):
                    if pred(m):
                        return self.msgs.pop(i)
                left = end - time.time()
                if left <= 0 or self.closed: return None
                self.lock.wait(min(left, 0.5))

    def wait_closed(self, seconds=15):
        end = time.time() + seconds
        with self.lock:
            while not self.closed and time.time() < end:
                self.lock.wait(0.5)
            return self.closed

    def count(self, pred):
        with self.lock:
            return sum(1 for m in self.msgs if pred(m))

    def drop(self, pred):
        with self.lock:
            self.msgs = [m for m in self.msgs if not pred(m)]

    def close(self):
        # shutdown first: a close alone does not end a socket another thread is reading.
        try: self.s.shutdown(socket.SHUT_RDWR)
        except OSError: pass
        try: self.s.close()
        except OSError: pass


def ok_for(eid):
    return lambda m: m[0] == 'OK' and m[1] == eid


# --- memory -----------------------------------------------------------------
class Rss:
    def __init__(self, pid):
        self.pid, self.samples, self.phase, self.stop = pid, [], 'start', False
        threading.Thread(target=self.run, daemon=True).start()

    def read(self):
        v = {}
        try:
            for line in open('/proc/%d/status' % self.pid):
                if line.startswith(('VmRSS:', 'VmHWM:')):
                    v[line.split(':')[0]] = int(line.split()[1])
        except OSError:
            return None
        return v

    def run(self):
        t0 = time.time()
        while not self.stop:
            v = self.read()
            if v: self.samples.append((time.time() - t0, self.phase, v['VmRSS'], v['VmHWM']))
            time.sleep(0.2)

    def mark(self, phase):
        self.phase = phase
        v = self.read()
        print('     [%s] VmRSS %.1f MiB, VmHWM %.1f MiB' % (phase, v['VmRSS'] / 1024, v['VmHWM'] / 1024), flush=True)
        return v


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--sanitize', action='store_true')
    ap.add_argument('--listen', default='127.0.0.1')
    ap.add_argument('--events', type=int, default=2100)
    ap.add_argument('--big', action='store_true')
    ap.add_argument('--extra', type=int, default=200)
    ap.add_argument('--out')
    ap.add_argument('--relay')
    a = ap.parse_args()
    out = pathlib.Path(a.out or tempfile.mkdtemp(prefix='relay-flood-'))
    out.mkdir(parents=True, exist_ok=True)
    binary = a.relay or str(out / 'relay')
    if not a.relay:
        t = time.time()
        subprocess.run([str(ROOT / 'coax/relay/build.sh'), binary] + (['--sanitize'] if a.sanitize else []), check=True, stdout=subprocess.DEVNULL)
        print('built %s in %.0f s' % ('ASan+UBSan' if a.sanitize else 'release (-O1, no sanitizers)', time.time() - t), flush=True)
    s = socket.socket(); s.bind((a.listen, 0)); port = s.getsockname()[1]; s.close()
    url = 'ws://%s:%d' % (a.listen, port)
    store = out / 'flood.store'
    if store.exists(): store.unlink()
    env = dict(os.environ, ASAN_OPTIONS='detect_leaks=0')
    log = open(out / 'relay.log', 'w')
    relay = subprocess.Popen([binary, '--listen', a.listen, '--port', str(port), '--url', url, '--store', str(store)],
                             stdout=log, stderr=subprocess.STDOUT, env=env)
    try:
        run(a, relay, port, url, out)
    except Exception as e:
        check(False, 'the driver ended with %r' % e)
        raise
    finally:
        if relay.poll() is None:
            relay.terminate()
            try: relay.wait(10)
            except subprocess.TimeoutExpired: relay.kill(); relay.wait()
        log.close()
        text = (out / 'relay.log').read_text(errors='replace')
        check(not any(x in text for x in ('AddressSanitizer', 'runtime error', 'LeakSanitizer')), 'no sanitizer report from the relay')
        print('relay flood: ok' if not failed else 'relay flood: %d failed' % len(failed), flush=True)
    return 0 if not failed else 1


def run(a, relay, port, url, out):
    addr = a.listen
    end = time.time() + 60
    while time.time() < end:
        try: socket.create_connection((addr, port), 0.5).close(); break
        except OSError: time.sleep(0.2)
    # The probe above took a slot; let the relay see its close.
    time.sleep(1.5)
    rss = Rss(relay.pid)
    base = rss.mark('idle')
    t_all = time.time()

    # NIP-11
    s = socket.create_connection((addr, port), 5)
    s.sendall(b'GET / HTTP/1.1\r\nHost: x\r\nAccept: application/nostr+json\r\n\r\n')
    body = b''
    while True:
        d = s.recv(65536)
        if not d: break
        body += d
    s.close()
    nip11 = json.loads(body.split(b'\r\n\r\n', 1)[1])
    lim = nip11.get('limitation', {})
    check(body.startswith(b'HTTP/1.1 200') and lim.get('max_subscriptions') == 32 and lim.get('max_message_length') == 131072,
          'NIP-11 on %s:%d: max_subscriptions 32, max_message_length 131072' % (addr, port))

    # Connections: 16, then EXTRA more.
    rss.mark('connections')
    conns = [Ws(addr, port, addr) for _ in range(16)]
    for c in conns: c.wait(lambda m: m[0] == 'AUTH', 10)
    check(all(c.challenge and len(c.challenge) == 64 for c in conns), '16 WebSockets accepted, each with its own NIP-42 challenge')
    refused = 0
    t = time.time()
    for i in range(a.extra):
        try:
            Ws(addr, port, addr, read=False, timeout=5).close()
        except (ConnectionError, OSError):
            refused += 1
    check(refused == a.extra, 'connections 17 to %d: %d of %d closed without a 101 (%.1f s)' % (16 + a.extra, refused, a.extra, time.time() - t))
    # A burst of raw sockets held open at once (beyond the C host's 32 handles).
    held = []
    for i in range(100):
        try: held.append(socket.create_connection((addr, port), 5))
        except OSError: pass
    time.sleep(2)
    eofs = 0
    for h in held:
        h.settimeout(5)
        try:
            eofs += h.recv(1) == b''
        except OSError:
            eofs += 1
        h.close()
    check(eofs == len(held), '100 raw sockets held open past the limit: %d of %d closed by the relay' % (eofs, len(held)))
    for c in conns: c.s.sendall(c.frame(9, b'still-here'))
    time.sleep(1)
    check(all(not c.closed for c in conns), 'the 16 accepted WebSockets still work after the burst')

    # Subscriptions: 32 per connection, sub 0 live on kind 1, 1..31 ~8 KiB of ids each.
    rss.mark('subscriptions')
    ids = [hashlib.sha256(b'%d' % i).hexdigest() for i in range(120)]
    t = time.time()
    for c in conns:
        c.send(['REQ', 's0', {'kinds': [1], 'limit': 0}])
        for k in range(1, 32):
            c.send(['REQ', 's%d' % k, {'ids': ids[:60], 'limit': 1}, {'ids': ids[60:], 'limit': 1}])
    eose = sum(1 for c in conns for k in range(32) if c.wait(lambda m, k=k: m[0] == 'EOSE' and m[1] == 's%d' % k, 30))
    if eose != 512:
        print('     closed connections: %s' % [(i, c.close_code) for i, c in enumerate(conns) if c.closed], flush=True)
    check(eose == 512, '32 subscriptions on each of 16 connections, about 8 KiB of filters each: %d of 512 EOSE (%.1f s)' % (eose, time.time() - t))
    for c in conns:
        c.send(['REQ', 's32', {'kinds': [1]}])
    over = sum(1 for c in conns if c.wait(lambda m: m[0] == 'CLOSED' and m[1] == 's32' and 'too many active subscriptions' in m[2], 15))
    check(over == 16, 'a 33rd subscription on each connection: %d of 16 CLOSED "restricted: too many active subscriptions"' % over)
    c = conns[0]
    c.send(['CLOSE', 's31'])
    cases = [
        ('17 filters', ['REQ', 'f17'] + [{'kinds': [1]}] * 17, 'too many filters'),
        ('filters over 8192 bytes', ['REQ', 'fbig', {'ids': ids[:42]}, {'ids': ids[42:84]}, {'ids': ids[84:120] + ids[:6]}], 'exceed 8192 bytes'),
        ('65 ids in one filter', ['REQ', 'f65', {'ids': ids[:65]}], 'malformed filter'),
        ('8 open filters (query cost)', ['REQ', 'fcost'] + [{}] * 8, 'query cost'),
        ('16 filters of 64 ids', ['REQ', 'fhuge'] + [{'ids': [hashlib.sha256(b'h%d.%d' % (j, i)).hexdigest() for i in range(64)]} for j in range(16)], 'exceed 8192 bytes'),
        ('a 65-character sub id', ['REQ', 'x' * 65, {'kinds': [1]}], None),
    ]
    for what, msg, text in cases:
        c.send(msg)
        if text:
            r = c.wait(lambda m, sub=msg[1]: m[0] == 'CLOSED' and m[1] == sub, 15)
            check(r is not None and text in r[2], '%s (%d bytes): CLOSED "%s"' % (what, len(json.dumps(msg)), r[2] if r else None))
        else:
            r = c.wait(lambda m: m[0] == 'NOTICE', 15)
            check(r is not None, '%s: NOTICE "%s"' % (what, r[1] if r else None))
    c.send(['REQ', 's31', {'ids': ids[:60], 'limit': 1}, {'ids': ids[60:], 'limit': 1}])
    check(c.wait(lambda m: m[0] == 'EOSE' and m[1] == 's31', 15) is not None, 'after a CLOSE the slot takes a new subscription')
    rss.mark('subscriptions held')

    # Frames past the limit, on fresh connections (a slot is freed first).
    rss.mark('frames')
    conns[15].close(); time.sleep(1.5)
    f = Ws(addr, port, addr)
    f.s.sendall(f.frame(1, b'[' + b' ' * 131072 + b']'))
    check(f.wait_closed(20) and f.close_code == 1009, 'a 131074-byte text frame: close %s' % f.close_code)
    f.close(); time.sleep(1.5)
    f = Ws(addr, port, addr)
    f.s.sendall(bytes([0x81, 0x80 | 127]) + struct.pack('>Q', 1 << 40) + os.urandom(4))
    check(f.wait_closed(20) and f.close_code == 1009, 'a frame header claiming 2^40 bytes: close %s' % f.close_code)
    f.close(); time.sleep(1.5)
    f = Ws(addr, port, addr)
    f.s.sendall(f.frame(1, b'["NOTICE"', fin=False) + f.frame(0, b' ' * 131000, fin=False))
    f.s.sendall(f.frame(0, b' ' * 200 + b']', fin=True))
    check(f.wait_closed(20) and f.close_code == 1009, 'fragments adding up past 131072 bytes: close %s' % f.close_code)
    f.close(); time.sleep(1.5)
    # 1 connection now free (conns[15] gone): hold 140 KiB of an unfinished frame on it.
    f = Ws(addr, port, addr)
    big = f.frame(1, b' ' * 131000)
    f.s.sendall(big[:-1])
    time.sleep(2)
    check(not f.closed, 'an unfinished 131000-byte frame is held (%d of %d bytes)' % (len(big) - 1, len(big)))
    for cc in conns[:15]: cc.s.sendall(cc.frame(9, b'p'))
    time.sleep(1)
    check(all(not cc.closed for cc in conns[:15]), 'other connections are served while it waits')
    f.s.sendall(big[-1:])
    r = f.wait(lambda m: m[0] == 'NOTICE', 15)
    check(r is not None, 'its last byte completes the frame (NOTICE "%s")' % (r[1] if r else None))
    conns[15] = f
    rss.mark('frames done')

    # Events: authenticate, then publish until the store is full.
    rss.mark('events')
    w = conns[0]
    auth = sign(22242, [['relay', url], ['challenge', w.challenge]], '', int(time.time()))
    w.send(['AUTH', auth])
    r = w.wait(ok_for(auth['id']), 30)
    check(r is not None and r[2] is True, 'AUTH with the test key: OK %s' % (r[2] if r else None))
    size = 60000 if a.big else 200
    t = time.time()
    now = int(time.time())
    with ProcessPoolExecutor(8) as pool:
        events = list(pool.map(sign_note, [(i, size, now) for i in range(a.events)], chunksize=16))
    print('     signed %d events of %d bytes in %.1f s' % (len(events), len(json.dumps(events[0])), time.time() - t), flush=True)
    stored = full = other = 0
    t = time.time()
    first_full = None
    for i, e in enumerate(events):
        w.send(['EVENT', e])
        r = w.wait(ok_for(e['id']), 120)
        if r is None:
            other += 1; print('     no OK for event %d' % i, flush=True); break
        if r[2] is True and r[3] == '': stored += 1
        elif 'relay storage is full' in r[3]:
            full += 1
            if first_full is None: first_full = i
        else:
            other += 1; print('     event %d: %s' % (i, r[3]), flush=True)
        if i % 200 == 199:
            for cc in conns: cc.drop(lambda m: m[0] == 'EVENT')
            rss.mark('events %d' % (i + 1))
    spent = time.time() - t
    check(other == 0 and stored > 0, '%d events published in %.1f s (%.1f ms each): %d stored, %d refused "relay storage is full" (first at #%s), %d other' %
          (len(events), spent, 1000 * spent / max(1, len(events)), stored, full, first_full, other))
    if a.events > 2048 or a.big:
        check(full > 0 and first_full == stored, 'past the store bound every further event is refused (stored %d)' % stored)
    time.sleep(1)
    live = [cc.count(lambda m: m[0] == 'EVENT' and m[1] == 's0') for cc in conns[1:15]]
    print('     live deliveries in the last window per connection: %s' % live, flush=True)
    rss.mark('events done')

    # Slowloris: every slot busy with a silent socket, a trickling head or a
    # trickling frame, and one idle WebSocket.
    rss.mark('slowloris')
    for cc in conns[1:]: cc.close()
    time.sleep(2)
    idle = conns[0]
    silent = [socket.create_connection((addr, port), 5) for _ in range(3)]
    heads = [socket.create_connection((addr, port), 5) for _ in range(4)]
    frames = [Ws(addr, port, addr) for _ in range(8)]
    head = b'GET / HTTP/1.1\r\nHost: x\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nX-Pad: ' + b'a' * 400
    payload = frames[0].frame(1, b'["REQ","q",{"kinds":[1]}]' + b' ' * 400)
    t = time.time(); i = 0
    early = late = None
    head_cut = None
    while time.time() - t < 36:
        for h in heads:
            try: h.sendall(head[i % len(head):i % len(head) + 1])
            except OSError:
                if head_cut is None: head_cut = time.time() - t
        for fr in frames:
            if i < len(payload) - 1 and not fr.closed:
                try: fr.s.sendall(payload[i:i + 1])
                except OSError: pass
        if early is None and time.time() - t > 4:
            try: Ws(addr, port, addr, read=False, timeout=5).close(); early = True
            except (ConnectionError, OSError): early = False
        if late is None and time.time() - t > 13:
            try:
                x = Ws(addr, port, addr, read=False, timeout=5); late = True; x.close()
            except (ConnectionError, OSError): late = False
        i += 1
        time.sleep(0.25)
    gone = 0
    for sk in silent:
        sk.settimeout(2)
        try: gone += sk.recv(1) == b''
        except OSError: gone += 1
    check(early is False, 'at 4 s, with all 16 slots held, a fresh client is refused')
    check(head_cut is not None and gone == 3, 'silent sockets and trickled heads are dropped at the 10 s head deadline (3 of 3 silent; first trickled write failed at %s s)' % (None if head_cut is None else '%.1f' % head_cut))
    check(late is True, 'at 13 s, after those drops, a fresh client is served')
    codes = [fr.close_code for fr in frames]
    check(all(c_ == 1008 for c_ in codes), 'frames trickled a byte every 0.25 s get close 1008 at the 30 s frame deadline: %s' % codes)
    idle.s.sendall(idle.frame(9, b'idle-ok'))
    time.sleep(1)
    check(not idle.closed, 'an idle WebSocket with nothing buffered is kept through it all (%.0f s idle)' % (time.time() - t))
    for sk in silent + heads: sk.close()
    for fr in frames: fr.close()
    # A client that pipelines two full-size frames in one write is not cut off.
    pp = Ws(addr, port, addr)
    pp.wait(lambda m: m[0] == 'AUTH', 5)
    big = b'["NOTICE"' + b' ' * 131000 + b']'
    pp.s.sendall(pp.frame(1, big) + pp.frame(1, big) + pp.frame(1, b'["REQ","pp",{"kinds":[1],"limit":1}]'))
    r1 = pp.wait(lambda m: m[0] == 'EOSE' and m[1] == 'pp', 20)
    check(r1 is not None and not pp.closed, 'two 131010-byte frames and a REQ pipelined in one write: both read, then EOSE (close %s)' % pp.close_code)
    pp.close()
    v = rss.mark('end')
    rss.stop = True
    check(relay.poll() is None, 'the relay is still running after %.0f s of flood' % (time.time() - t_all))
    phases = {}
    for _, ph, r_, hwm in rss.samples:
        phases.setdefault(ph, []).append(r_)
    print('     VmRSS by phase (KiB, min..max): ' + '; '.join('%s %d..%d' % (k, min(v_), max(v_)) for k, v_ in phases.items()), flush=True)
    print('     VmRSS idle %.1f MiB, peak %.1f MiB (VmHWM), end %.1f MiB' % (base['VmRSS'] / 1024, v['VmHWM'] / 1024, v['VmRSS'] / 1024), flush=True)
    (out / 'rss.tsv').write_text(''.join('%.1f\t%s\t%d\t%d\n' % s_ for s_ in rss.samples))
    print('     samples in %s' % (out / 'rss.tsv'), flush=True)
    return 0 if not failed else 1


if __name__ == '__main__':
    sys.exit(main())
