#!/usr/bin/env python3
# Firm reason: 2 (an independent oracle: it decides the served set of a synthetic export with its own NIP-01/09/40/62 rules, not the Coax store's)
"""The synthetic export for the relay's import and restart at scale (issue #12631).

Builds a JSON-lines export of --events lines in ingest order, as the Rust
relay's nostr_event table would export them (chapter 103's format), signed
with TEST-ONLY keys by relay_sign.ts (@noble/curves, the reference BIP-340
implementation; one bun process per key, in parallel). Then, with its own
rules and none of the Coax store's code, it works out what an import at
--now must do with every line, and what the store must serve afterwards:

- regular notes (kind 1), some with quotes, backslashes, newlines, tabs and
  non-ASCII content, so every id is recomputed over escaped text;
- replaceable (0, 3, 10002) and addressable (30023 with d tags) versions,
  newest wins, and older versions that arrive after newer ones (superseded);
- exact duplicate lines;
- NIP-40: events already expired at --now (refused as expired, kept out)
  and events that expire after it (served);
- NIP-09: kind 5 requests with `e` tags (an earlier event retired, or an
  event that isn't in the export: a tombstone only) and with `a` tags (an
  address's versions up to the request's time); a few deleted events are
  sent again afterwards and must be refused;
- NIP-62: one author's request to vanish from --url, retiring everything
  the author wrote up to it, with later notes kept;
- forged lines: a real event with one signature digit changed.

Writes OUT/export.jsonl and OUT/expected.txt, whose lines the Coax harness
(relay_scale.coax) finds in the relay's own output:

  import read L lines
  import outcome CODE COUNT          (the relay's reason codes)
  import tombstones T
  served set at NOW: N events, digest SHA256

The digest is chapter 103's: SHA-256 over the served events sorted by
(created_at, id), each as 8 bytes of created_at, big-endian, then the id.
Keys are sha256 of fixed labels: no one's keys, never used elsewhere, never
printed. Tooling only; nothing here ships or touches a network.

  python3 coax/tests/nostr/relay_scale_oracle.py --events N --now T --url U --out DIR
"""
import hashlib
import json
import os
import random
import struct
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
NOBLE = os.environ.get('NOBLE', '/home/christopherdavid/oa-board/coax/web/project/node_modules')
BUN = os.environ.get('BUN', 'bun')
AUTHORS = 16
VANISHER = AUTHORS - 1

# The relay's reason codes (relay_server.coax's RS_* and relay_import.coax's RI_*).
STORED, DUP, SUPERSEDED, BAD_SIG, EXPIRED, DELETED = 0, 1, 2, 5, 9, 31


def arg(name, default=None):
    a = sys.argv[1:]
    return a[a.index(name) + 1] if name in a else default


def nip01_id(e):
    ser = json.dumps([0, e['pubkey'], e['created_at'], e['kind'], e['tags'], e['content']],
                     separators=(',', ':'), ensure_ascii=False)
    return hashlib.sha256(ser.encode()).hexdigest()


def sign_all(per_author):
    """Signs each author's templates with its own bun process, all at once."""
    procs = []
    for k, templates in enumerate(per_author):
        env = {**os.environ, 'NODE_PATH': NOBLE, 'RELAY_TEST_KEY_LABEL': 'coax relay scale test key %d' % k}
        p = subprocess.Popen([BUN, str(HERE / 'relay_sign.ts')], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                             env=env, text=True)
        procs.append(p)
    outs = []
    for k, p in enumerate(procs):
        out, _ = p.communicate(''.join(json.dumps(t, ensure_ascii=False) + '\n' for t in per_author[k]), timeout=840)
        if p.returncode != 0:
            raise SystemExit('relay_sign.ts failed for author %d' % k)
        signed = [json.loads(line) for line in out.splitlines() if line.strip()]
        if len(signed) != len(per_author[k]):
            raise SystemExit('author %d: %d templates, %d signed' % (k, len(per_author[k]), len(signed)))
        outs.append(signed)
    return outs


ODD = ['quote " inside', 'back\\slash', 'line\nbreak', 'tab\there', 'café über', 'emoji \U0001f600 ok', '中文']


