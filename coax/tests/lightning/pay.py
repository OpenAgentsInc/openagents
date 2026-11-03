#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: a separate Python payer and network decide each line outside Coax)
"""The cross-check of Bitcoin chapter 113. It computes every line the
chapter's example prints and compares the transcript with the fixture:

  python3 -I coax/tests/lightning/pay.py                      # compare with the fixture
  python3 -I coax/tests/lightning/pay.py --coax coax/bin/coax   # also run the example

The payer here is written from BOLT 4, BOLT 7 and BOLT 11 and the
chapter's stated model: Dijkstra from the payee back to the payer over
the channel graph, BOLT 7's fee, a hop penalty and a uniform-liquidity
risk penalty, halving for multi-part payments, and the failure rules of
BOLT 4's "Receiving Failure Codes". Its graph is bolt7.py's, fed with
gossip that bolt7.py writes and signs; invoices come from bolt11.py and
onions from sphinx.py. The network is a balance model, not a second
BOLT 2 implementation: each channel keeps both sides' balances and its
HTLCs, and an HTLC is refused when the offerer could not pay it and
keep the reserve and, as opener, the commitment fee of BOLT 3
(bolt3_tx.py). Nothing here shares code with Coax.
"""
import hashlib
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import bolt11 as E  # noqa: E402
import bolt3_tx as B  # noqa: E402
import bolt7 as G  # noqa: E402
import bolts  # noqa: E402
import ln_crypto as C  # noqa: E402
import sphinx as S  # noqa: E402

REPO = HERE.parents[2]
EXAMPLE = 'bitcoin/examples/lightning-pay.coax'
FIXTURE = REPO / 'bitcoin/fixtures/lightning-pay.out'
RUN_STEPS = '250000000'

NAMES = ['Alice', 'Bob', 'Carol', 'Dave', 'Erin']
HEIGHT = 1000
FEERATE, RESERVE = 2500, 10000
HOP_PENALTY, RISK, MIN_PART, MAX_PARTS, MAX_HOPS, MAX_CLTV = 500, 10000, 10000000, 16, 20, 2016
UNKNOWN_HI = 1 << 40
TEMPORARY_CHANNEL_FAILURE, INCORRECT_OR_UNKNOWN = 0x1007, 0x400f
PERM, NODE = 0x4000, 0x2000
CHAIN = bytes.fromhex('06226e46111a0b59caaf126043eb5bbf28c34f3a5e332a1fc7b2b73cf188910f')


def sha(b):
    return hashlib.sha256(b).digest()


def secret(k, c):
    return bytes((k * i + c) & 255 for i in range(32))


def scid(height, tx, out):
    return height << 40 | tx << 16 | out


def scid_text(v):
    return f'{v >> 40}x{v >> 16 & 0xffffff}x{v & 0xffff}'


def tu(v):
    return v.to_bytes(8, 'big').lstrip(b'\0')


def fee(base, ppm, amount):
    return base + amount * ppm // 1000000


# ---- the network: nodes, channels as balances, and gossip

