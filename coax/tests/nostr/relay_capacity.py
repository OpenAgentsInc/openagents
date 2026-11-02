#!/usr/bin/env python3
"""The Coax relay's --store-capacity with a large --store file (chapters 35, 103).

Signs N synthetic kind 1 events (default 3,000) with relay_sign.ts (a
TEST-ONLY key), writes them as chapter 35's store file (each original as a
4-byte big-endian length and its bytes), and starts coax/relay with
--store FILE:

- at the default capacity (2,048 records) the replay can't hold the file
  and the relay refuses to start, leaving the file unchanged;
- --store-capacity 127 and 1048577 are refused before anything is read;
- with --store-capacity N + 10,000 every event replays (each signature is
  verified again), COUNT says N, REQ answers the newest 127, a search
  finds its one event, a new event is stored and the file grows by one
  frame; a restart replays N + 1;
- a crash's partial last append (a frame header and half the bytes) is cut
  on the next start, as at the default capacity (store_recover.coax).

Replay re-verifies every signature with the fast verifier over
admission's read (event_verify_fast.coax): 50,000 events replay in about
20 s in build.sh's -O2 build (2026-11-01; it was about 25 ms an event,
some 20 minutes, at -O1 with the reference verifier). --sanitize builds
-O1 under ASan, much slower; keep N small there.

Tooling only; nothing here ships. Never points at a public relay.

  python3 coax/tests/nostr/relay_capacity.py [--events N] [--sanitize]
"""
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

sys.path.insert(0, str(Path(__file__).resolve().parent))
import relay_session as rs  # noqa: E402  (Ws, sign, check, publish)

ROOT = Path(__file__).resolve().parents[3]


def frames(events):
    out = bytearray()
    for e in events:
        b = json.dumps(e, separators=(',', ':'), ensure_ascii=False).encode()
        out += struct.pack('>I', len(b)) + b
    return bytes(out)


def start(exe, log, store, capacity=None, expect_listen=True):
    args = [str(exe), '--listen', '127.0.0.1', '--port', '0', '--store', str(store), '--rate', '1000000,1000000,1000000']
    if capacity is not None:
        args += ['--store-capacity', str(capacity)]
    t0 = time.monotonic()
    p = subprocess.Popen(args, stdout=subprocess.PIPE, stderr=log, text=True,
                         env={**os.environ, 'ASAN_OPTIONS': 'detect_leaks=0:halt_on_error=1'})
    lines = []
    while True:
        line = p.stdout.readline()
        if not line:
            break
        lines.append(line.strip())
        if 'listening on port' in line:
            return p, int(line.split()[-1]), lines, time.monotonic() - t0
    p.wait(30)
    return None, 0, lines, time.monotonic() - t0


def refused(exe, log, store, capacity):
    out = subprocess.run([str(exe), '--listen', '127.0.0.1', '--port', '0', '--store', str(store), '--store-capacity', str(capacity)],
                         capture_output=True, text=True, timeout=30)
    return out.returncode, (out.stdout + out.stderr).strip()


