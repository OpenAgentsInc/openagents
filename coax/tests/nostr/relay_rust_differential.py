#!/usr/bin/env python3
"""The Coax relay and the old Rust relay, side by side (R1 differential).

Runs the same scripted sessions against crates/nostr-relay (0b401561a0^,
built and run by rust_relay_sandbox.sh in a network-less bubblewrap sandbox
with a throwaway Postgres) and against the Coax relay (coax/relay, an
ASan+UBSan build), and compares what each relay sends: OK verdicts and
reasons, CLOSED and NOTICE reasons, EOSE, event sets and order, live
delivery, the AUTH flow and NIP-11. Every difference is printed and
classified: a match, an intentional difference (KNOWN below, each one
written down in the R1 records), or unexplained (a failure). Events are
signed with relay_session.py's TEST-ONLY keys. It never connects to a public
relay; inside the sandbox there is no network but loopback.

  coax/relay/build.sh OUT --sanitize    # the Coax relay, outside the sandbox
  RUN_BINDS="NOBLE_DIR OUT_DIR" RUN_ENV="COAX_RELAY=OUT BUN=... NOBLE=..." \\
    coax/tests/nostr/rust_relay_sandbox.sh run python3 -I coax/tests/nostr/relay_rust_differential.py

(coax/tests/nostr/rust_relay_differential.sh does all of that.) Tooling only.

The R2 refresh (issue #12631, 2026-11-03) adds the operations wired since
R1, with their refusals: COUNT, search, NIP-09 deletion by id and by
address and a deleted event sent again, NIP-62 vanish, NIP-70 protected
events, NIP-77 syncs (negentropy_ref.py's client) and their NEG-ERR cases,
NIP-86 management over NIP-98 (both relays started with the test key as
their admin), and reads of gift wraps and app data by someone else.
"""
import json
import os
import socket
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import relay_session as rs  # noqa: E402

# Differences that are decided, with where they are written down. A key is
# the observation's label; the value says why the relays may differ there.
KNOWN = {
    'eose frame':
        'intentional: Rust adds a third element ["finish"] to EOSE; ours is the plain NIP-01 EOSE '
        '(R1 record, differential section)',
    'nip11 supported_nips':
        'intentional: Rust advertises 1, 9, 11, 40, 45, 50, 65, 67, 77, 94, 17, 42, 59, 70 (and 86, 98 with a management key); '
        'Coax advertises what it wires (docs/verification/2026-11-03-coax-relay-parity.md); dropping any is the owner\'s call',
    'nip11 limitation extras':
        'intentional: Rust also states max_event_tags, created_at_upper_limit, default_limit, '
        'max_not_before_delta and due_delivery_mode; R1 enforces the first three without stating them',
    'write before AUTH':
        'intentional: section 17 asks for NIP-42 on writes; production runs auth_required false and '
        'accepts anonymous writes (R1 record, "The AUTH policy, and why")',
    'AUTH two challenge tags':
        'intentional: nip42.coax requires exactly one relay and one challenge tag (R1 record, AUTH policy)',
    'EVENT malformed (no object)':
        'intentional: a malformed EVENT gets a NOTICE; Rust answers OK with an id hint (R1 record, Known differences)',
    'vanish: the author\'s note after a kind 62':
        'intentional: NIP-62 is a Coax extension the Rust relay never had; an imported kind 62 is reviewed by the owner',
    'NEG-OPEN over more events than a REQ returns':
        'intentional: Coax reconciles up to 100,000 matching events (RR_NEG_MAX); Rust refuses a sync whose filter '
        'matches more than one REQ returns (NEG-ERR blocked: incomplete)',
    'protected by another key':
        'intentional: NIP-70 refuses an event from anyone but its authenticated author; Coax answers restricted: '
        'to a connection authenticated as someone else, Rust auth-required:',
    'NEG-OPEN bad hex':
        'intentional: NIP-77 answers a bad NEG-OPEN with NEG-ERR, as Coax does; Rust sends CLOSED',
    'NEG-OPEN not Negentropy':
        'intentional: both refuse; Rust says blocked: incomplete, Coax invalid: (the message does not read)',
    'NIP-86 an unknown method, u as http':
        'intentional: u as http is refused by Coax (the owner question on the u tag decides)',
    'NIP-86 by a key that is not the admin, u as ws':
        'intentional: Rust refuses u as ws:// first (401); Coax reads it and refuses the key (403)',
}

# Differences the owner decides (NEEDS_OWNER.md, Coax relay). Reported, not failed.
OWNER = {
    'EVENT issue_no_repo':
        'owner: Rust refuses a NIP-34 issue without a 30617 a tag; R1 does not check kind-specific structure',
    'EVENT comment_no_P':
        'owner: Rust refuses a NIP-22 comment on a nostr root without a P tag; R1 does not check kind-specific structure',
    'REQ two filters, each limited':
        'owner: Rust merges a multi-filter REQ into one list, newest first, sharing 127 events across the filters '
        '(ceil(127/n) each); R1 answers filter by filter, each with its own limit',
    'REQ two-filters':
        'owner: as above, Rust orders a multi-filter REQ newest first across filters; R1 keeps filter order',
    'REQ gift wrap, not its recipient':
        'owner: Rust serves kind 1059 only to its authenticated single p recipient; Coax serves it to anyone '
        '(parity record, private reads: blocks cutover)',
    'REQ app data 30078, not its author':
        'owner: Rust serves kind 30078 only to its authenticated author; Coax serves it to anyone (parity record)',
    'search a word inside others':
        'owner: Rust matches a search term anywhere in the content (n1 finds n10 to n19); Coax matches whole words',
    'NIP-86 supportedmethods, u as ws':
        'owner: NIP-86 says the u tag is "the relay URL": Coax takes the ws:// URL, Rust the http:// one with a slash',
    'NIP-86 supportedmethods, u as http':
        'owner: as above; the method lists also differ (parity record)',
    'NIP-86 banpubkey, u as ws':
        'owner: the u tag form, as above',
    'NIP-86 banpubkey, u as http':
        'owner: the u tag form, as above',
    'NIP-86 an unknown method, u as ws':
        'owner: the u tag form, as above (Coax 400 with an error, Rust 401)',
}