class Net:
    def __init__(self):
        self.keys = [int.from_bytes(secret(k + 3, 2 * k + 1), 'big') for k in range(5)]
        self.secrets = [secret(k + 3, 2 * k + 1) for k in range(5)]
        self.ids = [C.compress(C.mul(k)) for k in self.keys]
        # opener, other end, funding sat; channel 4 is private
        self.links = []
        for l, (a, b, funding) in enumerate([(0, 1, 2000000), (1, 3, 1000000), (0, 2, 2000000), (2, 3, 1000000), (3, 4, 1000000)]):
            self.links.append({'nodes': (a, b), 'scid': scid(101 + l, 1, 0), 'funding': funding,
                               'bal': [funding * 1000, 0], 'htlcs': 0, 'policy': [self.policy(a, funding), self.policy(b, funding)]})
        self.invoices = {3: {}, 4: {}}

    @staticmethod
    def policy(k, funding):
        base = 500 if k == 2 else 1000
        ppm = 50 if k == 2 else 1000 if k == 3 else 100
        cltv = 80 if k == 2 else 144 if k == 3 else 40
        return {'base': base, 'ppm': ppm, 'cltv': cltv, 'min': 1000, 'max': (funding - 10000) * 1000}

    def link(self, sid):
        return next(i for i, x in enumerate(self.links) if x['scid'] == sid)

    def funding_key(self, l, side):
        seed = bytearray([0x40 + 2 * l] * 32)
        seed[0] += side
        return int.from_bytes(sha(bytes(seed) + b'\x01'), 'big')

    def gossip(self):
        """bolt7.py's graph, fed the four public channels' signed gossip."""
        g = G.Graph(CHAIN)
        for l in range(4):
            x = self.links[l]
            a, b = x['nodes']
            order = sorted([(self.ids[a], a, 0), (self.ids[b], b, 1)])
            m = G.write_channel_announcement(CHAIN, x['scid'], [self.keys[n] for _, n, _ in order],
                                             [self.funding_key(l, s) for _, _, s in order])
            assert g.add(m) == 'OK'
            for side, n in enumerate((a, b)):
                p = x['policy'][side]
                direction = 0 if self.ids[n] == order[0][0] else 1
                u = G.write_channel_update(self.keys[n], CHAIN, x['scid'], 1762156800, direction, 0,
                                           p['cltv'], p['min'], p['base'], p['ppm'], p['max'])
                assert g.add(u) == 'OK'
        return g

    def affords(self, l, side, amount):
        x = self.links[l]
        opener_fee = B.commit_fee(FEERATE, x['htlcs'] + 1, False) if side == 0 else 0
        return x['bal'][side] - amount >= (RESERVE + opener_fee) * 1000

    def spendable(self, l, side):
        x = self.links[l]
        opener_fee = B.commit_fee(FEERATE, x['htlcs'] + 1, False) if side == 0 else 0
        return max(0, x['bal'][side] - (RESERVE + opener_fee) * 1000)

    def offer(self, l, side, amount):
        self.links[l]['bal'][side] -= amount
        self.links[l]['htlcs'] += 1

    def resolve(self, l, side, amount, paid):
        self.links[l]['bal'][1 - side if paid else side] += amount
        self.links[l]['htlcs'] -= 1


# ---- invoices: bolt11.py's writer

def amount_hrp(msat):
    for div, m in [(100000000000, ''), (100000000, 'm'), (100000, 'u'), (100, 'n')]:
        if msat % div == 0:
            return f'{msat // div}{m}'
    return f'{msat * 10}p'