def main():
    args = sys.argv[1:]
    n = int(args[args.index('--events') + 1]) if '--events' in args else 3000
    sanitize = '--sanitize' in args
    with tempfile.TemporaryDirectory(prefix='coax-relay-capacity-') as tmp:
        d = Path(tmp)
        exe = d / 'relay'
        t0 = time.monotonic()
        subprocess.run([str(ROOT / 'coax/relay/build.sh'), str(exe)] + (['--sanitize'] if sanitize else []), check=True, stdout=subprocess.DEVNULL)
        print('build %.1f s%s' % (time.monotonic() - t0, ' (ASan/UBSan)' if sanitize else ''))
        now = int(time.time())
        t0 = time.monotonic()
        templates = [{"kind": 1, "created_at": now - 3600 - i, "tags": [["t", "capacity"]], "content": "synthetic event %d" % i} for i in range(n)]
        templates[n // 2]["content"] = "the needle qzxv"
        events = []
        for at in range(0, n, 5000):
            events += rs.sign(templates[at:at + 5000])
        print('signed %d events in %.1f s' % (len(events), time.monotonic() - t0))
        store = d / 'events.log'
        data = frames(events)
        store.write_bytes(data)
        digest = hashlib.sha256(data).hexdigest()
        print('store file: %d bytes' % len(data))
        with open(d / 'relay.err', 'w') as log:
            p, port, lines, took = start(exe, log, store)
            rs.check(p is None and any('refusing to start' in l for l in lines) and hashlib.sha256(store.read_bytes()).hexdigest() == digest,
                     'default capacity (2,048): the relay refuses to start and leaves the file unchanged (%r)' % lines[-1:])
            for bad in (127, 1048577):
                code, text = refused(exe, log, store, bad)
                rs.check(code == 2 and '--store-capacity takes 128 to 1048576 events' in text, '--store-capacity %d is refused (%r)' % (bad, text))
            cap = n + 10000
            p, port, lines, took = start(exe, log, store, cap)
            rs.check(p is not None and lines[0] == 'relay: replayed %d saved events: %d stored, 0 duplicates or replaced' % (n, n),
                     '--store-capacity %d: %d events replay, every signature verified again, in %.1f s (%r)' % (cap, n, took, lines[:1]))
            if p is None:
                print('relay_capacity: %d failures' % len(rs.failures))
                return 1
            try:
                w = rs.Ws(port)
                w.send(['COUNT', 'c', {'kinds': [1]}])
                rs.check(w.recv() == ['COUNT', 'c', {'count': n}], 'COUNT counts all %d' % n)
                w.send(['REQ', 'r', {'kinds': [1]}])
                got, end = w.until_eose('r')
                rs.check(end == ['EOSE', 'r'] and [e['id'] for e in got] == [e['id'] for e in events[:127]], 'REQ answers the newest 127, newest first')
                w.send(['REQ', 's', {'search': 'qzxv'}])
                got, _ = w.until_eose('s')
                rs.check([e['id'] for e in got] == [events[n // 2]['id']], 'a search over %d events finds its one match' % n)
                t1 = time.monotonic()
                w.send(['REQ', 'a', {'authors': [events[0]['pubkey']], 'limit': 5, 'until': now - 3600 - (n - 3)}])
                got, _ = w.until_eose('a')
                rs.check([e['id'] for e in got] == [e['id'] for e in events[n - 3:]], 'REQ with until reaches the oldest three in %.2f s' % (time.monotonic() - t1))
                fresh = rs.sign([{"kind": 1, "created_at": now - 5, "tags": [], "content": "after the replay"}])[0]
                size = store.stat().st_size
                rs.check(w.auth()[2] is True and rs.publish(w, fresh)[2] is True, 'a new event is accepted')
                body = len(json.dumps(fresh, separators=(',', ':'), ensure_ascii=False).encode())
                rs.check(store.stat().st_size == size + 4 + body, 'the file grew by one frame')
                w.close()
            finally:
                p.terminate(); p.wait(30)
            # a crash's partial last append: a frame header and half its bytes
            torn = rs.sign([{"kind": 1, "created_at": now - 4, "tags": [], "content": "torn"}])[0]
            b = json.dumps(torn, separators=(',', ':')).encode()
            with open(store, 'ab') as f:
                f.write(struct.pack('>I', len(b)) + b[:len(b) // 2])
            p, port, lines, took = start(exe, log, store, cap)
            rs.check(p is not None and lines[0] == 'relay: replayed %d saved events: %d stored, 0 duplicates or replaced, a partial last write cut' % (n + 1, n + 1),
                     'restart: %d events replay and the partial last write is cut (%r)' % (n + 1, lines[:1]))
            if p:
                p.terminate(); p.wait(30)
        err = (d / 'relay.err').read_text()
        rs.check('runtime error' not in err and 'AddressSanitizer' not in err, 'no sanitizer report')
    print('relay_capacity: ' + ('ok' if not rs.failures else '%d failures' % len(rs.failures)))
    return 1 if rs.failures else 0


if __name__ == '__main__':
    sys.exit(main())