failures = []
counts = {'match': 0, 'text': 0, 'intentional': 0, 'owner': 0, 'unexplained': 0}
diffs = []


def free_port():
    s = socket.socket()
    s.bind(('127.0.0.1', 0))
    p = s.getsockname()[1]
    s.close()
    return p


def start_rust(log, admin):
    port = free_port()
    env = {**os.environ, 'NOSTR_RELAY_PORT': str(port), 'NOSTR_RELAY_BIND_ADDR': '127.0.0.1',
           'NOSTR_RELAY_URL': 'ws://127.0.0.1:%d' % port,
           # production's NIP-11 states max_limit 127 (coax/nostr.md section 17)
           'NOSTR_RELAY_MAX_LIMIT': '127',
           # rate limits are R2: lift them so they never decide a verdict here
           'NOSTR_RELAY_RATE_EVENTS_PER_MIN_IP': '1000000', 'NOSTR_RELAY_RATE_EVENTS_PER_MIN_PUBKEY': '1000000',
           'NOSTR_RELAY_RATE_REQ_PER_MIN_IP': '1000000', 'NOSTR_RELAY_MAX_CONNECTIONS_PER_IP': '1000',
           'NOSTR_RELAY_LOG_LEVEL': 'warn',
           # NIP-86 with the test key, as Coax's --admin
           'NOSTR_RELAY_MANAGEMENT_PUBKEY': admin}
    p = subprocess.Popen([os.environ['RUST_RELAY']], stdout=subprocess.PIPE, stderr=log, text=True, env=env)
    line = p.stdout.readline()
    if 'nostr-relay listening' not in line:
        raise SystemExit('the Rust relay did not start: %r' % line)
    return p, port


def nip11(port):
    s = socket.create_connection(('127.0.0.1', port), timeout=5)
    s.sendall(b'GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nAccept: application/nostr+json\r\nConnection: close\r\n\r\n')
    data = b''
    while True:
        c = s.recv(65536)
        if not c:
            break
        data += c
        if b'\r\n\r\n' in data:
            head, body = data.split(b'\r\n\r\n', 1)
            n = [int(l.split(b':')[1]) for l in head.split(b'\r\n') if l.lower().startswith(b'content-length:')]
            if n and len(body) >= n[0]:
                break
    s.close()
    head, body = data.split(b'\r\n\r\n', 1)
    return head.split(b'\r\n')[0].decode(), json.loads(body)