def feature_words(bits):
    n = max(bits) // 5 + 1
    w = [0] * n
    for b in bits:
        w[n - 1 - b // 5] |= 1 << (b % 5)
    return w


def write_invoice(net, payee, amount, k, hints):
    preimage = bytes((k + i) & 255 for i in range(32))
    pay_secret = bytes((k + 2 * i + 1) & 255 for i in range(32))
    net.invoices[payee][sha(preimage)] = {'preimage': preimage, 'secret': pay_secret, 'amount': amount}
    fields = [(1, E.to_words(sha(preimage))), (16, E.to_words(pay_secret)), (13, E.to_words(b'Chapter 113')),
              (5, feature_words([8, 14, 17]))]
    if hints:
        fields.append((3, E.to_words(b''.join(h['pubkey'] + h['scid'].to_bytes(8, 'big') + h['base'].to_bytes(4, 'big')
                                                + h['ppm'].to_bytes(4, 'big') + h['cltv'].to_bytes(2, 'big') for h in hints))))
    return E.encode('lnbcrt' + amount_hrp(amount), 1700000000 + k, fields, net.keys[payee])


# ---- the payer

class Payer:
    def __init__(self, net, g):
        self.net = net
        self.edges = []  # dicts: scid, frm, to, base, ppm, cltv, min, max, disabled, failed, lo, hi, inflight
        for sid, c in g.channels.items():
            for d in (0, 1):
                u = c['updates'][d]
                frm, to = (c['node1'], c['node2']) if d == 0 else (c['node2'], c['node1'])
                self.edges.append({'scid': sid, 'frm': frm, 'to': to, 'base': u['fee_base'], 'ppm': u['fee_ppm'],
                                   'cltv': u['cltv'], 'min': u['htlc_min'], 'max': u['htlc_max'],
                                   'disabled': bool(u['channel_flags'] & 2), 'failed': False,
                                   'lo': 0, 'hi': u['htlc_max'], 'inflight': 0})

    def find(self, sid, frm):
        return next((i for i, e in enumerate(self.edges) if e['scid'] == sid and e['frm'] == frm), -1)

    def hint(self, h, payee):
        if self.find(h['scid'], h['pubkey']) < 0:
            self.edges.append({'scid': h['scid'], 'frm': h['pubkey'], 'to': payee, 'base': h['base'], 'ppm': h['ppm'],
                               'cltv': h['cltv'], 'min': 0, 'max': UNKNOWN_HI, 'disabled': False, 'failed': False,
                               'lo': 0, 'hi': UNKNOWN_HI, 'inflight': 0})

    @staticmethod
    def usable(e, a):
        return not e['disabled'] and not e['failed'] and e['min'] <= a <= e['max'] and a + e['inflight'] <= e['hi']

    @staticmethod
    def risk(e, a):
        lo, hi = max(0, e['lo'] - e['inflight']), e['hi'] - e['inflight']
        return 0 if a <= lo else RISK * (a - lo) // (hi - lo + 1)

    def route(self, payer, payee, amount, final_cltv):
        ids = [payee, payer]
        for e in self.edges:
            for n in (e['frm'], e['to']):
                if n not in ids:
                    ids.append(n)
        label = {0: (0, amount, final_cltv, 0, None)}  # node -> cost, need, cltv, hops, edge
        done = set()
        while True:
            open_ = [i for i in range(len(ids)) if i in label and i not in done]
            if not open_:
                return None
            u = min(open_, key=lambda i: (label[i][0], i))
            done.add(u)
            if u == 1:
                break
            cost, need, cltv, hops, _ = label[u]
            for k, e in enumerate(self.edges):
                if e['to'] != ids[u]:
                    continue
                v = ids.index(e['frm'])
                if v in done or not self.usable(e, need):
                    continue
                first = v == 1
                f = 0 if first else fee(e['base'], e['ppm'], need)
                c = cost + f + self.risk(e, need) + (0 if first else HOP_PENALTY)
                t = cltv + (0 if first else e['cltv'])
                if hops + 1 > MAX_HOPS or t - HEIGHT > MAX_CLTV:
                    continue
                if v not in label or c < label[v][0]:
                    label[v] = (c, need + f, t, hops + 1, k)
        r, at = [], 1
        while at != 0:
            k = label[at][4]
            at = ids.index(self.edges[k]['to'])
            r.append((k, label[at][1], label[at][2]))
        return {'hops': r, 'fee': label[1][1] - amount}


def payloads(payer, r, pay_secret, total):
    out = []
    hops = r['hops']
    for i, (k, amount, cltv) in enumerate(hops):
        last = i == len(hops) - 1
        rec = {2: tu(amount if last else hops[i + 1][1]), 4: tu(cltv if last else hops[i + 1][2])}
        if last:
            rec[8] = pay_secret + tu(total)
        else:
            rec[6] = payer.edges[hops[i + 1][0]]['scid'].to_bytes(8, 'big')
        p = S.tlv(rec)
        out.append(S.bigsize_bytes(len(p)) + p)
    return out


def pay(net, payer, text, out):
    inv = E.decode(text)
    payee = inv['payee']
    hints = inv.get('routes') or []
    to = net.ids.index(payee)
    out.append(f'  invoice: {len(text)} characters, signature verifies, {inv["amount"]} msat to {NAMES[to]}, '
               f'{sum(len(h) for h in hints)} route hint hops')
    for h in hints:
        for j, (key, sid, base, ppm, cltv) in enumerate(h):
            payer.hint({'pubkey': bytes.fromhex(key), 'scid': sid, 'base': base, 'ppm': ppm, 'cltv': cltv},
                       bytes.fromhex(h[j + 1][0]) if j + 1 < len(h) else payee)
    mpp = bool(inv['features'] >> 16 & 3)
    out.append(f'  multi-part allowed: {str(mpp).lower()}')
    phash, psecret, total = inv['p'], inv['s'], inv['amount']
    final_cltv = HEIGHT + inv['cltv']
    waiting, parts, settled, refused, code, rounds = [total], [], 0, False, None, 0
    alice = net.ids[0]
    while code is None:
        rounds += 1
        for l, x in enumerate(net.links):
            if 0 in x['nodes']:
                k = payer.find(x['scid'], alice)
                if k >= 0:
                    payer.edges[k]['lo'] = payer.edges[k]['hi'] = net.spendable(l, x['nodes'].index(0)) + payer.edges[k]['inflight']
        changed, launched = False, []
        while waiting and code is None:
            amount = waiting.pop(0)
            r = payer.route(alice, payee, amount, final_cltv)
            if r:
                if len(parts) == MAX_PARTS:
                    code = 'TOO_MANY'
                    break
                i = len(parts)
                session = sha(net.secrets[0] + bytes([i]))
                pubkeys = [payer.edges[k]['to'] for k, _, _ in r['hops']]
                onion = S.wrap(int.from_bytes(session, 'big'), pubkeys, payloads(payer, r, psecret, total), phash)
                part = {'amount': amount, 'route': r, 'session': session, 'onion': sha(onion), 'state': 'flying'}
                parts.append(part)
                for k, a, _ in r['hops']:
                    payer.edges[k]['inflight'] += a
                launched.append(part)
                changed = True
                continue
            if not mpp or amount < 2 * MIN_PART or len(waiting) + 2 > MAX_PARTS:
                code = 'NO_ROUTE'
                break
            waiting += [amount // 2, amount - amount // 2]
        if code:
            break
        for part in launched:
            run_part(net, payer, part, phash, total)
        flying = 0
        for part in parts:
            if part['state'] == 'settled_now':
                part['state'] = 'settled'
                settled += part['amount']
                for k, a, _ in part['route']['hops']:
                    payer.edges[k]['inflight'] -= a
                changed = True
            elif part['state'] == 'failed_now':
                part['state'] = 'failed'
                hops = part['route']['hops']
                h, fcode = part['fail_hop'], part['fail_code']
                if h == len(hops) - 1:
                    refused = True
                else:
                    e = payer.edges[hops[h + 1][0]]
                    if fcode & NODE:
                        bad = payer.edges[hops[h][0]]['to']
                        for x in payer.edges:
                            if bad in (x['frm'], x['to']):
                                x['failed'] = True
                    elif fcode == TEMPORARY_CHANNEL_FAILURE:
                        e['hi'] = min(e['hi'], e['inflight'] - 1)
                        e['lo'] = min(e['lo'], e['hi'])
                    else:
                        e['failed'] = True
                for k, a, _ in hops:
                    payer.edges[k]['inflight'] -= a
                changed = True
                if not refused:
                    waiting.append(part['amount'])
            elif part['state'] in ('flying', 'held'):
                flying += 1
        if settled >= total and flying == 0:
            code = 'OK'
        elif refused and flying == 0:
            code = 'REFUSED'
        elif not changed:
            code = 'STUCK'
    for i, part in enumerate(parts):
        r = part['route']
        path = ' > '.join(NAMES[net.ids.index(payer.edges[k]['to'])] for k, _, _ in r['hops'])
        out.append(f'  part {i + 1}: {part["amount"]} msat via Alice > {path}, fee {r["fee"]} msat')
        out.append('    channels' + ''.join(f' {scid_text(payer.edges[k]["scid"])} {a} msat to {t}' for k, a, t in r['hops']))
        line = f'    onion {part["onion"][:8].hex()}'
        if part['state'] == 'settled':
            out.append(line + ': settled')
        else:
            who = NAMES[net.ids.index(payer.edges[r['hops'][part['fail_hop']][0]]['to'])]
            name = {TEMPORARY_CHANNEL_FAILURE: 'temporary_channel_failure', INCORRECT_OR_UNKNOWN: 'incorrect_or_unknown_payment_details'}[part['fail_code']]
            out.append(line + f': failed at {who}, {name} (0x{part["fail_code"]:04x})')
    preimage = next((p['preimage'] for p in parts if 'preimage' in p), bytes(32))
    out.append(f'  result: {code}, rounds: {rounds}, settled: {settled} msat, preimage matches the hash: {str(sha(preimage) == phash).lower()}')


def run_part(net, payer, part, phash, total):
    """The part's HTLCs hop by hop on the balance model; a held part waits at the payee."""
    hops = part['route']['hops']
    nodes = [0] + [net.ids.index(payer.edges[k]['to']) for k, _, _ in hops]
    taken = []
    for i, (k, amount, cltv) in enumerate(hops):
        l = net.link(payer.edges[k]['scid'])
        side = net.links[l]['nodes'].index(nodes[i])
        if i > 0:
            p = net.links[l]['policy'][side]
            assert hops[i - 1][1] - amount >= fee(p['base'], p['ppm'], amount) and hops[i - 1][2] - p['cltv'] >= cltv
        if not net.affords(l, side, amount):
            assert i > 0, 'the payer refused its own HTLC'
            fail(net, part, taken, i - 1, TEMPORARY_CHANNEL_FAILURE)
            return
        net.offer(l, side, amount)
        taken.append((l, side, amount))
    payee, (_, amount, cltv) = nodes[-1], hops[-1]
    inv = net.invoices.get(payee, {}).get(phash)
    if inv is None or cltv < HEIGHT + 18 or total < inv['amount']:
        fail(net, part, taken, len(hops) - 1, INCORRECT_OR_UNKNOWN)
        return
    part['state'], part['taken'] = 'held', taken
    held = [p for p in payer_parts(part) if p.get('state') == 'held']
    if sum(p['route']['hops'][-1][1] for p in held) >= total:
        for p in held:
            for l, side, a in p['taken']:
                net.resolve(l, side, a, True)
            p['state'], p['preimage'] = 'settled_now', inv['preimage']


PARTS = []


def payer_parts(part):
    if part not in PARTS:
        PARTS.append(part)
    return PARTS


def fail(net, part, taken, hop, code):
    for l, side, a in taken:
        net.resolve(l, side, a, False)
    part['state'], part['fail_hop'], part['fail_code'] = 'failed_now', hop, code


def transcript():
    bolts.text('04-onion-routing.md')
    bolts.text('07-routing-gossip.md')
    bolts.text('11-payment-encoding.md')
    out = []
    net = Net()
    out.append('nodes')
    for k in range(5):
        out.append(f'  {NAMES[k]} {net.ids[k].hex()}')
    out.append("channels, opened with chapter 109's messages")
    for l, x in enumerate(net.links):
        a, b = x['nodes']
        out.append(f'  {NAMES[a]} - {NAMES[b]} {scid_text(x["scid"])}: {x["funding"]} sat from {NAMES[a]}, open, '
                   f'{"private" if l == 4 else "announced"}')
    out.append('gossip')
    g = net.gossip()
    for l in range(4):
        out.append(f'  {scid_text(net.links[l]["scid"])}: announced, both updates added')
    payer = Payer(net, g)
    out.append(f"  Alice's map: {len(g.channels)} channels, {len(payer.edges)} directions")
    out.append('1. Alice pays Dave 50,000 sat')
    pay(net, payer, write_invoice(net, 3, 50000000, 1, []), out)
    PARTS.clear()
    out.append('2. Alice pays Dave 1,400,000 sat: more than any one route carries')
    pay(net, payer, write_invoice(net, 3, 1400000000, 2, []), out)
    PARTS.clear()
    out.append("3. Alice pays Dave 280,000 sat: Carol's side has only 250,000")
    pay(net, payer, write_invoice(net, 3, 280000000, 3, []), out)
    PARTS.clear()
    out.append("4. Alice pays Erin 100,000 sat over Dave's private channel")
    hint = {'pubkey': net.ids[3], 'scid': net.links[4]['scid'], 'base': 1000, 'ppm': 1000, 'cltv': 144}
    pay(net, payer, write_invoice(net, 4, 100000000, 4, [hint]), out)
    PARTS.clear()
    out.append('5. Alice pays an invoice Dave has forgotten')
    text = write_invoice(net, 3, 10000000, 5, [])
    net.invoices[3].clear()
    pay(net, payer, text, out)
    out.append("balances, msat, as each side's commitment says")
    for x in net.links:
        a, b = x['nodes']
        out.append(f'  {scid_text(x["scid"])}: {NAMES[a]} {x["bal"][0]}, {NAMES[b]} {x["bal"][1]}; agree: true')
    return out


def main():
    lines = transcript()
    want = FIXTURE.read_text().splitlines()
    assert lines == want, '\n'.join(f'{x!r} != {y!r}' for x, y in zip(lines, want) if x != y) or 'line counts differ'
    print(f'{len(lines)} lines agree with {FIXTURE.name}')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            got = subprocess.run([coax, 'run', EXAMPLE, *mode, '--run-steps', RUN_STEPS], cwd=REPO,
                                 capture_output=True, text=True, check=True)
            assert got.stdout.splitlines() == lines, f'coax run {" ".join(mode)} differs'
            print(f'coax run {" ".join(mode) or "(debug)"}: {len(lines)} lines agree')


if __name__ == '__main__':
    main()