def plan(n, now, url, rnd):
    """The export as a list of steps, each a template to sign (author, template)
    or a reference to an earlier step (a duplicate, a forgery, a resend)."""
    steps = []
    t0 = now - 30 * 86400
    clock = [t0]

    def tick():
        clock[0] += rnd.randint(1, 8)
        return clock[0]

    def add(author, kind, tags, content, created=None):
        steps.append(('new', author, {'kind': kind, 'created_at': created if created is not None else tick(), 'tags': tags, 'content': content}))
        return len(steps) - 1

    regular = [[] for _ in range(AUTHORS)]     # (step) of each author's kind 1 notes
    heads = {}                                 # address -> [steps]
    vanish_at = None
    absent = []                                # (author, template) deleted before they are sent
    while len(steps) < n:
        r = rnd.random()
        a = rnd.randrange(AUTHORS - 1) if vanish_at is not None else rnd.randrange(AUTHORS)
        if r < 0.62:
            content = 'note %d %s' % (len(steps), rnd.choice(ODD) if rnd.random() < 0.02 else 'plain text')
            tags = [['t', 'scale']]
            x = rnd.random()
            if x < 0.02:
                tags.append(['expiration', str(now - rnd.randint(1, 900000))])
            elif x < 0.04:
                tags.append(['expiration', str(now + rnd.randint(100000, 900000))])
            regular[a].append(add(a, 1, tags, content))
        elif r < 0.72:
            kind = rnd.choice([0, 3, 10002])
            heads.setdefault((a, kind, None), []).append(add(a, kind, [], 'replaceable %d' % len(steps)))
        elif r < 0.84:
            d = 'post-%d' % rnd.randrange(400)
            heads.setdefault((a, 30023, d), []).append(add(a, 30023, [['d', d], ['title', 'post']], 'long form %d' % len(steps)))
        elif r < 0.85:
            # an older version of an address, sent after a newer one
            key = rnd.choice(list(heads)) if heads else None
            if key and steps[heads[key][-1]][2]['created_at'] > t0 + 10 and key[0] != VANISHER:
                older = steps[heads[key][0]][2]['created_at'] - 1
                tags = [['d', key[2]], ['title', 'post']] if key[2] is not None else []
                heads[key].append(add(key[0], key[1], tags, 'stale %d' % len(steps), created=older))
        elif r < 0.86:
            # a duplicate line: a note sent again at once
            if regular[a]:
                steps.append(('dup', regular[a][-1]))
        elif r < 0.87:
            # a forged line: a note with one signature digit changed
            if regular[a]:
                steps.append(('forge', regular[a][-1]))
        elif r < 0.875:
            # NIP-09 by id: one of the author's earlier notes
            if regular[a]:
                target = rnd.choice(regular[a][-200:])
                steps.append(('del-e', a, target, tick()))
        elif r < 0.877:
            # NIP-09 by address: every version up to now
            keys = [k for k in heads if k[0] == a and k[1] == 30023]
            if keys:
                key = rnd.choice(keys)
                steps.append(('del-a', a, key, tick()))
        elif r < 0.8775:
            # NIP-09 for an event the export never had, which arrives later
            tmpl = {'kind': 1, 'created_at': tick(), 'tags': [['t', 'late']], 'content': 'deleted before it came %d' % len(steps)}
            absent.append((a, tmpl))
            steps.append(('del-absent', a, len(absent) - 1, tick()))
        elif r < 0.878 and absent:
            # an event deleted earlier is sent: the import must refuse it
            i = rnd.randrange(len(absent))
            steps.append(('late', absent[i][0], i))
        if vanish_at is None and len(steps) > n * 0.6:
            vanish_at = tick()
            steps.append(('vanish', VANISHER, url, vanish_at))
    return steps[:n], absent