class Ws(rs.Ws):
    """relay_session's client, reading header names case-insensitively."""
    def __init__(self, port):
        import base64, hashlib
        self.s = socket.create_connection(('127.0.0.1', port), timeout=5)
        key = base64.b64encode(os.urandom(16)).decode()
        self.s.sendall(('GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n'
                        'Sec-WebSocket-Key: %s\r\nSec-WebSocket-Version: 13\r\n\r\n' % key).encode())
        head = b''
        while b'\r\n\r\n' not in head:
            head += self.s.recv(4096)
        head, self.buf = head.split(b'\r\n\r\n', 1)
        want = base64.b64encode(hashlib.sha1((key + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').encode()).digest()).decode()
        lines = head.decode().split('\r\n')
        fields = {l.split(':', 1)[0].strip().lower(): l.split(':', 1)[1].strip() for l in lines[1:] if ':' in l}
        self.ok = lines[0].startswith('HTTP/1.1 101') and fields.get('sec-websocket-accept') == want
        self.port = port
        m = self.recv() if self.ok else None
        self.challenge = m[1] if isinstance(m, list) and len(m) == 2 and m[0] == 'AUTH' else None


def drain(ws, timeout):
    out = []
    while True:
        m = ws.recv(timeout)
        if m is None:
            return out
        out.append(m)


def short(m):
    """An EVENT frame as its id, so observations stay readable."""
    if isinstance(m, list) and len(m) == 3 and m[0] == 'EVENT' and isinstance(m[2], dict):
        return ['EVENT', m[1], m[2].get('id')]
    return m


def until(ws, stop, timeout=15.0):
    """Frames until one whose verb is in stop (NOTICE always stops)."""
    out = []
    while True:
        m = ws.recv(timeout)
        if m is None:
            out.append('<timeout>')
            return out
        out.append(m)
        if isinstance(m, list) and m and (m[0] in stop or m[0] == 'NOTICE'):
            return out
        if isinstance(m, tuple):  # a control frame
            return out


def publish(ws, e):
    """EVENT, then the frames up to its OK (live EVENTs for this connection's
    own subscriptions are left out: live delivery is observed on another one)."""
    ws.send(['EVENT', e])
    return [short(m) for m in until(ws, ('OK',)) if not (isinstance(m, list) and m and m[0] == 'EVENT')]


def req(ws, sub, *filters, keep=False):
    """REQ, the frames up to EOSE or CLOSED, then CLOSE (unless keep), so a
    stored query's subscription never collects live events later."""
    ws.send(['REQ', sub] + list(filters))
    got = until(ws, ('EOSE', 'CLOSED'))
    if not keep and got and isinstance(got[-1], list) and got[-1][0] == 'EOSE':
        ws.send(['CLOSE', sub])
    return [short(m) for m in got]


def anon_id(frames):
    """An AUTH event is signed per relay (its challenge and URL differ), so
    its OK is compared without the id."""
    return [['OK', '<auth event id>'] + m[2:] if isinstance(m, list) and m and m[0] == 'OK' else m for m in frames]


def strip_eose(frames, seen):
    """Rust's EOSE carries a third element; fold it to NIP-01 and note it once."""
    out = []
    for m in frames:
        if isinstance(m, list) and len(m) == 3 and m[0] == 'EOSE':
            seen.add(json.dumps(m[2]))
            m = m[:2]
        out.append(m)
    return out


def script(port, ev, extra):
    """One relay's whole session. Returns [(label, observation)] in order."""
    obs = []
    eose = set()

    def note(label, value):
        obs.append((label, strip_eose(value, eose) if isinstance(value, list) else value))

    status, doc = nip11(port)
    lim = doc.get('limitation', {})
    note('nip11 status', status)
    note('nip11 limits', {k: lim.get(k) for k in ('max_message_length', 'max_subscriptions', 'max_limit',
                                                   'max_subid_length', 'auth_required', 'restricted_writes')})
    note('nip11 supported_nips', doc.get('supported_nips'))
    note('nip11 limitation extras', sorted(k for k in lim if k not in (
        'max_message_length', 'max_subscriptions', 'max_limit', 'max_subid_length', 'auth_required',
        'restricted_writes', 'payment_required', 'min_pow_difficulty')))

    a, b = Ws(port), Ws(port)
    note('handshake and challenge', [a.ok, b.ok, bool(a.challenge) and len(a.challenge) == 64,
                                     a.challenge != b.challenge])
    note('write before AUTH', publish(b, ev['anon']))
    note('REQ without AUTH', req(a, 'anon', {'kinds': [22242]}))
    url = 'ws://127.0.0.1:%d' % port
    now = int(time.time())
    for label, e in [
            ('AUTH wrong challenge', b.auth_event(tags=[['relay', url], ['challenge', a.challenge]])),
            ('AUTH another relay', b.auth_event(tags=[['relay', 'ws://127.0.0.1:%d' % (port + 1)], ['challenge', b.challenge]])),
            ('AUTH 601 s old', b.auth_event(created_at=now - 601)),
            ('AUTH kind 1', b.auth_event(kind=1)),
            ('AUTH no relay tag', b.auth_event(tags=[['challenge', b.challenge]])),
            ('AUTH two challenge tags', b.auth_event(tags=[['relay', url], ['challenge', b.challenge], ['challenge', b.challenge]]))]:
        b.send(['AUTH', e])
        note(label, anon_id(until(b, ('OK',))))
    good = b.auth_event()
    bad = dict(good); bad['sig'] = ('0' if bad['sig'][0] != '0' else '1') + bad['sig'][1:]
    forged = dict(good); forged['content'] = 'x'
    for label, e in [('AUTH bad signature', bad), ('AUTH wrong id', forged), ('AUTH right', good),
                     ('AUTH same key again', good)]:
        b.send(['AUTH', e])
        note(label, anon_id(until(b, ('OK',))))
    note('AUTH event not delivered', [short(m) for m in drain(a, 0.5)])
    a.auth()

    # live subscription on a, writes on b
    note('REQ live open', req(a, 'live', {'kinds': [1, 20001]}, keep=True))
    note('EVENT valid', publish(b, ev['first']))
    note('live delivery', [short(m) for m in drain(a, 1.0)])
    got = drain(a, 0.1)
    note('EVENT duplicate', publish(b, ev['first']))
    note('EVENT bad id', publish(b, ev['bad_id']))
    note('EVENT bad signature', publish(b, ev['bad_sig']))
    note('EVENT created_at 1000 s ahead', publish(b, ev['future']))
    note('EVENT created_at 800 s ahead', publish(b, ev['near_future']))
    note('EVENT expired (NIP-40)', publish(b, ev['expired']))
    note('EVENT expires in an hour', publish(b, ev['expires_later']))
    note('EVENT kind 22242', publish(b, ev['auth_kind']))
    note('EVENT 257 tags', publish(b, ev['tags257']))
    note('EVENT 256 tags', publish(b, ev['tags256']))
    note('EVENT ephemeral 20001', publish(b, ev['ephemeral']))
    note('refused events not delivered, ephemeral delivered', [short(m) for m in got + drain(a, 1.0)])
    note('REQ ephemeral not stored', req(b, 'eph', {'kinds': [20001]}))
    b.send(['EVENT', 'not an object'])
    note('EVENT malformed (no object)', [short(m) for m in until(b, ('OK',), 3.0)])

    # replaceable and addressable
    note('kind 0 newer', publish(b, ev['k0_new']))
    note('kind 0 older after newer', publish(b, ev['k0_old']))
    note('REQ kind 0', req(b, 'k0', {'kinds': [0], 'authors': [ev['first']['pubkey']]}))
    hi, lo = ev['k3_hi'], ev['k3_lo']
    note('kind 3 tie: higher id first', publish(b, hi))
    note('kind 3 tie: lower id replaces', publish(b, lo))
    note('kind 3 tie: higher id again', publish(b, hi))
    note('REQ kind 3', req(b, 'k3', {'kinds': [3]}))
    hi, lo = ev['k10002_hi'], ev['k10002_lo']
    note('kind 10002 tie: lower id first', publish(b, lo))
    note('kind 10002 tie: higher id after', publish(b, hi))
    note('REQ kind 10002', req(b, 'k10002', {'kinds': [10002]}))
    hi, lo = ev['a_hi'], ev['a_lo']
    note('30617 tie: higher id first', publish(b, hi))
    note('30617 tie: lower id replaces', publish(b, lo))
    note('30617 tie: higher id again', publish(b, hi))
    note('30617 newer replaces', publish(b, ev['a_newer']))
    note('30617 older after newer', publish(b, ev['a_older']))
    note('REQ 30617 d=tie', req(b, 'tie', {'kinds': [30617], '#d': ['tie']}))
    note('30617 other d kept', publish(b, ev['a_other_d']))
    note('REQ 30617 both d', req(b, 'ad', {'kinds': [30617]}))

    # the website's kinds and filters
    for name in ('repo', 'project', 'tracker', 'issue', 'status_open', 'status_closed', 'comment'):
        note('EVENT %s' % name, publish(b, ev[name]))
    pk = ev['first']['pubkey']
    for sub, *flts in [
            ('ids', {'ids': [ev['issue']['id'], ev['comment']['id']]}),
            ('authors', {'authors': [pk], 'kinds': [1621, 1111]}),
            ('kinds', {'kinds': [30621, 30623]}),
            ('since', {'kinds': [1], 'since': ev['k1'][120]['created_at']}),
            ('until', {'kinds': [1], 'until': ev['k1'][5]['created_at']}),
            ('since-until', {'kinds': [1], 'since': ev['k1'][10]['created_at'], 'until': ev['k1'][14]['created_at']}),
            ('limit', {'kinds': [1], 'limit': 3}),
            ('limit0', {'kinds': [1], 'limit': 0}),
            ('a', {'#a': ['30621:%s:coax' % pk]}),
            ('e', {'kinds': [1630, 1631, 1632, 1633], '#e': [ev['issue']['id']]}),
            ('p', {'#p': [extra['other_pk']]}),
            ('d', {'kinds': [30621, 30623], 'authors': [pk], '#d': ['coax']}),
            ('t', {'#t': ['website']}),
            ('E', {'#E': [ev['issue']['id']]}),
            ('project-page', {'kinds': [1621], 'authors': [pk], '#a': ['30621:%s:coax' % pk]}),
            ('two-filters', {'kinds': [1621]}, {'kinds': [1111]}),
            ('overlap', {'kinds': [1621]}, {'ids': [ev['issue']['id']]}),
            ('none', {'authors': ['0' * 64]}),
            ('inverted', {'kinds': [1], 'since': ev['k1'][20]['created_at'], 'until': ev['k1'][10]['created_at']})]:
        note('REQ %s' % sub, req(b, sub, *flts))

    # max_limit and default_limit need more than 127 stored kind 1 events
    for e in ev['k1']:
        b.send(['EVENT', e])
    oks = [m for m in (until(b, ('OK',)) for _ in ev['k1'])]
    note('130 kind 1 events', sorted(set(json.dumps(o[-1][2:]) for o in oks)))
    drain(a, 0.5)
    drain(b, 0.5)
    note('REQ limit 500 (max_limit)', req(b, 'max', {'kinds': [1], 'limit': 500}))
    note('REQ no limit (default_limit)', req(b, 'def', {'kinds': [1]}))
    note('REQ limit 127', req(b, 'l127', {'kinds': [1], 'limit': 127}))
    note('REQ limit 128', req(b, 'l128', {'kinds': [1], 'limit': 128}))
    note('REQ two filters, each limited', req(b, 'two', {'kinds': [1], 'limit': 2}, {'kinds': [1621], 'limit': 1}))
    full = [m for m in req(b, 'bytes', {'ids': [ev['issue']['id']]}) if m[0] == 'EVENT']
    b.send(['REQ', 'bytes2', {'ids': [ev['issue']['id']]}])
    raw = b.frame(5)
    note('EVENT bytes as published', raw is not None and json.loads(raw[1])[2] == ev['issue']
         and raw[1].decode() == '["EVENT","bytes2",%s]' % json.dumps(ev['issue'], separators=(',', ':'), ensure_ascii=False))
    until(b, ('EOSE',))
    del full

    # CLOSE
    a.send(['CLOSE', 'live'])
    note('CLOSE answer', [short(m) for m in drain(a, 0.5)])
    note('after CLOSE publish', publish(b, ev['after_close']))
    note('after CLOSE nothing arrives', [short(m) for m in drain(a, 1.0)])
    a.send(['CLOSE', 'never-opened'])
    note('CLOSE unknown sub', [short(m) for m in drain(a, 0.5)])

    # limits
    c = Ws(port)
    note('REQ sub id 64', req(c, 'x' * 64, {'kinds': [7]}, keep=True))
    note('REQ sub id 65', req(c, 'x' * 65, {'kinds': [7]}))
    note('REQ sub id empty', req(c, '', {'kinds': [7]}))
    note('REQ 16 filters', req(c, 'f16', *[{'kinds': [7]}] * 16, keep=True))
    note('REQ 17 filters', req(c, 'f17', *[{'kinds': [7]}] * 17))
    note('REQ empty kinds', req(c, 'empty', {'kinds': []}))
    note('REQ #e not hex', req(c, 'hex', {'#e': ['abc']}))
    note('REQ #p not hex', req(c, 'phex', {'#p': ['XYZ']}))
    note('REQ eight open filters', req(c, 'cost', *[{}] * 8))
    note('REQ unknown field', req(c, 'unk', {'kinds': [1], 'foo': 1}))
    note('REQ negative limit', req(c, 'neg', {'kinds': [1], 'limit': -1}))
    note('REQ no filter', req(c, 'nof'))
    for i in range(32 - 2):  # x*64 and f16 are open
        req(c, 's%d' % i, {'kinds': [7]}, keep=True)
    note('REQ 33rd subscription', req(c, 's32', {'kinds': [7]}))
    note('REQ reuse sub id', req(c, 's0', {'kinds': [8]}, keep=True))
    c.send(['COUNT', 'cnt', {'kinds': [1]}])
    note('COUNT', [short(m) for m in until(c, ('COUNT', 'CLOSED'), 3.0)])
    c.send(['HELLO'])
    note('unknown verb', until(c, ('NOTICE',), 3.0))
    c.send_frame(1, b'not json')
    note('not JSON', until(c, ('NOTICE',), 3.0))
    c.send_frame(9, b'ping!')
    note('ping', [c.frame(3)])
    c.send_frame(2, b'\x00')
    note('binary frame', until(c, ('NOTICE',), 3.0))
    c.send_frame(1, b'[' + b' ' * 131072 + b']')
    f = c.frame(5)
    note('131074-byte frame', None if f is None else [f[0], f[1][:2].hex()])
    c.close()
    d = Ws(port)
    d.send_frame(8, b'\x03\xe8')
    f = d.frame(5)
    note('close 1000 answered', None if f is None else [f[0], f[1][:2].hex()])
    d.close()
    a.close(); b.close()
    r2(port, ev, note)
    # kind-specific structure (NIP-34, NIP-22) last, so what one relay stores
    # and the other refuses changes no query above
    e = Ws(port)
    e.auth()
    for name in ('issue_no_repo', 'comment_no_P'):
        note('EVENT %s' % name, publish(e, ev[name]))
    e.close()
    note('eose frame', sorted(eose))
    return obs


OTHER_KEY = 'coax relay r2 diff other key'
VANISH_KEY = 'coax relay r2 diff vanish key'


def neg_open(ws, sub, flt, items):
    """NEG-OPEN with negentropy_ref's client over items; the frames up to the
    first answer, and the ids it says the client has and needs."""
    import negentropy_ref as nr
    c = nr.Side(items, True)
    ws.send(['NEG-OPEN', sub, flt, c.initiate().hex()])
    m = ws.recv(5)
    if isinstance(m, list) and m[:2] == ['NEG-MSG', sub]:
        nxt, have, need = c.reconcile(bytes.fromhex(m[2]))
        return ['NEG-MSG', sub, 'done' if nxt is None else 'more', sorted(h.hex() for h in have), sorted(n.hex() for n in need)]
    return m


def r2_events(now, pk):
    """The R2 session's events, signed once so both relays see the same ids."""
    import base64
    T = now - 300
    other, = rs.sign([{"kind": 1, "created_at": T, "tags": [], "content": "the other key"}], OTHER_KEY)
    opk = other['pubkey']
    x, = rs.sign([{"kind": 1, "created_at": T + 1, "tags": [["t", "r2"]], "content": "to be deleted"}])
    # a structurally valid NIP-44 v2 payload (version 2, nonce, 34 bytes, MAC): random bytes, no key behind them
    wrapped = base64.b64encode(bytes([2]) + os.urandom(32 + 34 + 32)).decode()
    r = rs.sign([
        {"kind": 5, "created_at": T + 2, "tags": [["e", x['id']]], "content": ""},
        {"kind": 5, "created_at": T + 3, "tags": [["e", other['id']]], "content": ""},
        {"kind": 5, "created_at": T + 4, "tags": [["a", "30617:%s:tie" % pk]], "content": ""},
        {"kind": 30617, "created_at": T + 200, "tags": [["d", "tie"]], "content": "after the deletion"},
        {"kind": 1, "created_at": T + 5, "tags": [["-"]], "content": "protected"},
        {"kind": 1, "created_at": T + 6, "tags": [["-"]], "content": "p2"},
        {"kind": 1059, "created_at": T + 7, "tags": [["p", opk]], "content": wrapped},
        {"kind": 30078, "created_at": T + 8, "tags": [["d", "settings"]], "content": "{\"theme\":\"dark\"}"}])
    o = rs.sign([{"kind": 1, "created_at": T + 9, "tags": [["-"]], "content": "protected, other"},
                 {"kind": 1, "created_at": T + 250, "tags": [], "content": "after the ban"}], OTHER_KEY)
    v1, = rs.sign([{"kind": 1, "created_at": T + 10, "tags": [], "content": "before vanishing"}], VANISH_KEY)
    names = ['d_e', 'd_other', 'd_a', 'a_after', 'prot', 'prot_noauth', 'wrap', 'app']
    e = dict(zip(names, r))
    e.update(other=other, x=x, prot_other=o[0], banned_later=o[1], v1=v1, wrapped=wrapped, T=T)
    return e


def per_relay(frames):
    """An OK for an event signed per relay (it names the relay's URL), compared without its id."""
    return [['OK', '<per-relay id>'] + m[2:] if isinstance(m, list) and m and m[0] == 'OK' else m for m in frames]


def r2(port, ev, note):
    """The operations wired since R1, with their refusals (#12631)."""
    pk = ev['first']['pubkey']
    e = ev['r2']
    opk, vpk = e['other']['pubkey'], e['v1']['pubkey']
    url = 'ws://127.0.0.1:%d' % port
    g, h = Ws(port), Ws(port)
    g.auth()
    note('r2 other key note', publish(g, e['other']))

    # NIP-45
    for sub, *flts in [('c-kind1', {'kinds': [1]}), ('c-two', {'kinds': [1621]}, {'kinds': [1111]}),
                       ('c-author', {'authors': [pk], 'kinds': [30617]}), ('c-none', {'authors': ['0' * 64]}),
                       ('c-ids', {'ids': [ev['issue']['id']]})]:
        g.send(['COUNT', sub] + list(flts))
        note('COUNT %s' % sub, [short(m) for m in until(g, ('COUNT', 'CLOSED'), 5.0)])
    g.send(['COUNT', 'c-bad', {'kinds': [1], 'foo': 1}])
    note('COUNT unknown field', [short(m) for m in until(g, ('COUNT', 'CLOSED'), 5.0)])
    g.send(['COUNT', 'c-hex', {'#e': ['abc']}])
    note('COUNT #e not hex', [short(m) for m in until(g, ('COUNT', 'CLOSED'), 5.0)])

    # NIP-50
    note('search issue', req(g, 'q1', {'search': 'issue'}))
    note('search a word inside others', req(g, 'q2', {'search': 'n1', 'kinds': [1], 'limit': 5}))
    note('search nothing', req(g, 'q3', {'search': 'zzqqxx'}))

    # NIP-09
    note('deletion: target stored', publish(g, e['x']))
    note('deletion by id stored', publish(g, e['d_e']))
    note('deletion: target gone', req(g, 'de', {'ids': [e['x']['id']]}))
    note('deletion: target sent again', publish(g, e['x']))
    note('deletion of another key\'s note stored', publish(g, e['d_other']))
    note('deletion of another key\'s note: it stays', req(g, 'do', {'ids': [e['other']['id']]}))
    note('deletion by address stored', publish(g, e['d_a']))
    note('deletion by address: versions gone', req(g, 'da', {'kinds': [30617], '#d': ['tie']}))
    note('deletion by address: an older version again', publish(g, ev['a_older']))
    note('deletion by address: a later version', publish(g, e['a_after']))
    note('REQ the requests', req(g, 'dk', {'kinds': [5], 'authors': [pk]}))

    # NIP-62 (the request names this relay's URL, so it is signed per relay)
    gv = Ws(port)
    gv.auth(gv.auth_event(label=VANISH_KEY))
    note('vanish: the author\'s note', publish(gv, e['v1']))
    v62, = rs.sign([{"kind": 62, "created_at": e['T'] + 11, "tags": [["relay", url]], "content": ""}], VANISH_KEY)
    note('vanish: kind 62 stored', per_relay(publish(gv, v62)))
    note('vanish: the author\'s note after a kind 62', req(g, 'van', {'authors': [vpk], 'kinds': [1]}))
    gv.close()

    # NIP-70
    note('protected by its author', publish(g, e['prot']))
    note('protected by another key', publish(g, e['prot_other']))
    note('protected without AUTH', publish(h, e['prot_noauth']))

    # private reads
    note('gift wrap stored', publish(g, e['wrap']))
    note('REQ gift wrap, not its recipient', req(h, 'gw', {'kinds': [1059]}))
    note('search gift wrap content', req(h, 'gws', {'search': e['wrapped'][4:12]}))
    note('app data stored', publish(g, e['app']))
    note('REQ app data 30078, not its author', req(h, 'ad', {'kinds': [30078]}))

    # NIP-77 over the project page's issues (fewer than any limit)
    issues = [m for m in req(g, 'iss', {'kinds': [1621]}) if m[0] == 'EVENT']
    full = {x['id']: x for x in [ev['issue']]}
    items = sorted((full[m[2]]['created_at'], bytes.fromhex(m[2])) for m in issues if m[2] in full)
    note('NEG-OPEN equal sets', neg_open(g, 'n-eq', {'kinds': [1621]}, items))
    note('NEG-OPEN client lacks one', neg_open(g, 'n-lack', {'kinds': [1621]}, []))
    note('NEG-OPEN client has 2 extra', neg_open(g, 'n-extra', {'kinds': [1621]}, items + [(1, b'\x01' * 32), (2, b'\x02' * 32)]))
    g.send(['NEG-CLOSE', 'n-eq'])
    g.send(['NEG-MSG', 'n-none', '61'])
    note('NEG-MSG with no sync open', [short(m) for m in until(g, ('NEG-ERR', 'NEG-MSG'), 5.0)])
    g.send(['NEG-OPEN', 'n-hex', {'kinds': [1621]}, '6g'])
    note('NEG-OPEN bad hex', [short(m) for m in until(g, ('NEG-ERR', 'NEG-MSG', 'CLOSED'), 3.0)])
    g.send(['NEG-OPEN', 'n-not', {'kinds': [1621]}, '00'])
    note('NEG-OPEN not Negentropy', [short(m) for m in until(g, ('NEG-ERR', 'NEG-MSG'), 5.0)])
    g.send(['NEG-OPEN', 'n-big', {'kinds': [1]}, '6100000200'])
    note('NEG-OPEN over more events than a REQ returns', [short(m)[:2] if isinstance(m, list) else m for m in until(g, ('NEG-ERR', 'NEG-MSG'), 5.0)])

    # NIP-86 over NIP-98: the relay URL as ws://, then as http://.../
    forms = (('ws', url), ('http', 'http://127.0.0.1:%d/' % port))
    for form, u in forms:
        body = '{"method":"supportedmethods","params":[]}'
        code, got = rs.post(port, body, rs.nip98(port, body, url=u))
        note('NIP-86 supportedmethods, u as %s' % form, [code, sorted(got.get('result') or []) if isinstance(got, dict) else 'not JSON'])
    for form, u in forms:
        body = '{"method":"listbannedpubkeys","params":[]}'
        code, got = rs.post(port, body, rs.nip98(port, body, url=u, label=OTHER_KEY))
        note('NIP-86 by a key that is not the admin, u as %s' % form, [code, sorted(got) if isinstance(got, dict) else 'not JSON'])
    for form, u in forms:
        body = '{"method":"banpubkey","params":["%s","spam"]}' % opk
        code, got = rs.post(port, body, rs.nip98(port, body, url=u))
        note('NIP-86 banpubkey, u as %s' % form, [code, got.get('result') if isinstance(got, dict) else 'not JSON'])
    for form, u in forms:
        body = '{"method":"nosuchmethod","params":[]}'
        code, got = rs.post(port, body, rs.nip98(port, body, url=u))
        note('NIP-86 an unknown method, u as %s' % form, [code, sorted(got) if isinstance(got, dict) else 'not JSON'])
    note('a banned key publishes', publish(g, e['banned_later']))
    g.close(); h.close()


def templates(now):
    T = now - 1000
    other = '7' * 64
    t = {
        'first': {"kind": 1, "created_at": T - 5, "tags": [], "content": "first"},
        'anon': {"kind": 9999, "created_at": T - 6, "tags": [], "content": "before AUTH"},
        'future': {"kind": 1, "created_at": now + 1000, "tags": [], "content": "future"},
        'near_future': {"kind": 1, "created_at": now + 800, "tags": [], "content": "near future"},
        'expired': {"kind": 1, "created_at": T, "tags": [["expiration", str(now - 10)]], "content": "gone"},
        'expires_later': {"kind": 1, "created_at": T, "tags": [["expiration", str(now + 3600)]], "content": "later"},
        'auth_kind': {"kind": 22242, "created_at": now, "tags": [["challenge", "x"]], "content": ""},
        'tags257': {"kind": 1, "created_at": T, "tags": [["t", str(i)] for i in range(257)], "content": "257"},
        'tags256': {"kind": 1, "created_at": T, "tags": [["t", str(i)] for i in range(256)], "content": "256"},
        'ephemeral': {"kind": 20001, "created_at": now, "tags": [], "content": "ephemeral"},
        'k0_old': {"kind": 0, "created_at": T, "tags": [], "content": "{\"name\":\"v1\"}"},
        'k0_new': {"kind": 0, "created_at": T + 1, "tags": [], "content": "{\"name\":\"v2\"}"},
        'k3_a': {"kind": 3, "created_at": T, "tags": [], "content": "a"},
        'k3_b': {"kind": 3, "created_at": T, "tags": [], "content": "b"},
        'k10002_a': {"kind": 10002, "created_at": T, "tags": [["r", "wss://a"]], "content": ""},
        'k10002_b': {"kind": 10002, "created_at": T, "tags": [["r", "wss://b"]], "content": ""},
        'a_a': {"kind": 30617, "created_at": T, "tags": [["d", "tie"]], "content": "a"},
        'a_b': {"kind": 30617, "created_at": T, "tags": [["d", "tie"]], "content": "b"},
        'a_newer': {"kind": 30617, "created_at": T + 10, "tags": [["d", "tie"]], "content": "newer"},
        'a_older': {"kind": 30617, "created_at": T + 5, "tags": [["d", "tie"]], "content": "older"},
        'a_other_d': {"kind": 30617, "created_at": T, "tags": [["d", "other"]], "content": "other"},
        'repo': {"kind": 30617, "created_at": T, "tags": [["d", "openagents"], ["name", "OpenAgents"]], "content": ""},
        'project': {"kind": 30621, "created_at": T, "tags": [["d", "coax"], ["t", "website"]], "content": "{}"},
        'tracker': {"kind": 30623, "created_at": T + 1, "tags": [["d", "coax"]], "content": "{}"},
        'after_close': {"kind": 1, "created_at": T + 200, "tags": [], "content": "after close"},
    }
    k1 = [{"kind": 1, "created_at": T + 10 + i // 2, "tags": [["n", str(i)]], "content": "n%d" % i} for i in range(130)]
    return t, k1, other


def sign_all(now):
    t, k1, other = templates(now)
    names = list(t)
    signed = rs.sign([t[n] for n in names] + k1)
    ev = dict(zip(names, signed[:len(names)]))
    ev['k1'] = signed[len(names):]
    pk = ev['first']['pubkey']
    # the project page's events reference each other, so sign in two rounds
    ev['project'] = rs.sign([{**t['project'], 'tags': t['project']['tags'] + [["a", "30617:%s:openagents" % pk]]}])[0]
    repo_a = ["a", "30617:%s:openagents" % pk]
    issue = rs.sign([{"kind": 1621, "created_at": now - 900, "content": "issue",
                      "tags": [repo_a, ["a", "30621:%s:coax" % pk], ["p", other], ["subject", "x"]]}])[0]
    ev['issue'] = issue
    root = lambda: [["e", issue['id'], "", "root"], ["p", pk], repo_a]
    hint = "wss://relay.example.com"
    comment = [["E", issue['id'], hint, pk], ["K", "1621"], ["P", pk], ["e", issue['id'], hint, pk], ["k", "1621"], ["p", pk]]
    (ev['status_open'], ev['status_closed'], ev['comment'], ev['issue_no_repo'],
     ev['comment_no_P']) = rs.sign([
        {"kind": 1630, "created_at": now - 899, "content": "", "tags": root()},
        {"kind": 1632, "created_at": now - 898, "content": "", "tags": root()},
        {"kind": 1111, "created_at": now - 897, "content": "c", "tags": comment},
        {"kind": 1621, "created_at": now - 896, "content": "no repo",
         "tags": [["a", "30621:%s:coax" % pk], ["subject", "y"]]},
        {"kind": 1111, "created_at": now - 895, "content": "no P", "tags": [t for t in comment if t[0] != 'P']}])
    bad = dict(ev['first']); bad['sig'] = ('0' if bad['sig'][0] != '0' else '1') + bad['sig'][1:]
    ev['bad_sig'] = dict(ev['after_close']); ev['bad_sig']['sig'] = bad['sig']
    forged = dict(ev['first']); forged['content'] = 'changed'
    ev['bad_id'] = forged
    ev['r2'] = r2_events(now, pk)
    for k in ('k3', 'k10002', 'a'):
        x, y = ev[k + '_a'], ev[k + '_b']
        ev[k + '_hi'], ev[k + '_lo'] = (x, y) if x['id'] > y['id'] else (y, x)
    return ev, {'other_pk': other}


def prefix(reason):
    return reason.split(':', 1)[0] if isinstance(reason, str) and ':' in reason else reason


def verdicts(value):
    """An observation with each OK, CLOSED and NOTICE reduced to its verdict
    and machine prefix (NIP-01: the text after the prefix is for humans)."""
    if not isinstance(value, list):
        return value
    out = []
    for m in value:
        if isinstance(m, list) and m and m[0] == 'OK' and len(m) == 4:
            m = m[:3] + [prefix(m[3])]
        elif isinstance(m, list) and m and m[0] == 'CLOSED' and len(m) == 3:
            m = m[:2] + [prefix(m[2])]
        elif isinstance(m, list) and m and m[0] == 'NOTICE' and len(m) == 2:
            m = ['NOTICE', prefix(m[1])]
        elif isinstance(m, list) and m and m[0] == 'NEG-ERR' and len(m) == 3:
            m = m[:2] + [prefix(m[2])]
        out.append(m)
    return out


def compare(rust, coax):
    rd, cd = dict(rust), dict(coax)
    for label, _ in rust:
        r, c = rd[label], cd.get(label, '<missing>')
        if r == c:
            counts['match'] += 1
            continue
        why = KNOWN.get(label)
        if not why and label in OWNER:
            counts['owner'] += 1
            diffs.append((label, 'owner', r, c, OWNER[label]))
            continue
        if not why and verdicts(r) == verdicts(c):
            counts['text'] += 1
            diffs.append((label, 'text', r, c, 'same verdicts and prefixes; only the human-readable text differs'))
            continue
        kind = 'intentional' if why else 'unexplained'
        counts[kind] += 1
        diffs.append((label, kind, r, c, why))
    for label in cd:
        if label not in rd:
            counts['unexplained'] += 1
            diffs.append((label, 'unexplained', '<missing>', cd[label], None))


def main():
    exe = os.environ.get('COAX_RELAY')
    if not exe or not os.environ.get('RUST_RELAY'):
        raise SystemExit('run through coax/tests/nostr/rust_relay_differential.sh')
    now = int(time.time())
    t0 = time.monotonic()
    ev, extra = sign_all(now)
    print('signed %d events (%.1f s)' % (len(ev) + len(ev['k1']) - 1, time.monotonic() - t0))
    results = {}
    with tempfile.TemporaryDirectory(prefix='relay-rust-diff-') as tmp:
        d = Path(tmp)
        with open(d / 'rust.err', 'w') as rlog, open(d / 'coax.err', 'w') as clog:
            rp, rport = start_rust(rlog, ev['first']['pubkey'])
            cp, cport = rs.start(Path(exe), clog, extra=['--admin', ev['first']['pubkey']])
            try:
                for name, port in (('rust', rport), ('coax', cport)):
                    t0 = time.monotonic()
                    results[name] = script(port, ev, extra)
                    print('%s session: %d observations (%.1f s)' % (name, len(results[name]), time.monotonic() - t0))
            finally:
                for p in (rp, cp):
                    p.terminate()
                    p.wait(10)
        cerr = (d / 'coax.err').read_text()
        sanitizer = 'runtime error' in cerr or 'AddressSanitizer' in cerr
        if sanitizer:
            sys.stderr.write(cerr[-4000:])
    compare(results['rust'], results['coax'])
    for label, kind, r, c, why in diffs:
        print('\n[%s] %s' % (kind, label))
        print('  rust: %s' % json.dumps(r)[:600])
        print('  coax: %s' % json.dumps(c)[:600])
        if why:
            print('  why:  %s' % why)
    used = {label for label, kind, *_ in diffs if kind in ('intentional', 'owner')}
    for label in list(KNOWN) + list(OWNER):
        if label not in used:
            print('\n[note] KNOWN %r no longer differs' % label)
    print('\nobservations %d: %d match, %d same verdict with other text, %d intentional, %d for the owner, %d unexplained; sanitizer %s'
          % (sum(counts.values()), counts['match'], counts['text'], counts['intentional'], counts['owner'], counts['unexplained'],
             'REPORT' if sanitizer else 'clean'))
    ok = counts['unexplained'] == 0 and not sanitizer
    print('relay_rust_differential: ' + ('ok' if ok else 'FAILED'))
    return 0 if ok else 1


if __name__ == '__main__':
    sys.exit(main())
