#!/usr/bin/env python3
# Firm reason: 2 (an independent oracle: the served set of a real export, worked out from the Rust relay's rows with none of the Coax store's code)
"""What the Rust relay serves from an export, as chapter 103's digest (#13361 step 5).

The export is every live row of the Rust relay's nostr_event table (nostr/relay/
export.sql). The Rust relay already applied replacement and NIP-09 deletion to
those rows: a replaced or deleted event has no row. So the set it serves at
--now T is every row except those NIP-40 has expired by T (a row the sweep had
not removed yet is not served either). That set's digest is chapter 103's:
SHA-256 over the events sorted by (created_at, id), each as 8 bytes of
created_at, big-endian, then the 32-byte id. The Coax relay's --import and
--check-store print the same digest for what they serve at T; equal digests
mean equal sets (relay_equal.coax).

Kind 3188 host envelopes are kept 7 days from created_at (the owner's
decision, 2026-11-12, #13361): the relay reads that as their NIP-40
expiration, or their own when it is sooner, so here too a 3188 older than
7 days at T is not served.

It also lists, for the record, what the Coax relay decides differently on
purpose: kind 62 requests to vanish from the relay's URL or ALL_RELAYS (the
Coax relay retires the author's earlier events; the Rust relay never did).

  python3 coax/tests/nostr/relay_export_served.py EXPORT.jsonl --now T [--url wss://relay.openagents.com] [--ids OUT.txt]
"""
import hashlib
import json
import sys


def expiration(tags):
    for t in tags:
        if len(t) >= 2 and t[0] == 'expiration':
            try:
                return int(t[1])
            except ValueError:
                return None
    return None


RETAIN_KIND, RETAIN_S = 3188, 604800


def kept_until(e):
    x = expiration(e['tags'])
    if e['kind'] != RETAIN_KIND:
        return x
    keep = e['created_at'] + RETAIN_S
    return x if x is not None and x < keep else keep


def main():
    a = sys.argv[1:]
    if not a or '--now' not in a:
        sys.exit(__doc__)
    path = a[0]
    now = int(a[a.index('--now') + 1])
    url = a[a.index('--url') + 1] if '--url' in a else 'wss://relay.openagents.com'
    ids_out = a[a.index('--ids') + 1] if '--ids' in a else None
    served, expired, retired, lines, vanish = [], 0, 0, 0, []
    with open(path, 'rb') as f:
        for raw in f:
            lines += 1
            e = json.loads(raw)
            x = kept_until(e)
            if x is not None and x <= now:
                if e['kind'] == RETAIN_KIND and (expiration(e['tags']) is None or expiration(e['tags']) > now):
                    retired += 1
                else:
                    expired += 1
                continue
            served.append((e['created_at'], e['id']))
            if e['kind'] == 62 and any(len(t) >= 2 and t[0] == 'relay' and t[1] in (url, 'ALL_RELAYS') for t in e['tags']):
                vanish.append((e['pubkey'], e['created_at'], e['id']))
    served.sort(key=lambda p: (p[0], bytes.fromhex(p[1])))
    h = hashlib.sha256()
    for created, i in served:
        h.update(created.to_bytes(8, 'big') + bytes.fromhex(i))
    print('export: %d lines, %d expired (NIP-40) and %d kind 3188 past 7 days at %d' % (lines, expired, retired, now))
    print('served set at %d: %d events, digest %s' % (now, len(served), h.hexdigest()))
    for p, c, i in vanish:
        print('kind 62 to vanish from %s: author %s at %d (event %s)' % (url, p, c, i))
    if ids_out:
        with open(ids_out, 'w') as o:
            for created, i in served:
                o.write('%d %s\n' % (created, i))


if __name__ == '__main__':
    main()
