#!/usr/bin/env python3
"""Crash recovery of the Coax relay's --store file (#966b), under ASan+UBSan.

Builds coax/relay with --sanitize and runs rounds over one --store file.
Each round starts the relay (it replays the file), checks what it serves
against what earlier rounds were told, then streams signed writes over one
authenticated WebSocket and kills the relay with SIGKILL at a random moment.
Before the next start it damages the file the way an interrupted append
can: nothing, a torn frame (a header and a prefix of an event), a frame
whose last bytes are still zero, a run of zero bytes, or a cut inside a
last frame that was never acknowledged. The checks:

  - every note (kind 1) acknowledged OK true is served after every restart
  - nothing is served that was not sent (acknowledged or in flight)
  - the replaceable (kind 0) and addressable (kind 30617) heads are never
    older than the newest acknowledged version
  - after a restart the file is whole frames only
  - damage to committed bytes (a byte of a middle frame's signature, a
    middle frame's length) makes the relay refuse to start and leaves the
    file byte-for-byte unchanged; with the file restored it starts again
  - a missing file starts empty and is created mode 0600
  - no sanitizer report

Test tooling only; nothing here ships. 127.0.0.1 only.

  python3 coax/tests/nostr/store_crash.py [ROUNDS] [SEED]   # COAX=, BUN=, NOBLE= as relay_session.py
"""
import json
import os
import random
import signal
import struct
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE))
import relay_session as rs  # noqa: E402  (Ws, sign)

ROUNDS = int(sys.argv[1]) if len(sys.argv) > 1 else 14
SEED = int(sys.argv[2]) if len(sys.argv) > 2 else int(time.time())
ENV = {**os.environ, 'ASAN_OPTIONS': 'abort_on_error=0:halt_on_error=1:detect_leaks=0',
       'UBSAN_OPTIONS': 'print_stacktrace=1:halt_on_error=1'}
fails = []


def check(ok, what):
    print(('  ok   ' if ok else '  FAIL ') + what)
    if not ok:
        fails.append(what)


def frames(data):
    """The file's whole frames as (offset, bytes), and the size they cover."""
    out, at = [], 0
    while at + 4 <= len(data):
        n = struct.unpack('>I', data[at:at + 4])[0]
        if n < 1 or at + 4 + n > len(data):
            break
        out.append((at, data[at + 4:at + 4 + n]))
        at += 4 + n
    return out, at


def start(exe, log, store):
    """(process, port) when the relay listens; (process, line) when it refuses."""
    p = subprocess.Popen([str(exe), '--listen', '127.0.0.1', '--port', '0', '--store', str(store)],
                         stdout=subprocess.PIPE, stderr=log, text=True, env=ENV)
    first = p.stdout.readline()
    if first.startswith('relay: refusing') or 'cannot' in first:
        p.wait(60)
        return p, first
    line = p.stdout.readline() if first.startswith('relay: replayed') else first
    if 'listening on port' not in line:
        raise SystemExit('relay did not start: %r %r' % (first, line))
    return p, int(line.split()[-1]), first


def served(port, ids):
    """The ids among ids the relay serves, and its kind 0 and 30617 heads."""
    w = rs.Ws(port)
    got, k = set(), 0
    ids = sorted(ids)
    for i in range(0, len(ids), 40):
        k += 1
        w.send(['REQ', 'q%d' % k, {'ids': ids[i:i + 40]}])
        evs, end = w.until_eose('q%d' % k)
        assert end == ['EOSE', 'q%d' % k], end
        got |= {e['id'] for e in evs}
        w.send(['CLOSE', 'q%d' % k])
    heads = {}
    for kind in (0, 30617):
        w.send(['REQ', 'h%d' % kind, {'kinds': [kind]}])
        evs, _ = w.until_eose('h%d' % kind)
        heads[kind] = [e['id'] for e in evs]
        w.send(['CLOSE', 'h%d' % kind])
    w.close()
    return got, heads


