#!/usr/bin/env python3
"""The Coax relay against relay.openagents.com's recorded behaviour.

coax/lib/nostr/fixtures/coax_relay.jsonl is what the production (Rust)
relay sent, read-only, for the project page's five REQs (capture.ts,
2026-10-28). This publishes every recorded event to a fresh Coax relay
(ASan/UBSan build, 127.0.0.1), sends the same five REQs, and compares each
subscription's events (ids, order and bytes) and its EOSE with the
recording. It never connects to a public relay. Tooling only.

  python3 coax/tests/nostr/relay_differential.py
"""
import json
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE))
import relay_session as rs  # noqa: E402

PK = "9ef92e341b43ebc5617a75cf7eca7e2e30fae41e08a0614a344d851d98731575"
REQS = [
    ["project", {"kinds": [30621, 30623], "authors": [PK], "#d": ["coax"]}],
    ["issues", {"kinds": [1621], "authors": [PK], "#a": ["30621:%s:coax" % PK]}],
    ["comments", {"kinds": [1111], "authors": [PK]}],
    ["status", {"kinds": [1630, 1631, 1632, 1633], "authors": [PK]}],
    ["repo", {"kinds": [30617], "authors": [PK]}],
]


def main():
    lines = (ROOT / 'coax/lib/nostr/fixtures/coax_relay.jsonl').read_text().splitlines()
    recorded = {sub: [] for sub, _ in REQS}
    eose = {}
    raw = {}
    for line in lines:
        f = json.loads(line)
        if f[0] == 'EVENT':
            recorded[f[1]].append(f[2]['id'])
            # the event's bytes exactly as the relay sent them
            start = line.index(',', line.index(',') + 1) + 1
            raw[f[2]['id']] = line[start:-1]
        elif f[0] == 'EOSE':
            eose[f[1]] = f
    with tempfile.TemporaryDirectory(prefix='coax-relay-diff-') as tmp:
        d = Path(tmp)
        exe = d / 'relay'
        subprocess.run([str(ROOT / 'coax/relay/build.sh'), str(exe), '--sanitize'], check=True, stdout=subprocess.DEVNULL)
        with open(d / 'relay.err', 'w') as log:
            p, port = rs.start(exe, log)
            try:
                w = rs.Ws(port)
                # NIP-42: writes need an authenticated key on the connection (any key,
                # as the Rust relay's auth_required); the test key then imports others' events
                rs.check(w.auth()[2] is True, 'AUTH with the test key before the import')
                t0 = time.monotonic()
                oks = 0
                for eid, text in raw.items():
                    w.s.sendall(b'')
                    w.send_frame(1, ('["EVENT",%s]' % text).encode())
                    m = w.recv(10)
                    if m == ['OK', eid, True, '']:
                        oks += 1
                    else:
                        rs.check(False, 'publish %s: %r' % (eid, m))
                rs.check(oks == len(raw), '%d recorded events published, every one OK true (%.1f s, each verified)' % (len(raw), time.monotonic() - t0))
                for sub, flt in REQS:
                    w.send(['REQ', sub, flt])
                    events = []
                    while True:
                        f = w.frame(10)
                        if f is None:
                            break
                        m = json.loads(f[1])
                        if m[0] == 'EOSE':
                            break
                        events.append(m[2]['id'])
                        if f[1].decode() != '["EVENT","%s",%s]' % (sub, raw.get(m[2]['id'], '')):
                            rs.check(False, '%s: event %s bytes differ from the recording' % (sub, m[2]['id']))
                    rs.check(set(events) == set(recorded[sub]), '%s: the same %d events as the Rust relay' % (sub, len(recorded[sub])))
                    rs.check(events == recorded[sub], '%s: in the same order' % sub)
                    print('  note %s: Rust EOSE frame %s, ours ["EOSE","%s"]' % (sub, json.dumps(eose.get(sub)), sub))
                w.close()
            finally:
                p.terminate(); p.wait(10)
        err = (d / 'relay.err').read_text()
        rs.check('runtime error' not in err and 'AddressSanitizer' not in err, 'no sanitizer report')
    print('relay_differential: ' + ('ok' if not rs.failures else '%d failures' % len(rs.failures)))
    return 1 if rs.failures else 0


if __name__ == '__main__':
    sys.exit(main())
