#!/usr/bin/env python3
# Firm reason: 2 (independent side of the Rust-to-Coax relay equality: it signs, publishes, reads and compares event sets with relay_live_cutover.py's client, not the Coax Nostr library under test)
"""Chapter 103's cutover and rollback, rehearsed on a staging name.

Runs inside rust_relay_sandbox.sh's network-less sandbox (loopback only),
next to the old Rust relay and its throwaway Postgres:

  RUN_BINDS="DIR" RUN_ENV="COAX_RELAY=EXE EXPORT=FILE NOW=T OUT=DIR" \\
    coax/tests/nostr/rust_relay_sandbox.sh run python3 -I coax/tests/nostr/relay_rollback_rehearsal.py

EXPORT is relay_live_cutover.py's export of relay.openagents.com; NOW its
newest created_at. The steps are the runbook's 7, 8, 9, 15, 17, 18, 19 and 20:

 1. The Rust relay's Postgres gets the export through its own import-jsonl;
    the Coax relay gets it through `relay --import`, then `--check-store`.
 2. Both start; their event sets are compared (sorted ids, SHA-256).
 3. A proxy on its own port stands for the staging name. Cutover: it points
    at the Coax relay, and a client writes three events through it and reads
    them back. The store file's size at cutover is recorded.
 4. Rollback: the Coax relay stops; `relay --export --from BYTE` exports what
    it took after the cutover; the Rust relay imports that with import-jsonl;
    the proxy points back at the Rust relay; the client reads the three
    events through it, writes one more and reads it back.
 5. Final equality: the Rust relay's set is the Coax store's (restarted from
    its file) plus exactly the event written after the rollback.

Test-only key: secret 0x10 3 (never a real identity).
"""
import hashlib
import json
import os
import socket
import subprocess
import sys
import threading
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
os.environ.setdefault('PACE', '0')
import relay_live_cutover as rl  # noqa: E402
from relay_live_cutover import event  # noqa: E402

OUT = os.environ['OUT']
LOG = open(os.path.join(OUT, 'rehearsal.log'), 'w')
RESULT = {}


def say(*a):
    line = ' '.join(str(x) for x in a)
    print(line, flush=True)
    LOG.write(line + '\n')
    LOG.flush()


# --- the two relays and the proxy -------------------------------------------
def free_port():
    s = socket.socket()
    s.bind(('127.0.0.1', 0))
    p = s.getsockname()[1]
    s.close()
    return p


def start_rust():
    port = free_port()
    env = {**os.environ, 'NOSTR_RELAY_PORT': str(port), 'NOSTR_RELAY_BIND_ADDR': '127.0.0.1',
           'NOSTR_RELAY_URL': 'ws://127.0.0.1:%d' % port, 'NOSTR_RELAY_MAX_LIMIT': '127',
           'NOSTR_RELAY_RATE_EVENTS_PER_MIN_IP': '1000000', 'NOSTR_RELAY_RATE_EVENTS_PER_MIN_PUBKEY': '1000000',
           'NOSTR_RELAY_RATE_REQ_PER_MIN_IP': '1000000', 'NOSTR_RELAY_MAX_CONNECTIONS_PER_IP': '1000',
           'NOSTR_RELAY_LOG_LEVEL': 'warn'}
    p = subprocess.Popen([os.environ['RUST_RELAY']], stdout=subprocess.PIPE, stderr=LOG, text=True, env=env)
    line = p.stdout.readline()
    if 'nostr-relay listening' not in line:
        raise SystemExit('the Rust relay did not start: %r' % line)
    return p, port


def rust_import(path):
    with open(path, 'rb') as f:
        r = subprocess.run([os.environ['RUST_RELAY'], 'import-jsonl'], stdin=f, capture_output=True, text=True)
    say('rust import-jsonl', os.path.basename(path), 'exit', r.returncode, (r.stdout + r.stderr).strip()[-400:])
    return r.returncode


def coax(*args, check=True):
    r = subprocess.run([os.environ['COAX_RELAY'], *args], capture_output=True, text=True)
    say('coax relay', ' '.join(a for a in args if not a.startswith('/')), 'exit', r.returncode)
    for line in (r.stdout + r.stderr).splitlines()[-12:]:
        say('  ' + line)
    if check and r.returncode != 0:
        raise SystemExit('coax relay %s exited %d' % (args[0], r.returncode))
    return r