def main():
    n = int(arg('--events', '250000'))
    now = int(arg('--now', '1793220000'))
    url = arg('--url', 'wss://relay.openagents.com')
    out = Path(arg('--out'))
    seed = int(arg('--seed', '12631'))
    rnd = random.Random(seed)
    out.mkdir(parents=True, exist_ok=True)
    t = time.monotonic()
    steps, absent = plan(n, now, url, rnd)

    # Every template, per author, to sign in one pass; deletion requests need
    # their targets' ids, so targets are signed first.
    per_author = [[] for _ in range(AUTHORS)]
    where = {}
    for i, s in enumerate(steps):
        if s[0] == 'new':
            where[i] = (s[1], len(per_author[s[1]]))
            per_author[s[1]].append(s[2])
    absent_at = []
    for a, tmpl in absent:
        absent_at.append((a, len(per_author[a])))
        per_author[a].append(tmpl)
    signed = sign_all(per_author)
    pubkeys = [ev[0]['pubkey'] if ev else None for ev in signed]
    if any(p is None for p in pubkeys):
        raise SystemExit('every author needs at least one event')

    def ev(i):
        a, k = where[i]
        return signed[a][k]

    # The deletion and vanish requests, signed in a second pass.
    second = [[] for _ in range(AUTHORS)]
    req_at = {}
    for i, s in enumerate(steps):
        if s[0] == 'del-e':
            tags = [['e', ev(s[2])['id']]]
        elif s[0] == 'del-a':
            key = s[2]
            tags = [['a', '%d:%s:%s' % (key[1], pubkeys[key[0]], key[2])]]
        elif s[0] == 'del-absent':
            a, k = absent_at[s[2]]
            tags = [['e', signed[a][k]['id']]]
        elif s[0] == 'vanish':
            tags = [['relay', s[2]]]
        else:
            continue
        kind = 62 if s[0] == 'vanish' else 5
        req_at[i] = (s[1], len(second[s[1]]))
        second[s[1]].append({'kind': kind, 'created_at': s[3], 'tags': tags, 'content': ''})
    signed2 = sign_all(second)
    print('relay_scale_oracle: %d lines planned and signed in %.1f s' % (len(steps), time.monotonic() - t))

    # The lines, in order, and what each must do.
    lines = []
    for i, s in enumerate(steps):
        if s[0] == 'new':
            lines.append(ev(i))
        elif s[0] == 'dup':
            lines.append(ev(s[1]))
        elif s[0] == 'forge':
            e = dict(ev(s[1]))
            e['sig'] = e['sig'][:10] + ('0' if e['sig'][10] != '0' else '1') + e['sig'][11:]
            lines.append({**e, '_forged': True})
        elif s[0] == 'late':
            a, k = absent_at[s[2]]
            lines.append(signed[a][k])
        else:
            a, k = req_at[i]
            lines.append(signed2[a][k])
    for e in lines:
        if nip01_id(e) != e['id']:
            raise SystemExit('relay_sign.ts and this oracle disagree on an id: %s' % e['id'])

    # The oracle's store: records by id, the head of each address, tombstones.
    outcomes = {}
    live = {}            # id -> event
    seen = set()         # every id stored
    head = {}            # address -> event
    tomb_e = set()       # (signer, id)
    tomb_a = {}          # (kind, author, d) -> until
    vanish = {}          # author -> until
    tombstones = 0

    def address(e):
        k = e['kind']
        if k in (0, 3) or 10000 <= k < 20000:
            return (k, e['pubkey'], '')
        if 30000 <= k < 40000:
            d = next((t[1] for t in e['tags'] if len(t) > 1 and t[0] == 'd'), '')
            return (k, e['pubkey'], d)
        return None

    def expiration(e):
        for t in e['tags']:
            if len(t) > 1 and t[0] == 'expiration':
                return int(t[1])
        return None

    def covered(e):
        if e['kind'] != 62 and e['pubkey'] in vanish and e['created_at'] <= vanish[e['pubkey']]:
            return True
        if e['kind'] not in (5, 62) and (e['pubkey'], e['id']) in tomb_e:
            return True
        ad = address(e)
        return ad is not None and ad in tomb_a and e['created_at'] <= tomb_a[ad]

    def retire_if(pred):
        for i in [i for i, x in live.items() if pred(x)]:
            del live[i]

    def outcome(e):
        nonlocal tombstones
        # the relay's order: policy (expiry) before the signature, the
        # signature before tombstones, tombstones before duplicates
        x = expiration(e)
        if x is not None and x <= now:
            return EXPIRED
        if e.get('_forged'):
            return BAD_SIG
        if covered(e):
            return DELETED
        if e['id'] in seen:
            return DUP
        ad = address(e)
        if ad is not None:
            h = head.get(ad)
            # newest wins; on the same second, the lowest id
            if h is not None and (h['created_at'] > e['created_at'] or (h['created_at'] == e['created_at'] and h['id'] < e['id'])):
                return SUPERSEDED
            if h is not None:
                live.pop(h['id'], None)
            head[ad] = e
        seen.add(e['id'])
        live[e['id']] = e
        signer = e['pubkey']
        if e['kind'] == 5:
            for t in e['tags']:
                if t[0] == 'e':
                    tomb_e.add((signer, t[1])); tombstones += 1
                    r = live.get(t[1])
                    if r is not None and r['pubkey'] == signer and r['kind'] not in (5, 62):
                        del live[t[1]]
                elif t[0] == 'a':
                    k, author, d = t[1].split(':', 2)
                    tombstones += 1
                    if author == signer:
                        key = (int(k), author, d)
                        tomb_a[key] = max(tomb_a.get(key, -1), e['created_at'])
                        # only an address's head is live: replacement retired the rest
                        h = head.get(key)
                        if h is not None and h['id'] in live and h['created_at'] <= e['created_at']:
                            del live[h['id']]
        if e['kind'] == 62 and any(t[0] == 'relay' and t[1] in (url, 'ALL_RELAYS') for t in e['tags']):
            vanish[signer] = max(vanish.get(signer, -1), e['created_at']); tombstones += 1
            retire_if(lambda r: r['pubkey'] == signer and r['kind'] != 62 and r['created_at'] <= e['created_at'])
        return STORED

    with open(out / 'export.jsonl', 'w', encoding='utf-8') as f:
        for e in lines:
            o = outcome(e)
            outcomes[o] = outcomes.get(o, 0) + 1
            f.write(json.dumps({k: v for k, v in e.items() if k != '_forged'}, ensure_ascii=False) + '\n')
    served = sorted((e['created_at'], e['id']) for e in live.values() if expiration(e) is None or expiration(e) > now)
    h = hashlib.sha256()
    for created, i in served:
        h.update(struct.pack('>Q', created) + bytes.fromhex(i))
    with open(out / 'expected.txt', 'w') as f:
        f.write('import read %d lines\n' % len(lines))
        for o in sorted(outcomes):
            f.write('import outcome %d %d\n' % (o, outcomes[o]))
        f.write('import tombstones %d\n' % tombstones)
        f.write('served set at %d: %d events, digest %s\n' % (now, len(served), h.hexdigest()))
    size = (out / 'export.jsonl').stat().st_size
    print('relay_scale_oracle: %d lines, %d bytes; outcomes %s; %d tombstones; serves %d (%.1f s)'
          % (len(lines), size, outcomes, tombstones, len(served), time.monotonic() - t))
    return 0


if __name__ == '__main__':
    sys.exit(main())