def main():
    rng = random.Random(SEED)
    print('seed %d, %d rounds' % (SEED, ROUNDS))
    now = int(time.time())
    # the stream: regular kind 1 notes, every 4th a new kind 0 version and
    # every 6th a new kind 30617 "repo" version, created_at increasing
    templates, version = [], {0: [], 30617: []}
    for i in range(ROUNDS * 8 + 8):
        t = {'kind': 1, 'created_at': now - 5000 + i, 'tags': [], 'content': 'crash note %d' % i}
        if i % 4 == 3:
            t = {'kind': 0, 'created_at': now - 5000 + i, 'tags': [], 'content': '{"name":"v%d"}' % i}
        elif i % 6 == 5:
            t = {'kind': 30617, 'created_at': now - 5000 + i, 'tags': [['d', 'repo']], 'content': 'v%d' % i}
        templates.append(t)
    events = rs.sign(templates)
    order = {e['id']: i for i, e in enumerate(events)}
    for e in events:
        if e['kind'] in version:
            version[e['kind']].append(e['id'])
    sent, acked, rounds_cut = set(), set(), 0
    nxt = 0
    with tempfile.TemporaryDirectory(prefix='coax-store-crash-') as tmp:
        d = Path(tmp)
        exe = d / 'relay'
        t0 = time.monotonic()
        subprocess.run([str(ROOT / 'coax/relay/build.sh'), str(exe), '--sanitize'], check=True, stdout=subprocess.DEVNULL)
        print('build %.1f s' % (time.monotonic() - t0))
        store = d / 'state' / 'events.log'
        store.parent.mkdir()
        log = open(d / 'relay.err', 'w')
        modes = {}
        for rnd in range(ROUNDS):
            t1 = time.monotonic()
            r = start(exe, log, store)
            if len(r) == 2:
                check(False, 'round %d: the relay starts (%s)' % (rnd, r[1].strip()))
                break
            p, port, first = r
            if rnd == 0:
                check(store.exists() and oct(store.stat().st_mode & 0o777) == '0o600',
                      'a missing file starts empty and is created mode 0600')
            if 'cut' in first:
                rounds_cut += 1
            data = store.read_bytes()
            _, whole = frames(data)
            ok_frames = whole == len(data)
            got, heads = served(port, sent | acked)
            # a replaced version is not served: those are checked by their heads below
            lost = {x for x in acked if events[order[x]]['kind'] == 1} - got
            extra = got - sent
            head_ok = True
            for kind, vs in version.items():
                newest = max([order[v] for v in vs if v in acked], default=-1)
                h = heads[kind]
                head_ok &= len(h) <= 1 and (newest < 0 or (len(h) == 1 and order[h[0]] >= newest and h[0] in sent))
            check(not lost and not extra and head_ok and ok_frames,
                  'round %d: %d acknowledged, %d served, heads %s, %d whole frames (%s) [%.1f s]'
                  % (rnd, len(acked), len(got), 'ok' if head_ok else heads, len(frames(data)[0]), first.strip(),
                     time.monotonic() - t1))
            if lost or extra:
                print('    lost %s extra %s' % (sorted(lost)[:3], sorted(extra)[:3]))
            # stream writes; kill at a random moment
            w = rs.Ws(port)
            okauth = w.auth()
            assert okauth and okauth[2] is True, okauth
            stop = threading.Event()
            srng = random.Random(SEED * 1000 + rnd)

            def stream():
                nonlocal nxt
                try:
                    while not stop.is_set() and nxt < len(events):
                        # now and then an acknowledged event again (a duplicate: OK true, nothing written)
                        e = events[srng.choice(sorted(order[x] for x in acked))] if acked and srng.random() < 0.15 else events[nxt]
                        fresh = e is events[nxt]
                        if fresh:
                            sent.add(e['id'])
                            nxt += 1
                        m = rs.publish(w, e)
                        if isinstance(m, list) and m[0] == 'OK' and m[1] == e['id'] and m[2] is True:
                            acked.add(e['id'])
                        elif m is None:
                            return
                except (OSError, ValueError):
                    return
            th = threading.Thread(target=stream)
            th.start()
            time.sleep(rng.uniform(0.1, 3.0))
            os.kill(p.pid, signal.SIGKILL)
            p.wait(30)
            stop.set()
            th.join(30)
            w.close()
            # damage the tail as an interrupted append can
            data = store.read_bytes()
            fr, whole = frames(data)
            mode = rng.choice(['none', 'torn', 'zero-body', 'zeros', 'cut-last'])
            if whole != len(data):
                mode = 'none (the kill left a partial frame)'
            if nxt < len(events):
                x = json.dumps(events[nxt], separators=(',', ':')).encode()
            if mode == 'torn' and nxt < len(events):
                sent.add(events[nxt]['id'])
                k = rng.randrange(0, len(x))
                store.write_bytes(data + (struct.pack('>I', len(x)) + x)[:4 + k] if rng.random() < 0.8
                                  else data + struct.pack('>I', len(x))[:rng.randrange(1, 4)])
            elif mode == 'zero-body' and nxt < len(events):
                sent.add(events[nxt]['id'])
                k = rng.randrange(0, len(x))
                store.write_bytes(data + struct.pack('>I', len(x)) + x[:k] + bytes(len(x) - k))
            elif mode == 'zeros':
                store.write_bytes(data + bytes(rng.randrange(1, 70)))
            elif mode == 'cut-last' and fr:
                at, body = fr[-1]
                last = json.loads(body)['id']
                if last not in acked:
                    store.write_bytes(data[:at + rng.randrange(1, 4 + len(body))])
                else:
                    mode = 'none (last acknowledged)'
            elif not mode.startswith('none'):
                mode = 'none'
            modes[mode] = modes.get(mode, 0) + 1
        print('damage: %s; restarts that cut a partial write: %d' % (modes, rounds_cut))
        # committed damage refuses and changes nothing
        good = store.read_bytes()
        fr, _ = frames(good)
        check(len(fr) >= 3, 'at least 3 frames to damage (%d)' % len(fr))
        mid_at, mid = fr[len(fr) // 2]
        cases = []
        sig = mid.index(b'"sig":"') + 7 + 10
        bad = bytearray(good)
        bad[mid_at + 4 + sig] = ord('0') if bad[mid_at + 4 + sig] != ord('0') else ord('1')
        cases.append(('a byte of a middle frame\'s signature', bytes(bad)))
        bad = bytearray(good)
        bad[mid_at + 3] = (bad[mid_at + 3] + 1) % 256
        cases.append(('a middle frame\'s length', bytes(bad)))
        bad = bytearray(good)
        bad[mid_at:mid_at + 4] = b'\xff\xff\xff\xff'
        cases.append(('a middle frame\'s length made impossible', bytes(bad)))
        bad = bytearray(good)
        bad[mid_at + 4 + 1] = 0
        cases.append(('a zero byte inside a middle frame', bytes(bad)))
        last_at, last = fr[-1]
        bad = bytearray(good)
        i = last.index(b'"sig":"') + 7 + 3
        bad[last_at + 4 + i] = ord('0') if bad[last_at + 4 + i] != ord('0') else ord('1')
        cases.append(('a byte of the last frame\'s signature', bytes(bad)))
        for what, b in cases:
            store.write_bytes(b)
            r = start(exe, log, store)
            refused = len(r) == 2 and r[1].startswith('relay: refusing')
            if len(r) == 3:
                os.kill(r[0].pid, signal.SIGKILL); r[0].wait(30)
            check(refused and store.read_bytes() == b, 'damaged %s: the relay refuses and the file is unchanged' % what)
        store.write_bytes(good)
        r = start(exe, log, store)
        if len(r) == 3:
            got, _ = served(r[1], sent | acked)
            regular = {x for x in acked if events[order[x]]['kind'] == 1}
            check(regular <= got, 'the file restored: the relay starts and serves all %d acknowledged notes' % len(regular))
            r[0].terminate(); r[0].wait(30)
        else:
            check(False, 'the file restored: the relay starts')
        log.close()
        err = (d / 'relay.err').read_text()
        check('runtime error' not in err and 'AddressSanitizer' not in err, 'no sanitizer report')
        if err.strip():
            sys.stderr.write(err[-4000:])
    print('store_crash: %d acknowledged, %d sent; ' % (len(acked), len(sent)) + ('ok' if not fails else '%d failed' % len(fails)))
    return 1 if fails else 0


if __name__ == '__main__':
    sys.exit(main())