def start_coax(port, store, cp, cap):
    p = subprocess.Popen([os.environ['COAX_RELAY'], '--port', str(port), '--url', URL, '--store', store,
                          '--checkpoint', cp, '--store-capacity', str(cap), '--rate', '100000,100000,100000'],
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    while True:
        line = p.stdout.readline()
        if not line:
            raise SystemExit('the Coax relay did not start')
        if 'listening' in line:
            threading.Thread(target=lambda: [LOG.write('coax: ' + x) for x in p.stdout], daemon=True).start()
            return p


class Proxy:
    """The staging name: a TCP forwarder whose upstream can be switched."""

    def __init__(self):
        self.upstream = None
        self.sock = socket.socket()
        self.sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.sock.bind(('127.0.0.1', 0))
        self.sock.listen(64)
        self.port = self.sock.getsockname()[1]
        threading.Thread(target=self.accept, daemon=True).start()

    def accept(self):
        while True:
            c, _ = self.sock.accept()
            u = socket.create_connection(('127.0.0.1', self.upstream))
            for a, b in ((c, u), (u, c)):
                threading.Thread(target=self.pipe, args=(a, b), daemon=True).start()

    @staticmethod
    def pipe(a, b):
        try:
            while True:
                d = a.recv(65536)
                if not d:
                    break
                b.sendall(d)
        except OSError:
            pass
        for s in (a, b):
            try:
                s.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass


def ids(port):
    got = sorted(rl.crawl('ws://127.0.0.1:%d' % port, log=LOG))
    return got, hashlib.sha256(''.join(got).encode()).hexdigest()


def publish(port, e, relay_url):
    """Through `port` (the staging name), authenticated to the relay behind
    it, which names itself relay_url."""
    ws = rl.Ws('ws://127.0.0.1:%d' % port)
    if not ws.auth(relay_url):
        say('NIP-42 refused by', relay_url)
    ws.send(['EVENT', e])
    m = ws.recv()
    while isinstance(m, list) and m[0] != 'OK':
        m = ws.recv()
    ws.close()
    return m


def read_back(port, want):
    ws = rl.Ws('ws://127.0.0.1:%d' % port)
    got, _ = ws.req('r', {'ids': want})
    ws.close()
    return sorted(e['id'] for e in got) == sorted(want)


def main():
    global URL
    export, now = os.environ['EXPORT'], os.environ['NOW']
    lines = sum(1 for _ in open(export, 'rb'))
    # 2 KiB of arena a record: room for twice the export's lines and bytes
    cap = max(4096, 2 * lines, 2 * os.path.getsize(export) // 2048)
    store, cp = os.path.join(OUT, 'coax.store'), os.path.join(OUT, 'coax.cp')
    for f in (store, cp, store + '.nip86'):
        if os.path.exists(f):
            os.remove(f)
    coax_port = free_port()
    URL = 'ws://127.0.0.1:%d' % coax_port
    say('export', export, 'lines', lines, 'now', now)

    # 1. both imports
    RESULT['rust_import_exit'] = rust_import(export)
    r = coax('--import', export, '--now', now, '--url', URL, '--store', store, '--checkpoint', cp,
             '--store-capacity', str(cap), check=False)
    RESULT['coax_import_exit'] = r.returncode
    served = [x for x in r.stdout.splitlines() if 'served' in x]
    c = coax('--check-store', '--now', now, '--url', URL, '--store', store, '--checkpoint', cp,
             '--store-capacity', str(cap))
    RESULT['check_store_same_served_line'] = served == [x for x in c.stdout.splitlines() if 'served' in x]

    # 2. both relays, first equality
    rust, rust_port = start_rust()
    cx = start_coax(coax_port, store, cp, cap)
    a, da = ids(rust_port)
    b, db = ids(coax_port)
    RESULT['equal_after_import'] = {'rust': len(a), 'coax': len(b), 'equal': a == b,
                                    'only_rust': sorted(set(a) - set(b))[:20], 'only_coax': sorted(set(b) - set(a))[:20],
                                    'digest_rust': da, 'digest_coax': db}
    say('equality after import', json.dumps(RESULT['equal_after_import']))

    # 3. cutover on the staging name
    proxy = Proxy()
    proxy.upstream = coax_port
    cut_byte = os.path.getsize(store)
    t = int(time.time())
    written = [event('rehearsal write %d after cutover' % i, t + i) for i in range(3)]
    oks = [publish(proxy.port, e, URL) for e in written]
    RESULT['cutover'] = {'store_bytes_at_cutover': cut_byte, 'writes_ok': [bool(m and m[2]) for m in oks],
                         'read_back_through_staging': read_back(proxy.port, [e['id'] for e in written])}
    say('cutover', json.dumps(RESULT['cutover']))

    # 4. rollback
    cx.terminate()
    cx.wait()
    after = os.path.join(OUT, 'after-cutover.jsonl')
    if os.path.exists(after):
        os.remove(after)
    coax('--export', after, '--from', str(cut_byte), '--now', str(t + 10), '--url', URL, '--store', store,
         '--checkpoint', cp, '--store-capacity', str(cap))
    exported = [json.loads(x)['id'] for x in open(after)]
    RESULT['rollback_export'] = {'lines': len(exported), 'same_as_written': sorted(exported) == sorted(e['id'] for e in written)}
    RESULT['rust_import_after_exit'] = rust_import(after)
    proxy.upstream = rust_port
    late = event('rehearsal write after rollback', t + 5)
    ok = publish(proxy.port, late, 'ws://127.0.0.1:%d' % rust_port)
    RESULT['rollback'] = {'reads_cutover_writes_through_staging': read_back(proxy.port, [e['id'] for e in written]),
                          'write_ok': bool(ok and ok[2]), 'reads_new_write': read_back(proxy.port, [late['id']])}
    say('rollback', json.dumps(RESULT['rollback']))

    # 5. final equality: Rust = Coax store + the one later write
    cx = start_coax(coax_port, store, cp, cap)
    a, _ = ids(rust_port)
    b, _ = ids(coax_port)
    RESULT['final'] = {'rust': len(a), 'coax': len(b), 'rust_is_coax_plus_late_write': sorted(set(b) | {late['id']}) == a}
    say('final', json.dumps(RESULT['final']))
    cx.terminate()
    rust.terminate()
    passed = (RESULT['equal_after_import']['equal'] and RESULT['check_store_same_served_line']
              and all(RESULT['cutover']['writes_ok']) and RESULT['cutover']['read_back_through_staging']
              and RESULT['rollback_export']['same_as_written'] and all(RESULT['rollback'].values())
              and RESULT['final']['rust_is_coax_plus_late_write'])
    RESULT['passed'] = passed
    json.dump(RESULT, open(os.path.join(OUT, 'result.json'), 'w'), indent=1)
    say('PASSED' if passed else 'FAILED', json.dumps(RESULT))
    sys.exit(0 if passed else 1)


if __name__ == '__main__':
    main()
